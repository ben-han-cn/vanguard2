use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use anyhow::{bail, Result};
use treebitmap::IpLookupTable;

#[derive(Clone, Copy, Debug)]
pub struct Address {
    pub ip: IpAddr,
    pub mask_len: u32,
}

impl FromStr for Address {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let segs: Vec<&str> = s.split('/').collect();
        match segs.len() {
            1 => {
                let ip = IpAddr::from_str(segs[0])?;
                Ok(Address { ip, mask_len: 32 })
            }
            2 => {
                let ip = IpAddr::from_str(segs[0])?;
                let mask_len = u32::from_str(segs[1])?;
                Ok(Address { ip, mask_len })
            }
            _ => {
                bail!("invalid address format");
            }
        }
    }
}

pub struct Acl {
    pub addrs: Vec<Address>,
}

impl Acl {
    pub fn new(addreses: Vec<&str>) -> Result<Self> {
        let addrs = addreses
            .iter()
            .map(|&s| Address::from_str(s))
            .collect::<Result<Vec<Address>, _>>()?;
        Ok(Self { addrs })
    }
}

pub struct View {
    name: String,
    v4_trie: IpLookupTable<Ipv4Addr, ()>,
    v6_trie: IpLookupTable<Ipv6Addr, ()>,
}

impl View {
    pub fn new(name: String) -> Self {
        Self {
            name,
            v4_trie: IpLookupTable::new(),
            v6_trie: IpLookupTable::new(),
        }
    }

    pub fn add_addr(&mut self, addr: Address) {
        match addr.ip {
            IpAddr::V4(v4) => self.v4_trie.insert(v4, addr.mask_len, ()),
            IpAddr::V6(v6) => self.v6_trie.insert(v6, addr.mask_len, ()),
        };
    }

    pub fn has_addr(&self, addr: IpAddr) -> bool {
        match addr {
            IpAddr::V4(v4) => self.v4_trie.longest_match(v4).is_some(),
            IpAddr::V6(v6) => self.v6_trie.longest_match(v6).is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Acl, Address, View};
    use std::net::IpAddr;
    use std::str::FromStr;

    #[test]
    fn test_address_search() {
        let acl = Acl::new(vec!["2.0.0.0/8", "1.1.2.0/24", "1.1.3.4"]).unwrap();
        let mut view = View::new("default".to_string());
        for addr in acl.addrs {
            view.add_addr(addr);
        }

        assert!(view.has_addr(IpAddr::from_str("1.1.3.4").unwrap()));
        assert!(view.has_addr(IpAddr::from_str("1.1.2.1").unwrap()));
        assert!(view.has_addr(IpAddr::from_str("1.1.2.0").unwrap()));
        assert!(!view.has_addr(IpAddr::from_str("1.1.3.1").unwrap()));
        assert!(view.has_addr(IpAddr::from_str("2.1.3.1").unwrap()));
        assert!(!view.has_addr(IpAddr::from_str("3.1.3.1").unwrap()));
    }
}
