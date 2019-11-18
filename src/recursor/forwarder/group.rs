use super::forwarder::Forwarder;
use crate::{
    config::ForwarderConfig,
    recursor::util::{Nameserver, NameserverStore},
};
use datasrc::RBTree;
use r53::Name;
use std::{cell::Cell, cmp::Eq, net::SocketAddr, ops::Rem, str::FromStr};

pub struct ForwarderPool {
    forwarders: Vec<Forwarder>,
}

impl ForwarderPool {
    pub fn new(conf: &ForwarderConfig) -> Self {
        let mut forwarders: Vec<Forwarder> = Vec::new();
        for conf in &conf.forwarders {
            for address in &conf.addresses {
                let address = address.parse().unwrap();
                if forwarders.iter().all(|f| f.get_addr() != address) {
                    forwarders.push(Forwarder::new(address));
                }
            }
        }
        ForwarderPool {
            forwarders: forwarders,
        }
    }

    pub fn init_groups(&self, groups: &mut RBTree<ForwarderGroup>, conf: &ForwarderConfig) {
        for conf in &conf.forwarders {
            let name = Name::new(conf.zone_name.as_ref()).unwrap();
            let indexes = conf
                .addresses
                .iter()
                .fold(Vec::new(), |mut indexes, address| {
                    let address = address.parse().unwrap();
                    indexes.push(
                        self.forwarders
                            .iter()
                            .position(|f| f.get_addr() == address)
                            .unwrap(),
                    );
                    indexes
                });
            groups.insert(name, Some(ForwarderGroup::new(indexes)));
        }
    }

    pub fn get_forwarder(&self, index: usize) -> Forwarder {
        self.forwarders[index]
    }

    pub fn update_rtt(&mut self, forwarder: &Forwarder) {
        let position = self.forwarders.iter().position(|s| s == forwarder);
        if let Some(pos) = position {
            self.forwarders[pos].set_rtt(forwarder.get_rtt());
        }
    }
}

#[derive(Clone)]
pub struct ForwarderGroup {
    indexes: Vec<usize>,
}

impl ForwarderGroup {
    pub fn new(indexes: Vec<usize>) -> Self {
        ForwarderGroup { indexes }
    }

    pub fn select_forwarder(&self, pool: &ForwarderPool) -> Forwarder {
        if self.indexes.len() == 1 {
            return pool.get_forwarder(self.indexes[0]);
        }

        let count = self.indexes.len();
        self.indexes
            .iter()
            .fold(Vec::with_capacity(count), |mut fs, &index| {
                fs.push(pool.get_forwarder(index));
                fs
            })
            .iter()
            .min()
            .map(|f| *f)
            .unwrap()
    }
}
