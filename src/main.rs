use vanguard2::config::VanguardConfig;
use vanguard2::logger;
use vanguard2::resolver::Resolver;

use clap::{App, Arg};

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

    //when guard is dropped, logger will be cleaned
    let _guard = logger::init_logger();

    let config_file = matches.value_of("config").unwrap_or("vanguard.conf");
    let config = VanguardConfig::load_config(config_file).expect("config load failed");
    let resolver = Resolver::new(&config);
    resolver.run();
}
