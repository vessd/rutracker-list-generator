#![allow(dead_code)]
#![feature(plugin)]
#![plugin(clippy)]
//#![allow(cyclomatic_complexity)]

extern crate encoding;
// https://github.com/seanmonstar/reqwest/issues/11
#[macro_use]
extern crate hyper;
extern crate kuchiki;
extern crate lmdb;
#[macro_use]
extern crate slog;
extern crate slog_async;
#[macro_use]
extern crate slog_scope;
extern crate slog_term;
#[macro_use]
extern crate quick_error;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
//#[macro_use]
//extern crate text_io;
extern crate bincode;
extern crate chrono;
// https://github.com/seanmonstar/reqwest/issues/14
extern crate cookie;
extern crate toml;

mod client;
mod config;
mod control;
mod database;
mod log;
mod report;
mod rutracker;

use config::{ClientName, Config};
use control::Control;
use report::{Report, SummaryReport};
use rutracker::{RutrackerApi, RutrackerForum, User};

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let config = Config::from_file("rlg.toml")?;
    let _guard = slog_scope::set_global_logger(log::init(&config.log)?);

    info!("Подключение к базе данных...");
    let database = match database::Database::new() {
        Ok(db) => db,
        Err(err) => {
            crit!("Подключение к базе данных завершилось с ошибкой: {}", err);
            return Ok(1);
        }
    };
    database.clear_db(database::DBName::SubforumList)?;
    database.clear_db(database::DBName::TopicInfo)?;
    database.clear_db(database::DBName::LocalList)?;

    info!("Соединение с Rutracker API...");
    let api = match RutrackerApi::new(config.api_url.as_str(), &database) {
        Ok(api) => api,
        Err(err) => {
            crit!("Соединение с Rutracker API завершилось с ошибкой: {}", err);
            return Ok(1);
        }
    };

    info!("Запрос списка имеющихся раздач...");
    let mut control = Control::new(&api, &database, config.dry_run);
    for c in &config.client {
        match c.client {
            ClientName::Deluge => control.add_client(Box::new(client::Deluge::new()))?,
            ClientName::Transmission => control.add_client(Box::new(client::Transmission::new(
                c.address.as_str(),
                None,
            )?))?,
        }
    }
    control.set_status(client::TorrentStatus::Other, &config.ignored_id);
    info!("Приминение настроек...");
    for f in &config.subforum {
        control.apply_config(f);
    }
    control.save_torrents()?;
    let forum = if let Some(forum) = config.forum {
        match User::new(&forum) {
            Ok(user) => Some(RutrackerForum::new(user, &forum)?),
            Err(rutracker::forum::Error::User) => None,
            Err(err) => {
                error!("RutrackerForum::new {}", err);
                None
            }
        }
    } else {
        None
    }.unwrap();
    let report = Report::new(1105, &api, &database)?;
    let report2 = Report::new(1105, &api, &database)?;
    let mut sumrep = SummaryReport::new(&database, &forum);
    sumrep.add_report(1105, report);
    sumrep.add_report(1105, report2);
    Ok(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exit_code = run()?;
    std::process::exit(exit_code);
}
