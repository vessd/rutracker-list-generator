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

#[derive(Debug, Clone)]
pub enum LogDestination {
    Stdout,
    Stderr,
    File(PathBuf),
}

impl<'de> ::serde::Deserialize<'de> for LogDestination {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::serde::Deserialize::deserialize(deserializer).map(|path: &str| match path {
            "stdout" => LogDestination::Stdout,
            "stderr" => LogDestination::Stderr,
            _ => LogDestination::File(PathBuf::from(path)),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    pub destination: LogDestination,
    pub level: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            destination: LogDestination::Stdout,
            level: 3,
        }
    }
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
    pub remove: usize,
    pub stop: usize,
    pub download: usize,
}

impl Default for ForumConfig {
    fn default() -> ForumConfig {
        ForumConfig {
            ids: Vec::new(),
            remove: 11,
            stop: 5,
            download: 2,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub forum: Vec<ForumConfig>,
    pub ignored_ids: Vec<usize>,
    pub log: LogConfig,
    pub client: Vec<Client>,
    pub user: Option<User>,
    pub api_url: String,
    pub forum_url: String,
    pub proxy: Option<String>,
    pub dry_run: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            forum: Vec::new(),
            ignored_ids: Vec::new(),
            log: LogConfig::default(),
            client: Vec::new(),
            user: None,
            api_url: String::from("https://api.t-ru.org/"),
            forum_url: String::from("https://rutracker.org/forum/"),
            proxy: None,
            dry_run: false,
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
        Ok(config)
    }
}
