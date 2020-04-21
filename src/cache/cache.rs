use super::message_cache::MessageLruCache;
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

    pub fn gen_response(&mut self, request: &Message) -> Option<Message> {
        let response = self.positive_cache.gen_response(request);
        if response.is_none() {
            self.negative_cache.gen_response(request)
        } else {
            response
        }
    }

    pub fn add_response(&mut self, response: Message) {
        if response.header.an_count > 0 {
            self.positive_cache.add_response(response);
        } else {
            self.negative_cache.add_response(response);
        }
    }

    pub fn add_rrset_in_response(&mut self, message: Message) {
        self.positive_cache.add_rrset_in_response(message);
    }

    pub fn get_deepest_ns(&mut self, name: &Name) -> Option<RRset> {
        self.positive_cache.get_deepest_ns(name)
    }

    pub fn get_rrset(&mut self, name: &Name, typ: RRType) -> Option<RRset> {
        self.positive_cache.get_rrset(name, typ)
    }
}
