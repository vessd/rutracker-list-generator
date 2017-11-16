//! A module to access Rutracker API

use std::collections::HashMap;
use reqwest::{self, Client, IntoUrl, Url};
use serde_json::{self, Value};

pub type Result<T> = ::std::result::Result<T, Error>;
pub type ForumName = HashMap<usize, Option<String>>;
pub type UserName = HashMap<usize, Option<String>>;
pub type PeerStats = HashMap<usize, Vec<usize>>;
pub type TopicId = HashMap<String, Option<usize>>;
pub type TopicData = HashMap<usize, Option<Data>>;

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
        SerdeJson(err: serde_json::Error) {
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
pub struct Data {
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
    ($name:ident, $arrayname:ident : $arraytype:ty, $type:ty) => {
        pub fn $name(&self, $arrayname: &[$arraytype]) -> Result<$type> {
            let base_url = {
                let mut url = self.url.join(concat!("v1/", stringify!($name)))?;
                url.query_pairs_mut().append_pair("by", stringify!($arrayname));
                url
            };
            debug!(concat!("RutrackerApi::",stringify!($name),"::base_url: {:?}"), base_url);
            let mut result = HashMap::new();
            for chunk in $arrayname.chunks(self.limit) {
                let val = chunk.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",");
                let mut url = base_url.clone();
                url.query_pairs_mut().append_pair("val", val.as_str());
                trace!(concat!("RutrackerApi::",stringify!($name),"::url: {:?}"), url);
                let res = self.http_client.get(url)?.send()?.json::<Response<Value>>()?;
                trace!(concat!("RutrackerApi::",stringify!($name),"::res: {:?}"), res);
                match res.error {
                    None => {
                        let res = serde_json::from_value::<$type>(res.result)?;
                        result.extend(res.into_iter());
                    },
                    Some(err) => return Err(Error::ApiError(stringify!($name), err)),
                }
            }
            trace!(concat!("RutrackerApi::",stringify!($name),"::result: {:?}"), result);
            Ok(result)
        }
    }
}

impl RutrackerApi {
    pub fn new<S: IntoUrl>(url: S) -> Result<Self> {
        let url = url.into_url()?;
        debug!("RutrackerApi::new::url: {:?}", url);
        let api = RutrackerApi {
            limit: RutrackerApi::get_limit(&url)?,
            url: url,
            http_client: Client::new()?,
        };
        debug!("RutrackerApi::new::api: {:?}", api);
        Ok(api)
    }
    /// Get limit of request.
    fn get_limit(url: &Url) -> Result<usize> {
        let res = reqwest::get(url.join("v1/get_limit")?)?.json::<Response<Limit>>()?;
        debug!("RutrackerApi::get_limit::res: {:?}", res);
        match res.error {
            None => Ok(res.result.limit),
            Some(err) => Err(Error::ApiError("get_limit", err)),
        }
    }

    dynamic!(get_forum_name, forum_id: usize, ForumName);
    dynamic!(get_user_name, user_id: usize, UserName);
    dynamic!(get_peer_stats, topic_id: usize, PeerStats);
    dynamic!(get_topic_id, hash: &str, TopicId);
    dynamic!(get_tor_topic_data, topic_id: usize, TopicData);

    /// Get peer stats for all topics of the sub-forum
    pub fn pvc(&self, forum_id: usize) -> Result<PeerStats> {
        let url = self.url
            .join("v1/static/pvc/f/")?
            .join(forum_id.to_string().as_str())?;
        trace!("RutrackerApi::pvc::url {:?}", url);
        let res = self.http_client
            .get(url)?
            .send()?
            .json::<Response<Value>>()?;
        trace!("RutrackerApi::pvc::res {:?}", res);
        match res.error {
            None => {
                let mut map: PeerStats = serde_json::from_value(res.result)?;
                map.retain(|_, v| !v.is_empty());
                Ok(map)
            }
            Some(err) => Err(Error::ApiError("pvc", err)),
        }
    }
}
