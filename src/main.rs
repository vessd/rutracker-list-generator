#![allow(dead_code)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

#[macro_use]
extern crate hyper;
//https://github.com/seanmonstar/reqwest/issues/11

extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate unqlite;

mod rpc;
mod rutracker;
mod torrent;

use rutracker::RutrackerApi;
use torrent::TorrentList;
use std::io::Write;

const APIURL: &str = "https://api.t-ru.org/";
const FORUMURL: &str = "https://rutracker.cr/forum/";
const TRANSMISSION: &str = "http://127.0.0.1:9091/transmission/rpc/";

fn print_flush(s: &str) {
    print!("{}", s);
    std::io::stdout().flush().expect("flush");
}

fn main() {
    print_flush("Соединение с Rutracker API...");
    let api = RutrackerApi::new(APIURL).unwrap();
    println!("готово");
    print_flush("Соединение с Transmission...");
    let mut torrent_client = rpc::Transmission::new(TRANSMISSION, None).unwrap();
    println!("готово");
    let list = TorrentList::new(&mut torrent_client, &api).unwrap();
    println!("{:?}", list);
}
