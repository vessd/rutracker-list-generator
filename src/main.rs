#![allow(dead_code)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

//https://github.com/seanmonstar/reqwest/issues/11
#[macro_use]
extern crate hyper;
#[macro_use]
extern crate quick_error;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate toml;
extern crate unqlite;

mod rpc;
mod rutracker;
mod torrent;
mod config;

use rutracker::RutrackerApi;
use torrent::TorrentList;
use config::Config;
use std::io::Write;
use std::env;

const APIURL: &str = "https://api.t-ru.org/";
const FORUMURL: &str = "https://rutracker.cr/forum/";
const TRANSMISSION: &str = "http://127.0.0.1:9091/transmission/rpc/";

fn print_flush(s: &str) {
    print!("{}", s);
    std::io::stdout().flush().expect("flush");
}

fn main() {
    let config = if let Some(f) = env::args().nth(1) {
        Config::from_file(f).unwrap()
    } else {
        Config::default()
    };
    println!("{:?}", config);
    /* print_flush("Соединение с Rutracker API...");
    let api = RutrackerApi::new(APIURL).unwrap();
    println!("готово");
    print_flush("Соединение с Transmission...");
    let mut torrent_client = rpc::Transmission::new(TRANSMISSION, None).unwrap();
    println!("готово");
    let list = TorrentList::new(&mut torrent_client, &api).unwrap();
    println!("{:?}", list); */
}
