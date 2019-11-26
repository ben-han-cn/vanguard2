use super::zones::AuthZone;
use crate::{config::AuthorityConfig, server::Query};
use failure;
use futures::{prelude::*, Future};
use r53::Name;
use std::fs;
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
        let mut zones = AuthZone::new();
        for zone_conf in conf.zones.iter() {
            let zone_cotent = fs::read_to_string(&zone_conf.file_path).unwrap();
            zones
                .add_zone(Name::new(&zone_conf.name).unwrap(), &zone_cotent)
                .unwrap();
        }
        AuthServer {
            zones: Arc::new(RwLock::new(zones)),
        }
    }

    pub fn handle_query(&self, mut query: Query) -> Query {
        let zones = self.zones.read().unwrap();
        if zones.handle_query(&mut query.message) {
            query.done = true;
        }
        query
    }
}
