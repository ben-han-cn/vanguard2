extern crate futures;
extern crate tokio;
mod resolver;

use clap::{App, Arg};
use metrics::start_metric_server;
use std::net::SocketAddr;
use std::thread;
use tokio::runtime::current_thread;

use vanguard2::auth::{AuthServer, DynamicUpdateHandler};
use vanguard2::config::VanguardConfig;
use vanguard2::server::{start_qps_calculate, Server};

fn main() {
    let matches = App::new("auth")
        .arg(
            Arg::with_name("config")
                .help("config file path")
                .long("config")
                .required(false)
                .takes_value(true),
        )
        .get_matches();

    let config_file = matches.value_of("config").unwrap_or("vanguard.conf");

    match VanguardConfig::load_config(config_file) {
        Err(e) => {
            eprintln!("load configure file failed: {:?}", e);
            return;
        }
        Ok(config) => {
            let auth_server = AuthServer::new(&config.auth);
            let dynamic_server = DynamicUpdateHandler::new(auth_server.zones());
            let resolver = resolver::Resolver::new(auth_server, &config);
            let server = Server::new(&config.server, resolver);

            let addr_and_port = config.vg_ctrl.address.split(":").collect::<Vec<&str>>();
            let _handler = dynamic_server.run(
                addr_and_port[0].to_string(),
                addr_and_port[1].parse().unwrap(),
            );

            let addr = config.metrics.address.parse::<SocketAddr>().unwrap();
            start_metrics(addr);

            tokio::run(server.into_future());
        }
    }
}

fn start_metrics(addr: SocketAddr) {
    thread::Builder::new()
        .name("metrics".into())
        .spawn(move || {
            let mut rt = current_thread::Runtime::new().unwrap();
            rt.spawn(start_metric_server(
                addr,
                "/metrics".to_string(),
                "/statistics".to_string(),
            ));
            rt.block_on(start_qps_calculate()).unwrap();
        })
        .unwrap();
}
