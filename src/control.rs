use client::{self, Torrent, TorrentClient, TorrentStatus};
use config::ForumConfig;
use rutracker::api::{self, RutrackerApi, TopicInfo};
use std::collections::HashMap;

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
    }
}

#[derive(Debug)]
struct Client {
    list: HashMap<usize, Torrent>,
    client: Box<TorrentClient>,
}

impl Client {
    fn get_list_to_change(&self, peers: usize, topics: &HashMap<usize, TopicInfo>) -> Vec<String> {
        let mut buf = Vec::new();
        for (id, t) in &self.list {
            if let Some(info) = topics.get(id) {
                if info.seeders >= peers && t.status != TorrentStatus::Other {
                    buf.push(t.hash.clone());
                }
            }
        }
        buf
    }

    fn start(&mut self, hash: &[String]) {
        if !hash.is_empty() {
            let vec = hash.iter().map(|h| h.as_str()).collect::<Vec<&str>>();
            match self.client.start(&vec) {
                Ok(()) => self.list.iter_mut().for_each(|(_, t)| {
                    if hash.contains(&t.hash) {
                        t.status = TorrentStatus::Seeding;
                    }
                }),
                Err(err) => error!("{:?}", err),
            }
        }
    }

    fn stop(&mut self, hash: &[String]) {
        if !hash.is_empty() {
            let vec = hash.iter().map(|h| h.as_str()).collect::<Vec<&str>>();
            match self.client.stop(&vec) {
                Ok(()) => self.list.iter_mut().for_each(|(_, t)| {
                    if hash.contains(&t.hash) {
                        t.status = TorrentStatus::Stopped;
                    }
                }),
                Err(err) => error!("{:?}", err),
            }
        }
    }

    fn remove(&mut self, hash: &[String]) {
        if !hash.is_empty() {
            let vec = hash.iter().map(|h| h.as_str()).collect::<Vec<&str>>();
            match self.client.remove(&vec, false) {
                Ok(()) => self.list.retain(|_, t| !hash.contains(&t.hash)),
                Err(err) => error!("{:?}", err),
            }
        }
    }
}

#[derive(Debug)]
pub struct Control<'a> {
    clients: Vec<Client>,
    api: &'a RutrackerApi<'a>,
}

impl<'a> Control<'a> {
    pub fn new(api: &'a RutrackerApi) -> Self {
        let control = Control {
            clients: Vec::new(),
            api,
        };
        trace!("Control::new::list: {:?}", control);
        control
    }

    pub fn add_client(&mut self, mut client: Box<TorrentClient>) -> Result<(), Error> {
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
        trace!("TorrentList::add_client::client: {:?}", client);
        self.clients.push(client);
        Ok(())
    }

    fn start(&mut self, topics: &HashMap<usize, TopicInfo>) {
        let mut buf = Vec::new();
        for client in &mut self.clients {
            for (id, t) in &client.list {
                if topics.get(id).is_some() && t.status != TorrentStatus::Other {
                    buf.push(t.hash.clone());
                }
            }
            client.start(&buf);
            buf.clear();
        }
    }

    fn stop(&mut self, peers: usize, topics: &HashMap<usize, TopicInfo>) {
        for client in &mut self.clients {
            let vec = client.get_list_to_change(peers, topics);
            client.stop(&vec)
        }
    }

    fn remove(&mut self, peers: usize, topics: &HashMap<usize, TopicInfo>) {
        for client in &mut self.clients {
            let vec = client.get_list_to_change(peers, topics);
            client.remove(&vec)
        }
    }

    pub fn exec(
        &mut self,
        real_kill: bool,
        forum: &ForumConfig,
        topics: &HashMap<usize, TopicInfo>,
    ) -> HashMap<usize, Torrent> {
        let mut map = HashMap::new();
        if real_kill {
            self.remove(forum.peers_for_kill, topics);
        }
        self.stop(forum.peers_for_stop, topics);
        self.start(topics);
        for client in &mut self.clients {
            let vec: Vec<usize> = client
                .list
                .keys()
                .filter(|id| topics.contains_key(id))
                .cloned()
                .collect();
            map.extend(
                vec.into_iter()
                    .map(|id| (id, client.list.remove(&id).unwrap())),
            );
        }
        map
    }
}
