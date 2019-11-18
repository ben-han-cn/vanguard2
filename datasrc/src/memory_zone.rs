use crate::domaintree::{
    node::NodePtr,
    node_chain::NodeChain,
    tree::{FindResultFlag, RBTree},
};
use crate::error::DataSrcError;
use crate::rdataset::Rdataset;
use crate::zone::{FindOption, FindResult, FindResultType, ZoneFinder, ZoneUpdater};
use failure::Result;
use r53::{LabelSequence, Name, NameRelation, RData, RRType, RRset};
use std::mem::swap;

type ZoneData = RBTree<Rdataset>;

pub struct MemoryZone {
    origin: Name,
    //root node is buffered to accelerate apex
    //soa and ns lookup
    root_node: NodePtr<Rdataset>,
    pub data: ZoneData,
}

impl MemoryZone {
    pub fn new(name: Name) -> Self {
        let mut data = ZoneData::new();
        //during node rotate in rbtree, root node is the parent node
        //which has down pointer point to the rest zone data, so root
        //node itself won't be changed
        let (root_node, _) = data.insert(name.clone(), None);
        MemoryZone {
            origin: name,
            root_node,
            data,
        }
    }
}

impl ZoneUpdater for MemoryZone {
    fn add_rrset(&mut self, rrset: RRset) -> Result<()> {
        if !rrset.name.is_subdomain(&self.origin) {
            return Err(DataSrcError::OutOfZone.into());
        }

        let is_delegation = rrset.typ == RRType::NS && !rrset.name.eq(&self.origin);
        let is_wildcard = rrset.name.is_wildcard();

        let mut find_result = self.data.find(&rrset.name);
        if find_result.flag == FindResultFlag::ExacatMatch {
            if let Some(rdataset) = find_result.node.get_value_mut().as_mut() {
                rdataset.add_rrset(rrset)?;
                if is_delegation {
                    find_result.node.set_callback(true);
                }
            } else {
                let mut rdataset = Rdataset::new();
                rdataset.add_rrset(rrset)?;
                find_result.node.set_value(Some(rdataset));
            }
        } else {
            let rrset_name = rrset.name.clone();
            let mut rdataset = Rdataset::new();
            rdataset.add_rrset(rrset)?;
            let (new_node, _) = self.data.insert(rrset_name.clone(), Some(rdataset));
            if is_delegation {
                new_node.set_callback(true);
            }
            if is_wildcard {
                let parent = rrset_name.parent(1).unwrap();
                let (node, old_data) = self.data.insert(parent, None);
                if let Some(old_value) = old_data {
                    node.set_value(old_value);
                }
                node.set_wildcard(true);
            }
        }
        Ok(())
    }

    fn delete_rrset(&mut self, name: &Name, typ: RRType) -> Result<()> {
        if !name.is_subdomain(&self.origin) {
            return Err(DataSrcError::OutOfZone.into());
        }

        if typ == RRType::SOA {
            return Err(DataSrcError::ZoneShortOfSOA.into());
        }

        if name.eq(&self.origin) && typ == RRType::NS {
            return Err(DataSrcError::ZoneShortOfNS.into());
        }

        let mut find_result = self.data.find(&name);
        if find_result.flag == FindResultFlag::ExacatMatch {
            if let Some(rdataset) = find_result.node.get_value_mut().as_mut() {
                rdataset.delete_rrset(typ)?;
                if rdataset.is_empty() {
                    let node = find_result.node;
                    if node != self.root_node {
                        self.data.remove_node(node);
                    }
                }
            }
            //ignore if rrset doesn't exists
            Ok(())
        } else {
            Err(DataSrcError::NameNotFound(name.to_string()).into())
        }
    }

    fn delete_rdata(&mut self, rrset: &RRset) -> Result<()> {
        if !rrset.name.is_subdomain(&self.origin) {
            return Err(DataSrcError::OutOfZone.into());
        }

        if rrset.typ == RRType::SOA {
            return Err(DataSrcError::ZoneShortOfSOA.into());
        }

        let mut find_result = self.data.find(&rrset.name);
        if find_result.flag == FindResultFlag::ExacatMatch {
            if let Some(rdataset) = find_result.node.get_value_mut().as_mut() {
                rdataset.delete_rdata(rrset)?;
                if rdataset.is_empty() {
                    let node = find_result.node;
                    self.data.remove_node(node);
                }
                Ok(())
            } else {
                Err(DataSrcError::RRsetNotFound(rrset.typ.to_string()).into())
            }
        } else {
            Err(DataSrcError::NameNotFound(rrset.name.to_string()).into())
        }
    }

    fn update_rdata(&mut self, old_rrset: &RRset, new_rrset: RRset) -> Result<()> {
        if !old_rrset.name.is_subdomain(&self.origin) {
            return Err(DataSrcError::OutOfZone.into());
        }

        let mut find_result = self.data.find(&old_rrset.name);
        if find_result.flag == FindResultFlag::ExacatMatch {
            if let Some(rdataset) = find_result.node.get_value_mut().as_mut() {
                rdataset.update_rdata(old_rrset, new_rrset)?;
                Ok(())
            } else {
                Err(DataSrcError::RRsetNotFound(old_rrset.typ.to_string()).into())
            }
        } else {
            Err(DataSrcError::NameNotFound(old_rrset.name.to_string()).into())
        }
    }

    fn delete_domain(&mut self, name: &Name) -> Result<()> {
        if !name.is_subdomain(&self.origin) {
            return Err(DataSrcError::OutOfZone.into());
        }

        if name.eq(&self.origin) {
            return Err(DataSrcError::ZoneOrginNotAllowToDelete.into());
        }

        let find_result = self.data.find(&name);
        if find_result.flag == FindResultFlag::ExacatMatch {
            let node = find_result.node;
            self.data.remove_node(node);
            Ok(())
        } else {
            Err(DataSrcError::NameNotFound(name.to_string()).into())
        }
    }
}

pub struct MemoryZoneFindResult<'a> {
    pub typ: FindResultType,
    pub node_chain: NodeChain<'a, Rdataset>,
    pub node: NodePtr<Rdataset>,
    pub zone: &'a MemoryZone,
    pub rrset: Option<RRset>,
}

impl<'a> MemoryZoneFindResult<'a> {
    fn new(zone: &'a MemoryZone) -> Self {
        MemoryZoneFindResult {
            typ: FindResultType::NXDomain,
            node_chain: NodeChain::<'a, Rdataset>::new(&zone.data),
            node: NodePtr::null(),
            zone,
            rrset: None,
        }
    }
}

impl<'a> FindResult for MemoryZoneFindResult<'a> {
    fn get_result_type(&self) -> FindResultType {
        self.typ
    }

    fn take_rrset(&mut self) -> Option<RRset> {
        self.rrset.take()
    }

    fn get_rrset(&self) -> &Option<RRset> {
        &self.rrset
    }

    fn get_additional(&self) -> Vec<RRset> {
        let rrsets = Vec::new();
        if self.rrset.is_none() {
            return rrsets;
        }

        let rrset = self.rrset.as_ref().unwrap();
        match rrset.typ {
            RRType::NS => rrset.rdatas.iter().fold(Vec::new(), |mut rrsets, rdata| {
                if let RData::NS(ns) = rdata {
                    rrsets.append(&mut self.get_address(&ns.name));
                }
                rrsets
            }),
            RRType::MX => rrset.rdatas.iter().fold(Vec::new(), |mut rrsets, rdata| {
                if let RData::MX(mx) = rdata {
                    rrsets.append(&mut self.get_address(&mx.name));
                }
                rrsets
            }),
            RRType::SRV => rrset.rdatas.iter().fold(Vec::new(), |mut rrsets, rdata| {
                if let RData::SRV(srv) = rdata {
                    rrsets.append(&mut self.get_address(&srv.target));
                }
                rrsets
            }),
            _ => Vec::new(),
        }
    }

    fn get_address(&self, name: &Name) -> Vec<RRset> {
        let mut result = self.zone.find(name, RRType::A, FindOption::GlueOK);
        let mut rrsets = Vec::new();
        let mut try_aaaa = false;
        if result.typ == FindResultType::Success {
            rrsets.push(result.rrset.take().unwrap());
            try_aaaa = true;
        } else if result.typ == FindResultType::NXRRset && !result.node.is_null() {
            try_aaaa = true;
        }

        if try_aaaa {
            if let Some(rdataset) = result.node.get_value().as_ref() {
                if let Some(aaaa) = rdataset.get_rrset(name, RRType::AAAA) {
                    rrsets.push(aaaa);
                }
            }
        }
        rrsets
    }

    fn get_apex_ns_and_glue(&self) -> (RRset, Vec<RRset>) {
        let ns = self
            .zone
            .root_node
            .get_value()
            .as_ref()
            .unwrap()
            .get_rrset(&self.zone.origin, RRType::NS)
            .unwrap();
        let mut addresses = Vec::with_capacity(ns.rdatas.len());
        for rdata in &ns.rdatas {
            if let RData::NS(ns) = rdata {
                addresses.append(&mut self.get_address(&ns.name));
            }
        }
        (ns, addresses)
    }

    fn get_apex_soa(&self) -> RRset {
        self.zone
            .root_node
            .get_value()
            .as_ref()
            .unwrap()
            .get_rrset(&self.zone.origin, RRType::SOA)
            .unwrap()
    }
}

struct FindState {
    zone_cut: NodePtr<Rdataset>,
    rrset: Option<RRset>,
    option: FindOption,
}

impl FindState {
    fn new(option: FindOption) -> Self {
        FindState {
            zone_cut: NodePtr::null(),
            rrset: None,
            option,
        }
    }
}

fn zonecut_handler<'a>(node: NodePtr<Rdataset>, name: Name, state: &mut FindState) -> bool {
    let ns = node
        .get_value()
        .as_ref()
        .unwrap()
        .get_rrset(&name, RRType::NS)
        .expect("zone cut has no ns");
    if !state.zone_cut.is_null() {
        return false;
    }
    state.zone_cut = node;
    state.rrset = Some(ns);
    state.option != FindOption::GlueOK
}

impl<'a> ZoneFinder<'a> for MemoryZone {
    type FindResult = MemoryZoneFindResult<'a>;

    fn get_origin(&self) -> &Name {
        &self.origin
    }

    fn find(&self, name: &Name, typ: RRType, opt: FindOption) -> MemoryZoneFindResult {
        let mut find_result = MemoryZoneFindResult::new(self);
        let mut state = FindState::new(opt);
        let result = self.data.find_node_ext(
            name,
            &mut find_result.node_chain,
            &mut Some(zonecut_handler),
            &mut state,
        );
        match result.flag {
            FindResultFlag::PartialMatch => {
                if !state.zone_cut.is_null() {
                    find_result.typ = FindResultType::Delegation;
                    swap(&mut find_result.rrset, &mut state.rrset);
                    swap(&mut find_result.node, &mut state.zone_cut);
                    return find_result;
                }

                if find_result.node_chain.last_compared_result.relation == NameRelation::SuperDomain
                {
                    find_result.typ = FindResultType::NXRRset;
                    return find_result;
                }

                if find_result.node_chain.top().is_wildcard() {
                    let wildcard_name = find_result
                        .node_chain
                        .get_absolute_name(&LabelSequence::new(vec![1, b'*'], vec![0]));
                    let mut node_chain = NodeChain::new(&self.data);
                    let result = self.data.find_node(&wildcard_name, &mut node_chain);
                    debug_assert!(result.flag == FindResultFlag::ExacatMatch);

                    let rdataset = result
                        .node
                        .get_value()
                        .as_ref()
                        .expect("wildcard domain is empty");
                    if let Some(rrset) = rdataset.get_rrset(name, typ) {
                        find_result.rrset = Some(rrset);
                        find_result.typ = FindResultType::Success;
                        return find_result;
                    }
                    if let Some(mut cname) = rdataset.get_rrset(name, RRType::CNAME) {
                        cname.name = name.clone();
                        find_result.rrset = Some(cname);
                        find_result.typ = FindResultType::CName;
                        return find_result;
                    }
                    find_result.typ = FindResultType::NXRRset;
                    return find_result;
                }
                find_result.typ = FindResultType::NXDomain;
                return find_result;
            }
            FindResultFlag::NotFound => {
                find_result.typ = FindResultType::NXDomain;
                return find_result;
            }
            FindResultFlag::ExacatMatch => {
                if result.node.get_value().is_none() {
                    find_result.typ = FindResultType::NXRRset;
                    return find_result;
                }

                find_result.node = result.node;
                if !name.eq(&self.origin) {
                    if let Some(ns) = result
                        .node
                        .get_value()
                        .as_ref()
                        .unwrap()
                        .get_rrset(name, RRType::NS)
                    {
                        find_result.typ = FindResultType::Delegation;
                        find_result.rrset = Some(ns);
                        return find_result;
                    }
                }

                if let Some(rrset) = result
                    .node
                    .get_value()
                    .as_ref()
                    .unwrap()
                    .get_rrset(name, typ)
                {
                    find_result.typ = FindResultType::Success;
                    find_result.rrset = Some(rrset);
                    return find_result;
                }

                if let Some(cname) = result
                    .node
                    .get_value()
                    .as_ref()
                    .unwrap()
                    .get_rrset(name, RRType::CNAME)
                {
                    find_result.typ = FindResultType::CName;
                    find_result.rrset = Some(cname);
                    return find_result;
                }

                find_result.typ = FindResultType::NXRRset;
                return find_result;
            }
        }
    }
}
