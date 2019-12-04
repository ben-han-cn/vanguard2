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

pub async fn send_query<NS: NameserverStore>(request: Message, mut nameserver: NS::Nameserver, nameserver_store: NS) -> failure::Result<Message> { 
    let mut render = MessageRender::new();
    request.rend(&mut render);
    let mut socket =
        UdpSocket::bind(&("0.0.0.0:0".parse::<SocketAddr>().unwrap())).await?;
    let target = nameserver.get_addr();
    let send_time = Instant::now();
    if let Err(e) = socket.send_to(&render.take_data(), &target).await {
        nameserver.set_unreachable();
        nameserver_store.update_nameserver_rtt(&nameserver);
        return Err(VgError::IoError(e).into());
    }

    let last_timeout = {
        let mut rtt = nameserver.get_rtt();
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
                    nameserver.set_rtt(send_time.elapsed());
                    nameserver_store.update_nameserver_rtt(&nameserver);
                    return Message::from_wire(&buf[..size]);
                }
                Err(e) => {
                    nameserver.set_unreachable();
                    nameserver_store.update_nameserver_rtt(&nameserver);
                    return Err(VgError::IoError(e).into());
                }
            }
        }
        Err(_) => {
            nameserver.set_rtt(DEFAULT_RECV_TIMEOUT);
            nameserver_store.update_nameserver_rtt(&nameserver);
            return Err(VgError::Timeout(nameserver.get_addr().ip().to_string()).into());
        }
    }
}
