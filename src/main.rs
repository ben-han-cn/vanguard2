use vanguard2::config::VanguardConfig;
use vanguard2::resolver::Resolver;
use vanguard2::server::Server;

use clap::{App, Arg};
use tokio::runtime::Runtime;

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
    let config = VanguardConfig::load_config(config_file).unwrap();
    let resolver = Resolver::new(&config);
    let server = Server::new(&config.server);
    let rt = Runtime::new().unwrap();
    rt.block_on(server.run(resolver));
}
