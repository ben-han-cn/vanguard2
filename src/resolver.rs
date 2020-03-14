use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use crate::auth::{AuthServer, AuthZone};
use crate::config::VanguardConfig;
use crate::iterator::{new_iterator, Iterator};
use crate::types::{Handler, Request, Response};
use anyhow;

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

    async fn do_resolve(&mut self, req: Request) -> anyhow::Result<Response> {
        if let Some(response) = self.auth_server.resolve(&req) {
            return Ok(Response::new(response));
        }

        self.iterator.resolve(req).await
    }
}

impl Handler for Resolver {
    fn resolve(
        &mut self,
        req: Request,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Response>> + Send + '_>> {
        Box::pin(self.do_resolve(req))
    }
}
