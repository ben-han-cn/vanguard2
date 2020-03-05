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
        let ns = cache.get_rrset(&closest_zone, RRType::NS)?;
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

    pub fn zone(&self) -> &Name {
        &self.zone
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

#[cfg(test)]
mod tests {
    use super::super::host_selector::{Host, HostSelector};
    use super::DelegationPoint;
    use r53::{Name, RRset};
    use std::str::FromStr;
    use std::time::Duration;

    fn merge_rrset(mut rrset1: RRset, mut rrset2: RRset) -> RRset {
        assert!(rrset1.is_same_rrset(&rrset2));
        rrset1.rdatas.append(&mut rrset2.rdatas);
        rrset1
    }

    #[test]
    fn test_delegation_point_new() {
        let zone = Name::new("com").unwrap();
        let ns1 = RRset::from_str("com. 3600  IN NS ns1.com").unwrap();
        let ns2 = RRset::from_str("com. 3600  IN NS ns2.com").unwrap();
        let ns = merge_rrset(ns1, ns2);

        let glue1 = RRset::from_str("ns1.com. 3600  IN A 1.1.1.1").unwrap();
        let glue2 = RRset::from_str("ns2.com. 3600  IN A 2.2.2.2").unwrap();

        let dp = DelegationPoint::new(zone, &ns, &vec![glue1, glue2]);
        assert!(dp.get_missing_server().is_empty());
    }

    #[test]
    fn test_missing_server() {
        let zone = Name::new("com").unwrap();
        let ns1 = RRset::from_str("com. 3600  IN NS ns1.com").unwrap();
        let ns2 = RRset::from_str("com. 3600  IN NS ns2.com").unwrap();
        let ns = merge_rrset(ns1, ns2);

        let glue1 = RRset::from_str("ns1.com. 3600  IN A 1.1.1.1").unwrap();

        let dp = DelegationPoint::new(zone, &ns, &vec![glue1]);
        assert_eq!(dp.get_missing_server()[0], &Name::new("ns2.com.").unwrap());
    }

    struct DumbSelector;
    impl HostSelector for DumbSelector {
        fn set_rtt(&mut self, host: Host, rtt: Duration) {}
        //assume hosts isn't empty
        fn select(&self, hosts: &[Host]) -> Host {
            hosts[0]
        }
    }

    #[test]
    fn test_select_target() {
        let zone = Name::new("com").unwrap();
        let ns = RRset::from_str("com. 3600  IN NS ns1.com").unwrap();

        let glue1 = RRset::from_str("ns1.com. 3600  IN A 2.2.2.2").unwrap();
        let glue2 = RRset::from_str("ns1.com. 3600  IN A 1.1.1.1").unwrap();
        let glue3 = RRset::from_str("ns1.com. 3600  IN A 3.3.3.3").unwrap();
        let glue = merge_rrset(merge_rrset(glue1, glue2), glue3);

        let dp = DelegationPoint::new(zone, &ns, &vec![glue]);
        let host = dp.get_target(&DumbSelector).unwrap();
        assert_eq!(host.to_string(), "2.2.2.2");

        let zone = Name::new("com").unwrap();
        let mut dp = DelegationPoint::new(zone, &ns, &Vec::new());
        let host = dp.get_target(&DumbSelector);
        assert!(host.is_none());
        let glue = RRset::from_str("ns2.com. 3600  IN A 1.1.1.1").unwrap();
        dp.add_glue(&glue);
        let host = dp.get_target(&DumbSelector);
        assert!(host.is_none());
        let glue = RRset::from_str("ns1.com. 3600  IN A 1.1.1.1").unwrap();
        dp.add_glue(&glue);
        let host = dp.get_target(&DumbSelector).unwrap();
        assert_eq!(host.to_string(), "1.1.1.1");
    }
}
