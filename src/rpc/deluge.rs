use rpc::{self, TorrentClient, TorrentStatus};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Deluge;

impl Deluge {
    pub fn new() -> Self {
        Deluge
    }
}

impl TorrentClient for Deluge {
    fn list(&mut self) -> rpc::Result<HashMap<String, TorrentStatus>> {
        unimplemented!();
    }
    fn start(&mut self, _hashes: &[&str]) -> rpc::Result<()> {
        unimplemented!();
    }
    fn stop(&mut self, _hashes: &[&str]) -> rpc::Result<()> {
        unimplemented!();
    }
    fn remove(&mut self, _hashes: &[&str], _delete: bool) -> rpc::Result<()> {
        unimplemented!();
    }
}
