use super::{
    forwarder::Forwarder,
    group::{ForwarderGroup, ForwarderPool},
};
use crate::{
    types::Query,
    config::ForwarderConfig,
    nameserver::{Nameserver, NameserverStore, Sender},
};
use failure;
use domaintree::DomainTree;
use futures::{prelude::*, Future};
use r53::{Message, Name, RRType};
use std::{
    mem,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct ForwarderManager {
    forwarders: Arc<DomainTree<ForwarderGroup>>,
    pool: Arc<RwLock<ForwarderPool>>,
}

impl ForwarderManager {
    pub fn new(conf: &ForwarderConfig) -> Self {
        let pool = ForwarderPool::new(conf);
        let mut groups = DomainTree::new();
        pool.init_groups(&mut groups, conf);
        ForwarderManager {
            forwarders: Arc::new(groups),
            pool: Arc::new(RwLock::new(pool)),
        }
    }

    pub async fn handle_query(&self, mut query: Query) -> Result<Query, failure::Error> {
        if let Some(forwarder) = self.select_nameserver(&query) {
            let question = &query.message.question.unwrap();
            let mut tmp_query = Message::with_query(question.name.clone(), question.typ);
            tmp_query.header.set_flag(r53::HeaderFlag::RecursionDesired, true);
            let mut sender = Sender::new(tmp_query, forwarder, self.clone());
            let mut response = sender.send_query().await?;
            response.header.id = query.message.header.id;
            query.message = response;
            query.done = true;
        }
        Ok(query)
    }
}

impl NameserverStore for ForwarderManager {
    type Nameserver = Forwarder;

    fn select_nameserver(&self, query: &Query) -> Option<Forwarder> {
        let result = self.forwarders.find(&query.message.question.as_ref().unwrap().name);
        if let Some(selecotr) = result.get_value() {
            let pool = self.pool.read().unwrap();
            return Some(selecotr.select_forwarder(&pool));
        } else {
            return None;
        }
    }

    fn update_nameserver_rtt(&self, forwarder: &Forwarder) {
        let mut pool = self.pool.write().unwrap();
        pool.update_rtt(forwarder);
    }
}
