use crate::recursor::{
    nsas::{
        error,
        message_util::{message_to_nameserver_entry, message_to_zone_entry},
        nameserver_cache::{self, Nameserver, NameserverCache},
        zone_cache::ZoneCache,
    },
    recursor::Resolver,
    running_query::RunningQuery,
    Recursor,
};
use failure::{self, Result};
use futures::{future, prelude::*, Future};
use lru::LruCache;
use r53::{Message, Name, RRType};
use std::{
    io, mem,
    sync::{Arc, Mutex},
};

enum FetcherState<F> {
    FetchNS(Name, Box<F>),
    FetchAddress(Name, Box<F>, Vec<Name>),
    Poisoned,
}

pub struct ZoneFetcher<R: Resolver> {
    state: FetcherState<R::Query>,
    resolver: R,
    nameservers: Arc<Mutex<NameserverCache>>,
    zones: Arc<Mutex<ZoneCache>>,
    depth: usize,
}

impl<R: Resolver> ZoneFetcher<R> {
    pub fn new(
        zone: Name,
        resolver: R,
        nameservers: Arc<Mutex<NameserverCache>>,
        zones: Arc<Mutex<ZoneCache>>,
        depth: usize,
    ) -> Self {
        let zone_copy = zone.clone();
        ZoneFetcher {
            state: FetcherState::FetchNS(
                zone,
                Box::new(resolver.new_query(Message::with_query(zone_copy, RRType::NS), depth + 1)),
            ),
            resolver,
            nameservers,
            zones,
            depth,
        }
    }
}

impl<R: Resolver> Future for ZoneFetcher<R> {
    type Item = Nameserver;
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match mem::replace(&mut self.state, FetcherState::Poisoned) {
                FetcherState::FetchNS(zone, mut fut) => match fut.poll() {
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(Async::NotReady) => {
                        self.state = FetcherState::FetchNS(zone, fut);
                        return Ok(Async::NotReady);
                    }
                    Ok(Async::Ready(msg)) => {
                        if let Ok((zone_entry, nameserver_entries)) =
                            message_to_zone_entry(&zone, msg)
                        {
                            if let Some(nameserver_entries) = nameserver_entries {
                                {
                                    let mut zones = self.zones.lock().unwrap();
                                    zones.add_zone(zone_entry);
                                }
                                let nameserver =
                                    nameserver_cache::select_from_nameservers(&nameserver_entries);
                                let mut nameservers = self.nameservers.lock().unwrap();
                                for nameserver_entry in nameserver_entries {
                                    nameservers.add_nameserver(nameserver_entry);
                                }
                                return Ok(Async::Ready(nameserver));
                            } else {
                                let (nameserver, mut missing_names) = {
                                    let mut nameservers = self.nameservers.lock().unwrap();
                                    zone_entry.select_nameserver(&mut nameservers)
                                };
                                {
                                    let mut zones = self.zones.lock().unwrap();
                                    zones.add_zone(zone_entry);
                                }
                                if let Some(nameserver) = nameserver {
                                    return Ok(Async::Ready(nameserver));
                                }

                                debug_assert!(!missing_names.is_empty());
                                let name = missing_names.pop().unwrap();
                                let fut = Box::new(self.resolver.new_query(
                                    Message::with_query(name.clone(), RRType::A),
                                    self.depth + 1,
                                ));
                                self.state = FetcherState::FetchAddress(name, fut, missing_names);
                            }
                        } else {
                            return Err(error::NSASError::InvalidNSResponse(
                                "not valid ns response".to_string(),
                            )
                            .into());
                        }
                    }
                },
                FetcherState::FetchAddress(current_name, mut fut, mut names) => {
                    match fut.poll() {
                        Err(e) => {
                            println!("fetch {:?} failed {:?}", current_name, e);
                        }
                        Ok(Async::NotReady) => {
                            self.state = FetcherState::FetchAddress(current_name, fut, names);
                            return Ok(Async::NotReady);
                        }
                        Ok(Async::Ready(msg)) => {
                            if let Ok(entry) =
                                message_to_nameserver_entry(current_name.clone(), msg)
                            {
                                let nameserver = entry.select_nameserver();
                                self.nameservers.lock().unwrap().add_nameserver(entry);
                                return Ok(Async::Ready(nameserver));
                            }
                        }
                    }

                    if names.is_empty() {
                        return Err(error::NSASError::NoValidNameserver.into());
                    }

                    let current_name = names.pop().unwrap();
                    let fut = Box::new(self.resolver.new_query(
                        Message::with_query(current_name.clone(), RRType::A),
                        self.depth + 1,
                    ));
                    self.state = FetcherState::FetchAddress(current_name, fut, names);
                }
                FetcherState::Poisoned => panic!("zone fetcher state panic inside pool"),
            }
        }
    }
}

mod test {
    use super::*;
    use crate::recursor::nsas::test_helper::DumbResolver;
    use lru::LruCache;
    use r53::{util::hex::from_hex, RData, RRset};
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

        let fetcher = ZoneFetcher::new(
            Name::new("knet.cn").unwrap(),
            resolver,
            nameservers.clone(),
            zones.clone(),
            0,
        );
        assert_eq!(nameservers.lock().unwrap().len(), 0);

        let mut rt = Runtime::new().unwrap();
        let select_nameserver = rt.block_on(fetcher).unwrap();
        assert_eq!(select_nameserver.name, Name::new("ns1.knet.cn").unwrap());
        assert_eq!(select_nameserver.address, Ipv4Addr::new(1, 1, 1, 1));

        assert_eq!(nameservers.lock().unwrap().len(), 3);
        assert_eq!(zones.lock().unwrap().len(), 1);
    }

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
        let fetcher = ZoneFetcher::new(
            Name::new("knet.cn").unwrap(),
            resolver,
            nameservers.clone(),
            zones.clone(),
            0,
        );

        let mut rt = Runtime::new().unwrap();
        let select_nameserver = rt.block_on(fetcher).unwrap();

        assert_eq!(select_nameserver.name, Name::new("ns3.knet.com").unwrap());
        assert_eq!(select_nameserver.address, Ipv4Addr::new(1, 1, 1, 1));

        assert_eq!(nameservers.lock().unwrap().len(), 1);
        assert_eq!(zones.lock().unwrap().len(), 1);
    }
}
