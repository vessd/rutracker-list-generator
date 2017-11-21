#![allow(dead_code)]
#![allow(cyclomatic_complexity)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

// https://github.com/seanmonstar/reqwest/issues/11
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
extern crate time;
extern crate toml;
extern crate unqlite;

mod rpc;
mod rutracker;
mod torrent;
mod config;
mod report;

use rutracker::RutrackerApi;
use torrent::TorrentList;
use config::{Client, Config};
use std::fs::File;
use std::env;
use std::collections::HashMap;
// https://github.com/Drakulix/simplelog.rs/issues/3
use simplelog::{LogLevel, LogLevelFilter, SimpleLogger, TermLogger, WriteLogger};

fn init_log(log_level: usize, log_file: &Option<String>) {
    let log_level = match log_level {
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
    if let Some(ref file) = *log_file {
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
        f
    } else {
        String::from("rlg.toml")
    };
    let config = match Config::from_file(&config) {
        Ok(conf) => conf,
        Err(err) => {
            println!("[ERROR] {}: {}", config, err);
            std::process::exit(1)
        }
    };
    init_log(config.log_level, &config.log_file);
    info!("Соединение с Rutracker API...");
    let api = match RutrackerApi::new(config.api_url.as_str()) {
        Ok(api) => api,
        Err(err) => {
            error!("{}", err);
            std::process::exit(1)
        }
    };
    let mut list = TorrentList::new(&api, &config.ignored_ids);
    info!("Запрос списка имеющихся раздач...");
    for r in &config.rpc {
        trace!("config.rpc: {:?}", r);
        match r.client {
            Client::Deluge => match list.add_client(Box::new(rpc::Deluge::new())) {
                Ok(()) => (),
                Err(err) => error!("{}", err),
            },
            Client::Transmission => match rpc::Transmission::new(r.address.as_str(), None) {
                Ok(client) => match list.add_client(Box::new(client)) {
                    Ok(()) => (),
                    Err(err) => error!("{}", err),
                },
                Err(err) => error!("{}", err),
            },
        }
    }
    trace!("list: {:?}", list);
    let mut report = report::Report::new();
    for f in &config.forum {
        let mut forum_name = match api.get_forum_name(&f.forum_ids) {
            Ok(name) => name,
            Err(err) => {
                error!("{}", err);
                f.forum_ids
                    .iter()
                    .map(|&id| (id, Some(id.to_string())))
                    .collect()
            }
        };
        let mut forum_name: HashMap<usize, String> = forum_name
            .into_iter()
            .map(|(k, v)| (k, v.unwrap_or_else(|| k.to_string())))
            .collect();
        debug!("forum_name: {:?}", forum_name);
        for id in &f.forum_ids {
            let mut topics_stats = match api.pvc(*id) {
                Ok(stats) => stats,
                Err(err) => {
                    error!("{:?}", err);
                    continue;
                }
            };
            let forum_list = list.exec(*id, config.real_kill, f, &mut topics_stats);
            let topics_data = match api.get_tor_topic_data(&topics_stats
                .into_iter()
                .map(|(k, _)| k)
                .collect::<Vec<usize>>())
            {
                Ok(data) => data.into_iter()
                    .filter_map(|(id, some)| {
                        if let Some(data) = some {
                            Some((id, data))
                        } else {
                            None
                        }
                    })
                    .collect(),
                Err(err) => {
                    error!("{}", err);
                    HashMap::new()
                }
            };
            report.add_forum(*id, forum_name.remove(id).unwrap(), forum_list, topics_data);
        }
    }
}
