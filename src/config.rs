use std::default::Default;
use std::path::PathBuf;
use std::io::Read;
use std::fs::File;
use toml;

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: ::std::io::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Toml(err: ::toml::de::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum Client {
    Deluge,
    Transmission,
}

#[derive(Debug, Deserialize)]
pub struct Rpc {
    pub client: Client,
    pub address: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Forum {
    pub forum_ids: Vec<usize>,
    pub peers_for_download: usize,
    pub peers_for_kill: usize,
    pub peers_for_stop: usize,
}

impl Default for Forum {
    fn default() -> Forum {
        Forum {
            forum_ids: Vec::new(),
            peers_for_download: 3,
            peers_for_kill: 10,
            peers_for_stop: 5,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub forum: Vec<Forum>,
    pub ignored_ids: Vec<usize>,
    pub log_file: Option<String>,
    pub log_level: usize,
    pub real_kill: bool,
    pub rpc: Vec<Rpc>,
    pub user_id: Option<usize>,
    pub password: Option<String>,
    pub api_url: String,
    pub forum_url: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            forum: Vec::new(),
            ignored_ids: Vec::new(),
            log_file: None,
            log_level: 3,
            real_kill: false,
            rpc: Vec::new(),
            user_id: None,
            password: None,
            api_url: String::from("https://api.t-ru.org/"),
            forum_url: String::from("https://rutracker.cr/forum/"),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Config::default()
    }

    pub fn from_file<F>(file: F) -> Result<Self>
    where
        F: Into<PathBuf>,
    {
        let mut file = File::open(file.into())?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        let config = toml::from_slice(&buf)?;
        debug!("Config::from_file::config: {:?}", config);
        Ok(config)
    }
}
