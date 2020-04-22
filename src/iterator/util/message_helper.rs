use anyhow::{self, bail};
use r53::{header_flag::HeaderFlag, opcode, Message, Name, RRType, Rcode, SectionType};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseCategory {
    Answer,
    CName,
    NXDomain,
    NXRRset,
    ServFail,
    Referral,
    FormErr,
}

pub fn sanitize_and_classify_response(
    zone: &Name,
    name: &Name,
    typ: RRType,
    resp: &mut Message,
) -> anyhow::Result<ResponseCategory> {
    if !resp.header.is_flag_set(HeaderFlag::QueryRespone) {
        bail!("not response message");
    }

    if resp.header.opcode != opcode::Opcode::Query {
        bail!("not a query message");
    }

    if resp.question.is_none() {
        bail!("short of question");
    }

    let question = resp.question.as_ref().unwrap();
    if !question.name.eq(name) || question.typ != typ {
        bail!("question doesn't match");
    }

    let is_auth_answer = resp.header.is_flag_set(HeaderFlag::AuthAnswer);
    let mut has_answer = false;
    let mut response_category = match resp.header.rcode {
        Rcode::NoError => ResponseCategory::NXRRset,
        Rcode::ServFail => ResponseCategory::ServFail,
        Rcode::NXDomain => ResponseCategory::NXDomain,
        _ => ResponseCategory::FormErr,
    };

    let rcode = resp.header.rcode;
    if let Some(rrsets) = resp.section_mut(SectionType::Answer) {
        if rcode != Rcode::NoError {
            bail!("only noerror rcode should have answer");
        }
        has_answer = true;

        if rrsets.len() != 1 {
            if rrsets[0].typ != RRType::CNAME {
                bail!("only cname may contain a chain which may has more than one rrset");
            } else {
                //only keep the first level name
                rrsets.truncate(1);
            }
        } else {
            if rrsets[0].typ != typ && rrsets[0].typ != RRType::CNAME {
                bail!("answer doesn't match query");
            }
        }

        if typ != RRType::CNAME && rrsets[0].typ == RRType::CNAME {
            response_category = ResponseCategory::CName;
        } else {
            response_category = ResponseCategory::Answer;
        }
    }

    let mut clean_auth = false;
    if let Some(rrsets) = resp.section_mut(SectionType::Authority) {
        rrsets.retain(|rrset| rrset.name.is_subdomain(zone));
        if rrsets.is_empty() {
            clean_auth = true;
        }
    }
    if clean_auth {
        resp.take_section(SectionType::Authority);
    }

    if let Some(rrsets) = resp.section_mut(SectionType::Authority) {
        if rrsets.len() > 1 {
            bail!("auth section should has more than one rrset");
        } else {
            match rcode {
                Rcode::NXDomain => {
                    //soa name should equal to zone name
                    //but with forwarder, this may not true
                    if rrsets[0].typ != RRType::SOA {
                        bail!("nxdomain response has no valid soa");
                    }
                }
                Rcode::NoError => {
                    if has_answer {
                        if is_auth_answer && rrsets[0].typ != RRType::NS {
                            bail!("auth positive answer has no ns");
                        }
                    } else {
                        if rrsets[0].typ == RRType::NS {
                            response_category = ResponseCategory::Referral;
                        }
                    }
                }
                _ => {}
            }
        }
    } else {
        //positive answer should have ns
        //nxdomain answer should have soa
    }

    let mut clean_additional = false;
    if let Some(rrsets) = resp.section_mut(SectionType::Additional) {
        rrsets.retain(|rrset| rrset.name.is_subdomain(zone));
        if rrsets.is_empty() {
            clean_additional = true;
        } else {
            for rrset in rrsets {
                if rrset.typ != RRType::A && rrset.typ != RRType::AAAA {
                    bail!("additional section has {} which isn't a or aaaa", rrset.typ);
                }
            }
        }
    }
    if clean_additional {
        resp.take_section(SectionType::Additional);
    }

    resp.recalculate_header();
    Ok(response_category)
}

#[cfg(test)]
mod test {
    use super::*;
    use r53::util::hex::from_hex;

    struct TestCase {
        raw: &'static str,
        zone: Name,
        qname: Name,
        qtype: RRType,
        category: ResponseCategory,
    }

    #[test]
    fn test_sanitize_and_classify_response() {
        for case in vec![TestCase {
            //root auth return baidu.com query
            raw: "cb7b830000010000000d000b05626169647503636f6d0000010001c012000200010002a3000014016c0c67746c642d73657276657273036e657400c012000200010002a30000040162c029c012000200010002a30000040163c029c012000200010002a30000040164c029c012000200010002a30000040165c029c012000200010002a30000040166c029c012000200010002a30000040167c029c012000200010002a30000040161c029c012000200010002a30000040168c029c012000200010002a30000040169c029c012000200010002a3000004016ac029c012000200010002a3000004016bc029c012000200010002a3000004016dc029c027000100010002a3000004c029a21ec027001c00010002a300001020010500d93700000000000000000030c047000100010002a3000004c0210e1ec047001c00010002a300001020010503231d00000000000000020030c057000100010002a3000004c01a5c1ec057001c00010002a30000102001050383eb00000000000000000030c067000100010002a3000004c01f501ec067001c00010002a300001020010500856e00000000000000000030c077000100010002a3000004c00c5e1ec077001c00010002a3000010200105021ca100000000000000000030c087000100010002a3000004c023331e",
            zone: Name::new(".").unwrap(),
            qname: Name::new("baidu.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::Referral,
        },
        TestCase {
            //including cname chain and final answer
            raw: "cb7b818000010004000000000377777705626169647503636f6d0000010001c00c00050001000000d2000f0377777701610673686966656ec016c02b0005000100000043000e03777777077773686966656ec016c04600010001000000df000468c1584dc04600010001000000df000468c1587b",
            zone: Name::new("baidu.com").unwrap(),
            qname: Name::new("www.baidu.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::CName,
        },
        TestCase {
            //baidu.com return one cname without the final answer
            raw: "cb7b850000010001000500050377777705626169647503636f6d0000010001c00c00050001000004b0000f0377777701610673686966656ec016c02f00020001000004b00006036e7332c02fc02f00020001000004b00006036e7334c02fc02f00020001000004b00006036e7335c02fc02f00020001000004b00006036e7333c02fc02f00020001000004b00006036e7331c02fc08e00010001000004b000043d87a5e0c04600010001000004b00004dcb52120c07c00010001000004b000047050fffdc05800010001000004b000040ed7b1e5c06a00010001000004b00004b44c4c5f",
            zone: Name::new("baidu.com").unwrap(),
            qname: Name::new("www.baidu.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::CName,
        },
        TestCase {
            raw: "cb7b818000010001000000000377777706676f6f676c6503636f6d0000010001c00c000100010000012b0004acd9a064",
            zone: Name::new("google.com").unwrap(),
            qname: Name::new("www.google.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::Answer,
        },
        ]
        {
            let raw = from_hex(case.raw);
            let mut message = Message::from_wire(raw.unwrap().as_ref()).unwrap();
            assert_eq!(sanitize_and_classify_response(&case.zone, &case.qname, case.qtype, &mut message).unwrap(), case.category,);
        }
    }
}
