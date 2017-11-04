use rpc::{TorrentClient, TorrentStatus};
use rutracker::api::{RutrackerApi, TopicData};

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
struct Torrent {
    data: TopicData,
    status: TorrentStatus,
}

impl Torrent {
    fn new(status: TorrentStatus, data: TopicData) -> Self {
        Torrent { data, status }
    }
}

#[derive(Debug)]
pub struct TorrentList {
    list: Vec<Torrent>,
}

impl TorrentList {
    pub fn new<C>(client: &mut C, api: &RutrackerApi) -> Result<Self>
    where
        C: TorrentClient,
    {
        let mut client_list = client.list()?;
        trace!("TorrentList::new::client_list: {:?}", client_list);
        let ids = api.get_topic_id(&client_list
            .iter()
            .map(|(hash, _)| hash.as_str())
            .collect::<Vec<&str>>())?
            .into_iter()
            .filter_map(|(_, id)| id)
            .collect::<Vec<usize>>();
        trace!("TorrentList::new::ids: {:?}", ids);
        let topics_data = api.get_tor_topic_data(&ids)?
            .into_iter()
            .filter_map(|(_, t)| t)
            .collect::<Vec<TopicData>>();
        trace!("TorrentList::new::topics_data: {:?}", topics_data);
        Ok(TorrentList {
            list: topics_data
                .into_iter()
                .filter(|data| data.tor_status == 2 || data.tor_status == 8)
                .map(|data| {
                    Torrent::new(client_list.remove(&data.info_hash).unwrap(), data)
                })
                .collect(),
        })
    }

    pub fn get(&self, status: TorrentStatus) -> Vec<&str> {
        self.list
            .iter()
            .filter(|t| t.status == status)
            .map(|t| t.data.info_hash.as_str())
            .collect()
    }

    pub fn remove_by_poster_id(&mut self, id: usize) {
        self.list.retain(|t| t.data.poster_id != id);
    }
}
