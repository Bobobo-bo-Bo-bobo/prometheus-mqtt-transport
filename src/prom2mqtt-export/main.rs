mod config;
mod constants;
mod data;
mod exporter;
mod http;
mod mqtt_sub;
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

    let configuration = match config::parse_config_file(&config_file) {
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

    debug!("registering internal metrics");
    exporter::register();

    let (data_send, data_recv) = mpsc::channel::<data::Data>();
    let (http_send, http_recv) = mpsc::channel::<String>();

    // Spawn threads
    let data_thread_id = thread::spawn(move || {
        match data::handler(data_recv, http_send) {
            Ok(_) => {
                process::exit(0);
            }
            Err(e) => {
                panic!("{}", e);
            }
        };
    });
    let dsc = data_send.clone();
    let cfg = configuration.clone();
    let mqtt_thread_id = thread::spawn(move || match mqtt_sub::run(&cfg, dsc) {
        Ok(_) => {
            process::exit(0);
        }
        Err(e) => {
            panic!("{}", e);
        }
    });
    let http_thread_id = thread::spawn(move || {
        match http::run(&configuration, data_send.clone(), http_recv) {
            Ok(_) => {
                process::exit(0);
            }
            Err(e) => {
                panic!("{}", e);
            }
        };
    });

    data_thread_id.join().unwrap();
    http_thread_id.join().unwrap();
    mqtt_thread_id.join().unwrap();
}
