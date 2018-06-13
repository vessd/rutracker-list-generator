use client::{self, Torrent, TorrentClient, TorrentStatus};
use config::ForumConfig;
use database::{self, DBName, Database};
use rutracker::api::{self, RutrackerApi, TopicInfo};
use std::collections::{HashMap, HashSet};

pub type Result<T> = ::std::result::Result<T, Error>;
type TopicInfoMap = HashMap<usize, TopicInfo>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Api(err: api::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Client(err: client::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Database(err: database::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

#[derive(Debug)]
struct Client {
    list: HashMap<usize, Torrent>,
    client: Box<TorrentClient>,
}

impl Client {
    fn start(&mut self, stop: usize, topics: &TopicInfoMap) {
        let mut ids = Vec::with_capacity(topics.len());
        {
            let mut buf = Vec::with_capacity(topics.len());
            for (id, info) in topics {
                if info.seeders < stop {
                    if let Some(torrent) = self.list.get(id) {
                        if torrent.status == TorrentStatus::Stopped {
                            buf.push(torrent.hash.as_str());
                            ids.push(id);
                        }
                    }
                }
            }
            match self.client.start(&buf) {
                Ok(()) => (),
                Err(err) => {
                    error!("{}", err);
                    return;
                }
            }
        }
        for id in ids {
            self.list.get_mut(id).unwrap().status = TorrentStatus::Seeding;
        }
    }

    fn stop(&mut self, remove: usize, stop: usize, topics: &TopicInfoMap) {
        let mut ids = Vec::with_capacity(topics.len());
        {
            let mut buf = Vec::with_capacity(topics.len());
            for (id, info) in topics {
                if info.seeders < remove && info.seeders >= stop {
                    if let Some(torrent) = self.list.get(id) {
                        if torrent.status == TorrentStatus::Seeding {
                            buf.push(torrent.hash.as_str());
                            ids.push(id);
                        }
                    }
                }
            }
            match self.client.stop(&buf) {
                Ok(()) => (),
                Err(err) => {
                    error!("{}", err);
                    return;
                }
            }
        }
        for id in ids {
            self.list.get_mut(id).unwrap().status = TorrentStatus::Stopped;
        }
    }

    fn remove(&mut self, remove: usize, topics: &TopicInfoMap) {
        let mut ids = Vec::with_capacity(topics.len());
        {
            let mut buf = Vec::with_capacity(topics.len());
            for (id, info) in topics {
                if info.seeders >= remove {
                    if let Some(torrent) = self.list.get(id) {
                        if torrent.status != TorrentStatus::Other {
                            buf.push(torrent.hash.as_str());
                            ids.push(id);
                        }
                    }
                }
            }
            match self.client.remove(&buf, true) {
                Ok(()) => (),
                Err(err) => {
                    error!("{}", err);
                    return;
                }
            }
        }
        for id in ids {
            self.list.remove(id);
        }
    }
}

#[derive(Debug)]
pub struct Control<'a> {
    clients: Vec<Client>,
    api: &'a RutrackerApi<'a>,
    db: &'a Database,
}

impl<'a> Control<'a> {
    pub fn new(api: &'a RutrackerApi, db: &'a Database) -> Self {
        Control {
            clients: Vec::new(),
            api,
            db,
        }
    }

    fn iter(&self) -> ::std::slice::Iter<Client> {
        self.clients.iter()
    }

    fn iter_mut(&mut self) -> ::std::slice::IterMut<Client> {
        self.clients.iter_mut()
    }

    fn torrent_ids(&self) -> HashSet<usize> {
        let mut set: HashSet<usize> = HashSet::new();
        for client in self.iter() {
            set.extend(client.list.keys().cloned());
        }
        set
    }

    pub fn add_client(&mut self, client: Box<TorrentClient>) -> Result<()> {
        let list = client.list()?;
        trace!("Control::add_client::list: {:?}", list);
        let mut ids = self.api.get_topic_id(
            &list.iter().map(|t| &t.hash).collect::<Vec<&String>>(),
            None,
        )?;
        trace!("Control::add_client::ids: {:?}", ids);
        let client = Client {
            list: list
                .into_iter()
                .filter_map(|torrent| Some((ids.remove(&torrent.hash)?, torrent)))
                .collect(),
            client,
        };
        trace!("Control::add_client::client: {:?}", client);
        self.clients.push(client);
        Ok(())
    }

    pub fn start(&mut self, stop: usize, topics: &TopicInfoMap) {
        self.iter_mut().for_each(|c| c.start(stop, topics))
    }

    pub fn stop(&mut self, remove: usize, stop: usize, topics: &TopicInfoMap) {
        self.iter_mut().for_each(|c| c.stop(remove, stop, topics))
    }

    pub fn remove(&mut self, remove: usize, topics: &TopicInfoMap) {
        self.iter_mut().for_each(|c| c.remove(remove, topics))
    }

    fn get_topic_info(&self, forum_id: usize) -> Result<TopicInfoMap> {
        self.api.pvc(forum_id)?;
        let forum_list: HashSet<usize> = self.db.get(DBName::ForumList, &forum_id)?.unwrap();
        let keys: Vec<usize> = forum_list
            .intersection(&self.torrent_ids())
            .cloned()
            .collect();
        let topics = self.db.get_map(DBName::TopicInfo, keys)?;
        Ok(topics
            .into_iter()
            .filter_map(|(k, v)| Some((k, v?)))
            .collect())
    }

    pub fn apply_config(&mut self, forum: &ForumConfig) {
        for id in &forum.ids {
            let topics = match self.get_topic_info(*id) {
                Ok(t) => t,
                Err(err) => {
                    error!("{}", err);
                    continue;
                }
            };
            if forum.remove != 0 {
                self.remove(forum.remove, &topics);
            }
            self.stop(forum.remove, forum.stop, &topics);
            self.start(forum.stop, &topics);
        }
    }
}
