use std::{
    collections::{hash_map::Entry, HashMap},
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use anyhow::{bail, Error};
use async_trait::async_trait;
use r53::{question::Question, Message, Name, RRType};
use tokio::sync::watch::{channel, Receiver};

use super::host_selector::Host;
use super::nsclient::NameServerClient;

const MAX_INFLIGHT_QUERY_COUNT: usize = 1000;

type ResponseSyncReceiver = Receiver<Option<Result<Message, String>>>;
struct Inflightkey {
    name: Name,
    typ: RRType,
}

impl Inflightkey {
    pub fn new(question: &Question) -> Self {
        Self {
            name: question.name.clone(),
            typ: question.typ,
        }
    }
}

impl Hash for Inflightkey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        state.write_u16(self.typ.to_u16());
    }
}

impl PartialEq for Inflightkey {
    fn eq(&self, other: &Inflightkey) -> bool {
        self.typ == other.typ && self.name.eq(&other.name)
    }
}

impl Eq for Inflightkey {}

#[derive(Clone)]
pub struct AggregateClient<C: NameServerClient> {
    inflight_queries: Arc<Mutex<HashMap<Inflightkey, ResponseSyncReceiver>>>,
    waiting_queries: Arc<AtomicU64>,
    client: C,
}

impl<C: NameServerClient> AggregateClient<C> {
    pub fn new(client: C) -> Self {
        Self {
            inflight_queries: Arc::new(Mutex::new(HashMap::new())),
            waiting_queries: Arc::new(AtomicU64::new(0)),
            client,
        }
    }

    pub(crate) fn inflight_query_count(&self) -> usize {
        self.inflight_queries.lock().unwrap().len()
    }

    pub(crate) fn waiting_query_count(&self) -> u64 {
        self.waiting_queries.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl<C: NameServerClient> NameServerClient for AggregateClient<C> {
    async fn query(&self, request: &Message, target: Host) -> anyhow::Result<Message> {
        let mut rx_for_same_query = None;
        let mut tx_after_new_query = None;
        {
            let mut inflight_queries = self.inflight_queries.lock().unwrap();
            let question = request.question.as_ref().unwrap();
            let inflight_querie_count = inflight_queries.len();
            let entry = inflight_queries.entry(Inflightkey::new(question));
            match entry {
                Entry::Occupied(o) => {
                    rx_for_same_query = Some(o.get().clone());
                }
                Entry::Vacant(o) => {
                    if inflight_querie_count + 1 > MAX_INFLIGHT_QUERY_COUNT {
                        bail!("too many outgoing quires");
                    }
                    let (tx, rx) = channel(None);
                    o.insert(rx);
                    tx_after_new_query = Some(tx);
                }
            }
        }

        if let Some(mut rx) = rx_for_same_query {
            self.waiting_queries.fetch_add(1 as u64, Ordering::Relaxed);
            debug!("merged query for {}", request.question.as_ref().unwrap());
            loop {
                if let Some(resp) = rx.recv().await {
                    if let Some(resp) = resp {
                        let wrapper = match resp {
                            Ok(msg) => Ok(msg),
                            Err(info) => Err(Error::msg(info)),
                        };
                        debug!(
                            "get response of merged query for {}",
                            request.question.as_ref().unwrap()
                        );
                        self.waiting_queries.fetch_sub(1 as u64, Ordering::Relaxed);
                        return wrapper;
                    }
                }
            }
        }

        let resp = self.client.query(request, target).await;
        {
            let mut inflight_queries = self.inflight_queries.lock().unwrap();
            let question = request.question.as_ref().unwrap();
            inflight_queries.remove(&Inflightkey::new(question));
        }

        let cloned_resp = match resp {
            Ok(ref msg) => Ok(msg.clone()),
            Err(ref e) => Err(e.to_string()),
        };

        //if there is no aggregate qury, broadcast will return err, since no
        //other receiver is waiting
        let _ = tx_after_new_query.unwrap().broadcast(Some(cloned_resp));
        resp
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Arc, Mutex};
    use std::{thread, time::Duration};

    use async_trait::async_trait;
    use r53::{Message, Name, RRType};
    use tokio::runtime::Runtime;
    use tokio::sync::watch::{channel, Receiver};
    use tokio::task::JoinHandle;

    use super::super::host_selector::Host;
    use super::super::nsclient::NameServerClient;
    use super::AggregateClient;

    #[derive(Clone)]
    struct DumbClient {
        receiver: Arc<Mutex<Receiver<u8>>>,
        query_count: Arc<AtomicU8>,
    }

    impl DumbClient {
        pub fn new(receiver: Receiver<u8>) -> Self {
            Self {
                receiver: Arc::new(Mutex::new(receiver)),
                query_count: Arc::new(AtomicU8::new(0)),
            }
        }

        pub fn query_count(&self) -> u8 {
            self.query_count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl NameServerClient for DumbClient {
        async fn query(&self, request: &Message, _target: Host) -> anyhow::Result<Message> {
            self.query_count.fetch_add(1 as u8, Ordering::Relaxed);
            let mut receiver = {
                let receiver = self.receiver.lock().unwrap();
                receiver.clone()
            };
            loop {
                if let Some(n) = receiver.recv().await {
                    if n == 1 {
                        break;
                    }
                }
            }
            Ok(request.clone())
        }
    }

    #[test]
    fn test_aggregate_all_client() {
        let (tx, rx) = channel(0);
        let inner_client = DumbClient::new(rx);
        let client = AggregateClient::new(inner_client.clone());
        let mut rt = Runtime::new().unwrap();

        let mut handlers: Vec<JoinHandle<_>> = (0..10)
            .map(|_| {
                let client_clone = client.clone();
                rt.spawn(async move {
                    let request = Message::with_query(Name::new("zdns.cn").unwrap(), RRType::A);
                    let resp = client_clone
                        .query(&request, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)))
                        .await;
                    assert!(resp.is_ok());
                })
            })
            .collect();

        handlers.push({
            let client_clone = client.clone();
            rt.spawn(async move {
                let request = Message::with_query(Name::new("zdns.com").unwrap(), RRType::A);
                let resp = client_clone
                    .query(&request, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)))
                    .await;
                assert!(resp.is_ok());
            })
        });

        let thread_handle = thread::Builder::new()
            .spawn(move || {
                loop {
                    let count = client.waiting_query_count();
                    if count != 9 as u64 {
                        thread::sleep(Duration::from_secs(1));
                    } else {
                        break;
                    }
                }
                assert_eq!(client.inflight_query_count(), 2 as usize);
                tx.broadcast(1).unwrap();
            })
            .unwrap();

        rt.block_on(async move {
            for h in handlers {
                h.await.expect("spawn task crashed");
            }
        });

        thread_handle.join().unwrap();
        assert_eq!(inner_client.query_count(), 2);
    }
}
