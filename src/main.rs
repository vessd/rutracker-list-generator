#![allow(dead_code)]
#![feature(plugin)]
#![plugin(clippy)]
#![allow(cyclomatic_complexity)]

extern crate encoding;
// https://github.com/seanmonstar/reqwest/issues/11
#[macro_use]
extern crate hyper;
extern crate kuchiki;
extern crate lmdb;
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
#[macro_use]
extern crate text_io;
extern crate bincode;
extern crate chrono;
// https://github.com/seanmonstar/reqwest/issues/14
extern crate cookie;
extern crate toml;

mod client;
mod config;
mod control;
mod database;
mod report;
mod rutracker;

use config::{ClientName, Config};
use control::Control;
use rutracker::{RutrackerApi, RutrackerForum};
use std::collections::HashMap;
use std::fs::File;
// https://github.com/Drakulix/simplelog.rs/issues/3
use simplelog::{Level, LevelFilter, SimpleLogger, TermLogger, WriteLogger};

fn init_log(log_level: usize, log_file: Option<&String>) {
    let log_level = match log_level {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let log_config = simplelog::Config {
        time: Some(Level::Error),
        level: Some(Level::Error),
        target: Some(Level::Error),
        location: Some(Level::Debug),
        time_format: Some("%T"),
    };
    if let Some(file) = log_file {
        match File::create(file) {
            Ok(f) => match WriteLogger::init(log_level, log_config, f) {
                Ok(()) => (),
                Err(e) => {
                    match TermLogger::init(LevelFilter::Error, log_config) {
                        Ok(()) => (),
                        Err(e) => if SimpleLogger::init(LevelFilter::Error, log_config).is_err() {
                            println!("couldn't init any logger");
                        } else {
                            error!("{}", e);
                        },
                    }
                    error!("{}", e);
                }
            },
            Err(e) => {
                match TermLogger::init(LevelFilter::Error, log_config) {
                    Ok(()) => (),
                    Err(e) => if SimpleLogger::init(LevelFilter::Error, log_config).is_err() {
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
            Err(e) => if SimpleLogger::init(LevelFilter::Error, log_config).is_err() {
                println!("couldn't init any logger");
            } else {
                error!("{}", e);
            },
        }
    }
}

fn main() -> Result<(), Box<std::error::Error>> {
    let config = Config::from_file("rlg.toml")?;
    init_log(config.log_level, config.log_file.as_ref());
    let database = database::Database::new()?;

    info!("Соединение с Rutracker API...");
    let api = RutrackerApi::new(config.api_url.as_str(), &database)?;
    let mut control = Control::new(&api, &database);
    info!("Запрос списка имеющихся раздач...");
    for r in &config.client {
        trace!("config.client: {:?}", r);
        match r.client {
            ClientName::Deluge => control.add_client(Box::new(client::Deluge::new()))?,
            ClientName::Transmission => control.add_client(Box::new(client::Transmission::new(
                r.address.as_str(),
                None,
            )?))?,
        }
    }
    /*trace!("list: {:?}", list);
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
    } */
    Ok(())
}
