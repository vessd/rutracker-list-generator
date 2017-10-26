//! A module to access Rutracker API

use std::collections::HashMap;
use reqwest::{self, Client, IntoUrl, Url};
use serde_json::Value;

type PeerStats = (usize, usize, usize);
pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        UrlError(err: ::reqwest::UrlError) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Reqwest(err: ::reqwest::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        ApiError(method: &'static str, err: ResponseError) {
            description("Rutracker API error")
            display( "{}: {{ code: {}, text: {} }}",
                method,
                err.code,
                err.text)
        }
    }
}

/// Limit of request.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
struct Limit {
    limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TopicData {
    pub info_hash: String,
    pub forum_id: usize,
    pub poster_id: usize,
    pub size: usize,
    pub reg_time: usize,
    pub tor_status: usize,
    pub seeders: usize,
    pub topic_title: String,
    pub seeder_last_seen: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseError {
    pub code: u16,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Response<T: Default> {
    format: Value,
    result: T,
    total_size_bytes: Value,
    update_time: Value,
    update_time_humn: Value,
    error: Option<ResponseError>,
}

#[derive(Debug)]
pub struct RutrackerApi {
    url: Url,
    http_client: Client,
    limit: usize,
}

macro_rules! dynamic {
    ($name:ident, $arrayname:ident : $arraytype:ty, ($key:ty, $val:ty)) => {
        pub fn $name(&self, $arrayname: &[$arraytype]) -> Result<HashMap<$key,Option<$val>>> {
            let base_url = {
                let mut url = self.url.join(concat!("v1/", stringify!($name)))?;
                url.query_pairs_mut().append_pair("by", stringify!($arrayname));
                url
            };
            let mut result = HashMap::new();
            for chunk in $arrayname.chunks(self.limit) {
                let val = chunk.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",");
                let mut url = base_url.clone();
                url.query_pairs_mut().append_pair("val", val.as_str());
                let res = self.http_client.get(url)?.send()?.json::<Response<HashMap<$key,Option<$val>>>>()?;
                match res.error {
                    None => result.extend(res.result.into_iter()),
                    Some(err) => return Err(Error::ApiError(stringify!($name), err)),
                }
            }
            Ok(result)
        }
    }
}

impl RutrackerApi {
    pub fn new<S: IntoUrl>(url: S) -> Result<Self> {
        let url = url.into_url()?;
        Ok(RutrackerApi {
            limit: RutrackerApi::get_limit(&url)?,
            url: url,
            http_client: Client::new()?,
        })
    }
    /// Get limit of request.
    fn get_limit(url: &Url) -> Result<usize> {
        let res = reqwest::get(url.join("v1/get_limit")?)?.json::<Response<Limit>>()?;
        match res.error {
            None => Ok(res.result.limit),
            Some(err) => Err(Error::ApiError("get_limit", err)),
        }
    }

    dynamic!(get_forum_name, forum_id: usize, (usize, String));
    dynamic!(get_user_name, user_id: usize, (usize, String));
    dynamic!(get_peer_stats, topic_id: usize, (String, PeerStats));
    dynamic!(get_topic_id, hash: &str, (String, usize));
    dynamic!(get_tor_topic_data, topic_id: usize, (usize, TopicData));

    /// Get peer stats for all topics of the sub-forum
    pub fn pvc(&self, forum_id: usize) -> Result<HashMap<String, Option<PeerStats>>> {
        let url = self.url
            .join("v1/static/pvc/f/")?
            .join(forum_id.to_string().as_str())?;
        let res = self.http_client
            .get(url)?
            .send()?
            .json::<Response<HashMap<String, Option<PeerStats>>>>()?;
        match res.error {
            None => Ok(res.result),
            Some(err) => Err(Error::ApiError("pvc", err)),
        }
    }
}
