use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow;
use r53::{message::SectionType, name::root, Message, MessageBuilder, RRType, RRset, Rcode};

use super::delegation_point::DelegationPoint;
use super::host_selector::{Host, HostSelector, RTTBasedHostSelector};
use super::iter_event::{IterEvent, QueryState, ResponseType};
use super::nsclient::NSClient;
use super::roothint::RootHint;
use crate::cache::MessageCache;
use crate::types::{classify_response, ResponseCategory};

const MAX_CNAME_REDIRECT_COUNT: u8 = 8;
const MAX_DEPENDENT_QUERY_COUNT: u8 = 4;
const MAX_REFERRAL_COUNT: u8 = 30;

#[derive(Clone)]
pub struct Iterator {
    cache: Arc<Mutex<MessageCache>>,
    roothint: Arc<RootHint>,
    host_selector: Arc<Mutex<RTTBasedHostSelector>>,
    client: NSClient,
}

impl Iterator {
    pub fn new(cache: Arc<Mutex<MessageCache>>) -> Self {
        let host_selector = Arc::new(Mutex::new(RTTBasedHostSelector::new(10000)));
        Self {
            cache: cache.clone(),
            roothint: Arc::new(RootHint::new()),
            host_selector: host_selector.clone(),
            client: NSClient::new(host_selector),
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
        loop {
            event = match event.get_state() {
                QueryState::InitQuery => self.process_init_query(event),
                QueryState::QueryTarget => self.process_query_target(event).await,
                QueryState::QueryResponse => self.process_query_response(event),
                QueryState::PrimeResponse => self.process_prime_response(event),
                QueryState::TargetResponse => self.process_target_response(event),
                QueryState::Finished => {
                    return Ok(self.process_finished(event));
                }
            };
        }
    }

    fn process_init_query(&mut self, mut event: IterEvent) -> IterEvent {
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
            return self.prime_root(event);
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

    fn prime_root(&mut self, event: IterEvent) -> IterEvent {
        let request = Message::with_query(root(), RRType::NS);
        let mut sub_event =
            IterEvent::new(request, QueryState::QueryTarget, QueryState::PrimeResponse);
        sub_event.set_delegation_point(self.roothint.get_delegation_point());
        sub_event.set_base_event(event);
        sub_event
    }

    async fn process_query_target(&mut self, mut event: IterEvent) -> IterEvent {
        if event.referral_count > MAX_REFERRAL_COUNT {
            self.error_response(&mut event, Rcode::ServFail);
            return event;
        }

        let dp = event
            .get_delegation_point()
            .expect("no dp set in query target state");
        let host = self.select_host(dp);
        match host {
            Some(host) => match self.client.send_query(event.get_request(), host).await {
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

    fn select_host(&mut self, dp: &DelegationPoint) -> Option<Host> {
        let selector = self.host_selector.lock().unwrap();
        dp.get_target(&*selector)
    }

    fn process_query_response(&mut self, mut event: IterEvent) -> IterEvent {
        let question = event.get_request().question.as_ref().unwrap();
        let query_type = question.typ;
        let response_category =
            classify_response(&question.name, query_type, event.get_response().unwrap());
        match response_category {
            ResponseCategory::Answer
            | ResponseCategory::AnswerCName
            | ResponseCategory::NXDomain
            | ResponseCategory::NXRRset => {
                let response = event.get_response().unwrap();
                self.cache.lock().unwrap().add_response(response.clone());
                event.next_state(event.get_final_state());
            }
            ResponseCategory::Referral => {
                let response = event.take_response().unwrap();
                let zone = response.question.as_ref().unwrap().name.clone();
                let dp = DelegationPoint::new(
                    zone,
                    &response.section(SectionType::Authority).unwrap()[0],
                    response.section(SectionType::Additional).unwrap(),
                );
                self.cache.lock().unwrap().add_response(response);
                event.referral_count += 1;
                event.set_delegation_point(dp);
                event.next_state(QueryState::QueryTarget);
            }
            ResponseCategory::CName(next) => {
                let response = event.take_response().unwrap();
                event.set_prepend_rrsets(response.section(SectionType::Answer).unwrap().clone());
                self.cache.lock().unwrap().add_response(response);
                event.set_current_request(Message::with_query(next, query_type));
                event.next_state(QueryState::InitQuery);
                event.query_restart_count += 1;
            }
            _ => {
                event.next_state(QueryState::QueryTarget);
            }
        }
        event
    }

    fn process_prime_response(&mut self, mut event: IterEvent) -> IterEvent {
        let mut base_event = event
            .take_base_event()
            .expect("prime event should always has base event");
        match event.take_response() {
            Some(response) => {
                let zone = response.question.as_ref().unwrap().name.clone();
                let dp = DelegationPoint::new(
                    zone,
                    &response.section(SectionType::Answer).unwrap()[0],
                    response.section(SectionType::Additional).unwrap(),
                );
                self.cache.lock().unwrap().add_response(response);
                base_event.set_delegation_point(dp);
                base_event.next_state(QueryState::QueryTarget);
            }
            None => {
                self.error_response(&mut base_event, Rcode::ServFail);
            }
        }
        base_event
    }

    fn process_target_response(&mut self, mut event: IterEvent) -> IterEvent {
        let mut base_event = event
            .take_base_event()
            .expect("prime event should always has base event");
        let mut response = event
            .take_response()
            .expect("target query should get response");
        let dp = base_event
            .get_mut_delegation_point()
            .expect("target query should has delegation point set");
        dp.add_glue(&response.take_section(SectionType::Answer).unwrap()[0]);
        base_event.next_state(QueryState::QueryTarget);
        base_event
    }

    fn process_finished(&mut self, mut event: IterEvent) -> Message {
        event.generate_final_response()
    }
}
