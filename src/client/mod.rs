//! A module that provides an interface for torrent clients

mod deluge;
mod transmission;

pub use self::{deluge::Deluge, transmission::Transmission};

use self::transmission::{ArgGet, DeleteLocalData, TorrentSelect, TorrentStatus as TStatus};
use crate::database::models::{LocalTorrent, Status};
use std::fmt::Debug;

pub type Result<T> = std::result::Result<T, failure::Error>;

/// A trait for any object that will represent a torrent client.
pub trait TorrentClient: Debug {
    fn url(&self) -> &str;
    /// Returns a list of all torrents in the client.
    fn list(&self) -> Result<Vec<LocalTorrent<'_>>>;
    /// Starts a list of torrents.
    fn start(&self, _: &[String]) -> Result<()>;
    /// Stop a list of torrents.
    fn stop(&self, _: &[String]) -> Result<()>;
    /// Remove a list of torrents from client.
    ///
    /// If the second parameter is true, then it also removes local data.
    fn remove(&self, _: &[String], _: bool) -> Result<()>;
}

impl From<TStatus> for Status {
    fn from(status: TStatus) -> Self {
        match status {
            TStatus::Seeding => Status::Seeding,
            TStatus::TorrentIsStopped => Status::Stopped,
            _ => Status::Other,
        }
    }
}

impl TorrentClient for Transmission {
    fn url(&self) -> &str {
        self.url()
    }
    fn list(&self) -> Result<Vec<LocalTorrent<'_>>> {
        Ok(self
            .get(TorrentSelect::All, &[ArgGet::HashString, ArgGet::Status])?
            .into_iter()
            .map(|resp| LocalTorrent {
                hash: resp.hash.to_uppercase(),
                status: resp.status.into(),
                url: self.url().into(),
            })
            .collect())
    }
    fn start(&self, hashes: &[String]) -> Result<()> {
        self.start(TorrentSelect::Ids(hashes))?;
        Ok(())
    }
    fn stop(&self, hashes: &[String]) -> Result<()> {
        self.stop(TorrentSelect::Ids(hashes))?;
        Ok(())
    }
    fn remove(&self, hashes: &[String], delete: bool) -> Result<()> {
        self.remove(TorrentSelect::Ids(hashes), DeleteLocalData(delete))?;
        Ok(())
    }
}

impl TorrentClient for Deluge {
    fn url(&self) -> &str {
        unimplemented!();
    }
    fn list(&self) -> Result<Vec<LocalTorrent<'_>>> {
        unimplemented!();
    }
    fn start(&self, _hashes: &[String]) -> Result<()> {
        unimplemented!();
    }
    fn stop(&self, _hashes: &[String]) -> Result<()> {
        unimplemented!();
    }
    fn remove(&self, _hashes: &[String], _delete: bool) -> Result<()> {
        unimplemented!();
    }
}
