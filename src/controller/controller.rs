use super::dynamic_server::{
    dynamic_dns::dynamic_update_interface_server::DynamicUpdateInterfaceServer,
    DynamicUpdateHandler,
};
use crate::{auth::AuthZone, config::ControllerConfig};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tonic::transport::Server;

pub struct Controller {
    addr: SocketAddr,
    dynamic_handler: DynamicUpdateHandler,
}

impl Controller {
    pub fn new(conf: &ControllerConfig, zones: Arc<RwLock<AuthZone>>) -> Self {
        Controller {
            addr: conf.address.parse().unwrap(),
            dynamic_handler: DynamicUpdateHandler::new(zones),
        }
    }

    pub async fn run(self) {
        Server::builder()
            .add_service(DynamicUpdateInterfaceServer::new(self.dynamic_handler))
            .serve(self.addr)
            .await;
    }
}
