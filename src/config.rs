use std::default::Default;
use std::fs;
use std::path::PathBuf;
use toml;

pub type Result<T> = ::std::result::Result<T, ::failure::Error>;

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ForumConfig {
    pub user: User,
    #[serde(default = "forum_url")]
    pub url: String,
    pub proxy: Option<String>,
}

fn forum_url() -> String {
    String::from("https://rutracker.org/forum/")
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
pub struct Log {
    pub destination: LogDestination,
    pub level: usize,
}

impl Default for Log {
    fn default() -> Self {
        Log {
            destination: LogDestination::Stdout,
            level: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum ClientName {
    Deluge,
    Transmission,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Client {
    pub name: ClientName,
    pub host: String,
    pub port: u16,
    pub user: Option<User>,
}

impl Default for Client {
    fn default() -> Self {
        Client {
            name: ClientName::Transmission,
            host: "localhost".to_string(),
            port: 9091,
            user: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Subforum {
    pub ids: Vec<usize>,
    pub remove: usize,
    pub stop: usize,
    pub download: usize,
}

impl Default for Subforum {
    fn default() -> Subforum {
        Subforum {
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
    pub subforum: Vec<Subforum>,
    pub ignored_id: Vec<usize>,
    pub log: Log,
    pub client: Vec<Client>,
    pub forum: Option<ForumConfig>,
    pub api_url: String,
    pub dry_run: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            subforum: Vec::new(),
            ignored_id: Vec::new(),
            log: Log::default(),
            client: vec![Client::default()],
            forum: None,
            api_url: String::from("https://api.t-ru.org/"),
            dry_run: false,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Config::default()
    }

    pub fn from_file<P: Into<PathBuf>>(path: P) -> Result<Self> {
        Ok(toml::from_slice(&fs::read(path.into())?)?)
    }
}
