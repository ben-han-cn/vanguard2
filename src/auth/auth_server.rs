use super::zones::AuthZone;
use crate::{config::AuthorityConfig, types::Request};
use r53::{Message, Name};
use std::fs;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AuthServer {
    zones: Arc<RwLock<AuthZone>>,
}

impl AuthServer {
    pub fn new(conf: &AuthorityConfig) -> Self {
        let mut zones = AuthZone::new();
        for zone_conf in conf.zones.iter() {
            let zone_content = fs::read_to_string(&zone_conf.file_path).unwrap();
            zones
                .add_zone(Name::new(&zone_conf.name).unwrap(), &zone_content)
                .unwrap();
        }
        AuthServer {
            zones: Arc::new(RwLock::new(zones)),
        }
    }

    pub fn resolve(&self, req: &Request) -> Option<Message> {
        self.zones.read().unwrap().resolve(req)
    }

    pub fn zone_data(&self) -> Arc<RwLock<AuthZone>> {
        self.zones.clone()
    }
}
