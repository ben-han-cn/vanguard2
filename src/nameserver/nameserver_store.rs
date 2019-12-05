use std::{net::SocketAddr, time::Duration};

pub trait Nameserver {
    fn get_addr(&self) -> SocketAddr;

    fn get_rtt(&self) -> Duration;
    fn set_rtt(&mut self, rtt: Duration);
    fn set_unreachable(&mut self);
}

pub trait NameserverStore {
    type Nameserver: Nameserver;

    fn update_nameserver_rtt(&self, nameserver: &Self::Nameserver);
}
