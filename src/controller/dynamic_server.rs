use crate::auth::{AuthZone, ZoneUpdater};
use anyhow::{self, bail};
use r53::{Name, RData, RRClass, RRTtl, RRType, RRset};
use std::sync::{Arc, RwLock};
use tonic::{Code, Request, Response, Status};

pub mod dynamic_dns {
    tonic::include_proto!("dynamicdns");
}

use dynamic_dns::{
    dynamic_update_interface_server::DynamicUpdateInterface, AddRRsetRequest, AddRRsetResponse,
    AddZoneRequest, AddZoneResponse, DeleteDomainRequest, DeleteDomainResponse, DeleteRRsetRequest,
    DeleteRRsetResponse, DeleteRdataRequest, DeleteRdataResponse, DeleteZoneRequest,
    DeleteZoneResponse, UpdateRdataRequest, UpdateRdataResponse,
};

#[derive(Clone)]
pub struct DynamicUpdateHandler {
    zones: Arc<RwLock<AuthZone>>,
}

impl DynamicUpdateHandler {
    pub fn new(zones: Arc<RwLock<AuthZone>>) -> Self {
        DynamicUpdateHandler { zones }
    }
}

impl DynamicUpdateHandler {
    fn do_add_rrsets(&self, zone: &Name, rrsets: Vec<RRset>) -> anyhow::Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(zone) {
            for rrset in rrsets {
                zone.add_rrset(rrset)?;
            }
            Ok(())
        } else {
            bail!("unknown zone {}", zone.to_string());
        }
    }

    fn do_delete_domains(&self, zone: &Name, names: Vec<Name>) -> anyhow::Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(_zone) = zones.get_exact_zone(zone) {
            for name in names {
                zones.delete_zone(&name)?;
            }
            Ok(())
        } else {
            bail!("unknown zone {}", zone.to_string());
        }
    }

    fn do_delete_rrsets(
        &self,
        zone: &Name,
        rrset_headers: Vec<(Name, RRType)>,
    ) -> anyhow::Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(zone) {
            for rrset_header in rrset_headers {
                zone.delete_rrset(&rrset_header.0, rrset_header.1)?;
            }
            Ok(())
        } else {
            bail!("unknown zone {}", zone.to_string());
        }
    }

    fn do_delete_rdatas(&self, zone: &Name, rrsets: Vec<RRset>) -> anyhow::Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(zone) {
            for rrset in rrsets {
                zone.delete_rdata(&rrset)?;
            }
            Ok(())
        } else {
            bail!("unknown zone {}", zone.to_string());
        }
    }

    fn do_update_rdata(
        &self,
        zone: &Name,
        old_rrset: RRset,
        new_rrset: RRset,
    ) -> anyhow::Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(zone) {
            zone.update_rdata(&old_rrset, new_rrset)
        } else {
            bail!("unknown zone {}", zone.to_string());
        }
    }
}

#[tonic::async_trait]
impl DynamicUpdateInterface for DynamicUpdateHandler {
    async fn add_zone(
        &self,
        request: Request<AddZoneRequest>,
    ) -> Result<Response<AddZoneResponse>, Status> {
        let mut zones = self.zones.write().unwrap();
        let AddZoneRequest { zone, zone_content } = request.into_inner();
        let zone = match r53::Name::new(&zone) {
            Ok(name) => name,
            Err(e) => {
                return Err(Status::new(Code::InvalidArgument, e.to_string()));
            }
        };
        match zones.add_zone(zone, zone_content.as_ref()) {
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
            _ => Ok(Response::new(AddZoneResponse {})),
        }
    }

    async fn delete_zone(
        &self,
        request: tonic::Request<DeleteZoneRequest>,
    ) -> Result<tonic::Response<DeleteZoneResponse>, tonic::Status> {
        let DeleteZoneRequest { zones } = request.into_inner();
        let names: Result<Vec<Name>, _> = zones.iter().map(|n| r53::Name::new(n)).collect();
        match names {
            Ok(names) => {
                let mut zones = self.zones.write().unwrap();
                for name in &names {
                    zones.delete_zone(name).unwrap();
                }
                Ok(Response::new(DeleteZoneResponse {}))
            }
            Err(e) => Err(Status::new(Code::InvalidArgument, e.to_string())),
        }
    }

    async fn add_r_rset(
        &self,
        request: tonic::Request<AddRRsetRequest>,
    ) -> Result<tonic::Response<AddRRsetResponse>, tonic::Status> {
        let AddRRsetRequest { zone, rrsets } = request.into_inner();
        let zone = r53::Name::new(zone.as_ref());
        if let Err(e) = zone {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        let rrsets = rrsets.iter().map(|rrset| proto_rrset_to_r53(rrset)).fold(
            Ok(Vec::new()),
            |rrsets: anyhow::Result<Vec<RRset>>, rrset| match rrsets {
                Ok(mut rrsets) => {
                    let rrset = rrset?;
                    rrsets.push(rrset);
                    Ok(rrsets)
                }
                Err(e) => Err(e),
            },
        );
        if let Err(e) = rrsets {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        match self.do_add_rrsets(&zone.unwrap(), rrsets.unwrap()) {
            Ok(_) => Ok(Response::new(AddRRsetResponse {})),
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
        }
    }

    async fn delete_domain(
        &self,
        request: tonic::Request<DeleteDomainRequest>,
    ) -> Result<tonic::Response<DeleteDomainResponse>, tonic::Status> {
        let DeleteDomainRequest { zone, names } = request.into_inner();
        let zone = r53::Name::new(zone.as_ref());
        if let Err(e) = zone {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }
        let names: Result<Vec<Name>, _> = names.iter().map(|n| r53::Name::new(n)).collect();
        if let Err(e) = names {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }
        match self.do_delete_domains(&zone.unwrap(), names.unwrap()) {
            Ok(_) => Ok(Response::new(DeleteDomainResponse {})),
            Err(e) => Err(Status::new(Code::InvalidArgument, e.to_string())),
        }
    }

    async fn delete_r_rset(
        &self,
        request: tonic::Request<DeleteRRsetRequest>,
    ) -> Result<tonic::Response<DeleteRRsetResponse>, tonic::Status> {
        let DeleteRRsetRequest { zone, rrsets } = request.into_inner();
        let zone = r53::Name::new(zone.as_ref());
        if let Err(e) = zone {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        let headers = rrsets.iter().fold(
            Ok(Vec::new()),
            |headers: anyhow::Result<Vec<(Name, RRType)>>, header| match headers {
                Ok(mut headers) => {
                    let name = Name::new(header.name.as_ref())?;
                    headers.push((name, proto_typ_to_r53(header.r#type)));
                    Ok(headers)
                }
                Err(e) => Err(e),
            },
        );
        if let Err(e) = headers {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        match self.do_delete_rrsets(&zone.unwrap(), headers.unwrap()) {
            Ok(_) => Ok(Response::new(DeleteRRsetResponse {})),
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
        }
    }

    async fn delete_rdata(
        &self,
        request: tonic::Request<DeleteRdataRequest>,
    ) -> Result<tonic::Response<DeleteRdataResponse>, tonic::Status> {
        let DeleteRdataRequest { zone, rrsets } = request.into_inner();
        let zone = r53::Name::new(zone.as_ref());
        if let Err(e) = zone {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        let rrsets = rrsets.iter().map(|rrset| proto_rrset_to_r53(rrset)).fold(
            Ok(Vec::new()),
            |rrsets: anyhow::Result<Vec<RRset>>, rrset| match rrsets {
                Ok(mut rrsets) => {
                    let rrset = rrset?;
                    rrsets.push(rrset);
                    Ok(rrsets)
                }
                Err(e) => Err(e),
            },
        );
        if let Err(e) = rrsets {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        match self.do_delete_rdatas(&zone.unwrap(), rrsets.unwrap()) {
            Ok(_) => Ok(Response::new(DeleteRdataResponse {})),
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
        }
    }

    async fn update_rdata(
        &self,
        request: tonic::Request<UpdateRdataRequest>,
    ) -> Result<tonic::Response<UpdateRdataResponse>, tonic::Status> {
        let UpdateRdataRequest {
            zone,
            old_rrset,
            new_rrset,
        } = request.into_inner();
        let zone = r53::Name::new(zone.as_ref());
        if let Err(e) = zone {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        if old_rrset.is_none() || new_rrset.is_none() {
            return Err(Status::new(
                Code::InvalidArgument,
                "old or new rrset is empty".to_string(),
            ));
        }

        let old_rrset = proto_rrset_to_r53(old_rrset.as_ref().unwrap());
        let new_rrset = proto_rrset_to_r53(new_rrset.as_ref().unwrap());
        if let Err(e) = old_rrset {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }
        if let Err(e) = new_rrset {
            return Err(Status::new(Code::InvalidArgument, e.to_string()));
        }

        match self.do_update_rdata(&zone.unwrap(), old_rrset.unwrap(), new_rrset.unwrap()) {
            Ok(_) => Ok(Response::new(UpdateRdataResponse {})),
            Err(e) => Err(Status::new(Code::Internal, e.to_string())),
        }
    }
}

fn proto_typ_to_r53(typ: i32) -> RRType {
    match typ {
        0 => RRType::A,
        1 => RRType::AAAA,
        2 => RRType::NS,
        3 => RRType::SOA,
        4 => RRType::CNAME,
        5 => RRType::MX,
        6 => RRType::TXT,
        7 => RRType::SRV,
        8 => RRType::PTR,
        _ => RRType::Unknown(typ as u16),
    }
}

fn proto_rrset_to_r53(rrset: &dynamic_dns::RRset) -> anyhow::Result<RRset> {
    let name = Name::new(rrset.name.as_ref())?;
    let typ = proto_typ_to_r53(rrset.r#type);
    let rdatas = rrset.rdatas.iter().fold(
        Ok(Vec::new()),
        |rdatas: anyhow::Result<Vec<RData>>, rdata| match rdatas {
            Ok(mut rdatas) => {
                let rdata = RData::from_str(typ, rdata)?;
                rdatas.push(rdata);
                Ok(rdatas)
            }
            Err(e) => Err(e),
        },
    )?;

    Ok(RRset {
        name,
        typ,
        class: RRClass::IN,
        ttl: RRTtl(rrset.ttl),
        rdatas,
    })
}
