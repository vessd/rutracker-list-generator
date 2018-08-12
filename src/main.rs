#![allow(dead_code)]

extern crate bincode;
extern crate chrono;
extern crate cookie;
extern crate encoding_rs;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate hyper;
extern crate kuchiki;
extern crate lmdb;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate slog;
extern crate slog_async;
#[macro_use]
extern crate slog_scope;
extern crate slog_term;
extern crate toml;
extern crate url;

#[macro_use]
mod log;

mod client;
mod config;
mod control;
mod database;
mod report;
mod rutracker;

use config::{ClientName, Config};
use control::Control;
use report::{List, Report};
use rutracker::{RutrackerApi, RutrackerForum};

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

    info!("Подключение к базе данных...");
    let database = crit_try!(
        database::Database::new(api),
        "Подключение к базе данных завершилось с ошибкой: {}"
    );

    info!("Запрос списка имеющихся раздач...");
    let mut control = Control::new(&database, config.dry_run);
    for c in &config.client {
        let user = c.user.clone().map(|u| (u.name, u.password));
        error_try!(
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
            continue,
            "Получение списка раздач из клиента завершилось с ошибкой: {}"
        );
    }
    control.set_status(client::TorrentStatus::Other, &config.ignored_id);
    info!("Приминение настроек...");
    for f in &config.subforum {
        control.apply_config(f);
    }
    crit_try!(
        control.save_torrents(),
        "Сохранение списка локальных раздач в базу данных завершилось с ошибкой: {}"
    );
    if let Some(forum) = config.forum {
        info!("Авторизация на форуме...");
        let forum = crit_try!(
            RutrackerForum::new(&forum),
            "Авторизация на форуме завершилась с ошибкой: {}"
        );
        info!("Сборка сводного отчёта...");
        let mut report = Report::new(&database, &forum);
        for subforum in &config.subforum {
            for id in &subforum.ids {
                let topic_id: Vec<usize> = error_try!(
                        database.get_local_by_forum(*id),
                        continue,
                        "Для подраздела {1} не удалось получить списко хранимых раздач: {}",
                        id
                    ).into_iter()
                    .filter(|(_, status)| *status == client::TorrentStatus::Seeding)
                    .map(|(id, _)| id)
                    .collect();
                if topic_id.is_empty() {
                    warn!(
                        "Список хранимых раздач для подраздела {} пуст",
                        id
                    );
                }
                let list = error_try!(
                    List::new(&database, *id, topic_id),
                    continue,
                    "Для хранимых раздач из подраздела {1} не удалось получить информацию: {}",
                    id
                );
                report.add_list(list);
            }
        }
        info!("Отправка списков на форум...");
        crit_try!(report.send_all(), "Не удалось отправить списки хранимых раздач на форум: {}");
    }
    info!("Готово!");
    0
}

fn main() {
    let exit_code = run();
    std::process::exit(exit_code);
}
