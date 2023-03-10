mod config;
mod constants;
mod exporter;
mod http;
mod massage;
mod mqtt_sender;
mod scrape;
mod usage;

use getopts::Options;
use log::{debug, error, info};
use std::sync::mpsc;
use std::thread;
use std::{env, process};

fn main() {
    let argv: Vec<String> = env::args().collect();
    let mut options = Options::new();
    let mut log_level = log::LevelFilter::Info;

    options.optflag("C", "check", "Check configuration file and exit");
    options.optflag("D", "debug", "Enable debug logs");
    options.optflag("V", "version", "Show version information");
    options.optflag("h", "help", "Show help text");
    options.optopt(
        "c",
        "config",
        "Configuration file",
        constants::DEFAULT_CONFIG_FILE,
    );
    options.optflag("q", "quiet", "Quiet operation");

    let opts = match options.parse(&argv[1..]) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: Can't parse command line arguments: {}", e);
            println!();
            usage::show_usage();
            process::exit(1);
        }
    };

    if opts.opt_present("h") {
        usage::show_usage();
        process::exit(0)
    };

    if opts.opt_present("V") {
        global::usage::show_version();
        process::exit(0);
    };

    if opts.opt_present("D") {
        log_level = log::LevelFilter::Debug;
    };

    if opts.opt_present("q") {
        log_level = log::LevelFilter::Warn;
    };

    let config_file = match opts.opt_str("c") {
        Some(v) => v,
        None => constants::DEFAULT_CONFIG_FILE.to_string(),
    };

    // XXX: Should never fail
    debug!("initialising logging");
    global::logging::init(log_level).unwrap();

    debug!("parsing configuration file {}", config_file);
    let mut configuration = match config::parse_config_file(&config_file) {
        Ok(v) => v,
        Err(e) => {
            error!("error while parsing configuration file: {}", e);
            process::exit(1);
        }
    };
    if opts.opt_present("C") {
        info!("valid configuration file");
        process::exit(0);
    }
    debug!("final configuration: {:?}", configuration);

    debug!("registering internal Prometheus metrics");
    exporter::register();

    let (send, receive) = mpsc::channel::<Vec<u8>>();
    let cfg = configuration.clone();
    debug!("spawning MQTT sender thread");
    thread::spawn(move || {
        if let Err(e) = mqtt_sender::run(&cfg, receive) {
            error!("can't start MQTT sender thread: {}", e);
            process::exit(1);
        }
    });

    debug!("spawning HTTP server for internal Prometheus metrics");
    let cfg = configuration.clone();
    thread::spawn(move || {
        if let Err(e) = http::run(&cfg) {
            error!(
                "can't start HTTP server for internal Prometheus metrics: {}",
                e
            );
            process::exit(1);
        }
    });

    // scrape loop
    if let Err(e) = scrape::run(&mut configuration, send) {
        error!("can't start scraping process: {}", e);
        process::exit(1);
    }
}
