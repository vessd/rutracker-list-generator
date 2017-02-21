use rpc::{self, Error, TorrentClient, Torrent, TorrentStatus};
use super::{Transmission, TorrentSelect, ArgGet, DeleteLocalData};

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
    fn list(&mut self) -> rpc::Result<Vec<Torrent>> {
        self.get(TorrentSelect::All,
                 &vec![ArgGet::HashString, ArgGet::Status])?
            .into_iter()
            .map(|resp| if Self::is_sha1(&resp.hash) {
                Ok(Torrent {
                    hash: resp.hash,
                    status: TorrentStatus::from(resp.status),
                })
            } else {
                Err(Error::NotSha1(resp.hash))
            })
            .collect()
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
