//! A module that provides an interface for torrent clients

mod deluge;
mod transmission;
mod error;

pub use self::error::{Error, Result};
pub use self::transmission::Transmission;

/// Torrent status.
#[derive(Debug,Clone,Copy)]
pub enum TorrentStatus {
    Seeding,
    Stopped,
    Other,
}

/// Hash and status of torrent
#[derive(Debug,Clone)]
pub struct Torrent {
    hash: String,
    status: TorrentStatus,
}

impl Torrent {
    /// Creates a new `Torrent` struct
    ///
    /// Fails if a hash is not valid sha1.
    pub fn new<H, S>(hash: H, status: S) -> Result<Torrent>
        where H: Into<String>,
              S: Into<TorrentStatus>
    {
        let hash = hash.into();
        if Torrent::is_sha1(hash.as_str()) {
            Ok(Torrent {
                hash: hash,
                status: status.into(),
            })
        } else {
            Err(Error::NotSha1(hash))
        }
    }

    /// Checks if a &str is sha1.
    fn is_sha1(hash: &str) -> bool {
        hash.len() == 40 && hash.chars().all(|c| c.is_digit(16))
    }

    /// Returns a reference to a hash of the torrent
    pub fn get_hash(&self) -> &str {
        self.hash.as_ref()
    }

    /// Returns a status of the torrent
    pub fn get_status(&self) -> TorrentStatus {
        self.status
    }
}

/// A trait for any object that will represent a torrent client.
pub trait TorrentClient {
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
