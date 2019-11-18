use super::message_cache::MessageLruCache;
use crate::recursor::message_classifier::ResponseCategory;
use r53::{Message, Name, RRType, RRset};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RRsetTrustLevel {
    AdditionalWithoutAA,
    AuthorityWithoutAA,
    AdditionalWithAA,
    NonAuthAnswerWithAA,
    AnswerWithoutAA,
    PrimGlue,
    AuthorityWithAA,
    AnswerWithAA,
    PrimNonGlue,
}

pub struct MessageCache {
    positive_cache: MessageLruCache,
    negative_cache: MessageLruCache,
}

impl MessageCache {
    pub fn new(cap: usize) -> Self {
        debug_assert!(cap > 0);
        MessageCache {
            positive_cache: MessageLruCache::new(cap),
            negative_cache: MessageLruCache::new(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.positive_cache.len() + self.negative_cache.len()
    }

    pub fn gen_response(&mut self, query: &mut Message) -> bool {
        self.positive_cache.gen_response(query) || self.negative_cache.gen_response(query)
    }

    pub fn add_response(&mut self, response_type: ResponseCategory, response: Message) {
        match response_type {
            ResponseCategory::Answer | ResponseCategory::AnswerCName => {
                self.positive_cache.add_response(response);
            }
            ResponseCategory::NXDomain | ResponseCategory::NXRRset => {
                self.negative_cache.add_response(response);
            }
            ResponseCategory::Referral => {
                self.positive_cache.add_rrset_in_response(response);
            }
            _ => {}
        }
    }

    pub fn add_rrset(&mut self, rrset: RRset, trust_level: RRsetTrustLevel) {
        if rrset.typ == RRType::SOA {
            self.negative_cache.add_rrset(rrset, trust_level);
        } else {
            self.positive_cache.add_rrset(rrset, trust_level);
        }
    }

    pub fn get_deepest_ns(&mut self, name: &Name) -> Option<Name> {
        self.positive_cache.get_deepest_ns(name)
    }
}
