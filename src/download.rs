use chrono::naive::NaiveDateTime;
use chrono::Local;
use database::Database;
use rutracker::RutrackerForum;
use std::collections::HashMap;
use std::collections::HashSet;

type Result<T> = std::result::Result<T, failure::Error>;

pub struct Downloader<'a> {
    db: &'a Database,
    forum: &'a RutrackerForum,
    ignored_id: Vec<usize>,
}

impl<'a> Downloader<'a> {
    pub fn new(db: &'a Database, forum: &'a RutrackerForum, ignored_id: Vec<usize>) -> Self {
        Self {
            db,
            forum,
            ignored_id,
        }
    }

    pub fn get_list_for_download(
        &self,
        forum_id: usize,
        download: i16,
    ) -> Result<HashMap<usize, (String, usize)>> {
        /* let date = Local::now().naive_local();
        let num_days = |time: NaiveDateTime| date.signed_duration_since(time).num_days();
        let check_reg_time_and_status = |status: i16, time: NaiveDateTime| {
            ([2, 3, 8].contains(&status) && num_days(time) > 30)
                || ([0, 10].contains(&status) && num_days(time) > 90)
        };
        let keeper_list: HashMap<String, Vec<usize>> =
            self.db.get_by_filter(DBName::KeeperList, |_, _| true)?;
        let keeper_list: HashSet<usize> = keeper_list.into_iter().flat_map(|(_, v)| v).collect();
        let topic_id: HashMap<_, _> = self
            .db
            .pvc(forum_id, None::<&[usize]>)?
            .into_iter()
            .filter(|(_, v)| v.seeders <= download)
            .filter(|(_, v)| check_reg_time_and_status(v.tor_status, v.reg_time))
            .filter(|(id, _)| !self.ignored_id.contains(id) && !keeper_list.contains(id))
            .collect(); */
        Ok(HashMap::new())
    }
}
