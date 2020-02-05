use super::{
    forwarder::Forwarder,
    group::{ForwarderGroup, ForwarderPool},
};
use crate::{
    config::ForwarderConfig,
    nameserver::{send_query, NameserverStore},
    types::Query,
};
use anyhow;
use domaintree::DomainTree;
use r53::Message;
use std::sync::{Arc, RwLock};

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

    pub async fn handle_query(&self, query: &Query) -> anyhow::Result<Option<Message>> {
        if let Some(forwarder) = self.select_nameserver(query) {
            let question = query.question();
            let mut tmp_query = Message::with_query(question.name.clone(), question.typ);
            tmp_query
                .header
                .set_flag(r53::HeaderFlag::RecursionDesired, true);
            let mut response = send_query(&tmp_query, forwarder, self.clone()).await?;
            response.header.id = query.request().header.id;
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }

    fn select_nameserver(&self, query: &Query) -> Option<Forwarder> {
        let result = self.forwarders.find(&query.question().name);
        if let Some(selecotr) = result.get_value() {
            let pool = self.pool.read().unwrap();
            return Some(selecotr.select_forwarder(&pool));
        } else {
            return None;
        }
    }
}

impl NameserverStore for ForwarderManager {
    type Nameserver = Forwarder;

    fn update_nameserver_rtt(&self, forwarder: &Forwarder) {
        let mut pool = self.pool.write().unwrap();
        pool.update_rtt(forwarder);
    }
}
