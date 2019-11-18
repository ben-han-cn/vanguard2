use std::{net::SocketAddr, time::Duration};

pub trait Nameserver {
    fn get_addr(&self) -> SocketAddr;
    fn set_rtt(&mut self, rtt: Duration);
    fn get_rtt(&self) -> Duration;
    fn set_unreachable(&mut self);
}

pub trait NameserverStore<S: Nameserver> {
    fn update_nameserver_rtt(&self, nameserver: &S);
}
