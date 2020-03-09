use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};

use crate::auth::{AuthServer, AuthZone};
use crate::cache::MessageCache;
use crate::config::VanguardConfig;
use crate::iterator::{Iterator, NewIterator};
use crate::recursor::Recursor;
use crate::types::{Query, QueryHandler};
use anyhow::{self, bail};
use r53::Message;

const DEFAULT_MESSAGE_CACHE_SIZE: usize = 10000;

#[derive(Clone)]
pub struct Resolver {
    auth_server: AuthServer,
    iterator: Iterator,
}

impl Resolver {
    pub fn new(config: &VanguardConfig) -> Self {
        let auth_server = AuthServer::new(&config.auth);
        let cache = Arc::new(Mutex::new(MessageCache::new(DEFAULT_MESSAGE_CACHE_SIZE)));
        Resolver {
            auth_server,
            iterator: NewIterator(cache),
        }
    }

    pub fn zone_data(&self) -> Arc<RwLock<AuthZone>> {
        self.auth_server.zone_data()
    }

    async fn do_query(&mut self, query: Query) -> anyhow::Result<Message> {
        if let Some(response) = self.auth_server.handle_query(&query) {
            return Ok(response);
        }

        self.iterator.resolve(query.request).await
    }
}

impl QueryHandler for Resolver {
    fn handle_query(
        &mut self,
        query: Query,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Message>> + Send + '_>> {
        Box::pin(self.do_query(query))
    }
}
