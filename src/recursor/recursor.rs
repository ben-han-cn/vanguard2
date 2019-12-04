use super::{
    nsas::NSAddressStore, resolver::Resolver, roothint::RootHint, running_query::RunningQuery,
};
use crate::{cache::MessageCache, config::RecursorConfig, error::VgError, types::Query};
use failure;
use futures::Future;
use r53::{name, Message, Name, RRType};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::prelude::*;

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
}

impl Resolver for Recursor {
    fn handle_query(
        &self,
        query: Message,
    ) -> Pin<Box<Future<Output = Result<Message, failure::Error>> + Send>> {
        Box::pin(RunningQuery::new(query, self.clone()).handle_query())
    }
}
