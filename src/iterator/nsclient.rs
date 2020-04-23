use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{self, bail};
use async_trait::async_trait;
use r53::{Message, MessageRender, Rcode};
use tokio::net::UdpSocket;
use tokio::time::timeout;

use super::host_selector::{Host, HostSelector, RTTBasedHostSelector};

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_secs(3); //3 secs
const DEFAULT_RECV_BUF_SIZE: usize = 1024;

#[async_trait]
pub trait NameServerClient: Clone + Sync + Send {
    async fn query(&self, request: &Message, target: Host) -> anyhow::Result<Message>;
}

#[derive(Clone)]
pub struct NSClient {
    host_selector: Arc<Mutex<RTTBasedHostSelector>>,
}

impl NSClient {
    pub fn new(selector: Arc<Mutex<RTTBasedHostSelector>>) -> Self {
        Self {
            host_selector: selector,
        }
    }

    pub async fn do_query(&self, request: &Message, target: Host) -> anyhow::Result<Message> {
        let mut render = MessageRender::new();
        request.to_wire(&mut render);
        let mut socket = UdpSocket::bind(&("0.0.0.0:0".parse::<SocketAddr>().unwrap())).await?;
        socket.connect(SocketAddr::new(target, 53)).await?;
        let send_time = Instant::now();
        if let Err(e) = socket.send(&render.take_data()).await {
            self.host_selector
                .lock()
                .unwrap()
                .set_timeout(target, DEFAULT_RECV_TIMEOUT);
            bail!(e);
        }

        let mut buf = vec![0; DEFAULT_RECV_BUF_SIZE];
        match timeout(DEFAULT_RECV_TIMEOUT, socket.recv(&mut buf)).await {
            Ok(result) => match result {
                Ok(size) => {
                    self.host_selector
                        .lock()
                        .unwrap()
                        .set_rtt(target, send_time.elapsed());
                    return Message::from_wire(&buf[..size]);
                }
                Err(e) => {
                    self.host_selector
                        .lock()
                        .unwrap()
                        .set_timeout(target, DEFAULT_RECV_TIMEOUT);
                    bail!(e);
                }
            },
            Err(e) => {
                self.host_selector
                    .lock()
                    .unwrap()
                    .set_timeout(target, DEFAULT_RECV_TIMEOUT);
                bail!(e);
            }
        }
    }
}

#[async_trait]
impl NameServerClient for NSClient {
    async fn query(&self, request: &Message, target: Host) -> anyhow::Result<Message> {
        let mut request = request.clone();
        request.header.id = rand::random::<u16>();
        let result = self.do_query(&request, target).await;
        if let Ok(ref response) = result {
            if response.header.rcode == Rcode::FormErr {
                request.header.id = rand::random::<u16>();
                request.edns = None;
                request.recalculate_header();
                return self.do_query(&request, target).await;
            }
        }
        result
    }
}
