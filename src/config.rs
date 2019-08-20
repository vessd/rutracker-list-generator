use serde::Deserialize;
use std::{default::Default, fs, path::PathBuf};

pub type Result<T> = std::result::Result<T, failure::Error>;

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

impl<'de> serde::Deserialize<'de> for LogDestination {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer).map(|path: &str| match path {
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
        Self {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Subforum {
    pub id: Vec<i16>,
    pub remove: i16,
    pub stop: i16,
    pub download: i16,
}

impl Default for Subforum {
    fn default() -> Self {
        Self {
            id: Vec::new(),
            remove: 11,
            stop: 5,
            download: 2,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub subforum: Vec<Subforum>,
    #[serde(default)]
    pub ignored_id: Vec<i32>,
    #[serde(default)]
    pub log: Log,
    pub client: Vec<Client>,
    pub forum: ForumConfig,
    #[serde(default = "api_url")]
    pub api_url: String,
    #[serde(default)]
    pub dry_run: bool,
}

fn api_url() -> String {
    String::from("https://api.t-ru.org/")
}

impl Config {
    pub fn from_file<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let mut config: Self = toml::from_slice(&fs::read(path.into())?)?;
        config.subforum.retain(|s| !s.id.is_empty());
        if config.subforum.is_empty() {
            Err(failure::err_msg(
                "Не указано ни одного подраздела",
            ))
        } else {
            Ok(config)
        }
    }
}
