use super::recursor::Recursor;
use crate::nameserver::send_query;
use crate::types::{classify_response, ResponseCategory};
use anyhow::{self, bail};
use r53::{message::SectionType, name, Message, MessageBuilder, Name, RRType, Rcode};
use std::time::Duration;
use tokio::time::timeout;

const MAX_CNAME_DEPTH: usize = 12;
const MAX_QUERY_DEPTH: usize = 10;
const RECURSOR_TIMEOUT: Duration = Duration::from_secs(10);

pub struct RunningQuery {
    current_name: Name,
    current_type: RRType,
    current_zone: Option<Name>,
    cname_depth: usize,
    response: Option<Message>,
    recursor: Recursor,
    depth: usize,
}

impl RunningQuery {
    pub fn new(request: &Message, recursor: Recursor, depth: usize) -> Self {
        let question = request.question.as_ref().unwrap();
        let current_name = question.name.clone();
        let current_type = question.typ;

        RunningQuery {
            current_name,
            current_type,
            current_zone: None,
            cname_depth: 0,
            response: Some(request.clone()),
            recursor,
            depth,
        }
    }

    fn lookup_in_cache(&mut self) -> Option<Message> {
        let current_query = Message::with_query(self.current_name.clone(), self.current_type);

        let cache = self.recursor.cache.clone();
        let mut cache = cache.lock().unwrap();
        if let Some(response) = cache.gen_response(&current_query) {
            let response = self.make_response(response);
            let origin_query_name = &response.question.as_ref().unwrap().name;
            if !origin_query_name.eq(&self.current_name) {
                cache.add_response(response.clone());
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

    pub fn handle_response(&mut self, response: Message) -> anyhow::Result<Option<Message>> {
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
                    .add_response(response.clone());
                return Ok(Some(response));
            }
            ResponseCategory::Referral => {
                self.recursor
                    .cache
                    .lock()
                    .unwrap()
                    .add_response(response.clone());
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

    pub async fn handle_query(self) -> anyhow::Result<Message> {
        match timeout(RECURSOR_TIMEOUT, self.do_recursive_query()).await {
            Err(e) => bail!(e),
            Ok(result) => result,
        }
    }

    async fn do_recursive_query(mut self) -> anyhow::Result<Message> {
        loop {
            if let Some(response) = self.lookup_in_cache() {
                return Ok(response);
            }

            let nameserver = {
                let (nameserver, missing_nameserver) = self
                    .recursor
                    .nsas
                    .get_nameserver(&self.current_zone.as_ref().unwrap());
                if missing_nameserver.is_some() {
                    let nsas = self.recursor.nsas.clone();
                    let resolver = self.recursor.clone();
                    tokio::spawn(
                        nsas.probe_missing_nameserver(missing_nameserver.unwrap(), resolver),
                    );
                }
                if nameserver.is_some() {
                    nameserver.unwrap()
                } else {
                    if self.depth + 1 > MAX_QUERY_DEPTH {
                        bail!("query depth is too big");
                    }
                    self.recursor
                        .nsas
                        .fetch_nameserver(
                            self.current_zone.as_ref().unwrap().clone(),
                            self.recursor.clone(),
                            self.depth + 1,
                        )
                        .await?
                }
            };

            if let Ok(response) = send_query(
                &Message::with_query(self.current_name.clone(), self.current_type),
                nameserver,
                self.recursor.nsas.clone(),
            )
            .await
            {
                if let Ok(Some(final_response)) = self.handle_response(response) {
                    return Ok(final_response);
                }
            }
        }
    }
}
