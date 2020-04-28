use super::cache::RRsetTrustLevel;
use r53::{header_flag, Message, RRType, RRset, SectionType};

//TODO: for cname rrset in answer section, but the name of the rrst isn't equtl to qname, the trust
//level of it should be AnswerWithoutAA
pub(crate) fn get_rrset_trust_level(
    rrset: &RRset,
    message: &Message,
    section: SectionType,
) -> RRsetTrustLevel {
    let aa = header_flag::is_flag_set(message.header.flag, header_flag::HeaderFlag::AuthAnswer);
    match section {
        SectionType::Answer => {
            if aa {
                if rrset.typ == RRType::CNAME
                    && rrset.name != message.question.as_ref().unwrap().name
                {
                    return RRsetTrustLevel::AnswerWithoutAA;
                }
                return RRsetTrustLevel::AnswerWithAA;
            } else {
                return RRsetTrustLevel::AnswerWithoutAA;
            }
        }
        SectionType::Authority => {
            if aa {
                return RRsetTrustLevel::AuthorityWithAA;
            } else {
                return RRsetTrustLevel::AuthorityWithoutAA;
            }
        }
        SectionType::Additional => {
            if aa {
                return RRsetTrustLevel::AdditionalWithAA;
            } else {
                return RRsetTrustLevel::AdditionalWithoutAA;
            }
        }
    }
}
