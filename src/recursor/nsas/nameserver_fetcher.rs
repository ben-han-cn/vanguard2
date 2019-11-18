use crate::recursor::{
    nsas::{
        entry_key::EntryKey,
        error::NSASError,
        message_util::{message_to_nameserver_entry, message_to_zone_entry},
        nameserver_cache::{Nameserver, NameserverCache, NameserverEntry},
    },
    recursor::Resolver,
    running_query::RunningQuery,
    Recursor,
};
use failure;
use futures::{future, prelude::*, Future};
use r53::{Message, Name, RRType};
use std::{
    io,
    sync::{Arc, Mutex},
};

pub struct NameserverFetcher<R: Resolver> {
    names: Vec<Name>,
    nameservers: Arc<Mutex<NameserverCache>>,
    resolver: R,
    fut: Option<R::Query>,
    current_name: Option<Name>,
}

impl<R: Resolver> NameserverFetcher<R> {
    pub fn new(names: Vec<Name>, nameservers: Arc<Mutex<NameserverCache>>, resolver: R) -> Self {
        NameserverFetcher {
            names,
            nameservers,
            resolver,
            fut: None,
            current_name: None,
        }
    }
}

impl<R: Resolver> Future for NameserverFetcher<R> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if self.fut.is_none() {
                let name = self.names.pop();
                if name.is_none() {
                    return Ok(Async::Ready(()));
                } else {
                    let name = name.unwrap();
                    self.fut = Some(
                        self.resolver
                            .new_query(Message::with_query(name.clone(), RRType::A), 1),
                    );
                    self.current_name = Some(name);
                }
            }

            match self.fut.as_mut().unwrap().poll() {
                Err(e) => {
                    eprintln!(
                        "probe {:?} failed {:?}",
                        self.current_name.as_ref().unwrap(),
                        e
                    );
                }
                Ok(Async::NotReady) => {
                    return Ok(Async::NotReady);
                }
                Ok(Async::Ready(msg)) => {
                    if let Ok(entry) =
                        message_to_nameserver_entry(self.current_name.take().unwrap(), msg)
                    {
                        self.nameservers.lock().unwrap().add_nameserver(entry);
                    }
                }
            }
            self.fut = None;
        }
    }
}

mod test {
    use super::*;
    use crate::recursor::nsas::test_helper::DumbResolver;
    use lru::LruCache;
    use r53::{util::hex::from_hex, RData, RRset};
    use std::net::Ipv4Addr;
    use tokio::runtime::Runtime;

    #[test]
    fn test_fetch_all() {
        let mut resolver = DumbResolver::new();
        let names = vec![
            Name::new("ns1.knet.cn").unwrap(),
            Name::new("ns2.knet.cn").unwrap(),
            Name::new("ns3.knet.cn").unwrap(),
        ];

        for name in names.iter() {
            resolver.set_answer(
                name.clone(),
                RRType::A,
                vec![RData::from_str(RRType::A, "1.1.1.1").unwrap()],
                Vec::new(),
            );
        }

        let nameservers = Arc::new(Mutex::new(NameserverCache(LruCache::new(100))));

        let mut fetcher = NameserverFetcher::new(names, nameservers.clone(), resolver);
        assert_eq!(nameservers.lock().unwrap().len(), 0);

        let mut rt = Runtime::new().unwrap();
        rt.block_on(fetcher).unwrap();

        assert_eq!(nameservers.lock().unwrap().len(), 3);
    }
}
