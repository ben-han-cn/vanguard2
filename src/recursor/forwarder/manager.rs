use super::{
    forwarder::Forwarder,
    group::{ForwarderGroup, ForwarderPool},
};
use crate::{
    config::ForwarderConfig,
    recursor::util::{Nameserver, NameserverStore, Sender},
};
use datasrc::RBTree;
use futures::{prelude::*, Future};
use r53::{Message, Name, RRType};
use std::{
    mem,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct ForwarderManager {
    forwarders: Arc<RBTree<ForwarderGroup>>,
    pool: Arc<RwLock<ForwarderPool>>,
}

impl ForwarderManager {
    pub fn new(conf: &ForwarderConfig) -> Self {
        let pool = ForwarderPool::new(conf);
        let mut groups = RBTree::new();
        pool.init_groups(&mut groups, conf);
        ForwarderManager {
            forwarders: Arc::new(groups),
            pool: Arc::new(RwLock::new(pool)),
        }
    }

    pub fn handle_query(
        &self,
        name: &Name,
        typ: RRType,
    ) -> Option<Sender<Forwarder, ForwarderManager>> {
        if let Some(forwarder) = self.get_forwarder(name) {
            Some(Sender::new(
                Message::with_query(name.clone(), typ),
                forwarder,
                self.clone(),
            ))
        } else {
            None
        }
    }

    fn get_forwarder(&self, name: &Name) -> Option<Forwarder> {
        let result = self.forwarders.find(name);
        if let Some(selecotr) = result.get_value() {
            let pool = self.pool.read().unwrap();
            return Some(selecotr.select_forwarder(&pool));
        } else {
            return None;
        }
    }
}

impl NameserverStore<Forwarder> for ForwarderManager {
    fn update_nameserver_rtt(&self, forwarder: &Forwarder) {
        let mut pool = self.pool.write().unwrap();
        pool.update_rtt(forwarder);
    }
}
