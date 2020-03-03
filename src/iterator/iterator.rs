use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow;
use r53::{Message, MessageBuilder, Rcode};

use super::delegation_point::DelegationPoint;
use super::iter_event::{IterEvent, QueryState, ResponseType};
use super::roothint::RootHint;
use crate::cache::MessageCache;

const MAX_CNAME_REDIRECT_COUNT: u8 = 8;
const MAX_DEPENDENT_QUERY_COUNT: u8 = 4;

#[derive(Clone)]
pub struct Iterator {
    cache: Arc<Mutex<MessageCache>>,
    roothint: Arc<RootHint>,
}

impl Iterator {
    pub fn new(cache: Arc<Mutex<MessageCache>>) -> Self {
        Self {
            cache: cache.clone(),
            roothint: Arc::new(RootHint::new()),
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
        while !done {
            done = match event.get_state() {
                QueryState::InitQuery => self.process_init_query(&mut event).await,
                QueryState::QueryTarget => self.process_query_target(&mut event),
                QueryState::QueryResponse => self.process_query_response(&mut event),
                QueryState::PrimeResponse => self.process_prime_response(&mut event),
                QueryState::TargetResponse => self.process_target_response(&mut event),
                QueryState::Finished => self.process_finished(&mut event),
            };
        }
        event.get_response()
    }

    async fn process_init_query(&mut self, event: &mut IterEvent) -> bool {
        if event.query_restart_count > MAX_CNAME_REDIRECT_COUNT {
            return self.error_response(event, Rcode::ServFail);
        }

        if event.get_depth() > MAX_DEPENDENT_QUERY_COUNT {
            return self.error_response(event, Rcode::ServFail);
        }
        /*

        let mut cache = self.cache.lock().unwrap();
        if let Some(response) = cache.gen_response(&event.get_request()) {
            event.set_response(response, ResponseType::Answer);
            return self.next_state(event, event.get_final_state());
        } else if let Some(dp) = DelegationPoint::from_cache(
            event.get_request().question.as_ref().unwrap().name.clone(),
            &mut cache,
        ) {
            event.set_delegation_point(dp);
            return self.next_state(event, QueryState::QueryTarget);
        } else {
            return self.prime_root(event).await;
        }
        */
        false
    }

    fn error_response(&mut self, event: &mut IterEvent, rcode: Rcode) -> bool {
        let mut response = event.get_request().clone();
        MessageBuilder::new(&mut response)
            .make_response()
            .rcode(rcode)
            .done();
        event.set_response(response, ResponseType::Throwaway);
        self.next_state(event, event.get_final_state())
    }

    fn lookup_cache(&mut self, event: &mut IterEvent) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if let Some(response) = cache.gen_response(&event.get_request()) {
            event.set_response(response, ResponseType::Answer);
            return self.next_state(event, event.get_final_state());
        } else if let Some(dp) = DelegationPoint::from_cache(
            event.get_request().question.as_ref().unwrap().name.clone(),
            &mut cache,
        ) {
            event.set_delegation_point(dp);
            return self.next_state(event, QueryState::QueryTarget);

    }

    fn next_state(&mut self, event: &mut IterEvent, next: QueryState) -> bool {
        event.set_state(next);
        false
    }

    async fn prime_root(&mut self, event: &mut IterEvent) -> bool {
        true
    }

    fn process_query_target(&mut self, event: &mut IterEvent) -> bool {
        false
    }

    fn process_query_response(&mut self, event: &mut IterEvent) -> bool {
        false
    }
    fn process_prime_response(&mut self, event: &mut IterEvent) -> bool {
        false
    }
    fn process_target_response(&mut self, event: &mut IterEvent) -> bool {
        false
    }
    fn process_finished(&mut self, event: &mut IterEvent) -> bool {
        false
    }
}
