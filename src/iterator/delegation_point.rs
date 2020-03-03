use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use r53::{Name, RData, RRType, RRset};

use super::host_selector::{Host, HostSelector};
use crate::cache::MessageCache;

#[derive(Debug, Clone)]
pub struct DelegationPoint {
    zone: Name,
    server_and_hosts: HashMap<Name, Vec<Host>>,
}

impl DelegationPoint {
    pub fn new(zone: Name, ns: &RRset, glues: &Vec<RRset>) -> Self {
        let mut server_and_hosts = ns.rdatas.iter().fold(
            HashMap::new(),
            |mut servers: HashMap<Name, Vec<Host>>, rdata| {
                if let RData::NS(ref ns) = rdata {
                    servers.insert(ns.name.clone(), Vec::new());
                }
                servers
            },
        );

        let mut dp = Self {
            zone,
            server_and_hosts,
        };

        glues.iter().for_each(|glue| dp.add_glue(glue));
        dp
    }

    pub fn from_cache(zone: Name, cache: &mut MessageCache) -> Option<Self> {
        let closest_zone = cache.get_deepest_ns(&zone)?;
        let ns = cache.get_rrset(&zone, RRType::NS)?;
        let glues = ns.rdatas.iter().fold(Vec::new(), |mut glues, rdata| {
            if let RData::NS(ref ns) = rdata {
                if let Some(rrset) = cache.get_rrset(&ns.name, RRType::A) {
                    glues.push(rrset);
                }
            }
            glues
        });
        Some(DelegationPoint::new(zone, &ns, &glues))
    }

    pub fn add_glue(&mut self, glue: &RRset) {
        if let Some(hosts_) = self.server_and_hosts.get_mut(&glue.name) {
            let mut hosts = glue
                .rdatas
                .iter()
                .fold(Vec::new(), |mut hosts: Vec<Host>, rdata| {
                    if let RData::A(ref a) = rdata {
                        hosts.push(IpAddr::V4(a.host));
                    }
                    hosts
                });
            hosts_.append(&mut hosts);
        }
    }

    pub fn get_target<S: HostSelector>(&self, selector: &S) -> Option<Host> {
        let hosts: Vec<Host> = self
            .server_and_hosts
            .values()
            .flatten()
            .map(|a| *a)
            .collect();
        if hosts.is_empty() {
            None
        } else {
            Some(selector.select(&hosts))
        }
    }

    pub fn get_missing_server(&self) -> Vec<&Name> {
        let mut missing_names = Vec::new();
        for (name, hosts) in self.server_and_hosts.iter() {
            if hosts.len() == 0 {
                missing_names.push(name);
            }
        }
        missing_names
    }
}
