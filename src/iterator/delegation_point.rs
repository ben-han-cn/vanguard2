use std::collections::HashMap;
use std::net::IpAddr;

use r53::{Message, Name, RData, RRType, RRset, SectionType};

use super::cache::MessageCache;
use super::host_selector::{Host, HostSelector};

#[derive(Debug, Clone)]
pub struct DelegationPoint {
    zone: Name,
    server_and_hosts: HashMap<Name, Vec<Host>>,
    probed_server: Vec<Name>,
    lame_host: Vec<Host>,
}

impl DelegationPoint {
    pub fn new(zone: Name, server_and_hosts: HashMap<Name, Vec<Host>>) -> Self {
        Self {
            zone,
            server_and_hosts,
            probed_server: Vec::new(),
            lame_host: Vec::new(),
        }
    }

    pub fn from_referral_response(response: &Message) -> Self {
        let mut dp = DelegationPoint::from_ns_rrset(
            &response
                .section(SectionType::Authority)
                .expect("referral response should has ns")[0],
            &Vec::new(),
        );
        if let Some(glues) = response.section(SectionType::Additional) {
            for glue in glues {
                dp.add_glue(glue);
            }
        }
        dp
    }

    pub fn from_ns_rrset(ns: &RRset, glues: &Vec<RRset>) -> Self {
        let server_and_hosts = ns.rdatas.iter().fold(
            HashMap::new(),
            |mut servers: HashMap<Name, Vec<Host>>, rdata| {
                if let RData::NS(ref ns) = rdata {
                    servers.insert(ns.name.clone(), Vec::new());
                }
                servers
            },
        );

        let mut dp = Self {
            zone: ns.name.clone(),
            server_and_hosts,
            probed_server: Vec::new(),
            lame_host: Vec::new(),
        };

        glues.iter().for_each(|glue| {
            dp.add_glue(glue);
        });
        dp
    }

    pub fn from_cache(qname: &Name, cache: &mut MessageCache) -> Option<Self> {
        let ns = cache.get_deepest_ns(qname)?;
        let mut all_glue_is_under_zone = true;
        let glues = ns.rdatas.iter().fold(Vec::new(), |mut glues, rdata| {
            if let RData::NS(ref glue) = rdata {
                if !glue.name.is_subdomain(&ns.name) {
                    all_glue_is_under_zone = false;
                }
                if let Some(rrset) = cache.get_rrset(&glue.name, RRType::A) {
                    glues.push(rrset);
                }
            } else {
                unreachable!();
            }
            glues
        });

        //avoid return a dp, whose glue is empty, but all glue is under the zone
        //in this case, the dp will cause endloop
        if !glues.is_empty() || !all_glue_is_under_zone {
            Some(DelegationPoint::from_ns_rrset(&ns, &glues))
        } else {
            match ns.name.parent(1) {
                Ok(parent) => DelegationPoint::from_cache(&parent, cache),
                Err(_) => None,
            }
        }
    }

    pub fn zone(&self) -> &Name {
        &self.zone
    }

    pub fn add_glue(&mut self, glue: &RRset) -> bool {
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
            true
        } else {
            false
        }
    }

    pub fn get_target<S: HostSelector>(&self, selector: &S) -> Option<Host> {
        let hosts: Vec<Host> = self
            .server_and_hosts
            .values()
            .flatten()
            .filter_map(|a| {
                if self.lame_host.iter().position(|h| h == a).is_none() {
                    Some(*a)
                } else {
                    None
                }
            })
            .collect();
        if hosts.is_empty() {
            None
        } else {
            selector.select(&hosts)
        }
    }

    pub fn get_missing_server(&self) -> Option<Name> {
        for (name, hosts) in self.server_and_hosts.iter() {
            if hosts.len() == 0
                && !name.is_subdomain(&self.zone)
                && self.probed_server.iter().position(|n| n == name).is_none()
            {
                return Some(name.clone());
            }
        }
        None
    }

    pub fn add_probed_server(&mut self, name: &Name) {
        assert!(self.server_and_hosts.contains_key(name));
        self.probed_server.push(name.clone());
    }

    pub fn mark_server_lame(&mut self, host: Host) {
        self.lame_host.push(host);
    }
}

#[cfg(test)]
mod tests {
    use super::super::cache::MessageCache;
    use super::super::host_selector::{Host, HostSelector};
    use super::DelegationPoint;
    use r53::{build_response, Name, RRType, RRset};
    use std::str::FromStr;
    use std::time::Duration;

    #[test]
    fn test_delegation_point_new() {
        let ns =
            RRset::from_strs(&["com. 3600  IN NS ns1.com", "com. 3600  IN NS ns2.com"]).unwrap();
        let glue1 = RRset::from_str("ns1.com. 3600  IN A 1.1.1.1").unwrap();
        let glue2 = RRset::from_str("ns2.com. 3600  IN A 2.2.2.2").unwrap();

        let dp = DelegationPoint::from_ns_rrset(&ns, &vec![glue1, glue2]);
        assert!(dp.get_missing_server().is_none());
    }

    #[test]
    fn test_delegation_point_from_cache() {
        let mut cache = MessageCache::new(100000);
        //as a replacement for root hint
        cache.add_response(
            build_response(
                "www.example.cn",
                RRType::A,
                vec![vec!["www.example.cn 3600 IN A 2.2.2.2"]],
                vec![vec!["example.cn 3600 IN NS a.example.cn."]],
                vec![],
                None,
            )
            .unwrap(),
        );
        cache.add_response(
            build_response(
                "www.example.com",
                RRType::A,
                vec![vec!["www.example.com 3600 IN A 2.2.2.2"]],
                vec![vec!["example.com 3600 IN NS a.example.com."]],
                vec![vec!["a.example.com 3600 IN A 3.3.3.3"]],
                None,
            )
            .unwrap(),
        );
        cache.add_response(
            build_response(
                "www.example.net",
                RRType::A,
                vec![vec!["www.example.net 3600 IN A 2.2.2.2"]],
                vec![vec![
                    "example.net 3600 IN NS a.example.net.",
                    "example.net 3600 IN NS a.example.org.",
                ]],
                vec![],
                None,
            )
            .unwrap(),
        );
        cache.add_response(
            build_response(
                "cn",
                RRType::NS,
                vec![vec!["cn 3600 IN NS a.cn", "cn 3600 IN NS b.cn"]],
                vec![],
                vec![vec!["a.cn 3600 IN A 3.3.3.3"]],
                None,
            )
            .unwrap(),
        );

        let dp =
            DelegationPoint::from_cache(&Name::new("xxx.example.cn").unwrap(), &mut cache).unwrap();
        assert_eq!(dp.zone(), &Name::new("cn").unwrap());

        let dp = DelegationPoint::from_cache(&Name::new("xxx.example.com").unwrap(), &mut cache)
            .unwrap();
        assert_eq!(dp.zone(), &Name::new("example.com").unwrap());

        let dp = DelegationPoint::from_cache(&Name::new("xxx.example.net").unwrap(), &mut cache)
            .unwrap();
        assert_eq!(dp.zone(), &Name::new("example.net").unwrap());
        assert!(!dp.get_missing_server().is_none());
    }

    #[test]
    fn test_missing_server() {
        let ns =
            RRset::from_strs(&["com. 3600  IN NS ns1.com", "com. 3600  IN NS ns2.com"]).unwrap();
        let glue1 = RRset::from_str("ns1.com. 3600  IN A 1.1.1.1").unwrap();
        let dp = DelegationPoint::from_ns_rrset(&ns, &vec![glue1]);
        assert!(dp.get_missing_server().is_none());

        let ns =
            RRset::from_strs(&["com. 3600  IN NS ns1.com", "com. 3600  IN NS ns2.cn"]).unwrap();
        let glue1 = RRset::from_str("ns1.com. 3600  IN A 1.1.1.1").unwrap();
        let mut dp = DelegationPoint::from_ns_rrset(&ns, &vec![glue1]);
        assert_eq!(
            dp.get_missing_server().unwrap(),
            Name::new("ns2.cn.").unwrap()
        );
        dp.add_probed_server(&Name::new("ns2.cn.").unwrap());
        assert!(dp.get_missing_server().is_none());
    }

    struct DumbSelector;
    impl HostSelector for DumbSelector {
        fn set_rtt(&mut self, _host: Host, _rtt: Duration) {}
        fn set_timeout(&mut self, _host: Host, _timeout: Duration) {}
        //assume hosts isn't empty
        fn select(&self, hosts: &[Host]) -> Option<Host> {
            Some(hosts[0])
        }
    }

    #[test]
    fn test_select_target() {
        let ns = RRset::from_str("com. 3600  IN NS ns1.com").unwrap();

        let glue = RRset::from_strs(&[
            "ns1.com. 3600  IN A 2.2.2.2",
            "ns1.com. 3600  IN A 1.1.1.1",
            "ns1.com. 3600  IN A 3.3.3.3",
        ])
        .unwrap();

        let dp = DelegationPoint::from_ns_rrset(&ns, &vec![glue]);
        let host = dp.get_target(&DumbSelector).unwrap();
        assert_eq!(host.to_string(), "2.2.2.2");

        let mut dp = DelegationPoint::from_ns_rrset(&ns, &Vec::new());
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
