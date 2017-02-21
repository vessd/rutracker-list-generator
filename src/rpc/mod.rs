mod deluge;
mod transmission;
mod error;

pub use self::error::{Error, Result};
pub use self::transmission::Transmission;

#[derive(Debug,Clone,Copy)]
pub enum TorrentStatus {
    Seeding,
    Stopped,
    Other,
}

#[derive(Debug,Clone)]
pub struct Torrent {
    hash: String,
    status: TorrentStatus,
}

impl Torrent {
    pub fn get_hash(&self) -> &str {
        self.hash.as_ref()
    }

    pub fn get_status(&self) -> TorrentStatus {
        self.status
    }
}

pub trait TorrentClient {
    fn is_sha1(hash: &str) -> bool {
        hash.len() == 40 && hash.chars().all(|c| c.is_digit(16))
    }
    fn list(&mut self) -> Result<Vec<Torrent>>;
    fn start(&mut self, &[&str]) -> Result<()>;
    fn stop(&mut self, &[&str]) -> Result<()>;
    fn remove(&mut self, &[&str], bool) -> Result<()>;
}
