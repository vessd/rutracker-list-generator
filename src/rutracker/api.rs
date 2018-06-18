//! A module to access Rutracker API

use database::{self, DBName, Database};
use reqwest::{self, Client, IntoUrl, Url};
use slog::Drain;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

pub type Result<T> = ::std::result::Result<T, Error>;
type TopicInfoMap = HashMap<usize, TopicInfo>;

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
        SerdeJson(err: ::serde_json::Error) {
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
        Database (err: database::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

/// Limit of request.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
struct Limit {
    limit: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopicInfo {
    pub tor_status: usize,
    pub seeders: usize,
    pub reg_time: usize,
}

struct OptionInfo(Option<TopicInfo>);

impl<'de> ::serde::Deserialize<'de> for OptionInfo {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::serde::Deserialize::deserialize(deserializer).map(|info: Vec<usize>| {
            if info.len() == 3 {
                OptionInfo(Some(TopicInfo {
                    tor_status: info[0],
                    seeders: info[1],
                    reg_time: info[2],
                }))
            } else {
                OptionInfo(None)
            }
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseError {
    pub code: u16,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Response<T: Default> {
    result: T,
    error: Option<ResponseError>,
}

#[derive(Debug)]
pub struct RutrackerApi<'a> {
    url: Url,
    http_client: Client,
    limit: usize,
    db: &'a Database,
}

macro_rules! dynamic {
    ($name:ident, $arrayname:ident : $arraytype:ty, $type:ty) => {
        pub fn $name<T>(&self, $arrayname: &[T], db_name: Option<DBName>) -> Result<HashMap<$arraytype, $type>>
        where
            T: Borrow<$arraytype> + Display
        {
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
                let res = self.http_client.get(url).send()?.json::<Response<HashMap<$arraytype, Option<$type>>>>()?;
                match res.error {
                    None => {
                        let res = res.result.into_iter().filter_map(|(k, v)| Some((k, v?))).collect();
                        if ::slog_scope::logger().is_trace_enabled() {
                            let location = concat!("RutrackerApi::", stringify!($name), "::reponse");
                            for (k,v) in &res {
                                trace!("{}", location; "value" => ?v, "key" => k);
                            }
                        }
                        if let Some(name) = db_name {
                            self.db.put_map(name, &res)?;
                        } else {
                            result.extend(res.into_iter());
                        }
                    },
                    Some(err) => return Err(Error::ApiError(stringify!($name), err)),
                }
            }
            Ok(result)
        }
    }
}

impl<'a> RutrackerApi<'a> {
    pub fn new<S: IntoUrl>(url: S, db: &'a Database) -> Result<Self> {
        let url = url.into_url()?;
        debug!("RutrackerApi::new::url: {:?}", url);
        let api = RutrackerApi {
            limit: RutrackerApi::get_limit(&url)?,
            url,
            http_client: Client::new(),
            db,
        };
        debug!("RutrackerApi::new::api: {:?}", api);
        Ok(api)
    }

    /// Get limit of request.
    fn get_limit(url: &Url) -> Result<usize> {
        let res = reqwest::get(url.join("v1/get_limit")?)?.json::<Response<Limit>>()?;
        debug!("RutrackerApi::get_limit::response: {:?}", res);
        match res.error {
            None => Ok(res.result.limit),
            Some(err) => Err(Error::ApiError("get_limit", err)),
        }
    }

    dynamic!(get_forum_name, forum_id: usize, String);
    dynamic!(get_user_name, user_id: usize, String);
    dynamic!(get_peer_stats, topic_id: usize, (usize, usize, usize));
    dynamic!(get_topic_id, hash: String, usize);
    dynamic!(get_tor_topic_data, topic_id: usize, TopicData);

    /// Get peer stats for all topics of the sub-forum
    pub fn pvc(&self, forum_id: usize) -> Result<()> {
        let url = self
            .url
            .join("v1/static/pvc/f/")?
            .join(forum_id.to_string().as_str())?;
        trace!("RutrackerApi::pvc::url {:?}", url);
        let res: Response<HashMap<usize, OptionInfo>> = self.http_client.get(url).send()?.json()?;
        match res.error {
            None => {
                let map = res
                    .result
                    .into_iter()
                    .filter_map(|(k, OptionInfo(v))| Some((k, v?)))
                    .collect();
                if ::slog_scope::logger().is_trace_enabled() {
                    for (id, info) in &map {
                        trace!("RutrackerApi::pvc::topic"; "info" => ?info, "id" => id);
                    }
                }
                self.db.put_map(DBName::TopicInfo, &map)?;
                let set: HashSet<usize> = map.into_iter().map(|(k, _)| k).collect();
                self.db.put(DBName::ForumList, &forum_id, &set)?;
                Ok(())
            }
            Some(err) => Err(Error::ApiError("pvc", err)),
        }
    }
}
