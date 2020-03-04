use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow;
use r53::{message::SectionType, name::root, Message, MessageBuilder, RRType, RRset, Rcode};

use super::delegation_point::DelegationPoint;
use super::host_selector::{Host, HostSelector};
use super::iter_event::{IterEvent, QueryState, ResponseType};
use super::nsclient::send_query;
use super::roothint::RootHint;
use crate::cache::MessageCache;
use crate::types::{classify_response, ResponseCategory};

const MAX_CNAME_REDIRECT_COUNT: u8 = 8;
const MAX_DEPENDENT_QUERY_COUNT: u8 = 4;

#[derive(Clone)]
pub struct Iterator<S> {
    cache: Arc<Mutex<MessageCache>>,
    roothint: Arc<RootHint>,
    host_selector: Arc<Mutex<S>>,
}

impl<S: HostSelector + 'static + Send + Clone> Iterator<S> {
    pub fn new(cache: Arc<Mutex<MessageCache>>, selector: S) -> Self {
        Self {
            cache: cache.clone(),
            roothint: Arc::new(RootHint::new()),
            host_selector: Arc::new(Mutex::new(selector)),
        }
    }

    pub fn resolve(
        &mut self,
        query: Message,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Message>> + Send>> {
        Box::pin(self.clone().handle_query(query))
    }

    async fn handle_query(mut self, query: Message) -> anyhow::Result<Message> {
        let mut event = IterEvent::new(query, QueryState::InitQuery, QueryState::Finished);
        let mut done = false;
        loop {
            event = match event.get_state() {
                QueryState::InitQuery => self.process_init_query(event).await,
                QueryState::QueryTarget => self.process_query_target(event).await,
                QueryState::QueryResponse => self.process_query_response(event),
                QueryState::PrimeResponse => self.process_prime_response(event),
                QueryState::TargetResponse => self.process_target_response(event),
                QueryState::Finished => {
                    self.process_finished(&mut event);
                    return Ok(event.get_response().unwrap());
                }
            };
        }
    }

    async fn process_init_query(&mut self, mut event: IterEvent) -> IterEvent {
        if event.query_restart_count > MAX_CNAME_REDIRECT_COUNT {
            self.error_response(&mut event, Rcode::ServFail);
            return event;
        }

        if event.get_depth() > MAX_DEPENDENT_QUERY_COUNT {
            self.error_response(&mut event, Rcode::ServFail);
            return event;
        }

        if self.lookup_cache(&mut event) {
            return event;
        } else {
            return self.prime_root(event).await;
        }
    }

    fn error_response(&mut self, event: &mut IterEvent, rcode: Rcode) {
        let mut response = event.get_request().clone();
        MessageBuilder::new(&mut response)
            .make_response()
            .rcode(rcode)
            .done();
        event.set_response(response, ResponseType::Throwaway);
        event.next_state(event.get_final_state())
    }

    fn lookup_cache(&mut self, event: &mut IterEvent) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if let Some(response) = cache.gen_response(&event.get_request()) {
            event.set_response(response, ResponseType::Answer);
            event.next_state(event.get_final_state());
            return true;
        } else if let Some(dp) = DelegationPoint::from_cache(
            event.get_request().question.as_ref().unwrap().name.clone(),
            &mut cache,
        ) {
            event.set_delegation_point(dp);
            event.next_state(QueryState::QueryTarget);
            return true;
        } else {
            return false;
        }
    }

    async fn prime_root(&mut self, event: IterEvent) -> IterEvent {
        let request = Message::with_query(root(), RRType::NS);
        let selector = self.host_selector.lock().unwrap().clone();
        let result = send_query(&request, self.select_root_server(), selector).await;
        let mut sub_event = IterEvent::new(
            request,
            QueryState::QueryResponse,
            QueryState::PrimeResponse,
        );
        match result {
            Ok(response) => {
                sub_event.set_response(response, ResponseType::Unknown);
            }
            Err(e) => {
                println!("prime query get error {}", e);
            }
        }
        sub_event.set_base_event(event);
        sub_event
    }

    fn select_root_server(&mut self) -> Host {
        let dp = self.roothint.get_delegation_point();
        let selector = self.host_selector.lock().unwrap();
        dp.get_target(&*selector).unwrap()
    }

    async fn process_query_target(&mut self, mut event: IterEvent) -> IterEvent {
        let dp = event
            .take_delegation_point()
            .expect("no dp set in query target state");
        let selector = self.host_selector.lock().unwrap().clone();
        let host = dp.get_target(&selector);
        match host {
            Some(host) => match send_query(event.get_request(), host, selector).await {
                Ok(response) => {
                    event.set_response(response, ResponseType::Unknown);
                    event.next_state(QueryState::QueryResponse);
                }
                Err(e) => {
                    println!("prime query get error {}", e);
                }
            },
            None => {
                let zone = dp.zone();
                let missing_server = dp
                    .get_missing_server()
                    .iter()
                    .find(|&n| !n.is_subdomain(zone))
                    .map(|&n| n.clone());
                if let Some(name) = missing_server {
                    let query = Message::with_query(name, RRType::A);
                    let mut sub_event =
                        IterEvent::new(query, QueryState::InitQuery, QueryState::TargetResponse);
                    sub_event.set_base_event(event);
                    return sub_event;
                } else {
                    self.error_response(&mut event, Rcode::ServFail);
                }
            }
        }
        event
    }

    fn process_query_response(&mut self, mut event: IterEvent) -> IterEvent {
        match event.get_response() {
            Some(mut response) => {
                let question = event.get_request().question.as_ref().unwrap();
                let query_type = question.typ;
                match classify_response(&question.name, query_type, &response) {
                    ResponseCategory::Answer
                    | ResponseCategory::AnswerCName
                    | ResponseCategory::NXDomain
                    | ResponseCategory::NXRRset => {
                        self.cache.lock().unwrap().add_response(response.clone());
                        event.next_state(event.get_final_state());
                    }
                    ResponseCategory::Referral => {
                        self.cache.lock().unwrap().add_response(response.clone());
                        let zone = response.question.as_ref().unwrap().name.clone();
                        let dp = DelegationPoint::new(
                            zone,
                            &response.section(SectionType::Authority).unwrap()[0],
                            response.section(SectionType::Additional).unwrap(),
                        );
                        event.set_delegation_point(dp);
                        event.next_state(QueryState::QueryTarget);
                    }
                    ResponseCategory::CName(next) => {
                        self.cache.lock().unwrap().add_response(response.clone());
                        event.set_prepend_rrsets(
                            response.take_section(SectionType::Answer).unwrap(),
                        );
                        event.set_current_request(Message::with_query(next, query_type));
                        event.next_state(QueryState::InitQuery);
                    }
                    _ => {
                        event.next_state(QueryState::QueryTarget);
                    }
                }
            }
            None => {
                event.next_state(QueryState::QueryTarget);
            }
        }
        event
    }

    fn process_prime_response(&mut self, event: IterEvent) -> IterEvent {
        event
    }
    fn process_target_response(&mut self, event: IterEvent) -> IterEvent {
        event
    }
    fn process_finished(&mut self, event: &mut IterEvent) {}
}
