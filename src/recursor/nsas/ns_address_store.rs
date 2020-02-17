use crate::nameserver::NameserverStore;
use crate::recursor::{
    nsas::{
        entry_key::EntryKey,
        nameserver_cache::{self, Nameserver, NameserverCache},
        nameserver_fetcher::fetch_nameserver_address,
        zone_cache::ZoneCache,
        zone_fetcher::fetch_zone,
    },
    RecursiveResolver,
};
use anyhow;
use lru::LruCache;
use r53::Name;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

const DEFAULT_ZONE_ENTRY_CACHE_SIZE: usize = 1009;
const DEFAULT_NAMESERVER_ENTRY_CACHE_SIZE: usize = 3001;
const MAX_PROBING_NAMESERVER_COUNT: usize = 1000;

#[derive(Clone)]
pub struct NSAddressStore {
    nameservers: Arc<Mutex<NameserverCache>>,
    zones: Arc<Mutex<ZoneCache>>,
    //some ns in authority section, maynot have related
    //glue in additional section, we will try to fetch
    //their address in back ground
    probing_name_servers: Arc<Mutex<HashSet<Name>>>,
}

impl NSAddressStore {
    pub fn new() -> Self {
        NSAddressStore {
            nameservers: Arc::new(Mutex::new(NameserverCache(LruCache::new(
                DEFAULT_NAMESERVER_ENTRY_CACHE_SIZE,
            )))),
            zones: Arc::new(Mutex::new(ZoneCache(LruCache::new(
                DEFAULT_ZONE_ENTRY_CACHE_SIZE,
            )))),
            probing_name_servers: Arc::new(Mutex::new(HashSet::with_capacity(
                MAX_PROBING_NAMESERVER_COUNT,
            ))),
        }
    }

    //this must be invoked in a future
    pub fn get_nameserver(&self, zone: &Name) -> (Option<Nameserver>, Option<Vec<Name>>) {
        let key = &EntryKey::from_name(zone);
        let (nameserver, missing_nameserver) = self
            .zones
            .lock()
            .unwrap()
            .get_nameserver(key, &mut self.nameservers.lock().unwrap());

        (
            nameserver,
            if missing_nameserver.is_none() {
                None
            } else {
                self.missing_server_to_probe(missing_nameserver.unwrap())
            },
        )
    }

    pub async fn probe_missing_nameserver<R: RecursiveResolver + Send>(
        self,
        missing_nameserver: Vec<Name>,
        mut resolver: R,
    ) {
        println!(
            "start to probe {:?}, waiting queue len is {}",
            missing_nameserver,
            self.probing_name_servers.lock().unwrap().len()
        );

        fetch_nameserver_address(
            missing_nameserver.clone(),
            self.nameservers.clone(),
            &mut resolver,
            0,
        )
        .await;

        let mut probing_name_servers = self.probing_name_servers.lock().unwrap();
        missing_nameserver.into_iter().for_each(|n| {
            probing_name_servers.remove(&n);
        });
    }

    pub async fn fetch_nameserver<R: RecursiveResolver>(
        &self,
        zone: Name,
        resolver: &mut R,
        depth: usize,
    ) -> anyhow::Result<Nameserver> {
        fetch_zone(
            zone,
            resolver,
            self.nameservers.clone(),
            self.zones.clone(),
            depth,
        )
        .await
    }

    fn missing_server_to_probe(&self, missing_nameserver: Vec<Name>) -> Option<Vec<Name>> {
        if self.probing_name_servers.lock().unwrap().len() >= MAX_PROBING_NAMESERVER_COUNT {
            return None;
        }

        let missing_nameserver = {
            let unprobe_nameserver = Vec::with_capacity(missing_nameserver.len());
            let mut probing_name_servers = self.probing_name_servers.lock().unwrap();
            missing_nameserver
                .into_iter()
                .fold(unprobe_nameserver, |mut servers, n| {
                    if probing_name_servers.insert(n.clone()) {
                        servers.push(n);
                    }
                    servers
                })
        };

        if missing_nameserver.is_empty() {
            None
        } else {
            Some(missing_nameserver)
        }
    }
}

impl NameserverStore for NSAddressStore {
    type Nameserver = nameserver_cache::Nameserver;

    fn update_nameserver_rtt(&self, nameserver: &Nameserver) {
        let mut nameservers = self.nameservers.lock().unwrap();
        let key = &EntryKey::from_name(&nameserver.name);
        if let Some(entry) = nameservers.get_nameserver_mut(key) {
            entry.update_nameserver(nameserver);
        }
    }
}
