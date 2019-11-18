use super::{
    forwarder::{Forwarder, ForwarderManager},
    message_classifier::{classify_response, ResponseCategory},
    nsas::{NSAddressStore, Nameserver, NameserverFuture},
    recursor::{Recursor, Resolver},
    util::Sender,
};
use crate::error::VgError;
use failure;
use futures::{future, prelude::*, Future};
use r53::{message::SectionType, name, Message, MessageBuilder, Name, RData, RRType, Rcode};
use std::{mem, time::Duration};

const MAX_CNAME_DEPTH: usize = 12;

enum State {
    Init,
    Forward(Sender<Forwarder, ForwarderManager>),
    GetNameServer(NameserverFuture),
    QueryAuthServer(Sender<Nameserver, NSAddressStore>),
    Poisoned,
}

pub struct RunningQuery {
    current_name: Name,
    current_type: RRType,
    current_zone: Option<Name>,
    cname_depth: usize,
    response: Option<Message>,
    recursor: Recursor,
    state: State,
    depth: usize,
}

impl RunningQuery {
    pub fn new(query: Message, recursor: Recursor, depth: usize) -> Self {
        let question = query.question.as_ref().unwrap();
        let current_name = question.name.clone();
        let current_type = question.typ;

        RunningQuery {
            current_name,
            current_type,
            current_zone: None,
            cname_depth: 0,
            response: Some(query),
            recursor,
            state: State::Init,
            depth,
        }
    }

    pub fn reset(&mut self) {
        let query = self.response.as_mut().unwrap();
        query.take_section(SectionType::Answer);
        query.take_section(SectionType::Authority);
        query.take_section(SectionType::Additional);
        let question = query.question.as_ref().unwrap();
        self.current_name = question.name.clone();
        self.current_type = question.typ;
        self.current_zone = None;
        self.cname_depth = 0;
        self.state = State::Init;
        self.depth = 0;
    }

    fn lookup_in_cache(&mut self) -> Option<Message> {
        let mut current_query = Message::with_query(self.current_name.clone(), self.current_type);

        let cache = self.recursor.cache.clone();
        let mut cache = cache.lock().unwrap();
        let found_in_cache = cache.gen_response(&mut current_query);
        if found_in_cache {
            let response = self.make_response(current_query);
            let origin_query_name = &response.question.as_ref().unwrap().name;
            if !origin_query_name.eq(&self.current_name) {
                let response_type =
                    classify_response(origin_query_name, self.current_type, &response);
                cache.add_response(response_type, response.clone());
            }
            return Some(response);
        }

        if let Some(ns) = cache.get_deepest_ns(&self.current_name) {
            self.current_zone = Some(ns);
            return None;
        }

        self.recursor.roothint.fill_cache(&mut cache);
        self.current_zone = Some(name::root());
        return None;
    }

    pub fn handle_response(&mut self, response: Message) -> failure::Result<Option<Message>> {
        let response_type = classify_response(&self.current_name, self.current_type, &response);
        match response_type {
            ResponseCategory::Answer
            | ResponseCategory::AnswerCName
            | ResponseCategory::NXDomain
            | ResponseCategory::NXRRset => {
                let response = self.make_response(response);
                self.recursor
                    .cache
                    .lock()
                    .unwrap()
                    .add_response(response_type, response.clone());
                return Ok(Some(response));
            }
            ResponseCategory::Referral => {
                self.recursor
                    .cache
                    .lock()
                    .unwrap()
                    .add_response(response_type, response.clone());
                if !self.fetch_closer_zone(response) {
                    return Ok(Some(self.make_server_failed()));
                } else {
                    return Ok(None);
                }
            }
            ResponseCategory::CName(next) => {
                println!("get cname and query {:?}", next);
                self.cname_depth += response.header.an_count as usize;
                if self.cname_depth > MAX_CNAME_DEPTH {
                    return Ok(Some(self.make_server_failed()));
                }
                self.merge_response(response);
                self.current_name = next.clone();
                self.current_zone = None;
                return Ok(None);
            }
            ResponseCategory::Invalid(_) | ResponseCategory::FormErr => {
                return Ok(Some(self.make_server_failed()));
            }
        }
    }

    fn make_response(&mut self, mut response: Message) -> Message {
        let mut accumulate_response = self.response.take().unwrap();
        let mut builder = MessageBuilder::new(&mut accumulate_response);
        builder.make_response();
        builder.rcode(response.header.rcode);
        if let Some(answers) = response.take_section(SectionType::Answer) {
            for answer in answers {
                builder.add_answer(answer);
            }
        }

        if let Some(auths) = response.take_section(SectionType::Authority) {
            for auth in auths {
                builder.add_auth(auth);
            }
        }

        if let Some(additionals) = response.take_section(SectionType::Additional) {
            for additional in additionals {
                builder.add_additional(additional);
            }
        }
        builder.done();
        accumulate_response
    }

    fn make_server_failed(&mut self) -> Message {
        let mut accumulate_response = self.response.take().unwrap();
        let mut builder = MessageBuilder::new(&mut accumulate_response);
        builder.rcode(Rcode::ServFail);
        builder.done();
        accumulate_response
    }

    fn merge_response(&mut self, mut response: Message) {
        let mut builder = MessageBuilder::new(self.response.as_mut().unwrap());
        if let Some(answers) = response.take_section(SectionType::Answer) {
            for answer in answers {
                builder.add_answer(answer);
            }
        }
    }

    fn fetch_closer_zone(&mut self, mut response: Message) -> bool {
        let auth = response
            .take_section(SectionType::Authority)
            .expect("refer response should has answer");
        if auth.len() != 1 || auth[0].typ != RRType::NS {
            return false;
        }

        let current_zone = self.current_zone.as_ref().unwrap();
        let zone = auth[0].name.clone();
        if zone.is_subdomain(current_zone) && self.current_name.is_subdomain(&zone) {
            self.current_zone = Some(zone);
            return true;
        }
        return false;
    }
}

impl Future for RunningQuery {
    type Item = Message;
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match mem::replace(&mut self.state, State::Poisoned) {
                State::Init => match self.lookup_in_cache() {
                    None => {
                        if let Some(fut) = self
                            .recursor
                            .forwarder
                            .handle_query(&self.current_name, self.current_type)
                        {
                            self.state = State::Forward(fut);
                        } else {
                            match NameserverFuture::new(
                                self.current_zone.as_ref().unwrap().clone(),
                                &self.recursor,
                                &self.recursor.nsas,
                                self.depth,
                            ) {
                                Ok(fut) => {
                                    self.state = State::GetNameServer(fut);
                                }
                                Err(e) => {
                                    return Err(e);
                                }
                            }
                        }
                    }
                    Some(resp) => {
                        return Ok(Async::Ready(resp));
                    }
                },
                State::Forward(mut sender) => match sender.poll() {
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(Async::NotReady) => {
                        self.state = State::Forward(sender);
                        return Ok(Async::NotReady);
                    }
                    Ok(Async::Ready(resp)) => match self.handle_response(resp) {
                        Err(e) => {
                            return Err(e);
                        }
                        Ok(Some(resp)) => {
                            return Ok(Async::Ready(resp));
                        }
                        Ok(none) => {
                            self.state = State::Init;
                        }
                    },
                },
                State::GetNameServer(mut fetcher) => match fetcher.poll() {
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(Async::NotReady) => {
                        self.state = State::GetNameServer(fetcher);
                        return Ok(Async::NotReady);
                    }
                    Ok(Async::Ready(nameserver)) => {
                        self.state = State::QueryAuthServer(Sender::new(
                            Message::with_query(self.current_name.clone(), self.current_type),
                            nameserver,
                            self.recursor.nsas.clone(),
                        ));
                    }
                },
                State::QueryAuthServer(mut sender) => match sender.poll() {
                    Err(e) => {
                        return Err(VgError::TimerErr(format!("{:?}", e)).into());
                    }
                    Ok(Async::NotReady) => {
                        self.state = State::QueryAuthServer(sender);
                        return Ok(Async::NotReady);
                    }
                    Ok(Async::Ready(resp)) => match self.handle_response(resp) {
                        Err(e) => {
                            return Err(e);
                        }
                        Ok(Some(resp)) => {
                            return Ok(Async::Ready(resp));
                        }
                        Ok(none) => {
                            self.state = State::Init;
                        }
                    },
                },
                State::Poisoned => {
                    panic!("running query state is corrupted");
                }
            }
        }
    }
}
