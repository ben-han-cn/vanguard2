use super::zones::AuthZone;
use crate::{config::AuthorityConfig, types::Query};
use r53::{Message, Name};
use std::error::Error;
use std::fs;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tonic::transport::Server;

#[derive(Clone)]
pub struct AuthServer {
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

    pub fn handle_query(&self, query: &Query) -> Option<Message> {
        self.zones.read().unwrap().handle_query(query)
    }

    pub fn zone_data(&self) -> Arc<RwLock<AuthZone>> {
        self.zones.clone()
    }
}
