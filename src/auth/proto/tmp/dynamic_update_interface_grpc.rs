// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]

const METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_ZONE: ::grpcio::Method<super::dynamic_update_interface::AddZoneRequest, super::dynamic_update_interface::AddZoneResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/client.DynamicUpdateInterface/AddZone",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_ZONE: ::grpcio::Method<super::dynamic_update_interface::DeleteZoneRequest, super::dynamic_update_interface::DeleteZoneResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/client.DynamicUpdateInterface/DeleteZone",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_R_RSET: ::grpcio::Method<super::dynamic_update_interface::AddRRsetRequest, super::dynamic_update_interface::AddRRsetResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/client.DynamicUpdateInterface/AddRRset",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_DOMAIN: ::grpcio::Method<super::dynamic_update_interface::DeleteDomainRequest, super::dynamic_update_interface::DeleteDomainResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/client.DynamicUpdateInterface/DeleteDomain",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_R_RSET: ::grpcio::Method<super::dynamic_update_interface::DeleteRRsetRequest, super::dynamic_update_interface::DeleteRRsetResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/client.DynamicUpdateInterface/DeleteRRset",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_RDATA: ::grpcio::Method<super::dynamic_update_interface::DeleteRdataRequest, super::dynamic_update_interface::DeleteRdataResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/client.DynamicUpdateInterface/DeleteRdata",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_DYNAMIC_UPDATE_INTERFACE_UPDATE_RDATA: ::grpcio::Method<super::dynamic_update_interface::UpdateRdataRequest, super::dynamic_update_interface::UpdateRdataResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/client.DynamicUpdateInterface/UpdateRdata",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

#[derive(Clone)]
pub struct DynamicUpdateInterfaceClient {
    client: ::grpcio::Client,
}

impl DynamicUpdateInterfaceClient {
    pub fn new(channel: ::grpcio::Channel) -> Self {
        DynamicUpdateInterfaceClient {
            client: ::grpcio::Client::new(channel),
        }
    }

    pub fn add_zone_opt(&self, req: &super::dynamic_update_interface::AddZoneRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::dynamic_update_interface::AddZoneResponse> {
        self.client.unary_call(&METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_ZONE, req, opt)
    }

    pub fn add_zone(&self, req: &super::dynamic_update_interface::AddZoneRequest) -> ::grpcio::Result<super::dynamic_update_interface::AddZoneResponse> {
        self.add_zone_opt(req, ::grpcio::CallOption::default())
    }

    pub fn add_zone_async_opt(&self, req: &super::dynamic_update_interface::AddZoneRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::AddZoneResponse>> {
        self.client.unary_call_async(&METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_ZONE, req, opt)
    }

    pub fn add_zone_async(&self, req: &super::dynamic_update_interface::AddZoneRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::AddZoneResponse>> {
        self.add_zone_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_zone_opt(&self, req: &super::dynamic_update_interface::DeleteZoneRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::dynamic_update_interface::DeleteZoneResponse> {
        self.client.unary_call(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_ZONE, req, opt)
    }

    pub fn delete_zone(&self, req: &super::dynamic_update_interface::DeleteZoneRequest) -> ::grpcio::Result<super::dynamic_update_interface::DeleteZoneResponse> {
        self.delete_zone_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_zone_async_opt(&self, req: &super::dynamic_update_interface::DeleteZoneRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteZoneResponse>> {
        self.client.unary_call_async(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_ZONE, req, opt)
    }

    pub fn delete_zone_async(&self, req: &super::dynamic_update_interface::DeleteZoneRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteZoneResponse>> {
        self.delete_zone_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn add_r_rset_opt(&self, req: &super::dynamic_update_interface::AddRRsetRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::dynamic_update_interface::AddRRsetResponse> {
        self.client.unary_call(&METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_R_RSET, req, opt)
    }

    pub fn add_r_rset(&self, req: &super::dynamic_update_interface::AddRRsetRequest) -> ::grpcio::Result<super::dynamic_update_interface::AddRRsetResponse> {
        self.add_r_rset_opt(req, ::grpcio::CallOption::default())
    }

    pub fn add_r_rset_async_opt(&self, req: &super::dynamic_update_interface::AddRRsetRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::AddRRsetResponse>> {
        self.client.unary_call_async(&METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_R_RSET, req, opt)
    }

    pub fn add_r_rset_async(&self, req: &super::dynamic_update_interface::AddRRsetRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::AddRRsetResponse>> {
        self.add_r_rset_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_domain_opt(&self, req: &super::dynamic_update_interface::DeleteDomainRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::dynamic_update_interface::DeleteDomainResponse> {
        self.client.unary_call(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_DOMAIN, req, opt)
    }

    pub fn delete_domain(&self, req: &super::dynamic_update_interface::DeleteDomainRequest) -> ::grpcio::Result<super::dynamic_update_interface::DeleteDomainResponse> {
        self.delete_domain_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_domain_async_opt(&self, req: &super::dynamic_update_interface::DeleteDomainRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteDomainResponse>> {
        self.client.unary_call_async(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_DOMAIN, req, opt)
    }

    pub fn delete_domain_async(&self, req: &super::dynamic_update_interface::DeleteDomainRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteDomainResponse>> {
        self.delete_domain_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_r_rset_opt(&self, req: &super::dynamic_update_interface::DeleteRRsetRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::dynamic_update_interface::DeleteRRsetResponse> {
        self.client.unary_call(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_R_RSET, req, opt)
    }

    pub fn delete_r_rset(&self, req: &super::dynamic_update_interface::DeleteRRsetRequest) -> ::grpcio::Result<super::dynamic_update_interface::DeleteRRsetResponse> {
        self.delete_r_rset_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_r_rset_async_opt(&self, req: &super::dynamic_update_interface::DeleteRRsetRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteRRsetResponse>> {
        self.client.unary_call_async(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_R_RSET, req, opt)
    }

    pub fn delete_r_rset_async(&self, req: &super::dynamic_update_interface::DeleteRRsetRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteRRsetResponse>> {
        self.delete_r_rset_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_rdata_opt(&self, req: &super::dynamic_update_interface::DeleteRdataRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::dynamic_update_interface::DeleteRdataResponse> {
        self.client.unary_call(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_RDATA, req, opt)
    }

    pub fn delete_rdata(&self, req: &super::dynamic_update_interface::DeleteRdataRequest) -> ::grpcio::Result<super::dynamic_update_interface::DeleteRdataResponse> {
        self.delete_rdata_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_rdata_async_opt(&self, req: &super::dynamic_update_interface::DeleteRdataRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteRdataResponse>> {
        self.client.unary_call_async(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_RDATA, req, opt)
    }

    pub fn delete_rdata_async(&self, req: &super::dynamic_update_interface::DeleteRdataRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::DeleteRdataResponse>> {
        self.delete_rdata_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_rdata_opt(&self, req: &super::dynamic_update_interface::UpdateRdataRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::dynamic_update_interface::UpdateRdataResponse> {
        self.client.unary_call(&METHOD_DYNAMIC_UPDATE_INTERFACE_UPDATE_RDATA, req, opt)
    }

    pub fn update_rdata(&self, req: &super::dynamic_update_interface::UpdateRdataRequest) -> ::grpcio::Result<super::dynamic_update_interface::UpdateRdataResponse> {
        self.update_rdata_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_rdata_async_opt(&self, req: &super::dynamic_update_interface::UpdateRdataRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::UpdateRdataResponse>> {
        self.client.unary_call_async(&METHOD_DYNAMIC_UPDATE_INTERFACE_UPDATE_RDATA, req, opt)
    }

    pub fn update_rdata_async(&self, req: &super::dynamic_update_interface::UpdateRdataRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::dynamic_update_interface::UpdateRdataResponse>> {
        self.update_rdata_async_opt(req, ::grpcio::CallOption::default())
    }
    pub fn spawn<F>(&self, f: F) where F: ::futures::Future<Item = (), Error = ()> + Send + 'static {
        self.client.spawn(f)
    }
}

pub trait DynamicUpdateInterface {
    fn add_zone(&mut self, ctx: ::grpcio::RpcContext, req: super::dynamic_update_interface::AddZoneRequest, sink: ::grpcio::UnarySink<super::dynamic_update_interface::AddZoneResponse>);
    fn delete_zone(&mut self, ctx: ::grpcio::RpcContext, req: super::dynamic_update_interface::DeleteZoneRequest, sink: ::grpcio::UnarySink<super::dynamic_update_interface::DeleteZoneResponse>);
    fn add_r_rset(&mut self, ctx: ::grpcio::RpcContext, req: super::dynamic_update_interface::AddRRsetRequest, sink: ::grpcio::UnarySink<super::dynamic_update_interface::AddRRsetResponse>);
    fn delete_domain(&mut self, ctx: ::grpcio::RpcContext, req: super::dynamic_update_interface::DeleteDomainRequest, sink: ::grpcio::UnarySink<super::dynamic_update_interface::DeleteDomainResponse>);
    fn delete_r_rset(&mut self, ctx: ::grpcio::RpcContext, req: super::dynamic_update_interface::DeleteRRsetRequest, sink: ::grpcio::UnarySink<super::dynamic_update_interface::DeleteRRsetResponse>);
    fn delete_rdata(&mut self, ctx: ::grpcio::RpcContext, req: super::dynamic_update_interface::DeleteRdataRequest, sink: ::grpcio::UnarySink<super::dynamic_update_interface::DeleteRdataResponse>);
    fn update_rdata(&mut self, ctx: ::grpcio::RpcContext, req: super::dynamic_update_interface::UpdateRdataRequest, sink: ::grpcio::UnarySink<super::dynamic_update_interface::UpdateRdataResponse>);
}

pub fn create_dynamic_update_interface<S: DynamicUpdateInterface + Send + Clone + 'static>(s: S) -> ::grpcio::Service {
    let mut builder = ::grpcio::ServiceBuilder::new();
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_ZONE, move |ctx, req, resp| {
        instance.add_zone(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_ZONE, move |ctx, req, resp| {
        instance.delete_zone(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_DYNAMIC_UPDATE_INTERFACE_ADD_R_RSET, move |ctx, req, resp| {
        instance.add_r_rset(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_DOMAIN, move |ctx, req, resp| {
        instance.delete_domain(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_R_RSET, move |ctx, req, resp| {
        instance.delete_r_rset(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_DYNAMIC_UPDATE_INTERFACE_DELETE_RDATA, move |ctx, req, resp| {
        instance.delete_rdata(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_DYNAMIC_UPDATE_INTERFACE_UPDATE_RDATA, move |ctx, req, resp| {
        instance.update_rdata(ctx, req, resp)
    });
    builder.build()
}
