use config::Forum;
use rpc::{TorrentClient, TorrentStatus};
use rutracker::api::{Data, PeerStats, RutrackerApi};
use std::collections::HashMap;

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Rpc(err: ::rpc::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Api(err: ::rutracker::api::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

#[derive(Debug)]
pub struct Torrent {
    data: Data,
    status: TorrentStatus,
}

impl Torrent {
    fn new(status: TorrentStatus, data: Data) -> Self {
        Torrent { data, status }
    }
}

#[derive(Debug)]
struct ClientList {
    list: HashMap<usize, Torrent>,
    client: Box<TorrentClient>,
}

impl ClientList {
    fn get_list_to_change(&self, peers: usize, topics: &mut PeerStats) -> Vec<String> {
        let mut buf = Vec::new();
        for (id, t) in &self.list {
            if let Some(stats) = topics.remove(id) {
                if stats[0] >= peers && t.status != TorrentStatus::Other {
                    buf.push(t.data.info_hash.clone());
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
                    if hash.contains(&t.data.info_hash) {
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
                    if hash.contains(&t.data.info_hash) {
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
                Ok(()) => self.list.retain(|_, t| !hash.contains(&t.data.info_hash)),
                Err(err) => error!("{:?}", err),
            }
        }
    }
}

#[derive(Debug)]
pub struct TorrentList<'a> {
    list: Vec<ClientList>,
    api: &'a RutrackerApi,
    ignored_ids: &'a [usize],
}

impl<'a> TorrentList<'a> {
    pub fn new(api: &'a RutrackerApi, ignored_ids: &'a [usize]) -> Self {
        let list = TorrentList {
            list: Vec::new(),
            api,
            ignored_ids,
        };
        trace!("TorrentList::new::list: {:?}", list);
        list
    }
    pub fn add_client(&mut self, mut client: Box<TorrentClient>) -> Result<()> {
        let mut list = client.list()?;
        trace!("TorrentList::new::list: {:?}", list);
        let mut ids = self.api.get_topic_id(&list.keys().collect::<Vec<&String>>())?;
        ids.retain(|_, id| !self.ignored_ids.contains(id));
        trace!("TorrentList::new::ids: {:?}", ids);
        let topics_data = self.api
            .get_tor_topic_data(&ids.values().collect::<Vec<&usize>>())?
            .into_iter()
            .map(|(_, d)| d)
            .collect::<Vec<Data>>();
        trace!("TorrentList::new::topics_data: {:?}", topics_data);
        let client_list = ClientList {
            list: topics_data
                .into_iter()
                .map(|data| {
                    (
                        ids.remove(&data.info_hash).unwrap(),
                        Torrent::new(list.remove(&data.info_hash).unwrap(), data),
                    )
                })
                .collect(),
            client,
        };
        trace!("TorrentList::add_client::client_list: {:?}", client_list);
        if !list.is_empty() {
            warn!("TorrentList::add_client::list не пуст, возможно это баг");
            trace!("TorrentList::add_client::list: {:?}", list);
        }
        self.list.push(client_list);
        Ok(())
    }

    fn start(&mut self, topics: &mut PeerStats) {
        let mut buf = Vec::new();
        for client in &mut self.list {
            for (id, t) in &client.list {
                if topics.remove(id).is_some() && t.status != TorrentStatus::Other {
                    buf.push(t.data.info_hash.clone());
                }
            }
            client.start(&buf);
            buf.clear();
        }
    }

    fn stop(&mut self, peers: usize, topics: &mut PeerStats) {
        for client in &mut self.list {
            let vec = client.get_list_to_change(peers, topics);
            client.stop(&vec)
        }
    }

    fn remove(&mut self, peers: usize, topics: &mut PeerStats) {
        for client in &mut self.list {
            let vec = client.get_list_to_change(peers, topics);
            client.remove(&vec)
        }
    }

    pub fn exec(&mut self, id: usize, real_kill: bool, forum: &Forum, topics: &mut PeerStats) -> HashMap<usize, Torrent> {
        let mut map = HashMap::new();
        topics.retain(|k, _| !self.ignored_ids.contains(k));
        if real_kill {
            self.remove(forum.peers_for_kill, topics);
        }
        self.stop(forum.peers_for_stop, topics);
        self.start(topics);
        for client in &mut self.list {
            let vec: Vec<usize> = client
                .list
                .iter()
                .filter_map(|(&t_id, t)| if t.data.forum_id == id { Some(t_id) } else { None })
                .collect();
            map.extend(vec.into_iter().map(|id| (id, client.list.remove(&id).unwrap())));
        }
        map
    }
}
