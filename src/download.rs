use crate::database::models::Torrent;
use crate::{config::Subforum, database::Database};

type Result<T> = std::result::Result<T, failure::Error>;

#[derive(Debug, Clone, Copy)]
pub struct Downloader<'a> {
    db: &'a Database,
    ignored_id: &'a [i32],
}

impl<'a> Downloader<'a> {
    pub fn new(db: &'a Database, ignored_id: &'a [i32]) -> Self {
        Self { db, ignored_id }
    }

    pub fn get_suggestion(&self, forum: &Subforum) -> Result<Vec<Torrent>> {
        Ok(Vec::new())
    }
}
