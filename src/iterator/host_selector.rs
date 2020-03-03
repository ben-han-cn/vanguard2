use std::cell::RefCell;
use std::collections::HashMap;
use std::{net::IpAddr, time::Duration};

use lru::LruCache;

const SERVER_INIT_RTT: Duration = Duration::from_secs(0); //0 secs

pub(crate) type Host = IpAddr;

pub trait HostSelector {
    fn set_rtt(&mut self, host: Host, rtt: Duration);
    //assume hosts isn't empty
    fn select(&self, hosts: &[Host]) -> Host;
}

pub struct RTTBasedHostSelector {
    host_and_rtt: RefCell<LruCache<Host, Duration>>,
}

impl RTTBasedHostSelector {
    pub fn new(cap: usize) -> Self {
        Self {
            host_and_rtt: RefCell::new(LruCache::new(cap)),
        }
    }

    fn get_rtt(&self, host: &Host) -> Duration {
        let mut inner = self.host_and_rtt.borrow_mut();
        if let Some(rtt) = inner.get(host) {
            *rtt
        } else {
            SERVER_INIT_RTT
        }
    }
}

impl HostSelector for RTTBasedHostSelector {
    fn set_rtt(&mut self, host: Host, rtt: Duration) {
        let mut inner = self.host_and_rtt.borrow_mut();
        let rtt = match inner.get(&host) {
            Some(old) => old
                .checked_mul(3)
                .unwrap()
                .checked_add(rtt.checked_mul(7).unwrap())
                .unwrap()
                .checked_div(10)
                .unwrap(),
            None => rtt,
        };
        inner.put(host, rtt);
    }

    fn select(&self, hosts: &[Host]) -> Host {
        assert!(!hosts.is_empty());

        let (_, index) = hosts.iter().enumerate().fold(
            (Duration::from_secs(u64::max_value()), 0),
            |(mut rtt, mut index): (Duration, usize), (i, host)| {
                let rtt_ = self.get_rtt(host);
                if rtt_ < rtt {
                    rtt = rtt_;
                    index = i;
                }
                (rtt, index)
            },
        );
        hosts[index]
    }
}
