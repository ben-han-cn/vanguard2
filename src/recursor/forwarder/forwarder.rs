use crate::recursor::util::Nameserver;
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    net::SocketAddr,
    time::Duration,
};

const UNREACHABLE_RTT: u64 = u64::max_value();

#[derive(Clone, Copy, Debug)]
pub struct Forwarder {
    address: SocketAddr,
    rtt: u64,
}

impl Forwarder {
    pub fn new(address: SocketAddr) -> Self {
        Forwarder { address, rtt: 0 }
    }
}

impl Nameserver for Forwarder {
    #[inline]
    fn get_addr(&self) -> SocketAddr {
        self.address
    }

    #[inline]
    fn set_unreachable(&mut self) {
        self.rtt = u64::max_value();
    }

    #[inline]
    fn set_rtt(&mut self, rtt: Duration) {
        let new = rtt.as_nanos() as u64;
        if self.rtt == UNREACHABLE_RTT {
            self.rtt = new
        } else {
            self.rtt = ((self.rtt * 3) + (new * 7)) / 10;
        }
    }

    #[inline]
    fn get_rtt(&self) -> Duration {
        Duration::from_nanos(self.rtt)
    }
}

impl PartialEq for Forwarder {
    fn eq(&self, other: &Forwarder) -> bool {
        self.address.eq(&other.address)
    }
}

impl Eq for Forwarder {}

impl PartialOrd for Forwarder {
    fn partial_cmp(&self, other: &Forwarder) -> Option<Ordering> {
        self.rtt.partial_cmp(&other.rtt)
    }
}

impl Ord for Forwarder {
    fn cmp(&self, other: &Forwarder) -> Ordering {
        self.rtt.cmp(&other.rtt)
    }
}
