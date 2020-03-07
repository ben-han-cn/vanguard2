use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Add;
use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

use lru::LruCache;

const SERVER_INIT_RTT: Duration = Duration::from_secs(0); //0 secs
const TIMECOUNT_SERVER_SLEEP_TIME: Duration = Duration::from_secs(60); //1 minute
const MAX_TIMEOUT_COUNT: u8 = 3;

pub(crate) type Host = IpAddr;

pub trait HostSelector {
    fn set_rtt(&mut self, host: Host, rtt: Duration);
    fn set_timeout(&mut self, host: Host);
    fn select(&self, hosts: &[Host]) -> Option<Host>;
}

#[derive(Clone, Copy, Debug)]
struct HostState {
    rtt: Duration,
    timeout_count: u8,
    wakeup_time: Option<Instant>,
}

impl HostState {
    pub fn new(rtt: Duration) -> Self {
        Self {
            rtt,
            timeout_count: 0,
            wakeup_time: None,
        }
    }

    pub fn unreachable_host() -> Self {
        Self {
            rtt: Duration::from_secs(u64::max_value()),
            timeout_count: 1,
            wakeup_time: None,
        }
    }

    pub fn set_rtt(&mut self, rtt: Duration) {
        if self.timeout_count > 0 {
            self.rtt = rtt;
            self.timeout_count = 0;
            self.wakeup_time = None;
        } else {
            self.rtt = self
                .rtt
                .checked_mul(3)
                .unwrap()
                .checked_add(rtt.checked_mul(7).unwrap())
                .unwrap()
                .checked_div(10)
                .unwrap();
        }
    }

    pub fn set_timout(&mut self) {
        self.rtt = Duration::from_secs(u64::max_value());
        if self.timeout_count < MAX_TIMEOUT_COUNT {
            self.timeout_count += 1;
        }

        if self.timeout_count == MAX_TIMEOUT_COUNT {
            self.wakeup_time = Some(Instant::now().add(TIMECOUNT_SERVER_SLEEP_TIME))
        }
    }

    pub fn is_usable(&self) -> bool {
        if let Some(wakeup_time) = self.wakeup_time {
            return Instant::now() > wakeup_time;
        } else {
            true
        }
    }

    pub fn get_rtt(&self) -> Duration {
        self.rtt
    }
}

pub struct RTTBasedHostSelector {
    host_and_rtt: RefCell<LruCache<Host, HostState>>,
}

impl RTTBasedHostSelector {
    pub fn new(cap: usize) -> Self {
        Self {
            host_and_rtt: RefCell::new(LruCache::new(cap)),
        }
    }

    fn get_rtt(&self, host: &Host) -> Duration {
        if let Some(state) = self.host_and_rtt.borrow_mut().get(host) {
            state.get_rtt()
        } else {
            SERVER_INIT_RTT
        }
    }

    fn is_host_usable(&self, host: &Host) -> bool {
        if let Some(state) = self.host_and_rtt.borrow_mut().get(host) {
            state.is_usable()
        } else {
            true
        }
    }
}

impl HostSelector for RTTBasedHostSelector {
    fn set_rtt(&mut self, host: Host, rtt: Duration) {
        let mut inner = self.host_and_rtt.borrow_mut();
        if let Some(state) = inner.get_mut(&host) {
            state.set_rtt(rtt)
        } else {
            inner.put(host, HostState::new(rtt));
        }
    }

    fn set_timeout(&mut self, host: Host) {
        let mut inner = self.host_and_rtt.borrow_mut();
        if let Some(state) = inner.get_mut(&host) {
            state.set_timout()
        } else {
            inner.put(host, HostState::unreachable_host());
        }
    }

    fn select(&self, hosts: &[Host]) -> Option<Host> {
        hosts
            .iter()
            .filter(|h| self.is_host_usable(h))
            .min_by(|h1, h2| self.get_rtt(h1).cmp(&self.get_rtt(h2)))
            .map(|h| *h)
    }
}

#[cfg(test)]
mod tests {
    use super::{Host, HostSelector, RTTBasedHostSelector};
    use std::{
        net::{IpAddr, Ipv4Addr},
        time::Duration,
    };

    #[test]
    fn test_rtt_based_selector() {
        let mut selector = RTTBasedHostSelector::new(10);
        let host1 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let host2 = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        selector.set_rtt(host1, Duration::from_secs(10));
        selector.set_rtt(host2, Duration::from_secs(11));
        assert_eq!(selector.select(vec![host1, host2].as_ref()), host1);
        selector.set_rtt(host1, Duration::from_secs(12));
        assert_eq!(selector.select(vec![host1, host2].as_ref()), host2);
    }
}
