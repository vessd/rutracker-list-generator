use client::{self, Torrent, TorrentClient, TorrentStatus};
use config::ForumConfig;
use database::{self, DBName, Database};
use rutracker::api::{self, RutrackerApi, TopicInfo};
use slog::Drain;
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
    fn get_ids<T, U>(&self, topics: &TopicInfoMap, seeders: T, status: U) -> Vec<usize>
    where
        T: Fn(usize) -> bool,
        U: Fn(TorrentStatus) -> bool,
    {
        topics
            .iter()
            .filter(|(_, info)| seeders(info.seeders))
            .filter_map(|(id, _)| Some((id, self.list.get(id)?.status)))
            .filter(|(_, s)| status(*s))
            .map(|(id, _)| id)
            .cloned()
            .collect()
    }

    fn get_hashs(&self, ids: &[usize]) -> Vec<&str> {
        ids.iter()
            .filter_map(|id| self.list.get(id))
            .map(|t| t.hash.as_str())
            .collect()
    }

    fn set_status(&mut self, status: TorrentStatus, ids: &[usize]) {
        for id in ids {
            if let Some(torrent) = self.list.get_mut(id) {
                torrent.status = status;
            }
        }
    }

    fn start(&mut self, ids: &[usize]) -> usize {
        match self.client.start(&self.get_hashs(ids)) {
            Ok(()) => (),
            Err(err) => {
                error!("Client::start {}", err);
                return 0;
            }
        }
        self.set_status(TorrentStatus::Seeding, ids);
        ids.iter().count()
    }

    fn stop(&mut self, ids: &[usize]) -> usize {
        match self.client.stop(&self.get_hashs(ids)) {
            Ok(()) => (),
            Err(err) => {
                error!("Client::stop {}", err);
                return 0;
            }
        }
        self.set_status(TorrentStatus::Stopped, ids);
        ids.iter().count()
    }

    fn remove(&mut self, ids: &[usize]) -> usize {
        match self.client.remove(&self.get_hashs(ids), true) {
            Ok(()) => (),
            Err(err) => {
                error!("Client::remove {}", err);
                return 0;
            }
        }
        for id in ids {
            self.list.remove(id);
        }
        ids.iter().count()
    }
}

#[derive(Debug)]
pub struct Control<'a> {
    clients: Vec<Client>,
    api: &'a RutrackerApi<'a>,
    db: &'a Database,
    dry_run: bool,
}

impl<'a> Control<'a> {
    pub fn new(api: &'a RutrackerApi, db: &'a Database, dry_run: bool) -> Self {
        Control {
            clients: Vec::new(),
            api,
            db,
            dry_run,
        }
    }

    fn torrent_ids(&self) -> HashSet<usize> {
        let mut set: HashSet<usize> = HashSet::new();
        for client in &self.clients {
            set.extend(client.list.keys().cloned());
        }
        set
    }

    pub fn add_client(&mut self, client: Box<TorrentClient>) -> Result<()> {
        let trace = ::slog_scope::logger().is_trace_enabled();
        let list = client.list()?;
        if trace {
            for torrent in &list {
                trace!("Control::add_client::torrent"; "status" => ?torrent.status, "hash" => &torrent.hash);
            }
        }
        let mut ids = self.api.get_topic_id(
            &list.iter().map(|t| &t.hash).collect::<Vec<&String>>(),
            None,
        )?;
        if trace {
            for (hash, id) in &ids {
                trace!("Control::add_client::torrent"; "id" => id, "hash" => hash);
            }
        }
        let client = Client {
            list: list
                .into_iter()
                .filter_map(|torrent| Some((ids.remove(&torrent.hash)?, torrent)))
                .collect(),
            client,
        };
        if trace {
            trace!("Control::add_client::client"; "client" => ?client.client);
            for (id, t) in &client.list {
                trace!("Control::add_client::client"; "status" => ?t.status, "hash" => &t.hash, "id" => id);
            }
        }
        self.clients.push(client);
        Ok(())
    }

    pub fn set_status(&mut self, status: TorrentStatus, ids: &[usize]) {
        self.clients
            .iter_mut()
            .for_each(|c| c.set_status(status, ids));
    }

    pub fn start(&mut self, stop: usize, topics: &TopicInfoMap) {
        let seeders = |seeders| seeders < stop;
        let status = |status| status == TorrentStatus::Stopped;
        let mut count = 0;
        for client in &mut self.clients {
            let ids = client.get_ids(topics, seeders, status);
            if self.dry_run {
                for id in ids {
                    info!("Раздача с id {} будет запущена", id);
                }
            } else {
                count += client.start(&ids);
            }
        }
        info!("Запущено раздач: {}", count);
    }

    pub fn stop(&mut self, remove: usize, stop: usize, topics: &TopicInfoMap) {
        let seeders = |seeders| seeders < remove && seeders >= stop;
        let status = |status| status == TorrentStatus::Seeding;
        let mut count = 0;
        for client in &mut self.clients {
            let ids = client.get_ids(topics, seeders, status);
            if self.dry_run {
                for id in ids {
                    info!(
                        "Раздача с id {} будет остановлена",
                        id
                    );
                }
            } else {
                count += client.stop(&ids);
            }
        }
        info!("Остановлено раздач: {}", count);
    }

    pub fn remove(&mut self, remove: usize, topics: &TopicInfoMap) {
        let seeders = |seeders| seeders >= remove;
        let status = |status| status != TorrentStatus::Other;
        let mut count = 0;
        for client in &mut self.clients {
            let ids = client.get_ids(topics, seeders, status);
            if self.dry_run {
                for id in ids {
                    info!("Раздача с id {} будет удалена", id);
                }
            } else {
                count += client.remove(&ids);
            }
        }
        info!("Удалено раздач: {}", count);
    }

    fn get_topic_info(&self, forum_id: usize) -> Result<TopicInfoMap> {
        self.api.pvc(forum_id)?;
        let forum_list: HashSet<usize> = self.db.get(DBName::ForumList, &forum_id)?.unwrap();
        let keys: Vec<usize> = forum_list
            .intersection(&self.torrent_ids())
            .cloned()
            .collect();
        let topics = self.db.get_map(DBName::TopicInfo, keys)?;
        let topics = topics
            .into_iter()
            .filter_map(|(k, v)| Some((k, v?)))
            .collect();
        if ::slog_scope::logger().is_trace_enabled() {
            for (id, info) in &topics {
                trace!("Control::get_topic_info::topic"; "info" => ?info, "id" => id);
            }
        }
        Ok(topics)
    }

    pub fn apply_config(&mut self, forum: &ForumConfig) {
        for id in &forum.ids {
            let topics = match self.get_topic_info(*id) {
                Ok(t) => t,
                Err(err) => {
                    error!("Control::apply_config::topics: {}", err);
                    continue;
                }
            };
            self.remove(forum.remove, &topics);
            self.stop(forum.remove, forum.stop, &topics);
            self.start(forum.stop, &topics);
        }
    }
}
