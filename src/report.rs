/* use chrono::Local;
use config::{Config, ForumConfig, Rpc};
use rutracker::{RutrackerApi, RutrackerForum, User};

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        RutrackerApi(err: ::rutracker::api::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        RutrackerForum(err: ::rutracker::forum::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Unexpected {
            description("unexpected error")
            display("unexpected error")
        }
    }
}

/* #[derive(Debug)]
struct ForumData {
    name: String,
    url: String,
    torrent: HashMap<usize, Data>,
    local_torrent: HashMap<usize, Torrent>,
}

impl ForumData {
    fn new(
        name: String,
        url: String,
        torrent: HashMap<usize, Data>,
        local_torrent: HashMap<usize, Torrent>,
    ) -> Self {
        ForumData {
            name,
            url,
            torrent,
            local_torrent,
        }
    }
} */

#[derive(Debug)]
pub struct Report {
    date: String,
    rutracker_api: RutrackerApi,
    rutracker_forum: Option<RutrackerForum>,
    forums: Vec<ForumConfig>,
    ignored_ids: Vec<usize>,
    real_kill: bool,
    rpc: Vec<Rpc>,
}

impl Report {
    pub fn new(config: &Config) -> Result<Self> {
        let date = Local::today();
        let api = RutrackerApi::new(config.api_url.as_str())?;
        let forum = match User::new(config) {
            Ok(user) => Some(RutrackerForum::new(user, config)?),
            Err(err) => match err {
                ::rutracker::forum::Error::User => None,
                _ => return Err(::std::convert::From::from(err)),
            },
        };
        let report = Report {
            date: date.format("%d.%m.%Y").to_string(),
            rutracker_api: api,
            rutracker_forum: forum,
            forums: config.forum.clone(),
            ignored_ids: config.ignored_ids.clone(),
            real_kill: config.real_kill,
            rpc: config.rpc.clone(),
        };
        debug!("Report::new::report: {:?}", report);
        Ok(report)
    }

    fn send(forum_id: usize) -> Result<()> {
        Ok(())
    }
}
 */
