#![allow(dead_code)]

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate reqwest;
extern crate unqlite;
// https://github.com/seanmonstar/reqwest/issues/11
#[macro_use]
extern crate hyper;

mod rpc;
mod rutracker;

use rpc::TorrentClient;

fn main() {
    let mut torrent_client = rpc::Transmission::new("192.168.1.104:9091", None).unwrap();
    let t_list = torrent_client.list().unwrap();
    println!("{:?}", t_list);
    println!("{:?}", t_list[0]);
    torrent_client.start(&vec![t_list[0].get_hash()]).unwrap();
}
