use std::default::Default;
use std::fs::read;
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
#[serde(default)]
pub struct Forum {
    pub user: Option<String>,
    pub password: Option<String>,
    pub url: String,
    pub proxy: Option<String>,
}

impl Default for Forum {
    fn default() -> Self {
        Forum {
            user: None,
            password: None,
            url: String::from("https://rutracker.org/forum/"),
            proxy: None,
        }
    }
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
    pub forum: Option<Forum>,
    pub api_url: String,
    pub dry_run: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            subforum: Vec::new(),
            ignored_id: Vec::new(),
            log: Log::default(),
            client: Vec::new(),
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
        Ok(toml::from_slice(&read(path.into())?)?)
    }
}
