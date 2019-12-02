use super::nameserver_store::{Nameserver, NameserverStore};
use crate::error::VgError;
use failure;
use r53::{Message, MessageRender};
use std::{
    error::Error,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::{udp, UdpSocket},
    time::timeout,
};

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_secs(2); //3 secs
const DEFAULT_RECV_BUF_SIZE: usize = 1024;

pub struct Sender<NS: NameserverStore> {
    query: Message,
    nameserver: NS::Nameserver,
    nameserver_store: NS,
}

impl<NS: NameserverStore> Sender<NS> {
    pub fn new(query: Message, nameserver: NS::Nameserver, nameserver_store: NS) -> Self {
        Sender {
            query,
            nameserver,
            nameserver_store,
        }
    }

    pub async fn send_query(&mut self) -> Result<Message, failure::Error> {
        let mut render = MessageRender::new();
        self.query.rend(&mut render);
        let mut socket =
            UdpSocket::bind(&("0.0.0.0:0".parse::<SocketAddr>().unwrap())).await?;
        let target = self.nameserver.get_addr();
        let send_time = Instant::now();
        if let Err(e) = socket.send_to(&render.take_data(), &target).await {
            self.nameserver.set_unreachable();
            self.nameserver_store.update_nameserver_rtt(&self.nameserver);
            return Err(VgError::IoError(e).into());
        }

        let last_timeout = {
            let mut rtt = self.nameserver.get_rtt();
            if rtt.as_millis() == 0 || rtt > DEFAULT_RECV_TIMEOUT {
                rtt = DEFAULT_RECV_TIMEOUT;
            }
            rtt
        };

        let mut buf = vec![0; DEFAULT_RECV_BUF_SIZE];
        match timeout(last_timeout, socket.recv(&mut buf)).await {
            Ok(result) => {
                match result {
                    Ok(size) => {
                        self.nameserver.set_rtt(send_time.elapsed());
                        self.nameserver_store.update_nameserver_rtt(&self.nameserver);
                        return Message::from_wire(&buf[..size]);
                    }
                    Err(e) => {
                        self.nameserver.set_unreachable();
                        self.nameserver_store.update_nameserver_rtt(&self.nameserver);
                        return Err(VgError::IoError(e).into());
                    }
                }
            }
            Err(_) => {
                self.nameserver.set_rtt(DEFAULT_RECV_TIMEOUT);
                self.nameserver_store.update_nameserver_rtt(&self.nameserver);
                return Err(VgError::Timeout(self.nameserver.get_addr().ip().to_string()).into());
            }
        }
    }
}
