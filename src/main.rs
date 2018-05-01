#![allow(dead_code)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]
#![allow(cyclomatic_complexity)]

extern crate encoding;
// https://github.com/seanmonstar/reqwest/issues/11
#[macro_use]
extern crate hyper;
extern crate kuchiki;
extern crate lmdb_rs as lmdb;
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
// https://github.com/seanmonstar/reqwest/issues/14
extern crate chrono;
extern crate cookie;
extern crate toml;

mod config;
mod report;
mod rpc;
mod rutracker;
mod torrent;

use config::{Client, Config};
use lmdb::{DbFlags, EnvBuilder};
use rutracker::{RutrackerApi, RutrackerForum};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use torrent::TorrentList;
// https://github.com/Drakulix/simplelog.rs/issues/3
use simplelog::{Level, LevelFilter, SimpleLogger, TermLogger, WriteLogger};

fn init_log(log_level: usize, log_file: &Option<String>) {
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
    if let Some(ref file) = *log_file {
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

fn main() {
    /* let env = EnvBuilder::new().open("data", 0o755).unwrap();

    let db_handle = env.get_default_db(DbFlags::empty()).unwrap();
    let txn = env.new_transaction().unwrap();
    {
        let db = txn.bind(&db_handle); // get a database bound to this transaction

        let pairs = vec![("Albert", "Einstein"), ("Joe", "Smith"), ("Jack", "Daniels")];

        for &(name, surname) in &pairs {
            db.set(&surname, &name).unwrap();
        }
    }

    // Note: `commit` is choosen to be explicit as
    // in case of failure it is responsibility of
    // the client to handle the error
    match txn.commit() {
        Err(_) => panic!("failed to commit!"),
        Ok(_) => (),
    }

    let reader = env.get_reader().unwrap();
    let db = reader.bind(&db_handle);
    let name = db.get::<&str>(&"Smith").unwrap();
    println!("It's {} Smith", name); */
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
    if let Some(user_id) = config.user_id {
        if let Some(pass) = config.password.clone() {
            info!("Получаем имя пользователя...");
            let user = api.get_user_name(&[user_id]).unwrap().remove(&user_id).unwrap();
            info!("Соединяемся с форумом...");
            let _forum = RutrackerForum::new(&user, &pass, &config).unwrap();
            //let topic = forum.get_topic(3186974).unwrap();
            //println!("{:?}", topic.get_stored_torrents());
            //forum.reply_topic(5494807, "test").unwrap();
            //let _line: String = read!("{}\n");
        }
    }
    /*  let mut list = TorrentList::new(&api, &config.ignored_ids);
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
    } */
}
