use super::{
    nsas::NSAddressStore, resolver::Resolver, roothint::RootHint, running_query::RunningQuery,
};
use crate::{cache::MessageCache, config::RecursorConfig};
use failure;
use futures::Future;
use r53::Message;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const DEFAULT_MESSAGE_CACHE_SIZE: usize = 10000;
#[derive(Clone)]
pub struct Recursor {
    pub(crate) cache: Arc<Mutex<MessageCache>>,
    pub(crate) nsas: NSAddressStore,
    pub(crate) roothint: Arc<RootHint>,
}

impl Recursor {
    pub fn new(recursor_cfg: &RecursorConfig, cache: Arc<Mutex<MessageCache>>) -> Self {
        Recursor {
            cache: cache,
            nsas: NSAddressStore::new(),
            roothint: Arc::new(RootHint::new()),
        }
    }

    pub fn handle_query(
        &self,
        query: &Message,
    ) -> Pin<Box<dyn Future<Output = Result<Message, failure::Error>> + Send>> {
        self.resolve(query, 0)
    }
}

impl Resolver for Recursor {
    fn resolve(
        &self,
        query: &Message,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Message, failure::Error>> + Send>> {
        Box::pin(RunningQuery::new(query, self.clone(), depth).handle_query())
    }
}
