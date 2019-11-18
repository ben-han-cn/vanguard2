use super::{cache::RRsetTrustLevel, entry_key::EntryKey, rrset_cache_entry::RRsetEntry};
use lru::LruCache;
use r53::{header_flag::HeaderFlag, Message, MessageBuilder, Name, RData, RRType, RRset};

pub struct RRsetLruCache {
    rrsets: LruCache<EntryKey, RRsetEntry>,
}

impl RRsetLruCache {
    pub fn new(cap: usize) -> Self {
        RRsetLruCache {
            rrsets: LruCache::new(cap),
        }
    }

    pub fn has_rrset(&self, key: &EntryKey) -> bool {
        self.rrsets.contains(key)
    }

    pub fn get_rrset_with_key(&mut self, key: &EntryKey) -> Option<RRset> {
        match self.rrsets.get(key) {
            Some(entry) => {
                let rrset = entry.get_rrset();
                if rrset.is_none() {
                    self.rrsets.pop(key);
                }
                rrset
            }
            _ => None,
        }
    }

    pub fn gen_response(&mut self, key: &EntryKey, message: &mut Message) -> bool {
        match self.get_rrset_with_key(key) {
            Some(rrset) => {
                let mut builder = MessageBuilder::new(message);
                builder
                    .make_response()
                    .set_flag(HeaderFlag::RecursionAvailable);
                if key.1 == RRType::NS {
                    for rdata in rrset.rdatas.iter() {
                        if let RData::NS(ref ns) = rdata {
                            if ns.name.is_subdomain(&rrset.name) {
                                let key = EntryKey(&ns.name as *const Name, RRType::A);
                                if let Some(rrset) = self.get_rrset_with_key(&key) {
                                    builder.add_additional(rrset);
                                }
                                let key = EntryKey(&ns.name as *const Name, RRType::AAAA);
                                if let Some(rrset) = self.get_rrset_with_key(&key) {
                                    builder.add_additional(rrset);
                                }
                            }
                        }
                    }
                }
                builder.add_answer(rrset).done();
                true
            }
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.rrsets.len()
    }

    pub fn get_rrset(&mut self, name: &Name, typ: RRType) -> Option<RRset> {
        self.get_rrset_with_key(&EntryKey(name as *const Name, typ))
    }

    pub fn add_rrset(&mut self, rrset: RRset, trust_level: RRsetTrustLevel) {
        let key = &EntryKey(&rrset.name as *const Name, rrset.typ);
        if let Some(entry) = self.rrsets.peek(key) {
            if !entry.is_expired() && entry.trust_level > trust_level {
                return;
            }
        }
        self.rrsets.pop(key);
        let entry = RRsetEntry::new(rrset, trust_level);
        self.rrsets.put(entry.key(), entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    #[test]
    fn test_rrset_cache() {
        let mut cache = RRsetLruCache::new(2);

        let rrset = RRset::from_str("www.zdns.cn 300 IN A 1.1.1.1").unwrap();
        assert!(cache.get_rrset(&rrset.name, rrset.typ).is_none());
        cache.add_rrset(rrset.clone(), RRsetTrustLevel::NonAuthAnswerWithAA);
        let insert_rrset = cache.get_rrset(&rrset.name, rrset.typ).unwrap();
        assert_eq!(insert_rrset.rdatas, rrset.rdatas);

        let low_trust_level_rrset = RRset::from_str("www.zdns.cn 300 IN A 2.2.2.2").unwrap();
        cache.add_rrset(
            low_trust_level_rrset.clone(),
            RRsetTrustLevel::AdditionalWithoutAA,
        );
        let insert_rrset = cache.get_rrset(&rrset.name, rrset.typ).unwrap();
        assert_eq!(insert_rrset.rdatas, rrset.rdatas);

        cache.add_rrset(low_trust_level_rrset.clone(), RRsetTrustLevel::PrimNonGlue);
        let insert_rrset = cache.get_rrset(&rrset.name, rrset.typ).unwrap();
        assert_eq!(insert_rrset.rdatas, low_trust_level_rrset.rdatas);
        assert_eq!(cache.len(), 1);

        let rrset = RRset::from_str("www1.zdns.cn 300 IN A 1.1.1.1").unwrap();
        cache.add_rrset(rrset.clone(), RRsetTrustLevel::NonAuthAnswerWithAA);
        let rrset = RRset::from_str("www2.zdns.cn 300 IN A 1.1.1.1").unwrap();
        cache.add_rrset(rrset.clone(), RRsetTrustLevel::NonAuthAnswerWithAA);
        assert_eq!(cache.len(), 2);
        assert!(cache
            .get_rrset(&Name::new("www.zdns.cn").unwrap(), RRType::A)
            .is_none());
        assert!(cache
            .get_rrset(&Name::new("www1.zdns.cn").unwrap(), RRType::A)
            .is_some());
    }

    #[test]
    fn test_rrset_cache_bench() {
        let mut cache = RRsetLruCache::new(10);
        assert_eq!(cache.len(), 0);
        for i in 0..1000 {
            let rrset = format!("www{}.zdns.cn 300 IN A 1.1.1.1", i);
            let rrset = RRset::from_str(rrset.as_ref()).unwrap();
            cache.add_rrset(rrset.clone(), RRsetTrustLevel::NonAuthAnswerWithAA);
        }
        assert_eq!(cache.len(), 10);

        for i in 0..989 {
            let rrset = format!("www{}.zdns.cn 300 IN A 1.1.1.1", i);
            let rrset = RRset::from_str(rrset.as_ref()).unwrap();
            assert!(cache.get_rrset(&rrset.name, rrset.typ).is_none());
        }

        for i in 990..1000 {
            let rrset = format!("www{}.zdns.cn 300 IN A 1.1.1.1", i);
            let rrset = RRset::from_str(rrset.as_ref()).unwrap();
            assert!(cache.get_rrset(&rrset.name, rrset.typ).is_some());
        }
    }
}
