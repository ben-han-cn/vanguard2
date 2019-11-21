use failure::Result;
use r53::{Name, RRType, RRset};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum FindResultType {
    Success,
    Delegation,
    NXDomain,
    NXRRset,
    CName,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum FindOption {
    FollowZoneCut,
    GlueOK,
}

pub trait FindResult {
    fn get_result_type(&self) -> FindResultType;
    fn take_rrset(&mut self) -> Option<RRset>;
    fn get_rrset(&self) -> &Option<RRset>;
    fn get_additional(&self) -> Vec<RRset>;
    fn get_address(&self, name: &Name) -> Vec<RRset>;
    fn get_apex_ns_and_glue(&self) -> (RRset, Vec<RRset>);
    fn get_apex_soa(&self) -> RRset;
}

pub trait ZoneFinder<'a> {
    type FindResult: FindResult;
    fn get_origin(&self) -> &Name;
    fn find(&'a self, name: &Name, typ: RRType, opt: FindOption) -> Self::FindResult;
}

pub trait ZoneUpdater {
    fn add_rrset(&mut self, rrset: RRset) -> Result<()>;
    fn delete_rrset(&mut self, name: &Name, typ: RRType) -> Result<()>;
    fn delete_rdata(&mut self, rrset: &RRset) -> Result<()>;
    fn update_rdata(&mut self, old_rrset: &RRset, new_rrset: RRset) -> Result<()>;
    fn delete_domain(&mut self, name: &Name) -> Result<()>;
}
