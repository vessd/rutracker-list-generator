//! A module that provides an implementation of the torrent client interface for the Transmission.

use rpc::{self, TorrentClient, TorrentStatus};
use super::{ArgGet, DeleteLocalData, TorrentSelect, Transmission};
use std::collections::HashMap;

impl From<super::TorrentStatus> for TorrentStatus {
    fn from(status: super::TorrentStatus) -> TorrentStatus {
        match status {
            super::TorrentStatus::Seeding => TorrentStatus::Seeding,
            super::TorrentStatus::TorrentIsStopped => TorrentStatus::Stopped,
            _ => TorrentStatus::Other,
        }
    }
}

impl TorrentClient for Transmission {
    fn list(&mut self) -> rpc::Result<HashMap<String, TorrentStatus>> {
        Ok(
            self.get(TorrentSelect::All, &[ArgGet::HashString, ArgGet::Status])?
                .into_iter()
                .map(|resp| {
                    (resp.hash.to_uppercase(), TorrentStatus::from(resp.status))
                })
                .collect(),
        )
    }
    fn start(&mut self, hashes: &[&str]) -> rpc::Result<()> {
        Ok(self.start(TorrentSelect::Ids(hashes))?)
    }
    fn stop(&mut self, hashes: &[&str]) -> rpc::Result<()> {
        Ok(self.stop(TorrentSelect::Ids(hashes))?)
    }
    fn remove(&mut self, hashes: &[&str], delete: bool) -> rpc::Result<()> {
        Ok(self.remove(TorrentSelect::Ids(hashes), DeleteLocalData(delete))?)
    }
}
