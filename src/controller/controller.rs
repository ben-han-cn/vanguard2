use super::{
    dynamic_server::{dynamic_dns::server::DynamicUpdateInterfaceServer, DynamicUpdateHandler},
}
use crate::auth::AuthZone;

pub struct Controller{
    dynamic_dns_server: DynamicUpdateHandler, 
}

impl Controller {
    pub fn new(zones: Arc<RwLock<AuthZone>>) -> Self {
        Controller {
            dynamic_handler: DynamicUpdateHandler::new(zones),
        }
    }

    pub async fn run(self,  conf: &VgCtrlConfig) {
        let addr = conf.address.parse().unwrap();
        let dynamic_handler = DynamicUpdateHandler::new(self.zones.clone());
        Server::builder()
            .add_service(DynamicUpdateInterfaceServer::new(dynamic_handler))
            .serve(addr)
            .await.unwrap();
        Ok(())
    }
}
