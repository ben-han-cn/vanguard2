use super::{entry_key::EntryKey, message_util::get_rrset_trust_level, rrset_cache::RRsetLruCache};
use r53::{
    header_flag::HeaderFlag, message::SectionType, Message, MessageBuilder, Name, RRTtl, RRType,
    RRset,
};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct RRsetRef {
    pub name: Name,
    pub typ: RRType,
}

#[derive(Clone, Debug)]
pub struct MessageEntry {
    name: *mut Name,
    typ: RRType,
    answer_rrset_count: u16,
    auth_rrset_count: u16,
    additional_rrset_count: u16,
    rrset_refs: Vec<RRsetRef>,
    expire_time: Instant,
}

unsafe impl Send for MessageEntry {}

impl MessageEntry {
    pub fn new(mut message: Message, rrset_cache: &mut RRsetLruCache) -> Self {
        let answer_rrset_count = MessageEntry::section_rrset_count(&message, SectionType::Answer);
        let auth_rrset_count = MessageEntry::section_rrset_count(&message, SectionType::Authority);
        let additional_rrset_count =
            MessageEntry::section_rrset_count(&message, SectionType::Additional);
        let question = message.question.take().unwrap();
        let qtype = question.typ;
        let mut entry = MessageEntry {
            name: Box::into_raw(Box::new(question.name)),
            typ: qtype,
            answer_rrset_count,
            auth_rrset_count,
            additional_rrset_count,
            rrset_refs: Vec::with_capacity(
                (answer_rrset_count + auth_rrset_count + additional_rrset_count) as usize,
            ),
            expire_time: Instant::now(),
        };

        let mut min_ttl = RRTtl(u32::max_value());
        if answer_rrset_count > 0 {
            entry.add_section(&mut message, SectionType::Answer, rrset_cache, &mut min_ttl);
        }
        if auth_rrset_count > 0 {
            entry.add_section(
                &mut message,
                SectionType::Authority,
                rrset_cache,
                &mut min_ttl,
            );
        }
        if additional_rrset_count > 0 {
            entry.add_section(
                &mut message,
                SectionType::Additional,
                rrset_cache,
                &mut min_ttl,
            );
        }
        entry.expire_time = entry
            .expire_time
            .checked_add(Duration::from_secs(min_ttl.0 as u64))
            .unwrap();
        entry
    }

    fn section_rrset_count(message: &Message, section: SectionType) -> u16 {
        message
            .section(section)
            .map_or(0, |rrsets| rrsets.len() as u16)
    }

    fn add_section(
        &mut self,
        message: &mut Message,
        section: SectionType,
        rrset_cache: &mut RRsetLruCache,
        min_ttl: &mut RRTtl,
    ) {
        let trust_level = get_rrset_trust_level(message, section);
        for rrset in message.take_section(section).unwrap().into_iter() {
            self.rrset_refs.push(RRsetRef {
                name: rrset.name.clone(),
                typ: rrset.typ,
            });
            if rrset.ttl.0 < min_ttl.0 {
                *min_ttl = rrset.ttl;
            }
            rrset_cache.add_rrset(rrset, trust_level);
        }
    }

    #[inline]
    pub fn key(&self) -> EntryKey {
        EntryKey(self.name, self.typ)
    }

    #[inline]
    pub fn is_expired(&self) -> bool {
        self.expire_time <= Instant::now()
    }

    pub fn fill_message(&self, query: &mut Message, rrset_cache: &mut RRsetLruCache) -> bool {
        if self.is_expired() {
            return false;
        }

        let rrsets = self.get_rrsets(rrset_cache);
        if rrsets.is_none() {
            return false;
        }

        let mut builder = MessageBuilder::new(query);
        builder
            .make_response()
            .set_flag(HeaderFlag::RecursionAvailable);
        let mut iter = rrsets.unwrap().into_iter();
        for _ in 0..self.answer_rrset_count {
            builder.add_answer(iter.next().unwrap());
        }
        for _ in 0..self.auth_rrset_count {
            builder.add_auth(iter.next().unwrap());
        }

        for _ in 0..self.additional_rrset_count {
            builder.add_additional(iter.next().unwrap());
        }
        builder.done();
        true
    }

    fn get_rrsets(&self, rrset_cache: &mut RRsetLruCache) -> Option<Vec<RRset>> {
        let rrset_count = self.rrset_refs.len();
        let mut rrsets = Vec::with_capacity(rrset_count);
        for rrset_ref in &self.rrset_refs {
            if let Some(rrset) = rrset_cache.get_rrset(&rrset_ref.name, rrset_ref.typ) {
                rrsets.push(rrset);
            } else {
                return None;
            }
        }
        Some(rrsets)
    }
}

impl Drop for MessageEntry {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.name);
        }
    }
}

fn add_rrset_in_section(
    rrset_cache: &mut RRsetLruCache,
    message: &Message,
    rrsets: Vec<RRset>,
    section: SectionType,
    min_ttl: &mut RRTtl,
) -> Vec<RRsetRef> {
    let mut refs = Vec::with_capacity(rrsets.len());
    let trust_level = get_rrset_trust_level(message, section);
    for rrset in rrsets.into_iter() {
        refs.push(RRsetRef {
            name: rrset.name.clone(),
            typ: rrset.typ,
        });
        if rrset.ttl.0 < min_ttl.0 {
            *min_ttl = rrset.ttl;
        }
        rrset_cache.add_rrset(rrset, trust_level);
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use r53::{edns::Edns, Rcode};
    use std::str::FromStr;

    fn build_positive_response() -> Message {
        let mut msg = Message::with_query(Name::new("test.example.com.").unwrap(), RRType::A);
        {
            let mut builder = MessageBuilder::new(&mut msg);
            builder
                .id(1200)
                .rcode(Rcode::NoError)
                .set_flag(HeaderFlag::RecursionDesired)
                .add_answer(RRset::from_str("test.example.com. 3600 IN A 192.0.2.2").unwrap())
                .add_answer(RRset::from_str("test.example.com. 3600 IN A 192.0.2.1").unwrap())
                .add_auth(RRset::from_str("example.com. 10 IN NS ns1.example.com.").unwrap())
                .add_additional(RRset::from_str("ns1.example.com. 3600 IN A 2.2.2.2").unwrap())
                .edns(Edns {
                    versoin: 0,
                    extened_rcode: 0,
                    udp_size: 4096,
                    dnssec_aware: false,
                    options: None,
                })
                .done();
        }
        msg
    }

    fn build_negative_response() -> Message {
        let mut msg = Message::with_query(Name::new("test.example.com.").unwrap(), RRType::A);
        {
            let mut builder = MessageBuilder::new(&mut msg);
            builder
                .id(1200)
                .rcode(Rcode::NXDomain)
                .set_flag(HeaderFlag::RecursionDesired)
                .add_auth(RRset::from_str("example.com. 30 IN SOA a.gtld-servers.net. nstld.verisign-grs.com. 1563935574 1800 900 604800 86400").unwrap())
                .edns(Edns {
                    versoin: 0,
                    extened_rcode: 0,
                    udp_size: 4096,
                    dnssec_aware: false,
                    options: None,
                })
                .done();
        }
        msg
    }
    #[test]
    fn test_positive_message() {
        let message = build_positive_response();
        let mut rrset_cache = RRsetLruCache::new(100);
        let entry = MessageEntry::new(message.clone(), &mut rrset_cache);
        assert_eq!(rrset_cache.len(), 3);
        assert_eq!(
            unsafe { (*entry.name).clone() },
            Name::new("test.example.com").unwrap()
        );
        assert_eq!(entry.typ, RRType::A);
        assert_eq!(entry.answer_rrset_count, 1);
        assert_eq!(entry.auth_rrset_count, 1);
        assert_eq!(entry.additional_rrset_count, 1);
        assert_eq!(entry.rrset_refs.len(), 3);
        assert!(entry.expire_time < Instant::now().checked_add(Duration::from_secs(10)).unwrap());

        let mut query = Message::with_query(Name::new("test.example.com.").unwrap(), RRType::A);
        assert!(entry.fill_message(&mut query, &mut rrset_cache));
        assert_eq!(query.header.qd_count, message.header.qd_count);
        assert_eq!(query.header.an_count, message.header.an_count);
        assert_eq!(query.header.ns_count, message.header.ns_count);
        assert_eq!(query.header.ar_count, message.header.ar_count - 1);

        for section in vec![
            SectionType::Answer,
            SectionType::Authority,
            SectionType::Additional,
        ] {
            let gen_message_sections = query.section(section).unwrap();
            for (i, rrset) in message.section(section).unwrap().iter().enumerate() {
                assert_eq!(rrset.typ, gen_message_sections[i].typ);
                assert_eq!(rrset.rdatas, gen_message_sections[i].rdatas);
                assert_eq!(rrset.name, gen_message_sections[i].name);
                assert!(rrset.ttl.0 > gen_message_sections[i].ttl.0);
            }
        }
    }

    #[test]
    fn test_negative_message() {
        let message = build_negative_response();
        let mut rrset_cache = RRsetLruCache::new(100);
        let entry = MessageEntry::new(message.clone(), &mut rrset_cache);
        assert_eq!(rrset_cache.len(), 1);
        assert_eq!(
            unsafe { (*entry.name).clone() },
            Name::new("test.example.com").unwrap()
        );
        assert_eq!(entry.typ, RRType::A);
        assert_eq!(entry.answer_rrset_count, 0);
        assert_eq!(entry.auth_rrset_count, 1);
        assert_eq!(entry.additional_rrset_count, 0);
        assert_eq!(entry.rrset_refs.len(), 1);
        assert!(entry.expire_time < Instant::now().checked_add(Duration::from_secs(30)).unwrap());
        assert!(entry.expire_time > Instant::now().checked_add(Duration::from_secs(20)).unwrap());

        let mut query = Message::with_query(Name::new("test.example.com.").unwrap(), RRType::A);
        assert!(entry.fill_message(&mut query, &mut rrset_cache));
        assert_eq!(query.header.qd_count, message.header.qd_count);
        assert_eq!(query.header.an_count, message.header.an_count);
        assert_eq!(query.header.ns_count, message.header.ns_count);
        assert_eq!(query.header.ar_count, message.header.ar_count - 1);

        for section in vec![SectionType::Authority] {
            let gen_message_sections = query.section(section).unwrap();
            for (i, rrset) in message.section(section).unwrap().iter().enumerate() {
                assert_eq!(rrset.typ, gen_message_sections[i].typ);
                assert_eq!(rrset.rdatas, gen_message_sections[i].rdatas);
                assert_eq!(rrset.name, gen_message_sections[i].name);
                assert!(rrset.ttl.0 > gen_message_sections[i].ttl.0);
            }
        }
    }
}
