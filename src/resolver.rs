use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use crate::auth::{AuthServer, AuthZone};
use crate::config::VanguardConfig;
use crate::iterator::{new_iterator, Iterator};
use crate::types::{Query, QueryHandler};
use anyhow;
use r53::Message;

#[derive(Clone)]
pub struct Resolver {
    auth_server: AuthServer,
    iterator: Iterator,
}

impl Resolver {
    pub fn new(config: &VanguardConfig) -> Self {
        let auth_server = AuthServer::new(&config.auth);
        Resolver {
            auth_server,
            iterator: new_iterator(config),
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
