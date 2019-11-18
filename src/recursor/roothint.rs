use super::{MessageCache, RRsetTrustLevel};
use r53::{Message, Name, RData, RRType, RRset};
use std::str::FromStr;

const ROOT_SERVERS: [&'static str; 13] = [
    ".			518400	IN	NS	b.root-servers.net.",
    ".			518400	IN	NS	m.root-servers.net.",
    ".			518400	IN	NS	i.root-servers.net.",
    ".			518400	IN	NS	g.root-servers.net.",
    ".			518400	IN	NS	d.root-servers.net.",
    ".			518400	IN	NS	a.root-servers.net.",
    ".			518400	IN	NS	h.root-servers.net.",
    ".			518400	IN	NS	k.root-servers.net.",
    ".			518400	IN	NS	l.root-servers.net.",
    ".			518400	IN	NS	f.root-servers.net.",
    ".			518400	IN	NS	e.root-servers.net.",
    ".			518400	IN	NS	j.root-servers.net.",
    ".			518400	IN	NS	c.root-servers.net.",
];

const ROOT_GLUES: [&'static str; 13] = [
    "a.root-servers.net.	3600000	IN	A	198.41.0.4",
    "b.root-servers.net.	3600000	IN	A	199.9.14.201",
    "c.root-servers.net.	3600000	IN	A	192.33.4.12",
    "d.root-servers.net.	3600000	IN	A	199.7.91.13",
    "e.root-servers.net.	3600000	IN	A	192.203.230.10",
    "f.root-servers.net.	3600000	IN	A	192.5.5.241",
    "g.root-servers.net.	3600000	IN	A	192.112.36.4",
    "h.root-servers.net.	3600000	IN	A	198.97.190.53",
    "i.root-servers.net.	3600000	IN	A	192.36.148.17",
    "j.root-servers.net.	3600000	IN	A	192.58.128.30",
    "k.root-servers.net.	3600000	IN	A	193.0.14.129",
    "l.root-servers.net.	3600000	IN	A	199.7.83.42",
    "m.root-servers.net.	3600000	IN	A	202.12.27.33",
];

#[derive(Debug, Clone)]
pub struct RootHint {
    root_ns: RRset,
    root_glues: Vec<RRset>,
}

impl RootHint {
    pub fn new() -> RootHint {
        let mut ns_records: Vec<RRset> = ROOT_SERVERS
            .iter()
            .map(|ns| RRset::from_str(ns).unwrap())
            .collect();
        let root_ns = {
            let rrset = ns_records.pop().unwrap();
            ns_records.into_iter().fold(rrset, |mut rrset, mut other| {
                rrset.rdatas.push(other.rdatas.pop().unwrap());
                rrset
            })
        };

        let root_glues = ROOT_GLUES
            .iter()
            .fold(Vec::new(), |mut rrsets: Vec<RRset>, glue| {
                rrsets.push(RRset::from_str(glue).unwrap());
                rrsets
            });
        RootHint {
            root_ns,
            root_glues,
        }
    }

    pub fn fill_cache(&self, cache: &mut MessageCache) {
        cache.add_rrset(self.root_ns.clone(), RRsetTrustLevel::AuthorityWithoutAA);
        for glue in self.root_glues.iter() {
            cache.add_rrset(glue.clone(), RRsetTrustLevel::AdditionalWithoutAA);
        }
    }
}
