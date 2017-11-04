#![allow(dead_code)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

//https://github.com/seanmonstar/reqwest/issues/11
#[macro_use]
extern crate hyper;
#[macro_use]
extern crate log;
#[macro_use]
extern crate quick_error;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate simplelog;
extern crate toml;
extern crate unqlite;

mod rpc;
mod rutracker;
mod torrent;
mod config;

use rutracker::RutrackerApi;
use torrent::TorrentList;
use config::Config;
use std::fs::File;
use std::env;
use simplelog::{LogLevelFilter, SimpleLogger, TermLogger, WriteLogger, LogLevel};

fn init_log(config: &Config) {
    let log_level = match config.log_level {
        0 => LogLevelFilter::Off,
        1 => LogLevelFilter::Error,
        2 => LogLevelFilter::Warn,
        3 => LogLevelFilter::Info,
        4 => LogLevelFilter::Debug,
        _ => LogLevelFilter::Trace,
    };
    let log_config = simplelog::Config {
        time: Some(LogLevel::Error),
        level: Some(LogLevel::Error),
        target: Some(LogLevel::Error),
        location: Some(LogLevel::Debug),
    };
    if let Some(ref file) = config.log_file {
        match File::create(file) {
            Ok(f) => match WriteLogger::init(log_level, log_config, f) {
                Ok(()) => (),
                Err(e) => {
                    match TermLogger::init(LogLevelFilter::Error, log_config) {
                        Ok(()) => (),
                        Err(e) => if SimpleLogger::init(LogLevelFilter::Error, log_config).is_err() {
                            println!("couldn't init any logger");
                        } else {
                            error!("{}", e);
                        },
                    }
                    error!("{}", e);
                }
            },
            Err(e) => {
                match TermLogger::init(LogLevelFilter::Error, log_config) {
                    Ok(()) => (),
                    Err(e) => if SimpleLogger::init(LogLevelFilter::Error, log_config).is_err() {
                        println!("couldn't init any logger");
                    } else {
                        error!("{}", e);
                    },
                }
                error!("{}", e);
            }
        }
    } else {
        match TermLogger::init(log_level, log_config) {
            Ok(()) => (),
            Err(e) => if SimpleLogger::init(LogLevelFilter::Error, log_config).is_err() {
                println!("couldn't init any logger");
            } else {
                error!("{}", e);
            },
        }
    }
}

fn main() {
    let config = if let Some(f) = env::args().nth(1) {
        Config::from_file(f).unwrap()
    } else {
        Config::default()
    };
    init_log(&config);
    debug!("config: {:?}", config);
    info!("Соединение с Rutracker API...");
    let api = RutrackerApi::new(config.api_url.as_str()).unwrap();
    debug!("api: {:?}", api);
    info!("Соединение с Transmission...");
    let mut torrent_client = rpc::Transmission::new(config.rpc_address.as_str(), None).unwrap();
    debug!("torrent_client: {:?}", torrent_client);
    info!("Запрос списка раздач из клиента...");
    let list = TorrentList::new(&mut torrent_client, &api).unwrap();
    trace!("list: {:?}", list);
}
