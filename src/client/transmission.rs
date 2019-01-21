//! A minimal implementation of rpc client for Tranmission.
use reqwest::header::HeaderValue;
use reqwest::{self, Client, IntoUrl, StatusCode, Url};
use serde_json::Value;
use std::{fmt, result};

pub type Result<T> = result::Result<T, ::failure::Error>;

#[derive(Debug, Fail)]
enum TransmissionError {
    #[fail(display = "failed to get SessionId from header")]
    SessionIdNotFound,
    #[fail(display = "unexpected status code: {}", status)]
    UnexpectedStatus { status: ::reqwest::StatusCode },
    #[fail(display = "the transmission server responded with an error: {}", error)]
    ResponseError { error: String },
}

/// A enum that represents the "ids" field in request body.
#[derive(Debug, Clone, Copy)]
pub enum TorrentSelect<'a> {
    Ids(&'a [String]),
    All,
}

/// A struct that represents the "delete-local-data" field in request body.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DeleteLocalData(pub bool);

/// A structure that represents fields for torrent-get request.
///
/// It provides only the minimum required fields.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum ArgGet {
    #[serde(rename = "hashString")]
    HashString,
    #[serde(rename = "status")]
    Status,
}

// https://github.com/serde-rs/serde/issues/497
macro_rules! enum_number_de {
    ($name:ident { $($variant:ident = $value:expr, )* }) => {
        #[derive(Debug, Clone, Copy)]
        pub enum $name {
            $($variant = $value,)*
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
                where D: ::serde::Deserializer<'de>
            {
                struct Visitor;

                impl<'de> ::serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("positive integer")
                    }

                    fn visit_u64<E>(self, value: u64) -> result::Result<$name, E>
                        where E: ::serde::de::Error
                    {
                        match value {
                            $( $value => Ok($name::$variant), )*
                            _ => Err(E::custom(
                                format!("unknown {} value: {}",
                                stringify!($name), value))),
                        }
                    }
                }
                deserializer.deserialize_u64(Visitor)
            }
        }
    }
}

/// A enum that represents a torrent status.
enum_number_de!(TorrentStatus {
    TorrentIsStopped = 0,
    QueuedToCheckFiles = 1,
    CheckingFiles = 2,
    QueuedToDownload = 3,
    Downloading = 4,
    QueuedToSeed = 5,
    Seeding = 6,
});

/// A struct that represents a "torrents" object in response body.
///
/// It provides only the minimum required fields.
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseGet {
    #[serde(rename = "hashString")]
    pub hash: String,
    pub status: TorrentStatus,
}

/// A struct that represents a "arguments" object in response body.
#[derive(Debug, Clone, Deserialize)]
struct ResponseArgument {
    #[serde(default)]
    torrents: Vec<ResponseGet>,
}

/// A enum that represents a response status.
#[derive(Debug, Clone, Deserialize)]
pub enum ResponseStatus {
    #[serde(rename = "success")]
    Success,
    Error(String),
}

/// A struct that represents a response body.
#[derive(Debug, Clone, Deserialize)]
struct Response {
    arguments: ResponseArgument,
    result: ResponseStatus,
}

/// RPC username and password.
#[derive(Debug)]
struct User {
    name: String,
    password: String,
}

/// Torrent client.
#[derive(Debug)]
pub struct Transmission {
    url: Url,
    user: Option<User>,
    sid: HeaderValue,
    http_client: Client,
}

macro_rules! requ_json {
    ($var:ident , $method:tt $(,$argstring:tt : $argname:ident)*) => {
        match $var {
            TorrentSelect::Ids(vec) => json!({"arguments":{$($argstring:$argname,)* "ids":vec}, "method":$method}),
            TorrentSelect::All => json!({"arguments":{$($argstring:$argname,)*}, "method":$method}),
        }
    }
}

macro_rules! empty_response {
    ($name:ident, $method:tt $(,$argname:ident : $argtype:ident : $argstring:tt)*) => {
        pub fn $name(&self, t: TorrentSelect<'_> $(,$argname:$argtype)*) -> Result<()> {
            match self.request(&requ_json!(t,$method $(,$argstring:$argname)*))?.json::<Response>()?.result {
                ResponseStatus::Success => Ok(()),
                ResponseStatus::Error(error) => Err(TransmissionError::ResponseError{ error }.into()),
            }
        }
    }
}

impl Transmission {
    /// Crate new `Transmission` struct.
    ///
    /// Fails if a `url` can not be parsed or if HTTP client fails.
    pub fn new<U>(url: U, user: Option<(String, String)>) -> Result<Self>
    where
        U: IntoUrl,
    {
        let user = if let Some((n, p)) = user {
            Some(User {
                name: n,
                password: p,
            })
        } else {
            None
        };
        let url = url.into_url()?;
        let http_client = Client::new();
        let sid = http_client
            .get(url.clone())
            .send()?
            .headers()
            .get("X-Transmission-Session-Id")
            .ok_or(TransmissionError::SessionIdNotFound)?
            .clone();
        Ok(Transmission {
            url,
            user,
            sid,
            http_client,
        })
    }

    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    /// Make a request to the Transmission.
    ///
    /// If the response status is 200, then return a response.
    /// If the response status is 409, then try again with a new SID.
    /// Otherwise return an error.
    fn request(&self, json: &Value) -> Result<reqwest::Response> {
        let resp = self
            .http_client
            .post(self.url.clone())
            .json(json)
            .header("X-Transmission-Session-Id", self.sid.clone())
            .send()?;
        match resp.status() {
            StatusCode::OK => Ok(resp),
            _ => Err(TransmissionError::UnexpectedStatus {
                status: resp.status(),
            }
            .into()),
        }
    }

    /// Start a list of torrents in the Transmission.
    empty_response!(start, "torrent-start");

    /// Stop a list of torrents in the Transmission.
    empty_response!(stop, "torrent-stop");

    /// Remove a list of torrents in the Transmission.
    empty_response!(remove, "torrent-remove", d:DeleteLocalData:"delete-local-data");

    /// Get a list of torrents from the Transmission.
    pub fn get(&self, t: TorrentSelect<'_>, f: &[ArgGet]) -> Result<Vec<ResponseGet>> {
        let responce = self
            .request(&requ_json!(t, "torrent-get", "fields": f))?
            .json::<Response>()?;
        match responce.result {
            ResponseStatus::Success => Ok(responce.arguments.torrents),
            ResponseStatus::Error(error) => Err(TransmissionError::ResponseError { error }.into()),
        }
    }
}
