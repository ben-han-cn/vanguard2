use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow;
use r53::{name::root, Message, MessageBuilder, RData, RRType, Rcode, SectionType};

use super::aggregate_client::AggregateClient;
use super::cache::MessageCache;
use super::delegation_point::DelegationPoint;
use super::forwarder::ForwarderManager;
use super::host_selector::{Host, RTTBasedHostSelector};
use super::iter_event::{IterEvent, QueryState};
use super::nsclient::{NSClient, NameServerClient};
use super::roothint::RootHint;
use super::util::{sanitize_and_classify_response, ResponseCategory};
use crate::config::VanguardConfig;
use crate::types::{Request, Response};

const MAX_CNAME_REDIRECT_COUNT: u8 = 8;
const MAX_DEPENDENT_QUERY_COUNT: u8 = 4;
const MAX_REFERRAL_COUNT: u8 = 10;
const MAX_ERROR_COUNT: u8 = 5;
const ITERATOR_TIMEOUT: Duration = Duration::from_secs(10);

pub fn new_iterator(conf: &VanguardConfig) -> Iterator<AggregateClient<NSClient>> {
    let host_selector = Arc::new(Mutex::new(RTTBasedHostSelector::new(10000)));
    let cache = Arc::new(Mutex::new(MessageCache::new(conf.recursor.cache_size)));
    let client = NSClient::new(host_selector.clone());
    let forwarder = Arc::new(ForwarderManager::new(&conf.forwarder));
    Iterator::new(
        cache,
        host_selector,
        forwarder,
        AggregateClient::new(client),
    )
}

#[derive(Clone)]
pub struct Iterator<C = AggregateClient<NSClient>> {
    cache: Arc<Mutex<MessageCache>>,
    roothint: Arc<RootHint>,
    host_selector: Arc<Mutex<RTTBasedHostSelector>>,
    forwarder: Arc<ForwarderManager>,
    client: C,
}

impl<C: NameServerClient + 'static> Iterator<C> {
    pub fn new(
        cache: Arc<Mutex<MessageCache>>,
        host_selector: Arc<Mutex<RTTBasedHostSelector>>,
        forwarder: Arc<ForwarderManager>,
        client: C,
    ) -> Self {
        Self {
            cache: cache,
            roothint: Arc::new(RootHint::new()),
            host_selector,
            forwarder,
            client,
        }
    }

    pub fn resolve(
        &mut self,
        req: Request,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Response>> + Send>> {
        Box::pin(self.clone().do_resolve(req))
    }

    async fn do_resolve(mut self, req: Request) -> anyhow::Result<Response> {
        let mut event = IterEvent::new(req.request, QueryState::InitQuery, QueryState::Finished);
        loop {
            debug!(
                "event {:?} with query {}",
                event.get_state(),
                event.get_request().question.as_ref().unwrap()
            );
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
        }

        if self.find_delegation_point(&mut event) {
            return event;
        }

        return self.prime_root(event);
    }

    fn error_response(&mut self, event: &mut IterEvent, rcode: Rcode) {
        let mut response = event.get_request().clone();
        MessageBuilder::new(&mut response)
            .make_response()
            .rcode(rcode)
            .done();
        event.set_response(response, ResponseCategory::ServerFail);
        event.next_state(event.get_final_state())
    }

    fn lookup_cache(&mut self, event: &mut IterEvent) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if let Some(response) = cache.gen_response(&event.get_request()) {
            event.set_response(response, ResponseCategory::Answer);
            event.next_state(event.get_final_state());
            event.cache_hit = true;
            true
        } else if let Some(response) = cache.gen_cname_response(&event.get_request()) {
            event.set_response(response, ResponseCategory::CName);
            event.next_state(QueryState::QueryResponse);
            true
        } else {
            false
        }
    }

    fn find_delegation_point(&mut self, event: &mut IterEvent) -> bool {
        let qname = &event.get_request().question.as_ref().unwrap().name;
        if let Some(dp) = self.forwarder.get_delegation_point(qname).or_else(|| {
            let mut cache = self.cache.lock().unwrap();
            DelegationPoint::from_cache(qname, &mut cache)
        }) {
            event.set_delegation_point(dp);
            event.next_state(QueryState::QueryTarget);
            true
        } else {
            false
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
        if event.referral_count > MAX_REFERRAL_COUNT || event.error_count > MAX_ERROR_COUNT {
            self.error_response(&mut event, Rcode::ServFail);
            return event;
        }

        let dp = event
            .get_delegation_point()
            .expect("no dp set in query target state");
        let host = self.select_host(dp);
        match host {
            Some(host) => match self.client.query(event.get_request(), host).await {
                Ok(mut response) => {
                    let question = event.get_request().question.as_ref().unwrap();
                    let response_category = match sanitize_and_classify_response(
                        dp.zone(),
                        &question.name,
                        question.typ,
                        &mut response,
                    ) {
                        Ok(category) => category,
                        Err(e) => {
                            warn!(
                                "send query [{}] to {}[{}] get response {} with err {:?}",
                                event.get_request().question.as_ref().unwrap(),
                                dp.zone(),
                                host.to_string(),
                                response,
                                e
                            );

                            ResponseCategory::ServerFail
                        }
                    };

                    match response_category {
                        ResponseCategory::Answer
                        | ResponseCategory::NXDomain
                        | ResponseCategory::NXRRset => {
                            self.cache.lock().unwrap().add_response(response.clone());
                        }
                        ResponseCategory::CName | ResponseCategory::Referral => {
                            self.cache
                                .lock()
                                .unwrap()
                                .add_rrset_in_response(response.clone());
                        }
                        ResponseCategory::ServerFail => {
                            event
                                .get_mut_delegation_point()
                                .expect("no dp set in query target state")
                                .mark_server_lame(host);
                            return event;
                        }
                    }
                    event.set_response(response, response_category);
                    event.next_state(QueryState::QueryResponse);
                }
                Err(e) => {
                    debug!(
                        "send query [{}] to {}[{}] failed with err {:?}",
                        event.get_request().question.as_ref().unwrap(),
                        dp.zone(),
                        host.to_string(),
                        e
                    );

                    if event.start_time.elapsed() > ITERATOR_TIMEOUT {
                        self.error_response(&mut event, Rcode::ServFail);
                    } else {
                        event.error_count += 1;
                    }
                }
            },
            None => {
                let missing_server = dp.get_missing_server();
                if let Some(name) = missing_server {
                    let query = Message::with_query(name, RRType::A);
                    let mut sub_event =
                        IterEvent::new(query, QueryState::InitQuery, QueryState::TargetResponse);
                    sub_event.set_base_event(event);
                    return sub_event;
                } else {
                    warn!("no nameserver is usable zone {}", dp.zone());
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
        let (response, response_category) = event.take_response();
        match response_category {
            ResponseCategory::Answer | ResponseCategory::NXDomain | ResponseCategory::NXRRset => {
                event.set_response(response, response_category);
                event.next_state(event.get_final_state());
            }

            ResponseCategory::Referral => {
                let dp = DelegationPoint::from_referral_response(&response);
                event.referral_count += 1;
                event.set_delegation_point(dp);
                event.next_state(QueryState::QueryTarget);
            }

            ResponseCategory::CName => {
                let answers = response.section(SectionType::Answer).unwrap();
                let next = match answers[answers.len() - 1].rdatas[0] {
                    RData::CName(ref cname) => cname.name.clone(),
                    _ => unreachable!(),
                };
                let query_type = event.get_request().question.as_ref().unwrap().typ;
                event.add_prepend_rrsets(response.section(SectionType::Answer).unwrap().clone());
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
        let (response, category) = event.take_response();
        match category {
            ResponseCategory::Answer => {
                let dp = DelegationPoint::from_ns_rrset(
                    &response.section(SectionType::Answer).unwrap()[0],
                    response.section(SectionType::Additional).unwrap(),
                );
                base_event.set_delegation_point(dp);
                base_event.next_state(QueryState::QueryTarget);
            }
            _ => {
                self.error_response(&mut base_event, Rcode::ServFail);
            }
        }
        base_event
    }

    fn process_target_response(&mut self, mut event: IterEvent) -> IterEvent {
        let mut base_event = event
            .take_base_event()
            .expect("prime event should always has base event");

        base_event.next_state(QueryState::QueryTarget);
        let dp = base_event
            .get_mut_delegation_point()
            .expect("target query should has delegation point set");

        let (mut response, category) = event.take_response();
        if category == ResponseCategory::Answer {
            if let Some(mut answers) = response.take_section(SectionType::Answer) {
                let mut last_rrset = answers.pop().unwrap();
                //in caes crruent response has cname
                //glue has out of zone canme
                let original_name = event
                    .get_original_request()
                    .question
                    .as_ref()
                    .unwrap()
                    .name
                    .clone();
                if last_rrset.name != original_name {
                    warn!("glue {} has cname", original_name);
                    last_rrset.name = original_name;
                }

                if last_rrset.typ == RRType::A && dp.add_glue(&last_rrset) {
                    return base_event;
                }
            }
        }

        let nameserver = &event.get_original_request().question.as_ref().unwrap().name;
        debug!("couldn't get any address for nameserver {}", nameserver);
        dp.add_probed_server(nameserver);
        base_event
    }

    fn process_finished(&mut self, event: IterEvent) -> Response {
        let has_cname_redirect = event.query_restart_count > 0;
        let resp = event.generate_final_response();
        if has_cname_redirect && resp.response.header.rcode == Rcode::NoError {
            self.cache
                .lock()
                .unwrap()
                .add_response(resp.response.clone());
        }
        resp
    }
}
