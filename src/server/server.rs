use std::{net::SocketAddr, str::FromStr, sync::Arc};

use super::{handler::QueryHandler, tcp_server::TcpServer, udp_server::UdpServer};
use crate::config::ServerConfig;
use futures::{future, Future};
use tokio::executor::spawn;

pub struct Server<S: QueryHandler> {
    addr: SocketAddr,
    handler: S,
}

impl<S: QueryHandler + 'static> Server<S> {
    pub fn new(conf: &ServerConfig, handler: S) -> Self {
        let addr = conf.address.parse().unwrap();
        Server { addr, handler }
    }

    pub fn into_future(self) -> impl Future<Item = (), Error = ()> + Send + 'static {
        let handler = Arc::new(self.handler);
        let addr = self.addr;
        future::lazy(move || {
            spawn(UdpServer::new(addr, handler.clone()).map_err(|e| println!("udp errr {:?}", e)));
            TcpServer::new(addr, handler.clone()).into_future()
        })
    }
}
