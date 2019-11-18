use crate::recursor::{
    message_classifier::{classify_response, ResponseCategory},
    nsas::{
        address_entry::AddressEntry, error::NSASError, nameserver_cache::NameserverEntry,
        zone_cache::ZoneEntry,
    },
};
use failure::Result;
use r53::{message::SectionType, Message, Name, RData, RRType, RRset};
use std::{net::IpAddr, time::Duration};

//the message could be an answer to the right zone
//or the refer which the zone doens't exists, but a parent zone exists
pub fn message_to_zone_entry(
    zone: &Name,
    mut msg: Message,
) -> Result<(ZoneEntry, Option<Vec<NameserverEntry>>)> {
    let category = classify_response(zone, RRType::NS, &msg);
    let answer = if category == ResponseCategory::Answer {
        msg.take_section(SectionType::Answer).unwrap()
    } else if category == ResponseCategory::Referral {
        msg.take_section(SectionType::Authority).unwrap()
    } else {
        return Err(
            NSASError::InvalidNSResponse("ns query doesn't return answer".to_string()).into(),
        );
    };

    let glue = msg.take_section(SectionType::Additional);
    let ns_count = answer[0].rdatas.len();
    let names = answer[0]
        .rdatas
        .iter()
        .fold(Vec::with_capacity(ns_count), |mut names, rdata| {
            if let RData::NS(ref ns) = rdata {
                names.push(ns.name.clone());
            }
            names
        });

    let zone = if category == ResponseCategory::Answer {
        zone.clone()
    } else {
        answer[0].name.clone()
    };

    let nameservers = match glue {
        None => None,
        Some(mut glue) => {
            let mut nameservers = Vec::with_capacity(names.len());
            for name in &names {
                let mut rrset_index = glue.len();
                for (i, rrset) in glue.iter().enumerate() {
                    //if rrset.name.eq(name) && (rrset.typ == RRType::A || rrset.typ == RRType::AAAA) {
                    if rrset.name.eq(name) && (rrset.typ == RRType::A) {
                        nameservers.push(NameserverEntry::new(
                            name.clone(),
                            rrset_to_address_entry(rrset),
                            Duration::new(rrset.ttl.0 as u64, 0),
                        ));
                        rrset_index = i;
                        break;
                    }
                }
                if rrset_index != glue.len() {
                    glue.remove(rrset_index);
                }
            }
            if nameservers.is_empty() {
                None
            } else {
                Some(nameservers)
            }
        }
    };

    if nameservers.is_none() && names.iter().all(|n| n.is_subdomain(&zone)) {
        return Err(NSASError::InvalidNSResponse(
            "subdomain ns has no related v4 glue".to_string(),
        )
        .into());
    }
    Ok((
        ZoneEntry::new(zone, names, Duration::new(answer[0].ttl.0 as u64, 0)),
        nameservers,
    ))
}

pub fn message_to_nameserver_entry(nameserver: Name, msg: Message) -> Result<NameserverEntry> {
    let category = classify_response(&nameserver, RRType::A, &msg);
    if category != ResponseCategory::Answer {
        return Err(NSASError::InvalidNSResponse(
            "address query doesn't return answer".to_string(),
        )
        .into());
    }
    let answer = msg.section(SectionType::Answer).unwrap();
    Ok(NameserverEntry::new(
        nameserver,
        rrset_to_address_entry(&answer[0]),
        Duration::new(answer[0].ttl.0 as u64, 0),
    ))
}

fn rrset_to_address_entry(rrset: &RRset) -> Vec<AddressEntry> {
    let rdata_count = rrset.rdatas.len();
    rrset
        .rdatas
        .iter()
        .fold(Vec::with_capacity(rdata_count), |mut entries, rdata| {
            match rdata {
                RData::A(ref a) => {
                    entries.push(AddressEntry::new(IpAddr::V4(a.host), 0));
                }
                RData::AAAA(ref aaaa) => {
                    entries.push(AddressEntry::new(IpAddr::V6(aaaa.host), 0));
                }
                _ => {
                    unreachable!();
                }
            }
            entries
        })
}

#[cfg(test)]
mod test {
    use super::*;
    use r53::{util::hex::from_hex, MessageBuilder};

    #[test]
    fn test_create_nameserver_from_message() {
        let raw = from_hex("cb7b85000001000d0000001b03636f6d0000020001c00c000200010002a3000014016c0c67746c642d73657276657273036e657400c00c000200010002a3000004016dc023c00c000200010002a30000040164c023c00c000200010002a30000040165c023c00c000200010002a3000004016ac023c00c000200010002a30000040167c023c00c000200010002a30000040166c023c00c000200010002a30000040162c023c00c000200010002a30000040161c023c00c000200010002a3000004016bc023c00c000200010002a30000040168c023c00c000200010002a30000040169c023c00c000200010002a30000040163c023c021000100010002a3000004c029a21ec021001c00010002a300001020010500d93700000000000000000030c041000100010002a3000004c037531ec041001c00010002a300001020010501b1f900000000000000000030c051000100010002a3000004c01f501ec051001c00010002a300001020010500856e00000000000000000030c061000100010002a3000004c00c5e1ec061001c00010002a3000010200105021ca100000000000000000030c071000100010002a3000004c0304f1ec071001c00010002a300001020010502709400000000000000000030c081000100010002a3000004c02a5d1ec081001c00010002a300001020010503eea300000000000000000030c091000100010002a3000004c023331ec091001c00010002a300001020010503d41400000000000000000030c0a1000100010002a3000004c0210e1ec0a1001c00010002a300001020010503231d00000000000000020030c0b1000100010002a3000004c005061ec0b1001c00010002a300001020010503a83e00000000000000020030c0c1000100010002a3000004c034b21ec0c1001c00010002a3000010200105030d2d00000000000000000030c0d1000100010002a3000004c036701ec0d1001c00010002a30000102001050208cc00000000000000000030c0e1000100010002a3000004c02bac1ec0e1001c00010002a30000102001050339c100000000000000000030c0f1000100010002a3000004c01a5c1ec0f1001c00010002a30000102001050383eb000000000000000000300000291000000000000000");
        let message = Message::from_wire(raw.unwrap().as_ref()).unwrap();
        let (_, nameservers) = message_to_zone_entry(&Name::new("com").unwrap(), message).unwrap();
        assert_eq!(nameservers.unwrap().len(), 13);

        let raw = from_hex("cb7b8500000100060000000106616d617a6f6e03636f6d0000020001c00c0002000100000e100014036e7333037033310664796e656374036e657400c00c0002000100000e100006036e7332c02cc00c0002000100000e100006036e7331c02cc00c0002000100000e1000110570646e733108756c747261646e73c037c00c0002000100000e1000160570646e733608756c747261646e7302636f02756b00c00c0002000100000e100006036e7334c02c0000291000000000000000");
        let message = Message::from_wire(raw.unwrap().as_ref()).unwrap();
        let (zone_entry, nameservers) =
            message_to_zone_entry(&Name::new("amazon.com").unwrap(), message).unwrap();
        assert!(nameservers.is_none());
        assert_eq!(zone_entry.get_server_names().len(), 6);

        let raw = from_hex("cb7b850000010004000000060668756177656903636f6d0000020001c00c0002000100000e10000b086e73616c6c736563c00cc00c0002000100000e10000b086e73616c6c347468c00cc00c0002000100000e10000b086e73616c6c337264c00cc00c0002000100000e100008056e73616c6cc00cc06d000100010000025800042df9d4e6c056000100010000025800047442b8c9c05600010001000002580004a8c35d29c03f000100010000025800047a606842c02800010001000002580004b9b04ce50000291000000000000000
");
        let message = Message::from_wire(raw.unwrap().as_ref()).unwrap();
        let (_, nameservers) =
            message_to_zone_entry(&Name::new("huawei.com").unwrap(), message).unwrap();
        assert_eq!(nameservers.unwrap().len(), 4);
    }
}
