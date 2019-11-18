use super::{
    error::AuthError,
    proto::{self, dynamic_update_interface, dynamic_update_interface_grpc},
    zones::AuthZone,
};
use datasrc::ZoneUpdater;
use failure::Result;
use grpc_helpers::provide_grpc_response;
use grpc_helpers::{spawn_service_thread, ServerHandle};
use r53::{Name, RData, RRClass, RRTtl, RRType, RRset};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct DynamicUpdateHandler {
    zones: Arc<RwLock<AuthZone>>,
}

//note: current implementation doesn't support transaction
impl DynamicUpdateHandler {
    pub fn new(zones: Arc<RwLock<AuthZone>>) -> Self {
        DynamicUpdateHandler { zones }
    }

    pub fn run(self, ip: String, port: u16) -> ServerHandle {
        let update_service = dynamic_update_interface_grpc::create_dynamic_update_interface(self);
        spawn_service_thread(update_service, ip, port, "dynamic_update_service")
    }
}

impl DynamicUpdateHandler {
    fn do_add_zone(&mut self, name: Name, zone_content: &str) -> Result<()> {
        let mut zones = self.zones.write().unwrap();
        zones.add_zone(name, zone_content)
    }

    fn do_delete_zones(&mut self, names: &Vec<Name>) -> Result<()> {
        let mut zones = self.zones.write().unwrap();
        for name in names {
            zones.delete_zone(name)?;
        }
        Ok(())
    }

    fn do_add_rrsets(&mut self, zone: &Name, rrsets: Vec<RRset>) -> Result<()> {
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

    fn do_delete_domains(&mut self, zone: &Name, names: &Vec<Name>) -> Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(&zone) {
            for name in names {
                zone.delete_domain(name)?;
            }
            Ok(())
        } else {
            Err(AuthError::UnknownZone(zone.to_string()).into())
        }
    }

    fn do_delete_rrsets(&mut self, zone: &Name, rrset_headers: &Vec<(Name, RRType)>) -> Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(&zone) {
            for rrset_header in rrset_headers {
                zone.delete_rrset(&rrset_header.0, rrset_header.1)?;
            }
            Ok(())
        } else {
            Err(AuthError::UnknownZone(zone.to_string()).into())
        }
    }

    fn do_delete_rdatas(&mut self, zone: &Name, rrsets: &Vec<RRset>) -> Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(&zone) {
            for rrset in rrsets {
                zone.delete_rdata(rrset)?;
            }
            Ok(())
        } else {
            Err(AuthError::UnknownZone(zone.to_string()).into())
        }
    }

    fn do_update_rdatas(&mut self, zone: &Name, old_rrset: &RRset, new_rrset: RRset) -> Result<()> {
        let mut zones = self.zones.write().unwrap();
        if let Some(zone) = zones.get_exact_zone(&zone) {
            zone.update_rdata(old_rrset, new_rrset)
        } else {
            Err(AuthError::UnknownZone(zone.to_string()).into())
        }
    }
}

impl dynamic_update_interface_grpc::DynamicUpdateInterface for DynamicUpdateHandler {
    fn add_zone(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: dynamic_update_interface::AddZoneRequest,
        sink: ::grpcio::UnarySink<dynamic_update_interface::AddZoneResponse>,
    ) {
        let resp =
            Name::new(req.get_zone()).map(|zone| self.do_add_zone(zone, req.get_zone_content()));
        provide_grpc_response(
            resp.map(|_| dynamic_update_interface::AddZoneResponse::new()),
            ctx,
            sink,
        );
    }

    fn delete_zone(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: dynamic_update_interface::DeleteZoneRequest,
        sink: ::grpcio::UnarySink<dynamic_update_interface::DeleteZoneResponse>,
    ) {
        let resp = req
            .get_zones()
            .iter()
            .fold(
                Ok(Vec::new()),
                |names: Result<Vec<Name>>, name| match names {
                    Ok(mut names) => {
                        let name = Name::new(name)?;
                        names.push(name);
                        Ok(names)
                    }
                    Err(e) => Err(e),
                },
            )
            .map(|zones| self.do_delete_zones(&zones));
        provide_grpc_response(
            resp.map(|_| dynamic_update_interface::DeleteZoneResponse::new()),
            ctx,
            sink,
        );
    }

    fn add_r_rset(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: dynamic_update_interface::AddRRsetRequest,
        sink: ::grpcio::UnarySink<dynamic_update_interface::AddRRsetResponse>,
    ) {
        let resp = Name::new(req.get_zone()).map(|zone| {
            req.get_rrsets()
                .iter()
                .map(|rrset| proto_rrset_to_r53(rrset))
                .fold(
                    Ok(Vec::new()),
                    |rrsets: Result<Vec<RRset>>, rrset| match rrsets {
                        Ok(mut rrsets) => {
                            let rrset = rrset?;
                            rrsets.push(rrset);
                            Ok(rrsets)
                        }
                        Err(e) => Err(e),
                    },
                )
                .map(|rrsets| self.do_add_rrsets(&zone, rrsets))
        });
        provide_grpc_response(
            resp.map(|_| dynamic_update_interface::AddRRsetResponse::new()),
            ctx,
            sink,
        );
    }

    fn delete_domain(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: dynamic_update_interface::DeleteDomainRequest,
        sink: ::grpcio::UnarySink<dynamic_update_interface::DeleteDomainResponse>,
    ) {
        let resp = Name::new(req.get_zone()).map(|zone| {
            req.get_names()
                .iter()
                .fold(
                    Ok(Vec::new()),
                    |names: Result<Vec<Name>>, name| match names {
                        Ok(mut names) => {
                            let name = Name::new(name)?;
                            names.push(name);
                            Ok(names)
                        }
                        Err(e) => Err(e),
                    },
                )
                .map(|names| self.do_delete_domains(&zone, &names))
        });
        provide_grpc_response(
            resp.map(|_| dynamic_update_interface::DeleteDomainResponse::new()),
            ctx,
            sink,
        );
    }

    fn delete_r_rset(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: dynamic_update_interface::DeleteRRsetRequest,
        sink: ::grpcio::UnarySink<dynamic_update_interface::DeleteRRsetResponse>,
    ) {
        let resp = Name::new(req.get_zone()).map(|zone| {
            req.get_rrsets()
                .iter()
                .fold(
                    Ok(Vec::new()),
                    |headers: Result<Vec<(Name, RRType)>>, header| match headers {
                        Ok(mut headers) => {
                            let name = Name::new(header.name.as_ref())?;
                            headers.push((name, proto_typ_to_r53(header.field_type)));
                            Ok(headers)
                        }
                        Err(e) => Err(e),
                    },
                )
                .map(|headers| self.do_delete_rrsets(&zone, &headers))
        });
        provide_grpc_response(
            resp.map(|_| dynamic_update_interface::DeleteRRsetResponse::new()),
            ctx,
            sink,
        );
    }

    fn delete_rdata(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: dynamic_update_interface::DeleteRdataRequest,
        sink: ::grpcio::UnarySink<dynamic_update_interface::DeleteRdataResponse>,
    ) {
        let resp = Name::new(req.get_zone()).map(|zone| {
            req.get_rrsets()
                .iter()
                .map(|rrset| proto_rrset_to_r53(rrset))
                .fold(
                    Ok(Vec::new()),
                    |rrsets: Result<Vec<RRset>>, rrset| match rrsets {
                        Ok(mut rrsets) => {
                            let rrset = rrset?;
                            rrsets.push(rrset);
                            Ok(rrsets)
                        }
                        Err(e) => Err(e),
                    },
                )
                .map(|rrsets| self.do_delete_rdatas(&zone, &rrsets))
        });
        provide_grpc_response(
            resp.map(|_| dynamic_update_interface::DeleteRdataResponse::new()),
            ctx,
            sink,
        );
    }

    fn update_rdata(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: dynamic_update_interface::UpdateRdataRequest,
        sink: ::grpcio::UnarySink<dynamic_update_interface::UpdateRdataResponse>,
    ) {
        let resp = Name::new(req.get_zone()).map(|zone| {
            let old_rrset = proto_rrset_to_r53(req.get_old_rrset())?;
            let new_rrset = proto_rrset_to_r53(req.get_new_rrset())?;
            self.do_update_rdatas(&zone, &old_rrset, new_rrset)
        });
        provide_grpc_response(
            resp.map(|_| dynamic_update_interface::UpdateRdataResponse::new()),
            ctx,
            sink,
        );
    }
}

fn proto_typ_to_r53(typ: proto::rrset::RRType) -> RRType {
    match typ {
        proto::rrset::RRType::A => RRType::A,
        proto::rrset::RRType::AAAA => RRType::AAAA,
        proto::rrset::RRType::NS => RRType::NS,
        proto::rrset::RRType::SOA => RRType::SOA,
        proto::rrset::RRType::CNAME => RRType::CNAME,
        proto::rrset::RRType::MX => RRType::MX,
        proto::rrset::RRType::TXT => RRType::TXT,
        proto::rrset::RRType::SRV => RRType::SRV,
        proto::rrset::RRType::PTR => RRType::PTR,
    }
}

fn proto_rrset_to_r53(rrset: &proto::rrset::RRset) -> Result<RRset> {
    let name = Name::new(rrset.name.as_ref())?;
    let typ = proto_typ_to_r53(rrset.get_field_type());
    let rdatas =
        rrset
            .get_rdatas()
            .iter()
            .fold(
                Ok(Vec::new()),
                |rdatas: Result<Vec<RData>>, rdata| match rdatas {
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
        ttl: RRTtl(rrset.get_ttl()),
        rdatas,
    })
}
