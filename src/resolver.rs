use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};

use crate::auth::{AuthServer, AuthZone};
use crate::cache::MessageCache;
use crate::config::VanguardConfig;
use crate::forwarder::ForwarderManager;
use crate::recursor::Recursor;
use crate::types::{Query, QueryHandler};
use anyhow::{self, bail};
use r53::Message;

const DEFAULT_MESSAGE_CACHE_SIZE: usize = 10000;

#[derive(Clone)]
pub struct Resolver {
    auth_server: AuthServer,
    forwarder: ForwarderManager,
    recursor: Recursor,
    cache: Arc<Mutex<MessageCache>>,
}

impl Resolver {
    pub fn new(config: &VanguardConfig) -> Self {
        let auth_server = AuthServer::new(&config.auth);
        let forwarder = ForwarderManager::new(&config.forwarder);
        let cache = Arc::new(Mutex::new(MessageCache::new(DEFAULT_MESSAGE_CACHE_SIZE)));
        let recursor = Recursor::new(&config.recursor, cache.clone());
        Resolver {
            auth_server,
            forwarder,
            recursor,
            cache,
        }
    }

    pub fn zone_data(&self) -> Arc<RwLock<AuthZone>> {
        self.auth_server.zone_data()
    }

    async fn do_query(&mut self, query: Query) -> anyhow::Result<Message> {
        if let Some(response) = self.auth_server.handle_query(&query) {
            return Ok(response);
        }

        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(response) = cache.gen_response(query.request()) {
                return Ok(response);
            }
        }

        match self.forwarder.handle_query(&query).await {
            Ok(Some(response)) => {
                self.cache.lock().unwrap().add_response(response.clone());
                return Ok(response);
            }
            Ok(None) => {}
            Err(e) => {
                bail!("forward get err {:?}", e);
            }
        }

        match self.recursor.handle_query(query.request()).await {
            Ok(response) => {
                return Ok(response);
            }
            Err(e) => {
                bail!("recursor get err {:?}", e);
            }
        }
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
