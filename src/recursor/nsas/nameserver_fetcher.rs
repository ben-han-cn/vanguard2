use crate::recursor::{
    nsas::{message_util::message_to_nameserver_entry, nameserver_cache::NameserverCache},
    RecursiveResolver,
};
use r53::{Message, Name, RRType};
use std::sync::{Arc, Mutex};

pub async fn fetch_nameserver_address<R: RecursiveResolver>(
    names: Vec<Name>,
    nameservers: Arc<Mutex<NameserverCache>>,
    resolver: &mut R,
    depth: usize,
) {
    for name in names {
        match resolver
            .resolve(&Message::with_query(name.clone(), RRType::A), depth + 1)
            .await
        {
            Ok(response) => {
                if let Ok(entry) = message_to_nameserver_entry(name, response) {
                    nameservers.lock().unwrap().add_nameserver(entry);
                }
            }
            Err(e) => {
                eprintln!("probe {:?} failed {:?}", name, e);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::recursor::mock_resolver::DumbResolver;
    use lru::LruCache;
    use r53::RData;
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
        rt.block_on(fetch_nameserver_address(
            names,
            nameservers.clone(),
            resolver,
            0,
        ));
        assert_eq!(nameservers.lock().unwrap().len(), 3);
    }
}
