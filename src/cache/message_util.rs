use super::cache::RRsetTrustLevel;
use r53::{header_flag, message::SectionType, Message, RRType, Rcode};

pub(crate) fn get_rrset_trust_level(message: &Message, section: SectionType) -> RRsetTrustLevel {
    let aa = header_flag::is_flag_set(message.header.flag, header_flag::HeaderFlag::AuthAnswer);
    match section {
        SectionType::Answer => {
            if aa {
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
