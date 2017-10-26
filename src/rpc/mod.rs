//! A module that provides an interface for torrent clients

mod deluge;
mod transmission;

pub use self::transmission::Transmission;
use std::collections::HashMap;

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error!{
    #[derive(Debug)]
    pub enum Error {
        Transmission(err: self::transmission::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

/// Torrent status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TorrentStatus {
    Seeding,
    Stopped,
    Other,
}

/// A trait for any object that will represent a torrent client.
pub trait TorrentClient {
    /// Returns a list of all torrents in the client.
    fn list(&mut self) -> Result<HashMap<String, TorrentStatus>>;
    /// Starts a list of torrents.
    fn start(&mut self, &[&str]) -> Result<()>;
    /// Stop a list of torrents.
    fn stop(&mut self, &[&str]) -> Result<()>;
    /// Remove a list of torrents from client.
    ///
    /// If the second parameter is true, then it also removes local data.
    fn remove(&mut self, &[&str], bool) -> Result<()>;
}
