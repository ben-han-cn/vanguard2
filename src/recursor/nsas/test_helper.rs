use crate::recursor::{nsas::error::NSASError, recursor::Resolver};
use failure;
use futures::{future, Future};
use r53::{
    HeaderFlag, Message, MessageBuilder, Name, Opcode, RData, RRClass, RRTtl, RRType, RRset, Rcode,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{Error, ErrorKind};

type FackResponse = (Vec<RData>, Vec<RRset>);

#[derive(Clone, Eq, PartialEq)]
struct Question {
    name: Name,
    typ: RRType,
}

impl Hash for Question {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        state.write_u16(self.typ.to_u16());
    }
}

#[derive(Clone)]
pub struct DumbResolver {
    responses: HashMap<Question, FackResponse>,
}

impl DumbResolver {
    pub fn new() -> Self {
        DumbResolver {
            responses: HashMap::new(),
        }
    }

    pub fn set_answer(
        &mut self,
        name: Name,
        typ: RRType,
        answer: Vec<RData>,
        additional: Vec<RRset>,
    ) {
        self.responses
            .insert(Question { name, typ }, (answer, additional));
    }
}

impl Resolver for DumbResolver {
    type Query = Box<Future<Item = Message, Error = failure::Error> + Send>;

    fn new_query(
        &self,
        mut query: Message,
        _depth: usize,
    ) -> Box<Future<Item = Message, Error = failure::Error> + Send> {
        let question = query.question.as_ref().unwrap();
        let name = question.name.clone();
        let typ = question.typ;
        match self.responses.get(&Question {
            name: name.clone(),
            typ,
        }) {
            None => {
                return Box::new(future::err(
                    NSASError::InvalidNSResponse("time out".to_string()).into(),
                ));
            }
            Some((ref answer, ref additional)) => {
                let mut builder = MessageBuilder::new(&mut query);
                builder
                    .make_response()
                    .opcode(Opcode::Query)
                    .rcode(Rcode::NoError)
                    .set_flag(HeaderFlag::QueryRespone)
                    .set_flag(HeaderFlag::AuthAnswer)
                    .set_flag(HeaderFlag::RecursionDesired);

                let rrset = RRset {
                    name: name,
                    typ: typ,
                    class: RRClass::IN,
                    ttl: RRTtl(2000),
                    rdatas: answer.clone(),
                };
                builder.add_answer(rrset);

                for rrset in additional {
                    builder.add_additional(rrset.clone());
                }
                builder.done();
                return Box::new(future::ok(query));
            }
        }
    }
}
