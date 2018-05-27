use std::default::Default;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
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

#[derive(Debug, Deserialize)]
pub struct User {
    pub name: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum ClientName {
    Deluge,
    Transmission,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Client {
    pub client: ClientName,
    pub address: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ForumConfig {
    pub ids: Vec<usize>,
    pub peers_for_download: usize,
    pub peers_for_kill: usize,
    pub peers_for_stop: usize,
}

impl Default for ForumConfig {
    fn default() -> ForumConfig {
        ForumConfig {
            ids: Vec::new(),
            peers_for_download: 3,
            peers_for_kill: 10,
            peers_for_stop: 5,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub forum: Vec<ForumConfig>,
    pub ignored_ids: Vec<usize>,
    pub log_file: Option<String>,
    pub log_level: usize,
    pub real_kill: bool,
    pub client: Vec<Client>,
    pub user: Option<User>,
    pub api_url: String,
    pub forum_url: String,
    pub proxy: Option<String>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            forum: Vec::new(),
            ignored_ids: Vec::new(),
            log_file: None,
            log_level: 3,
            real_kill: false,
            client: Vec::new(),
            user: None,
            api_url: String::from("https://api.t-ru.org/"),
            forum_url: String::from("https://rutracker.org/forum/"),
            proxy: None,
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
