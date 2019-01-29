#![allow(dead_code)]
#![warn(rust_2018_idioms)]
//#![warn(clippy::pedantic)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;

#[macro_use]
mod log;

mod client;
mod config;
mod control;
mod database;
//mod download;
mod report;
mod rutracker;

use crate::config::{ClientName, Config};
use crate::control::Control;
//use download::Downloader;
use crate::report::Report;
use crate::rutracker::{RutrackerApi, RutrackerForum};

fn run() -> i32 {
    let guard = slog_scope::set_global_logger(log::pre_init());
    let config = crit_try!(
        Config::from_file("rlg.toml"),
        "Ошибка при чтении файла: {}"
    );
    let logger = crit_try!(
        log::init(&config.log),
        "Не удалось инициализировать логгер: {}"
    );
    drop(guard);
    let _guard = slog_scope::set_global_logger(logger);

    info!("Соединение с Rutracker API...");
    let api = crit_try!(
        RutrackerApi::new(config.api_url.as_str()),
        "Соединение с Rutracker API завершилось с ошибкой: {}"
    );

    info!("Авторизация на форуме...");
    let forum = crit_try!(
        RutrackerForum::new(&config.forum, config.dry_run),
        "Авторизация на форуме завершилась с ошибкой: {}"
    );

    info!("Подключение к базе данных...");
    let database = crit_try!(
           database::Database::new(api, forum),
           "Подключение к базе данных завершилось с ошибкой: {}"
       );

    info!("Запрос списка имеющихся раздач...");
    let mut control = Control::new(&database, config.dry_run);
    for c in &config.client {
        let user = c.user.clone().map(|u| (u.name, u.password));
        crit_try!(
               control.add_client(match c.name {
                   ClientName::Transmission => {
                       let url = format!("http://{}:{}/transmission/rpc", c.host, c.port);
                       Box::new(error_try!(
                       client::Transmission::new(url.as_str(), user),
                       continue,
                       "Подключение к Transmission завершилось с ошибкой: {}"
                   ))
                   }
                   ClientName::Deluge => Box::new(error_try!(
                       client::Deluge::new(),
                       continue,
                       "Подключение к Deluge завершилось с ошибкой: {}"
                   )),
               }),
               "Получение списка раздач из клиента завершилось с ошибкой: {}"
           );
    }
    crit_try!(database.set_status_by_id(client::TorrentStatus::Other as i16, &config.ignored_id),
              "Не удалось изменить статус для игнорируемых торрентов: {}");

    info!("Приминение настроек...");
    for f in &config.subforum {
        control.apply_config(f);
    }

    info!("Сборка сводного отчёта...");
    let forum_id = config
        .subforum
        .iter()
        .flat_map(|f| f.id.iter().cloned())
        .collect();
    let report = Report::new(&database, forum_id);

    info!("Отправка списков на форум...");
    crit_try!(report.send_all(), "Не удалось отправить списки хранимых раздач на форум: {}");

    /* info!("Формирование списка раздач для загрузки...");
    let downloader = Downloader::new(&database, &forum, config.ignored_id.to_vec());
    downloader.get_list_for_download(281, 2).unwrap(); */

    info!("Готово!");
    0
}

fn main() {
    let exit_code = run();
    std::process::exit(exit_code);
}
