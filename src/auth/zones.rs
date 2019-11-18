use super::error::AuthError;
use datasrc::{
    load_zone, zone::FindResult, zone::ZoneFinder, FindOption, FindResultFlag, FindResultType,
    MemoryZone, RBTree,
};
use failure::Result;
use r53::{HeaderFlag, Message, MessageBuilder, Name, RRType, Rcode};

pub struct AuthZone {
    zones: RBTree<MemoryZone>,
}

impl AuthZone {
    pub fn new() -> Self {
        AuthZone {
            zones: RBTree::new(),
        }
    }

    pub fn add_zone(&mut self, name: Name, zone_content: &str) -> Result<()> {
        if self.get_exact_zone(&name).is_some() {
            return Err(AuthError::DuplicateZone(name.to_string()).into());
        }

        let zone = load_zone(name.clone(), zone_content)?;
        self.zones.insert(name, Some(zone));
        Ok(())
    }

    pub fn delete_zone(&mut self, name: &Name) -> Result<()> {
        let result = self.zones.find(name);
        if result.flag != FindResultFlag::ExacatMatch {
            return Err(AuthError::UnknownZone(name.to_string()).into());
        }
        let target = result.node;
        self.zones.remove_node(target);
        Ok(())
    }

    pub fn handle_query(&self, req: &mut Message) -> bool {
        let question = req.question.as_ref().unwrap();
        let zone = self.get_zone(&question.name);
        if zone.is_none() {
            //let mut builder = MessageBuilder::new(req);
            //builder.make_response().rcode(Rcode::Refused).done();
            return false;
        }

        let zone = zone.unwrap();
        let mut result = zone.find(&question.name, question.typ, FindOption::FollowZoneCut);

        let query_type = question.typ;
        let mut builder = MessageBuilder::new(req);
        builder.make_response().set_flag(HeaderFlag::AuthAnswer);
        match result.typ {
            FindResultType::CName => {
                builder.add_answer(result.rrset.take().unwrap());
            }
            FindResultType::Success => {
                for rrset in result.get_additional() {
                    builder.add_additional(rrset);
                }
                builder.add_answer(result.rrset.take().unwrap());
                if query_type != RRType::NS {
                    let (auth, additional) = result.get_apex_ns_and_glue();
                    builder.add_auth(auth);
                    for rrset in additional {
                        builder.add_additional(rrset);
                    }
                }
            }
            FindResultType::Delegation => {
                for rrset in result.get_additional() {
                    builder.add_additional(rrset);
                }
                builder
                    .clear_flag(HeaderFlag::AuthAnswer)
                    .add_auth(result.rrset.take().unwrap());
            }
            FindResultType::NXDomain => {
                builder
                    .rcode(Rcode::NXDomain)
                    .add_auth(result.get_apex_soa());
            }
            FindResultType::NXRRset => {
                builder
                    .rcode(Rcode::NXRRset)
                    .add_auth(result.get_apex_soa());
            }
        }
        builder.done();
        true
    }

    pub fn get_zone<'a>(&'a self, name: &Name) -> Option<&'a MemoryZone> {
        let result = self.zones.find(&name);
        result.get_value()
    }

    pub fn get_exact_zone<'a>(&'a mut self, name: &Name) -> Option<&'a mut MemoryZone> {
        let result = self.zones.find(&name);
        if result.flag == FindResultFlag::ExacatMatch {
            result.get_value_mut()
        } else {
            None
        }
    }
}
