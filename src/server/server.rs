use super::{tcp_server::TcpServer, udp_server::UdpServer};
use crate::config::ServerConfig;
use crate::types::QueryHandler;
use std::net::SocketAddr;

pub struct Server {
    addr: SocketAddr,
}

impl Server {
    pub fn new(conf: &ServerConfig) -> Self {
        let addr = conf.address.parse().unwrap();
        Server { addr }
    }

    pub async fn run<H: QueryHandler + Send + Sync>(&self, handler: H) {
        let mut udp_server = UdpServer::new(handler.clone());
        let mut tcp_server = TcpServer::new(handler);
        tokio::spawn(tcp_server.run(self.addr));
        udp_server.run(self.addr).await
    }
}
