#![allow(dead_code)]

extern crate encoding_rs;
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
extern crate url;
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
        let user = c
            .user
            .as_ref()
            .map(|u| (u.name.clone(), u.password.clone()));
        match c.name {
            ClientName::Transmission => {
                let url = format!("http://{}:{}/transmission/rpc", c.host, c.port);
                control.add_client(Box::new(client::Transmission::new(url.as_str(), user)?))?;
            }
            ClientName::Deluge => {
                control.add_client(Box::new(client::Deluge::new()))?;
            }
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

    let mut sumrep = SummaryReport::new(&database, &api, &forum);
    for subforum in &config.subforum {
        for id in &subforum.ids {
            let topics_id = database
                .get_map(
                    database::DBName::LocalList,
                    database.get(database::DBName::SubforumList, id)?,
                )?
                .into_iter()
                .filter(|(_, status)| *status == Some(client::TorrentStatus::Seeding))
                .map(|(id, _)| id)
                .collect();
            let report = match Report::new(&api, *id, topics_id) {
                Ok(r) => r,
                Err(err) => {
                    error!("Report::new {}", err);
                    return Ok(1);
                }
            };
            sumrep.add_report(report);
        }
    }
    sumrep.send()?;
    Ok(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exit_code = run()?;
    std::process::exit(exit_code);
}
