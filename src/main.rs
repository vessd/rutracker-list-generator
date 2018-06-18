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
mod log;
mod report;
mod rutracker;

use config::{ClientName, Config};
use control::Control;
use rutracker::{RutrackerApi, RutrackerForum};

fn main() -> Result<(), Box<std::error::Error>> {
    let config = Config::from_file("rlg.toml")?;
    let _guard = slog_scope::set_global_logger(log::init(&config.log)?);

    let database = database::Database::new()?;
    database.clear_db(database::DBName::ForumList)?;
    database.clear_db(database::DBName::TopicInfo)?;

    info!("Соединение с Rutracker API...");
    let api = RutrackerApi::new(config.api_url.as_str(), &database)?;
    let mut control = Control::new(&api, &database, config.dry_run);
    info!("Запрос списка имеющихся раздач...");
    for c in &config.client {
        match c.client {
            ClientName::Deluge => control.add_client(Box::new(client::Deluge::new()))?,
            ClientName::Transmission => control.add_client(Box::new(client::Transmission::new(
                c.address.as_str(),
                None,
            )?))?,
        }
    }
    control.set_status(client::TorrentStatus::Other, &config.ignored_ids);
    info!("Приминение настроек...");
    for f in &config.forum {
        control.apply_config(f);
    }
    Ok(())
}
