use super::{cache::RRsetTrustLevel, entry_key::EntryKey};
use r53::{Name, RData, RRClass, RRTtl, RRType, RRset};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct RRsetEntry {
    name: *mut Name,
    typ: RRType,
    pub trust_level: RRsetTrustLevel,
    rdatas: Vec<RData>,
    expire_time: Instant,
}

unsafe impl Send for RRsetEntry {}

impl RRsetEntry {
    pub fn new(rrset: RRset, trust_level: RRsetTrustLevel) -> Self {
        let expire_time = Instant::now()
            .checked_add(Duration::from_secs(rrset.ttl.0 as u64))
            .unwrap();

        let name = Box::into_raw(Box::new(rrset.name));
        RRsetEntry {
            name,
            typ: rrset.typ,
            trust_level,
            rdatas: rrset.rdatas,
            expire_time,
        }
    }

    pub fn key(&self) -> EntryKey {
        EntryKey(self.name, self.typ)
    }

    pub fn is_expired(&self) -> bool {
        self.expire_time <= Instant::now()
    }

    pub fn get_rrset(&self) -> Option<RRset> {
        let now = Instant::now();
        if self.expire_time <= now {
            return None;
        }

        unsafe {
            Some(RRset {
                name: (*self.name).clone(),
                typ: self.typ,
                class: RRClass::IN,
                ttl: RRTtl(self.expire_time.duration_since(now).as_secs() as u32),
                rdatas: self.rdatas.clone(),
            })
        }
    }
}

impl Drop for RRsetEntry {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    #[test]
    fn test_rrset_entry() {
        let rrset = RRset::from_str("www.zdns.cn 300 IN A 2.2.2.2").unwrap();
        let entry = RRsetEntry::new(rrset.clone(), RRsetTrustLevel::AdditionalWithoutAA);
        let entry_key = entry.key();
        assert_eq!(
            entry_key,
            EntryKey(
                &Name::from_str("www.zdns.cn").unwrap() as *const Name,
                RRType::A,
            )
        );

        let mut rrset_with_new_ttl = entry.get_rrset().unwrap();
        assert!(rrset_with_new_ttl.ttl != rrset.ttl);
        rrset_with_new_ttl.ttl = rrset.ttl;
        assert_eq!(rrset_with_new_ttl, rrset);
    }
}
