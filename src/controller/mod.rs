mod controller;
mod dynamic_server;

pub use controller::Controller;
pub use dynamic_server::dynamic_dns::{client::DynamicUpdateInterfaceClient, AddZoneRequest};
