//! A module that provides an interface for torrent clients

mod deluge;
mod transmission;

pub use self::deluge::Deluge;
pub use self::transmission::Transmission;

use self::transmission::{ArgGet, DeleteLocalData, TorrentSelect, TorrentStatus as TStatus};

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Transmission(err: self::transmission::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Deluge(err: self::deluge::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

/// Torrent
#[derive(Debug)]
pub struct Torrent {
    pub hash: String,
    pub status: TorrentStatus,
}

/// Torrent status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TorrentStatus {
    Seeding,
    Stopped,
    Other,
}

/// A trait for any object that will represent a torrent client.
pub trait TorrentClient: ::std::fmt::Debug {
    /// Returns a list of all torrents in the client.
    fn list(&mut self) -> Result<Vec<Torrent>>;
    /// Starts a list of torrents.
    fn start(&mut self, &[&str]) -> Result<()>;
    /// Stop a list of torrents.
    fn stop(&mut self, &[&str]) -> Result<()>;
    /// Remove a list of torrents from client.
    ///
    /// If the second parameter is true, then it also removes local data.
    fn remove(&mut self, &[&str], bool) -> Result<()>;
}

impl From<TStatus> for TorrentStatus {
    fn from(status: TStatus) -> TorrentStatus {
        match status {
            TStatus::Seeding => TorrentStatus::Seeding,
            TStatus::TorrentIsStopped => TorrentStatus::Stopped,
            _ => TorrentStatus::Other,
        }
    }
}

impl TorrentClient for Transmission {
    fn list(&mut self) -> Result<Vec<Torrent>> {
        Ok(self
            .get(TorrentSelect::All, &[ArgGet::HashString, ArgGet::Status])?
            .into_iter()
            .map(|resp| Torrent {
                hash: resp.hash.to_uppercase(),
                status: resp.status.into(),
            })
            .collect())
    }
    fn start(&mut self, hashes: &[&str]) -> Result<()> {
        self.start(TorrentSelect::Ids(hashes)).map_err(From::from)
    }
    fn stop(&mut self, hashes: &[&str]) -> Result<()> {
        self.stop(TorrentSelect::Ids(hashes)).map_err(From::from)
    }
    fn remove(&mut self, hashes: &[&str], delete: bool) -> Result<()> {
        self.remove(TorrentSelect::Ids(hashes), DeleteLocalData(delete))
            .map_err(From::from)
    }
}

impl TorrentClient for Deluge {
    fn list(&mut self) -> Result<Vec<Torrent>> {
        unimplemented!();
    }
    fn start(&mut self, _hashes: &[&str]) -> Result<()> {
        unimplemented!();
    }
    fn stop(&mut self, _hashes: &[&str]) -> Result<()> {
        unimplemented!();
    }
    fn remove(&mut self, _hashes: &[&str], _delete: bool) -> Result<()> {
        unimplemented!();
    }
}
