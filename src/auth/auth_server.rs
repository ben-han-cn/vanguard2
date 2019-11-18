use super::zones::AuthZone;
use crate::{config::AuthorityConfig, server::Query};
use failure;
use futures::{prelude::*, Future};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AuthServer {
    zones: Arc<RwLock<AuthZone>>,
}

pub struct AuthFuture {
    query: Option<Query>,
    zones: Arc<RwLock<AuthZone>>,
}

impl AuthServer {
    pub fn new(conf: &AuthorityConfig) -> Self {
        AuthServer {
            zones: Arc::new(RwLock::new(AuthZone::new())),
        }
    }

    pub fn zones(&self) -> Arc<RwLock<AuthZone>> {
        self.zones.clone()
    }

    pub fn handle_query(&self, query: Query) -> AuthFuture {
        AuthFuture::new(self.zones.clone(), query)
    }
}

impl AuthFuture {
    pub fn new(zones: Arc<RwLock<AuthZone>>, query: Query) -> Self {
        AuthFuture {
            query: Some(query),
            zones,
        }
    }
}

impl Future for AuthFuture {
    type Item = Query;
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let zones = self.zones.read().unwrap();
        let query = self.query.as_mut().unwrap();
        if zones.handle_query(&mut query.message) {
            query.done = true;
        }
        return Ok(Async::Ready(self.query.take().unwrap()));
    }
}
