use std::sync::{Arc, Mutex};
use crate::auth::AuthServer;
use crate::config::VanguardConfig;
use crate::types::{Query, QueryHandler};
use crate::forwarder::ForwarderManager;
use crate::cache::MessageCache;
use crate::recursor::{Recursor};
use r53::Message;
use futures::Future;
use std::pin::Pin;

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

    async fn do_query(self, query: &Query) -> Option<Message> {
        if let Some(response) = self.auth_server.handle_query(&query) {
            return Some(response);
        }

        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(response) = cache.gen_response(query.request()) {
                return Some(response);
            }
        }

        match self.forwarder.handle_query(&query).await {
            Ok(Some(response)) => {
                self.cache.lock().unwrap().add_response(response.clone());
                return Some(response);
            }
            Ok(None) => {
            }
            Err(e) => {
                println!("forward get err {:?}", e);
            }
        }

        match self.recursor.handle_query(query.request()).await {
            Ok(response) => {
                return Some(response);
            }
            Err(e) => {
                println!("recursor get err {:?}", e);
            }
        }
        return None;
    }
}

impl QueryHandler for Resolver {
    fn handle_query(self, query: &Query) -> Pin<Box<dyn Future<Output=Option<Message>> + Send + '_>> {
        Box::pin(self.do_query(query))
    }
}
