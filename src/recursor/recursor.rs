use super::{
    cache::MessageCache, forwarder::ForwarderManager, nsas::NSAddressStore,
    recursor_future::RecursorFuture, roothint::RootHint, running_query::RunningQuery,
};
use crate::{config::ForwarderConfig, config::RecursorConfig, error::VgError, server::Query};
use failure;
use futures::Future;
use r53::{name, Message, Name, RRType};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::prelude::*;

const DEFAULT_MESSAGE_CACHE_SIZE: usize = 10000;

pub trait Resolver: Clone + Send + 'static {
    type Query: Future<Item = Message, Error = failure::Error> + Send + 'static;
    fn new_query(&self, query: Message, depth: usize) -> Self::Query;
}

#[derive(Clone)]
pub struct Recursor {
    pub(crate) cache: Arc<Mutex<MessageCache>>,
    pub(crate) nsas: NSAddressStore,
    pub(crate) roothint: Arc<RootHint>,
    pub(crate) forwarder: ForwarderManager,
}

impl Recursor {
    pub fn new(recursor_cfg: &RecursorConfig, forwarder_cfg: &ForwarderConfig) -> Self {
        Recursor {
            cache: Arc::new(Mutex::new(MessageCache::new(DEFAULT_MESSAGE_CACHE_SIZE))),
            nsas: NSAddressStore::new(),
            roothint: Arc::new(RootHint::new()),
            forwarder: ForwarderManager::new(forwarder_cfg),
        }
    }

    pub fn handle_query(&self, query: Query) -> RecursorFuture {
        RecursorFuture::new(self.clone(), query)
    }
}

impl Resolver for Recursor {
    type Query = RunningQuery;
    fn new_query(&self, query: Message, depth: usize) -> RunningQuery {
        RunningQuery::new(query, self.clone(), depth)
    }
}
