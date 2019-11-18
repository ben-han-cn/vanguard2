use crate::error::DataSrcError;
use failure::Result;
use r53::{Name, RData, RRClass, RRTtl, RRType, RRset};
use std::mem::swap;

type RRsetTuple = (RRType, RRTtl, Vec<RData>);

pub struct Rdataset {
    rrsets: Vec<RRsetTuple>,
}

impl Rdataset {
    pub fn new() -> Self {
        Rdataset { rrsets: Vec::new() }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.rrsets.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rrsets.is_empty()
    }

    pub fn add_rrset(&mut self, rrset: RRset) -> Result<()> {
        self.validate_rrset(&rrset)?;

        if let Some(index) = self.get_rrset_tuple(rrset.typ) {
            self.merge_rrset(index, rrset);
        } else {
            if rrset.typ == RRType::CNAME && !self.rrsets.is_empty() {
                return Err(DataSrcError::CNameCoExistsWithOtherRR.into());
            }
            if rrset.typ != RRType::CNAME && self.get_rrset_tuple(RRType::CNAME).is_some() {
                return Err(DataSrcError::CNameCoExistsWithOtherRR.into());
            }
            self.rrsets.push((rrset.typ, rrset.ttl, rrset.rdatas));
        }
        Ok(())
    }

    pub fn validate_rrset(&self, rrset: &RRset) -> Result<()> {
        if rrset.rdatas.len() == 0 {
            Err(DataSrcError::RRsetHasNoRdata.into())
        } else if (rrset.typ == RRType::CNAME || rrset.typ == RRType::SOA)
            && rrset.rdatas.len() != 1
        {
            Err(DataSrcError::ExclusiveRRsetHasMoreThanOneRdata.into())
        } else {
            Ok(())
        }
    }

    fn merge_rrset(&mut self, index: usize, mut rrset: RRset) {
        if rrset.typ == RRType::CNAME || rrset.typ == RRType::SOA {
            self.rrsets[index].1 = rrset.ttl;
            swap(&mut self.rrsets[index].2, &mut rrset.rdatas);
        } else {
            self.rrsets[index].1 = rrset.ttl;
            //todo: add duplicate check
            self.rrsets[index].2.append(&mut rrset.rdatas);
        }
    }

    pub fn get_rrset(&self, name: &Name, typ: RRType) -> Option<RRset> {
        self.get_rrset_tuple(typ).map(|index| RRset {
            name: name.clone(),
            typ,
            class: RRClass::IN,
            ttl: self.rrsets[index].1,
            rdatas: self.rrsets[index].2.clone(),
        })
    }

    pub fn delete_rrset(&mut self, typ: RRType) -> Result<()> {
        if let Some(index) = self.get_rrset_tuple(typ) {
            self.rrsets.remove(index);
            Ok(())
        } else {
            Err(DataSrcError::RRsetNotFound(typ.to_string()).into())
        }
    }

    pub fn delete_rdata(&mut self, rrset: &RRset) -> Result<()> {
        if let Some(index) = self.get_rrset_tuple(rrset.typ) {
            for rdata in &rrset.rdatas {
                if let Some(index_) = self.rrsets[index]
                    .2
                    .iter()
                    .position(|current| rdata.eq(current))
                {
                    self.rrsets[index].2.remove(index_);
                } else {
                    return Err(DataSrcError::RdataNotFound(rdata.to_string()).into());
                }
            }

            if self.rrsets[index].2.is_empty() {
                self.rrsets.remove(index);
            }
            Ok(())
        } else {
            Err(DataSrcError::RRsetNotFound(rrset.typ.to_string()).into())
        }
    }

    pub fn update_rdata(&mut self, old_rrset: &RRset, mut new_rrset: RRset) -> Result<()> {
        if let Some(index) = self.get_rrset_tuple(old_rrset.typ) {
            for (pos, rdata) in old_rrset.rdatas.iter().enumerate() {
                if let Some(index_) = self.rrsets[index]
                    .2
                    .iter()
                    .position(|current| rdata.eq(current))
                {
                    swap(
                        &mut self.rrsets[index].2[index_],
                        &mut new_rrset.rdatas[pos],
                    );
                } else {
                    return Err(DataSrcError::RdataNotFound(rdata.to_string()).into());
                }
            }
            Ok(())
        } else {
            Err(DataSrcError::RRsetNotFound(old_rrset.typ.to_string()).into())
        }
    }

    fn get_rrset_tuple(&self, typ: RRType) -> Option<usize> {
        self.rrsets.iter().position(|rrset| rrset.0 == typ)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn build_a_rrset(name: &str, ips: &[&str]) -> RRset {
        ips.iter()
            .map(|ip| format!("{} 3600 IN A {}", name, ip))
            .fold(None, |rrset: Option<RRset>, s| {
                let mut new = RRset::from_str(s.as_str()).unwrap();
                if let Some(mut old) = rrset {
                    old.rdatas.append(&mut new.rdatas);
                    Some(old)
                } else {
                    Some(new)
                }
            })
            .unwrap()
    }

    #[test]
    fn test_get_rrset() {
        let a_rrset = build_a_rrset("a.cn", &["1.1.1.1", "2.2.2.2"]);
        let mut rrset = Rdataset::new();
        rrset.add_rrset(a_rrset.clone()).unwrap();
        assert_eq!(
            rrset.get_rrset(&Name::new("a.cn").unwrap(), RRType::A),
            Some(a_rrset)
        );
    }

    #[test]
    fn test_delete_rrset() {
        let name = Name::new("a.cn").unwrap();
        let mut rrset = Rdataset::new();
        rrset
            .add_rrset(build_a_rrset("a.cn", &["1.1.1.1", "2.2.2.2"]))
            .unwrap();
        assert_eq!(rrset.len(), 1);
        rrset
            .delete_rdata(&build_a_rrset("a.cn", &["1.1.1.1"]))
            .unwrap();
        assert_eq!(rrset.len(), 1);
        assert_eq!(
            rrset.get_rrset(&name, RRType::A),
            Some(build_a_rrset("a.cn", &["2.2.2.2"]))
        );
        rrset
            .delete_rdata(&build_a_rrset("a.cn", &["2.2.2.2"]))
            .unwrap();
        assert_eq!(rrset.get_rrset(&name, RRType::A), None,);
        assert_eq!(rrset.len(), 0);

        let new_rrset = build_a_rrset("a.cn", &["1.1.1.1", "2.2.2.2"]);
        rrset.add_rrset(new_rrset.clone()).unwrap();
        assert_eq!(rrset.get_rrset(&name, RRType::A), Some(new_rrset));
        rrset.delete_rrset(RRType::A).unwrap();
        assert_eq!(rrset.get_rrset(&name, RRType::A), None);
    }

    #[test]
    fn test_cname_add_and_delete() {
        let name = Name::new("a.cn").unwrap();
        let alias = Name::new("a.com").unwrap();
        let mut rrset = Rdataset::new();
        let cname = RRset::from_str(format!("{} 3600 IN CNAME {}", name, alias).as_ref()).unwrap();
        rrset.add_rrset(cname.clone()).unwrap();
        assert_eq!(rrset.get_rrset(&name, RRType::CNAME), Some(cname));
        let alias = Name::new("b.com").unwrap();
        let cname = RRset::from_str(format!("{} 3600 IN CNAME {}", name, alias).as_ref()).unwrap();
        rrset.add_rrset(cname.clone()).unwrap();
        assert_eq!(rrset.get_rrset(&name, RRType::CNAME), Some(cname));
        let result = rrset.add_rrset(build_a_rrset("a.cn", &["1.1.1.1"]));
        assert!(result.is_err());
        rrset.delete_rrset(RRType::CNAME).unwrap();
        let result = rrset.add_rrset(build_a_rrset("a.cn", &["1.1.1.1"]));
        assert!(result.is_ok());
        let cname = RRset::from_str(format!("{} 3600 IN CNAME {}", name, alias).as_ref()).unwrap();
        let result = rrset.add_rrset(cname.clone());
        assert!(result.is_err());
    }
}
