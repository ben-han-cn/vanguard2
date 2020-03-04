use crate::recursor::{
    nsas::{
        message_util::{message_to_nameserver_entry, message_to_zone_entry},
        nameserver_cache::{self, Nameserver, NameserverCache},
        zone_cache::ZoneCache,
    },
    RecursiveResolver,
};
use anyhow::{self, bail};
use r53::{Message, Name, RRType};
use std::sync::{Arc, Mutex};

pub async fn fetch_zone<R: RecursiveResolver>(
    zone: Name,
    resolver: &mut R,
    nameservers: Arc<Mutex<NameserverCache>>,
    zones: Arc<Mutex<ZoneCache>>,
    depth: usize,
) -> anyhow::Result<Nameserver> {
    let response = resolver
        .resolve(&Message::with_query(zone.clone(), RRType::NS), depth + 1)
        .await?;
    if let Ok((zone_entry, nameserver_entries)) = message_to_zone_entry(&zone, response) {
        if let Some(nameserver_entries) = nameserver_entries {
            {
                let mut zones = zones.lock().unwrap();
                zones.add_zone(zone_entry);
            }
            let nameserver = nameserver_cache::select_from_nameservers(&nameserver_entries);
            let mut nameservers = nameservers.lock().unwrap();
            for nameserver_entry in nameserver_entries {
                nameservers.add_nameserver(nameserver_entry);
            }
            return Ok(nameserver);
        } else {
            let (nameserver, missing_names) = {
                let mut nameservers = nameservers.lock().unwrap();
                zone_entry.select_nameserver(&mut nameservers)
            };
            {
                let mut zones = zones.lock().unwrap();
                zones.add_zone(zone_entry);
            }
            if let Some(nameserver) = nameserver {
                return Ok(nameserver);
            }

            debug_assert!(missing_names.is_some());
            let missing_names = missing_names.unwrap();
            for name in missing_names {
                if let Ok(response) = resolver
                    .resolve(&Message::with_query(name.clone(), RRType::A), depth + 1)
                    .await
                {
                    if let Ok(entry) = message_to_nameserver_entry(name, response) {
                        let nameserver = entry.select_nameserver();
                        nameservers.lock().unwrap().add_nameserver(entry);
                        return Ok(nameserver);
                    }
                }
            }
            bail!("no valid nameserver");
        }
    } else {
        bail!("no valid ns response");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::recursor::mock_resolver::DumbResolver;
    use lru::LruCache;
    use r53::{RData, RRset};
    use std::net::Ipv4Addr;
    use std::str::FromStr;
    use tokio::runtime::Runtime;

    #[test]
    fn test_fetch_zone_with_glue() {
        let mut resolver = DumbResolver::new();
        resolver.set_answer(
            Name::new("knet.cn").unwrap(),
            RRType::NS,
            vec![
                RData::from_str(RRType::NS, "ns1.knet.cn").unwrap(),
                RData::from_str(RRType::NS, "ns2.knet.cn").unwrap(),
                RData::from_str(RRType::NS, "ns3.knet.cn").unwrap(),
            ],
            vec![
                RRset::from_str("ns1.knet.cn 200 IN A 1.1.1.1").unwrap(),
                RRset::from_str("ns2.knet.cn 200 IN A 2.2.2.2").unwrap(),
                RRset::from_str("ns3.knet.cn 200 IN A 3.3.3.3").unwrap(),
            ],
        );

        let nameservers = Arc::new(Mutex::new(NameserverCache(LruCache::new(100))));
        let zones = Arc::new(Mutex::new(ZoneCache(LruCache::new(100))));
        assert_eq!(nameservers.lock().unwrap().len(), 0);

        let mut rt = Runtime::new().unwrap();
        let select_nameserver = rt
            .block_on(fetch_zone(
                Name::new("knet.cn").unwrap(),
                &mut resolver,
                nameservers.clone(),
                zones.clone(),
                0,
            ))
            .unwrap();
        assert_eq!(select_nameserver.name, Name::new("ns1.knet.cn").unwrap());
        assert_eq!(select_nameserver.address, Ipv4Addr::new(1, 1, 1, 1));

        assert_eq!(nameservers.lock().unwrap().len(), 3);
        assert_eq!(zones.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_fetch_without_glue() {
        let mut resolver = DumbResolver::new();
        resolver.set_answer(
            Name::new("knet.cn").unwrap(),
            RRType::NS,
            vec![
                RData::from_str(RRType::NS, "ns1.knet.cn").unwrap(),
                RData::from_str(RRType::NS, "ns2.knet.cn").unwrap(),
                RData::from_str(RRType::NS, "ns3.knet.com").unwrap(),
            ],
            Vec::new(),
        );

        resolver.set_answer(
            Name::new("ns3.knet.com").unwrap(),
            RRType::A,
            vec![
                RData::from_str(RRType::A, "1.1.1.1").unwrap(),
                RData::from_str(RRType::A, "2.2.2.2").unwrap(),
            ],
            Vec::new(),
        );

        let nameservers = Arc::new(Mutex::new(NameserverCache(LruCache::new(100))));
        let zones = Arc::new(Mutex::new(ZoneCache(LruCache::new(100))));
        let mut rt = Runtime::new().unwrap();
        let select_nameserver = rt
            .block_on(fetch_zone(
                Name::new("knet.cn").unwrap(),
                &mut resolver,
                nameservers.clone(),
                zones.clone(),
                0,
            ))
            .unwrap();

        assert_eq!(select_nameserver.name, Name::new("ns3.knet.com").unwrap());
        assert_eq!(select_nameserver.address, Ipv4Addr::new(1, 1, 1, 1));
        assert_eq!(nameservers.lock().unwrap().len(), 1);
        assert_eq!(zones.lock().unwrap().len(), 1);
    }
}
