use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

const UNREACHABLE_CACHE_TIME: u64 = 5;
pub(crate) const UNREACHABLE_RTT: u64 = u64::max_value();

#[derive(Clone, Copy, Debug)]
pub struct AddressEntry {
    address: IpAddr,
    rtt: u64,
}

impl AddressEntry {
    pub fn new(address: IpAddr, rtt: u64) -> Self {
        AddressEntry { address, rtt }
    }

    #[inline]
    pub fn get_addr(&self) -> IpAddr {
        self.address
    }

    #[inline]
    pub fn get_v4_addr(&self) -> Ipv4Addr {
        match self.address {
            IpAddr::V4(addr) => addr,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn get_rtt(&self) -> u64 {
        self.rtt
    }

    #[inline]
    pub fn set_rtt(&mut self, rtt: Duration) {
        let new = rtt.as_nanos() as u64;
        if self.rtt == UNREACHABLE_RTT {
            self.rtt = new
        } else {
            self.rtt = ((self.rtt * 3) + (new * 7)) / 10;
        }
    }

    #[inline]
    pub fn is_reachable(&self) -> bool {
        self.rtt != UNREACHABLE_RTT
    }

    #[inline]
    pub fn set_unreachable(&mut self) {
        self.rtt = UNREACHABLE_RTT;
    }

    #[inline]
    pub fn is_v4(&self) -> bool {
        self.address.is_ipv4()
    }

    #[inline]
    pub fn is_v6(&self) -> bool {
        self.address.is_ipv6()
    }
}

impl PartialEq for AddressEntry {
    fn eq(&self, other: &AddressEntry) -> bool {
        self.address.eq(&other.address)
    }
}

impl Eq for AddressEntry {}

impl PartialOrd for AddressEntry {
    fn partial_cmp(&self, other: &AddressEntry) -> Option<Ordering> {
        self.rtt.partial_cmp(&other.rtt)
    }
}

impl Ord for AddressEntry {
    fn cmp(&self, other: &AddressEntry) -> Ordering {
        self.rtt.cmp(&other.rtt)
    }
}

pub fn select_address(addresses: &Vec<AddressEntry>) -> Option<AddressEntry> {
    addresses
        .iter()
        .filter(|a| a.get_addr().is_ipv4())
        .min()
        .map(|a| *a)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::net::Ipv6Addr;

    #[test]
    fn test_address_selector() {
        let addresses = vec![
            AddressEntry::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 0),
            AddressEntry::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 1),
            AddressEntry::new(IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)), 2),
        ];

        let target = select_address(&addresses);
        assert_eq!(target.unwrap().get_addr(), Ipv4Addr::new(1, 1, 1, 1));

        let mut addr = AddressEntry::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 0);
        addr.set_unreachable();
        assert_eq!(addr.get_rtt(), UNREACHABLE_RTT);
        addr.set_rtt(Duration::from_nanos(10));
        assert_eq!(addr.get_rtt(), 10);

        addr.set_rtt(Duration::from_nanos(70));
        assert_eq!(addr.get_rtt(), 52);
    }
}
