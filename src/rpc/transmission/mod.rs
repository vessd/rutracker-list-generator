mod error;
mod torrent;
#[cfg(test)]
mod test;

use std::net::{ToSocketAddrs, SocketAddr};
use std::{io,fmt, result};
use serde_json::Value;
use reqwest::{self, Client, StatusCode};
pub use self::error::{Error, Result};

#[derive(Debug,Clone,Copy)]
enum TorrentSelect<'a> {
    Ids(&'a [&'a str]),
    All,
}

#[derive(Debug,Clone,Copy,Serialize)]
struct DeleteLocalData(bool);

#[derive(Debug,Clone,Copy,Serialize)]
enum ArgGet {
    #[serde(rename = "hashString")]
    HashString,
    #[serde(rename = "status")]
    Status,
}

// https://github.com/serde-rs/serde/issues/497
macro_rules! enum_number_de {
    ($name:ident { $($variant:ident = $value:expr, )* }) => {
        #[derive(Debug, Clone, Copy)]
        enum $name {
            $($variant = $value,)*
        }

        impl ::serde::Deserialize for $name {
            fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
                where D: ::serde::Deserializer
            {
                struct Visitor;

                impl ::serde::de::Visitor for Visitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
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

enum_number_de!(TorrentStatus {
    TorrentIsStopped = 0,
    QueuedToCheckFiles = 1,
    CheckingFiles = 2,
    QueuedToDownload = 3,
    Downloading = 4,
    QueuedToSeed = 5,
    Seeding = 6,
});

#[derive(Debug,Clone,Deserialize)]
struct ResponseGet {
    #[serde(rename = "hashString")]
    hash: String,
    status: TorrentStatus,
}

#[derive(Debug,Clone,Deserialize)]
struct ResponseArgument {
    #[serde(default)]
    torrents: Vec<ResponseGet>,
}

#[derive(Debug,Clone,Deserialize)]
enum ResponseStatus {
    #[serde(rename = "success")]
    Success,
    Error(String),
}

#[derive(Debug,Clone,Deserialize)]
struct Response {
    arguments: ResponseArgument,
    result: ResponseStatus,
}

header! { (SessionId, "X-Transmission-Session-Id") => [String] }

#[derive(Debug)]
struct Credentials {
    user: String,
    password: String,
}

#[derive(Debug)]
pub struct Transmission {
    address: String,
    credentials: Option<Credentials>,
    sid: SessionId,
    http_client:Client,
}

impl Transmission {
    pub fn new<A>(address: A, credentials: Option<(&str, &str)>) -> Result<Transmission>
        where A: ToSocketAddrs
    {
        let address = if let Some(a) = address.to_socket_addrs()?.next() {
                match a {
                    SocketAddr::V4(socket) => socket.to_string(),
                    SocketAddr::V6(_) => return Err(Error::Ipv6),
                }
            } else {
                return Err(Error::from(io::Error::new(io::ErrorKind::Other, "fail cast to socket address")));
            };
        let credentials = if let Some((u, p)) = credentials {
            Some(Credentials {
                user: u.to_string(),
                password: p.to_string(),
            })
        } else {
            None
        };

        Ok(Transmission {
            address: address,
            credentials: credentials,
            sid: SessionId(String::new()),
            http_client: Client::new()?,
        })
    }

    fn url(&self) -> String {
        String::new() + "http://" + self.address.as_str() + "/transmission/rpc"
    }

    fn request(&mut self, json: &Value) -> Result<reqwest::Response> {
        let resp = self.http_client.post(&self.url())
            .json(json)
            .header(self.sid.clone())
            .send()?;
        match *resp.status() {
            StatusCode::Ok => return Ok(resp),
            StatusCode::Conflict => {
                self.sid = resp.headers().get::<SessionId>().ok_or(Error::ParseIdError)?.clone();
                return self.request(json);
            }
            _ => return Err(Error::UnexpectedResponse(*resp.status())),
        }
    }

    fn empty_response(&mut self, json: &Value) -> Result<()> {
        match self.request(&json)?.json::<Response>()?.result {
            ResponseStatus::Success => Ok(()),
            ResponseStatus::Error(err) => Err(Error::TransmissionError(err)),
        }
    }

    fn start(&mut self, t: TorrentSelect) -> Result<()> {
        let requ_json = match t {
            TorrentSelect::Ids(vec) => json!({"arguments":{"ids":vec}, "method":"torrent-start"}),
            TorrentSelect::All => json!({"arguments":{}, "method":"torrent-start"}),
        };
        self.empty_response(&requ_json)
    }

    fn stop(&mut self, t: TorrentSelect) -> Result<()> {
        let requ_json = match t {
            TorrentSelect::Ids(vec) => json!({"arguments":{"ids":vec}, "method":"torrent-stop"}),
            TorrentSelect::All => json!({"arguments":{}, "method":"torrent-stop"}),
        };
        self.empty_response(&requ_json)
    }

    fn get(&mut self, t: TorrentSelect, f: &[ArgGet]) -> Result<Vec<ResponseGet>> {
        let requ_json = match t {
            TorrentSelect::Ids(vec) => json!({"arguments":{"fields":f, "ids":vec}, "method":"torrent-get"}),
            TorrentSelect::All => json!({"arguments":{"fields":f}, "method":"torrent-get"}),
        };
        let responce = self.request(&requ_json)?.json::<Response>()?;
        match responce.result {
            ResponseStatus::Success => {
                Ok(responce.arguments.torrents)
            }
            ResponseStatus::Error(err) => Err(Error::TransmissionError(err)),
        }
    }

    fn remove(&mut self, t: TorrentSelect, d: DeleteLocalData) -> Result<()> {
        let requ_json = match t {
            TorrentSelect::Ids(vec) => json!({"arguments":{"delete-local-data":d, "ids":vec}, "method":"torrent-remove"}),
            TorrentSelect::All => json!({"arguments":{"delete-local-data":d}, "method":"torrent-remove"}),
        };
        self.empty_response(&requ_json)
    }
}
