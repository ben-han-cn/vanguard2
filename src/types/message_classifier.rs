use r53::{
    header_flag::HeaderFlag, message::SectionType, opcode, Message, Name, RData, RRType, Rcode,
};

#[derive(Debug, Eq, PartialEq)]
pub enum ResponseCategory {
    Answer,
    AnswerCName, //has final answer, but with cname chain
    CName(Name),
    NXDomain,
    NXRRset,
    Referral,
    FormErr,
    Invalid(String),
}

pub fn classify_response(name: &Name, typ: RRType, msg: &Message) -> ResponseCategory {
    if !msg.header.is_flag_set(HeaderFlag::QueryRespone) {
        return ResponseCategory::Invalid("not response message".to_string());
    }

    if msg.header.opcode != opcode::Opcode::Query {
        return ResponseCategory::Invalid("not a query message".to_string());
    }

    if msg.question.is_none() {
        return ResponseCategory::Invalid("short of question".to_string());
    }

    let question = msg.question.as_ref().unwrap();
    if !question.name.eq(name) || question.typ != typ {
        return ResponseCategory::Invalid("question doesn't match".to_string());
    }

    let rcode = msg.header.rcode;
    if rcode != Rcode::NoError {
        match rcode {
            Rcode::NXDomain => {
                return ResponseCategory::NXDomain;
            }
            Rcode::FormErr => {
                return ResponseCategory::FormErr;
            }
            _ => {
                return ResponseCategory::Invalid("invalid rcode".to_string());
            }
        }
    }

    let answer = msg.section(SectionType::Answer);
    let authority = msg.section(SectionType::Authority);
    if answer.is_none() {
        if !authority.is_none() {
            for rrset in authority.unwrap() {
                if rrset.typ == RRType::NS {
                    return ResponseCategory::Referral;
                }
            }
        }
        return ResponseCategory::NXRRset;
    }

    let answer = answer.unwrap();
    if answer.len() == 1 {
        if !answer[0].name.eq(name) {
            return ResponseCategory::Invalid("answer name doesn't match question".to_string());
        }

        if answer[0].typ == typ {
            return ResponseCategory::Answer;
        } else if answer[0].typ == RRType::CNAME {
            if answer[0].rdatas.len() != 1 {
                return ResponseCategory::Invalid("cname doesn't have one rdata".to_string());
            }
            return ResponseCategory::CName(get_cname_target(&answer[0].rdatas[0]).clone());
        } else {
            return ResponseCategory::Invalid("answer type doesn't match question".to_string());
        }
    }

    //check cname chain
    let mut last_name = name;
    let answer_count = answer.len();
    for (i, rrset) in answer.iter().enumerate() {
        if !rrset.name.eq(last_name) {
            return ResponseCategory::Invalid("cname doesn't form a chain".to_string());
        }

        if i != answer_count - 1 {
            if rrset.typ != RRType::CNAME {
                return ResponseCategory::Invalid("cname chain is broken".to_string());
            }
            if rrset.rdatas.len() != 1 {
                return ResponseCategory::Invalid("cname doesn't have one rdata".to_string());
            }
            last_name = get_cname_target(&rrset.rdatas[0]);
        } else {
            if rrset.typ == RRType::CNAME {
                if rrset.rdatas.len() != 1 {
                    return ResponseCategory::Invalid("cname doesn't have one rdata".to_string());
                }
                return ResponseCategory::CName(get_cname_target(&rrset.rdatas[0]).clone());
            } else if rrset.typ != typ {
                return ResponseCategory::Invalid("answer type doesn't match question".to_string());
            } else {
                return ResponseCategory::AnswerCName;
            }
        }
    }

    unreachable!()
}

fn get_cname_target(rdata: &RData) -> &Name {
    match rdata {
        RData::CName(ref cname) => &cname.name,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use r53::{util::hex::from_hex, MessageBuilder};

    struct TestCase {
        raw: &'static str,
        qname: Name,
        qtype: RRType,
        category: ResponseCategory,
    }

    #[test]
    fn test_classify_message() {
        for case in vec![TestCase {
            //root auth return baidu.com query
            raw: "cb7b830000010000000d000b05626169647503636f6d0000010001c012000200010002a3000014016c0c67746c642d73657276657273036e657400c012000200010002a30000040162c029c012000200010002a30000040163c029c012000200010002a30000040164c029c012000200010002a30000040165c029c012000200010002a30000040166c029c012000200010002a30000040167c029c012000200010002a30000040161c029c012000200010002a30000040168c029c012000200010002a30000040169c029c012000200010002a3000004016ac029c012000200010002a3000004016bc029c012000200010002a3000004016dc029c027000100010002a3000004c029a21ec027001c00010002a300001020010500d93700000000000000000030c047000100010002a3000004c0210e1ec047001c00010002a300001020010503231d00000000000000020030c057000100010002a3000004c01a5c1ec057001c00010002a30000102001050383eb00000000000000000030c067000100010002a3000004c01f501ec067001c00010002a300001020010500856e00000000000000000030c077000100010002a3000004c00c5e1ec077001c00010002a3000010200105021ca100000000000000000030c087000100010002a3000004c023331e",
            qname: Name::new("baidu.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::Referral,
        },
        TestCase {
            //including cname chain and final answer
            raw: "cb7b818000010004000000000377777705626169647503636f6d0000010001c00c00050001000000d2000f0377777701610673686966656ec016c02b0005000100000043000e03777777077773686966656ec016c04600010001000000df000468c1584dc04600010001000000df000468c1587b",
            qname: Name::new("www.baidu.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::AnswerCName,
        },
        TestCase {
            //baidu.com return one cname without the final answer
            raw: "cb7b850000010001000500050377777705626169647503636f6d0000010001c00c00050001000004b0000f0377777701610673686966656ec016c02f00020001000004b00006036e7332c02fc02f00020001000004b00006036e7334c02fc02f00020001000004b00006036e7335c02fc02f00020001000004b00006036e7333c02fc02f00020001000004b00006036e7331c02fc08e00010001000004b000043d87a5e0c04600010001000004b00004dcb52120c07c00010001000004b000047050fffdc05800010001000004b000040ed7b1e5c06a00010001000004b00004b44c4c5f",
            qname: Name::new("www.baidu.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::CName(Name::new("www.a.shifen.com.").unwrap()),
        },
        TestCase {
            raw: "cb7b818000010001000000000377777706676f6f676c6503636f6d0000010001c00c000100010000012b0004acd9a064",
            qname: Name::new("www.google.com.").unwrap(),
            qtype: RRType::A,
            category: ResponseCategory::Answer,
        },
        ]
        {
            let raw = from_hex(case.raw);
            let message = Message::from_wire(raw.unwrap().as_ref()).unwrap();
            assert_eq!(classify_response(&case.qname, case.qtype, &message), case.category,);
        }
    }
}
