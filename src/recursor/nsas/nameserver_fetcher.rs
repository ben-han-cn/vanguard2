use crate::recursor::{
    nsas::{
        message_util::{message_to_nameserver_entry},
        nameserver_cache::{NameserverCache},
    },
    resolver::Resolver,
};
use failure;
use r53::{Message, Name, RRType};
use std::{
    sync::{Arc, Mutex},
};

pub async fn fetch_nameserver<R: Resolver> (names: Vec<Name>, nameservers: Arc<Mutex<NameserverCache>>, resolver: R) {
    for name in names {
        match resolver.handle_query(Message::with_query(name.clone(), RRType::A)).await {
            Ok(response) => { 
                if let Ok(entry) = message_to_nameserver_entry(name, response) {
                    nameservers.lock().unwrap().add_nameserver(entry);
                }
            }
            Err(e) => {
                eprintln!(
                    "probe {:?} failed {:?}",
                    name,
                    e
                );
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
        assert_eq!(nameservers.lock().unwrap().len(), 0);
        let mut rt = Runtime::new().unwrap();
        rt.block_on(fetch_nameserver(names, nameservers.clone(), resolver));
        assert_eq!(nameservers.lock().unwrap().len(), 3);
    }
}
