use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::{self, bail};
use r53::{Message, MessageRender};
use tokio::net::UdpSocket;
use tokio::time::timeout;

use super::host_selector::{Host, HostSelector};

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_secs(3); //3 secs
const DEFAULT_RECV_BUF_SIZE: usize = 1024;

pub async fn send_query<S: HostSelector>(
    request: &Message,
    target: Host,
    selector: &mut HostSelector,
) -> anyhow::Result<Message> {
    let mut render = MessageRender::new();
    request.to_wire(&mut render);
    let mut socket = UdpSocket::bind(&("0.0.0.0:0".parse::<SocketAddr>().unwrap())).await?;
    let send_time = Instant::now();
    if let Err(e) = socket
        .send_to(&render.take_data(), SocketAddr::new(target, 53))
        .await
    {
        selector.set_rtt(target, DEFAULT_RECV_TIMEOUT);
        bail!(e);
    }

    let mut buf = vec![0; DEFAULT_RECV_BUF_SIZE];
    match timeout(DEFAULT_RECV_TIMEOUT, socket.recv(&mut buf)).await {
        Ok(result) => match result {
            Ok(size) => {
                selector.set_rtt(target, send_time.elapsed());
                return Message::from_wire(&buf[..size]);
            }
            Err(e) => {
                selector.set_rtt(target, DEFAULT_RECV_TIMEOUT);
                bail!(e);
            }
        },
        Err(_) => {
            selector.set_rtt(target, DEFAULT_RECV_TIMEOUT);
            bail!("{} timeout", target.to_string());
        }
    }
}
