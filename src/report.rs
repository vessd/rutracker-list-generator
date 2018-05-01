use chrono::Local;
use rutracker::api::Data;
use std::collections::HashMap;
use torrent::Torrent;

#[derive(Debug)]
struct ForumData {
    name: String,
    url: String,
    torrent: HashMap<usize, Data>,
    local_torrent: HashMap<usize, Torrent>,
}

impl ForumData {
    fn new(name: String, url: String, torrent: HashMap<usize, Data>, local_torrent: HashMap<usize, Torrent>) -> Self {
        ForumData {
            name,
            url,
            torrent,
            local_torrent,
        }
    }
}

#[derive(Debug)]
pub struct Report {
    forum: HashMap<usize, ForumData>,
    date: String,
}

impl Report {
    pub fn new() -> Self {
        let date = Local::today();
        let report = Report {
            forum: HashMap::new(),
            date: date.format("%d.%m.%Y").to_string(),
        };
        debug!("Report::new::report: {:?}", report);
        report
    }

    pub fn add_forum(&mut self, id: usize, name: String, list: HashMap<usize, Torrent>, topics: HashMap<usize, Data>) {
        let url = String::new(); //TODO
        self.forum.insert(id, ForumData::new(name, url, topics, list));
    }
}
