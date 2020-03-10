use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;

use crate::config::ForwarderConfig;
use anyhow;
use domaintree::{DomainTree, NodeChain};
use r53::{LabelSequence, Name};

use super::delegation_point::DelegationPoint;
use super::host_selector::Host;

#[derive(Clone)]
pub struct ForwarderManager {
    forwarders: DomainTree<Vec<Host>>,
}

impl ForwarderManager {
    pub fn new(conf: &ForwarderConfig) -> Self {
        let mut forwarders = DomainTree::new();
        for conf in &conf.forwarders {
            let name = Name::new(conf.zone_name.as_ref()).unwrap();
            let hosts = conf
                .addresses
                .iter()
                .map(|address| IpAddr::from_str(address).unwrap())
                .collect();
            forwarders.insert(name, Some(hosts));
        }
        ForwarderManager {
            forwarders: (forwarders),
        }
    }

    pub fn get_delegation_point(&self, name: &Name) -> Option<DelegationPoint> {
        let mut node_chain = NodeChain::new(&self.forwarders);
        let result = self.forwarders.find_node(&name, &mut node_chain);
        if let Some(hosts) = result.get_value() {
            let top = node_chain.pop();
            let zone = node_chain.get_absolute_name(top.get_name());
            let mut server_and_hosts = HashMap::new();
            server_and_hosts.insert(zone.clone(), hosts.clone());
            Some(DelegationPoint::new(zone, server_and_hosts))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::super::delegation_point::DelegationPoint;
    use super::super::host_selector::Host;
    use super::ForwarderManager;
    use crate::config::{ForwarderConfig, ZoneForwarderConfig};
    use r53::{Name, RRset};

    #[test]
    fn test_forwarder_get_delegation_point() {
        let mut conf: ForwarderConfig = Default::default();
        conf.forwarders.push(ZoneForwarderConfig {
            zone_name: "cn.".to_string(),
            addresses: vec!["1.1.1.1".to_string(), "2.2.2.2".to_string()],
        });
        conf.forwarders.push(ZoneForwarderConfig {
            zone_name: "zdns.cn.".to_string(),
            addresses: vec!["3.3.3.3".to_string(), "2.2.2.2".to_string()],
        });
        let forwarder_manager = ForwarderManager::new(&conf);

        let dp = forwarder_manager.get_delegation_point(&Name::new("a.cn.").unwrap());
        assert!(dp.is_some());
        let dp = dp.unwrap();
        assert!(dp.get_missing_server().is_none());
        assert_eq!(dp.zone(), &Name::new("cn").unwrap());

        let dp = forwarder_manager.get_delegation_point(&Name::new("a.com.").unwrap());
        assert!(dp.is_none());

        let dp = forwarder_manager.get_delegation_point(&Name::new("zdns.cn.").unwrap());
        assert!(dp.is_some());
        let dp = dp.unwrap();
        assert!(dp.get_missing_server().is_none());
        assert_eq!(dp.zone(), &Name::new("zdns.cn").unwrap());

        let dp = forwarder_manager.get_delegation_point(&Name::new("a.zdns.cn.").unwrap());
        assert!(dp.is_some());
        let dp = dp.unwrap();
        assert!(dp.get_missing_server().is_none());
        assert_eq!(dp.zone(), &Name::new("zdns.cn").unwrap());
    }
}
