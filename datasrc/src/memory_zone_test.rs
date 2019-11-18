use crate::memory_zone::MemoryZone;
use crate::zone::{FindOption, FindResult, FindResultType, ZoneFinder, ZoneUpdater};
use r53::{Name, RRType, RRset};
use std::str::FromStr;

fn default_zone() -> Vec<&'static str> {
    vec![
        "example.org. 300 IN SOA xxx.net. ns.example.org. 100 1800 900 604800 86400",
        "example.org. 300 IN NS ns.example.org.",
        "example.org. 300 IN A 192.0.2.1",
        "ns.example.org. 300 IN A 192.0.2.2",
        "ns.example.org. 300 IN AAAA 2001:db8::2",
        "cname.example.org. 300 IN CNAME canonical.example.org",
        "dname.example.org. 300 IN NS ns.dname.example.org.",
        "child.example.org. 300 IN NS ns.child.example.org.",
        "ns.child.example.org. 300 IN A 192.0.2.153",
        "grand.child.example.org. 300 IN NS ns.grand.child.example.org.",
        "ns.grand.child.example.org. 300 IN AAAA 2001:db8::253",
        "foo.wild.example.org. 300 IN A 192.0.2.3",
        "wild.*.foo.example.org. 300 IN A 192.0.2.1",
        "wild.*.foo.*.bar.example.org. 300 IN A 192.0.2.1",
        "bar.foo.wild.example.org. 300 IN A 192.0.2.2",
        "baz.foo.wild.example.org. 300 IN A 192.0.2.3",
    ]
}

fn build_zone(name: &str, rrset_strs: Vec<&'static str>) -> MemoryZone {
    let mut zone = MemoryZone::new(Name::new(name).unwrap());
    for rrset_str in rrset_strs {
        let rrset = RRset::from_str(rrset_str).unwrap();
        zone.add_rrset(rrset).unwrap();
    }
    zone
}

#[test]
fn test_find_cname() {
    let mut zone = build_zone("example.org", default_zone());
    let mut result = zone.find(
        &Name::new("cname.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::CName);
    assert_eq!(result.rrset.take().unwrap().typ, RRType::CNAME);

    let mut result = zone.find(
        &Name::new("cname.example.org.").unwrap(),
        RRType::CNAME,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "canonical.example.org."
    );

    zone.add_rrset(RRset::from_str("cname.child.example.org. 300 IN CNAME www.knet.cn").unwrap())
        .unwrap();
    let mut result = zone.find(
        &Name::new("cname.child.example.org.").unwrap(),
        RRType::AAAA,
        FindOption::GlueOK,
    );
    assert_eq!(result.typ, FindResultType::CName);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "www.knet.cn."
    );
}

#[test]
fn test_delegation() {
    let zone = build_zone("example.org", default_zone());

    let mut result = zone.find(
        &Name::new("www.child.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Delegation);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.child.example.org."
    );

    let mut result = zone.find(
        &Name::new("child.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Delegation);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.child.example.org."
    );

    let mut result = zone.find(
        &Name::new("child.example.org.").unwrap(),
        RRType::NS,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Delegation);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.child.example.org."
    );

    let mut result = zone.find(
        &Name::new("example.org.").unwrap(),
        RRType::NS,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.example.org."
    );

    let mut result = zone.find(
        &Name::new("www.grand.child.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Delegation);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.child.example.org."
    );
}

#[test]
fn test_general_find() {
    let zone = build_zone("example.org", default_zone());

    let mut result = zone.find(
        &Name::new("example.org.").unwrap(),
        RRType::NS,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.example.org."
    );

    let mut result = zone.find(
        &Name::new("ns.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "192.0.2.2"
    );

    let result = zone.find(
        &Name::new("example.org.").unwrap(),
        RRType::AAAA,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXRRset);

    let result = zone.find(
        &Name::new("ns.example.org.").unwrap(),
        RRType::NS,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXRRset);

    let result = zone.find(
        &Name::new("nothere.example.org").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXDomain);

    let result = zone.find(
        &Name::new("example.net").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXDomain);
}

#[test]
fn test_find_glue() {
    let zone = build_zone("example.org", default_zone());

    let mut result = zone.find(
        &Name::new("ns.child.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Delegation);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.child.example.org."
    );

    let mut result = zone.find(
        &Name::new("ns.child.example.org.").unwrap(),
        RRType::A,
        FindOption::GlueOK,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "192.0.2.153"
    );

    let result = zone.find(
        &Name::new("ns.child.example.org.").unwrap(),
        RRType::AAAA,
        FindOption::GlueOK,
    );
    assert_eq!(result.typ, FindResultType::NXRRset);

    let mut result = zone.find(
        &Name::new("www.child.example.org.").unwrap(),
        RRType::A,
        FindOption::GlueOK,
    );
    assert_eq!(result.typ, FindResultType::Delegation);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.child.example.org."
    );

    let mut result = zone.find(
        &Name::new("ns.grand.child.example.org").unwrap(),
        RRType::AAAA,
        FindOption::GlueOK,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "2001:db8::253"
    );

    let mut result = zone.find(
        &Name::new("www.grand.child.example.org.").unwrap(),
        RRType::TXT,
        FindOption::GlueOK,
    );
    assert_eq!(result.typ, FindResultType::Delegation);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "ns.child.example.org."
    );
}

#[test]
fn test_wildcard_find() {
    let rrset_strs = vec![
        "example. 300 IN SOA xxx.net. ns.example. 100 1800 900 604800 86400",
        "*.example.              3600     IN AAAA   ::1",
        "*.example.               3600     IN MX    10 host1.example.",
        "*.c.example.               3600     IN A 2.2.2.2",
        "sub.*.example.           3600     IN AAAA ::2",
        "host1.example.           3600     IN A     192.0.2.1",
        "_ssh._tcp.host1.example. 3600     IN SRV 10 60 5060 bigbox.example.com.",
        "_ssh._tcp.host2.example. 3600     IN SRV 10 60 5060 b.c.example.",
        "subdel.example.          3600     IN NS    ns.example.com.",
        "subdel.example.          3600     IN NS    ns.example.net.",
    ];
    let zone = build_zone("example", rrset_strs);

    let result = zone.find(
        &Name::new("host3.example.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXRRset);

    let mut result = zone.find(
        &Name::new("host3.example").unwrap(),
        RRType::MX,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "10 host1.example."
    );

    let mut result = zone.find(
        &Name::new("foo.bar.example").unwrap(),
        RRType::AAAA,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(result.rrset.take().unwrap().rdatas[0].to_string(), "::1");

    let result = zone.find(
        &Name::new("ghost.*.example").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXDomain);

    let mut result = zone.find(
        &Name::new("b.c.example").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    assert_eq!(
        result.rrset.take().unwrap().rdatas[0].to_string(),
        "2.2.2.2"
    );
}

#[test]
fn test_srv_find_glue() {
    let rrset_strs = vec![
        "example.org. 300 IN SOA xxx.net. ns.example.org. 100 1800 900 604800 86400",
        "example.org. 300 IN NS ns.example.org.",
        "ns.example.org. 300 IN A 192.0.2.2",
        "_service._proto.example.org. 100 IN SRV 1 2 3 glue.example.org.",
        "glue.example.org. 100 IN A 1.1.1.4",
    ];
    let zone = build_zone("example.org", rrset_strs);

    let result = zone.find(
        &Name::new("_service._proto.example.org.").unwrap(),
        RRType::SRV,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::Success);
    let glue = result.get_additional();
    assert_eq!(glue.len(), 1);
    assert_eq!(glue[0].rdatas[0].to_string(), "1.1.1.4");
}

#[test]
fn test_empty_node() {
    let rrset_strs = vec![
        /*
           example.org
                |
               baz (empty; easy case)
             /  |  \
           bar  |  x.foo ('foo' part is empty; a bit trickier)
               bbb
              /
            aaa
        */
        "example.org. 300 IN A 192.0.2.1",
        "bar.example.org. 300 IN A 192.0.2.1",
        "x.foo.example.org. 300 IN A 192.0.2.1",
        "aaa.baz.example.org. 300 IN A 192.0.2.1",
        "bbb.baz.example.org. 300 IN A 192.0.2.1",
    ];
    let zone = build_zone("example.org", rrset_strs);

    let result = zone.find(
        &Name::new("baz.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXRRset);

    let result = zone.find(
        &Name::new("foo.example.org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXRRset);

    let result = zone.find(
        &Name::new("org.").unwrap(),
        RRType::A,
        FindOption::FollowZoneCut,
    );
    assert_eq!(result.typ, FindResultType::NXDomain);
}
