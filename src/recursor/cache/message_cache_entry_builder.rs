use super::{
    entry_key::EntryKey,
    message_util::{get_rrset_trust_level, is_negative_response},
    rrset_cache::RRsetLruCache,
};
use r53::{
    header_flag::HeaderFlag, message::SectionType, Message, MessageBuilder, Name, RRTtl, RRType,
    RRset,
};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct MessageEntryGenerateor {
    response: Message,
    response_type: ResponseCategory,
    positive_cache: &mut RRsetLruCache,
    negative_cache: &mut RRsetLruCache,
}

unsafe impl Send for MessageEntry {}

impl MessageEntryGenerateor {
    pub fn new(
        mut message: Message,
        positive_cache: &mut RRsetLruCache,
        negative_cache: &mut RRsetLruCache,
    ) -> Self {
        let mut min_ttl = RRTtl(u32::max_value());
        let response_is_negative = is_negative_response(&message);
        let mut rrset_refs = Vec::new();
        let mut answer_rrset_count = 0;
        let mut auth_rrset_count = 0;
        let mut additional_rrset_count = 0;
        if let Some(answers) = message.take_section(SectionType::Answer) {
            answer_rrset_count = answers.len() as u16;
            rrset_refs.append(&mut add_rrset_in_section(
                positive_cache,
                &message,
                answers,
                SectionType::Answer,
                &mut min_ttl,
            ));
        }
        if response_is_negative {
            if let Some(authorities) = message.take_section(SectionType::Authority) {
                auth_rrset_count = authorities.len() as u16;
                rrset_refs.append(&mut add_rrset_in_negative_response_auth_section(
                    positive_cache,
                    negative_cache,
                    &message,
                    authorities,
                    &mut min_ttl,
                ));
            }
        } else {
            if let Some(authorities) = message.take_section(SectionType::Authority) {
                auth_rrset_count = authorities.len() as u16;
                rrset_refs.append(&mut add_rrset_in_section(
                    positive_cache,
                    &message,
                    authorities,
                    SectionType::Authority,
                    &mut min_ttl,
                ));
            }
        }

        if let Some(additionals) = message.take_section(SectionType::Additional) {
            additional_rrset_count = additionals.len() as u16;
            rrset_refs.append(&mut add_rrset_in_section(
                positive_cache,
                &message,
                additionals,
                SectionType::Additional,
                &mut min_ttl,
            ));
        }
        let expire_time = Instant::now()
            .checked_add(Duration::from_secs(min_ttl.0 as u64))
            .unwrap();

        let question = message.question.take().unwrap();
        let qtype = question.typ;
        MessageEntry {
            name: Box::into_raw(Box::new(question.name)),
            typ: qtype,
            answer_rrset_count,
            auth_rrset_count,
            additional_rrset_count,
            rrset_refs,
            expire_time,
        }
    }

    pub fn key(&self) -> EntryKey {
        EntryKey(self.name, self.typ)
    }

    pub fn fill_message(
        &self,
        query: &mut Message,
        positive_cache: &mut RRsetLruCache,
        negative_cache: &mut RRsetLruCache,
    ) -> bool {
        if self.expire_time <= Instant::now() {
            return false;
        }

        let rrsets = self.get_rrsets(positive_cache, negative_cache);
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

    fn get_rrsets(
        &self,
        positive_cache: &mut RRsetLruCache,
        negative_cache: &mut RRsetLruCache,
    ) -> Option<Vec<RRset>> {
        let rrset_count = self.rrset_refs.len();
        let mut rrsets = Vec::with_capacity(rrset_count);
        for rrset_ref in &self.rrset_refs {
            let rrset = if rrset_ref.is_negative {
                negative_cache.get_rrset(&rrset_ref.name, rrset_ref.typ)
            } else {
                positive_cache.get_rrset(&rrset_ref.name, rrset_ref.typ)
            };

            if let Some(rrset) = rrset {
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


fn add_rrset_in_response_to_rrset_cache(
    response: &Message,
    response_type: ResponseCategory
    min_ttl: &mut RRTtl
    positive_cache: &mut RRsetLruCache,
    negative_cache: &mut RRsetLruCache,
    ) -> (u16, u16, u16, Vec<RRsetRef>) {
    let answer_rrset_count = message.section(Answer).map_or(0, |rrset| rrset.len());
    let auth_rrset_count = message.section(Authority).map_or(0, |rrset| rrset.len());
    let additional_rrset_count = message.section(Additional).map_or(0, |rrset| rrset.len());
    let mut refs = Vec::with_capacity(answer_rrset_count+auth_rrset_count+additional_rrset_count);
    match response_type {
        ResponseCategory::Answer | ResponseCategory::AnswerCName => {
            add_rrset_in_section(message.take_section(Answer).unwrap(), positive_cache, negative_cache, response, false, min_ttl); 
            if auth_rrset_count > 0 {
            add_rrset_in_section(message.take_section(Authority).unwrap(), positive_cache, negative_cache, response, false, min_ttl); 
            }
            if additional_rrset_count > 0 {
            add_rrset_in_section(message.take_section(Additional).unwrap(), positive_cache, negative_cache, response, false, min_ttl); 
            }

        }
    }
}

fn add_section_to_cache(
    refs: &mut Vec<RRsetRef>,
    rrsets: Vec<RRset>,
    positive_cache: &mut RRsetLruCache,
    negative_cache: &mut RRsetLruCache,
    repsonse: &Message,
    is_negative: bool,
    min_ttl: &mut RRTtl,
    ) {
    let trust_level = get_rrset_trust_level(message, section);
    for rrset in rrsets.into_iter() {
        refs.push(RRsetRef {
            name: rrset.name.clone(),
            typ: rrset.typ,
            is_negative: is_negative,
        });
        if rrset.ttl.0 < min_ttl.0 {
            *min_ttl = rrset.ttl;
        }

        if is_negative {
            negative_cache.add_rrset(rrset, trust_level);
        } else {
            positive_cache.add_rrset(rrset, trust_level);
        }
    }
}

fn add_rrset_in_negative_response_auth_section(
    positive_cache: &mut RRsetLruCache,
    negative_cache: &mut RRsetLruCache,
    message: &Message,
    rrsets: Vec<RRset>,
    min_ttl: &mut RRTtl,
) -> Vec<RRsetRef> {
    let mut refs = Vec::with_capacity(rrsets.len());
    let trust_level = get_rrset_trust_level(message, SectionType::Authority);
    for rrset in rrsets.into_iter() {
        refs.push(RRsetRef {
            name: rrset.name.clone(),
            typ: rrset.typ,
            is_negative: rrset.typ == RRType::SOA,
        });
        if rrset.ttl.0 < min_ttl.0 {
            *min_ttl = rrset.ttl;
        }
        if rrset.typ == RRType::SOA {
            negative_cache.add_rrset(rrset, trust_level);
        } else {
            positive_cache.add_rrset(rrset, trust_level);
        }
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
        let mut positive_cache = RRsetLruCache::new(100);
        let mut negative_cache = RRsetLruCache::new(100);
        let entry = MessageEntry::new(message.clone(), &mut positive_cache, &mut negative_cache);
        assert_eq!(positive_cache.len(), 3);
        assert_eq!(negative_cache.len(), 0);
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
        assert!(entry.fill_message(&mut query, &mut positive_cache, &mut negative_cache));
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
        let mut positive_cache = RRsetLruCache::new(100);
        let mut negative_cache = RRsetLruCache::new(100);
        let entry = MessageEntry::new(message.clone(), &mut positive_cache, &mut negative_cache);
        assert_eq!(positive_cache.len(), 0);
        assert_eq!(negative_cache.len(), 1);
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
        assert!(entry.fill_message(&mut query, &mut positive_cache, &mut negative_cache));
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
