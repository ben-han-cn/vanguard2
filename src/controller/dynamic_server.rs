use crate::auth::{AuthError, AuthZone, ZoneUpdater};
use r53::{Name, RData, RRClass, RRTtl, RRType, RRset};
use std::sync::{Arc, RwLock};
use tonic::{transport::Server, Code, Request, Response, Status};

pub mod dynamic_dns {
    tonic::include_proto!("dynamicdns");
}

use dynamic_dns::{
    server::{DynamicUpdateInterface, DynamicUpdateInterfaceServer},
    AddRRsetRequest, AddRRsetResponse, AddZoneRequest, AddZoneResponse, DeleteDomainRequest,
    DeleteDomainResponse, DeleteRRsetRequest, DeleteRRsetResponse, DeleteRdataRequest,
    DeleteRdataResponse, DeleteZoneRequest, DeleteZoneResponse, UpdateRdataRequest,
    UpdateRdataResponse,
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
    fn do_add_rrsets(&self, zone: &Name, rrsets: Vec<RRset>) -> Result<(), failure::Error> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(zone) {
            for rrset in rrsets {
                zone.add_rrset(rrset)?;
            }
            Ok(())
        } else {
            Err(AuthError::UnknownZone(zone.to_string()).into())
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
                    zones.delete_zone(name);
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
            return Err(Status::new(Code::Internal, e.to_string()));
        }

        let rrsets = rrsets.iter().map(|rrset| proto_rrset_to_r53(rrset)).fold(
            Ok(Vec::new()),
            |rrsets: Result<Vec<RRset>, failure::Error>, rrset| match rrsets {
                Ok(mut rrsets) => {
                    let rrset = rrset?;
                    rrsets.push(rrset);
                    Ok(rrsets)
                }
                Err(e) => Err(e),
            },
        );
        if let Err(e) = rrsets {
            return Err(Status::new(Code::Internal, e.to_string()));
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
        Err(tonic::Status::unimplemented("Not yet implemented"))
    }
    async fn delete_r_rset(
        &self,
        request: tonic::Request<DeleteRRsetRequest>,
    ) -> Result<tonic::Response<DeleteRRsetResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Not yet implemented"))
    }
    async fn delete_rdata(
        &self,
        request: tonic::Request<DeleteRdataRequest>,
    ) -> Result<tonic::Response<DeleteRdataResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Not yet implemented"))
    }
    async fn update_rdata(
        &self,
        request: tonic::Request<UpdateRdataRequest>,
    ) -> Result<tonic::Response<UpdateRdataResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Not yet implemented"))
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

fn proto_rrset_to_r53(rrset: &dynamic_dns::RRset) -> Result<RRset, failure::Error> {
    let name = Name::new(rrset.name.as_ref())?;
    let typ = proto_typ_to_r53(rrset.r#type);
    let rdatas = rrset.rdatas.iter().fold(
        Ok(Vec::new()),
        |rdatas: Result<Vec<RData>, failure::Error>, rdata| match rdatas {
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
