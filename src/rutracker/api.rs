//! A module to access Rutracker API

use reqwest::{self, Client, IntoUrl, Url};
use std::collections::HashMap;

pub type Result<T> = ::std::result::Result<T, ::failure::Error>;

#[derive(Debug, Fail)]
#[fail(
    display = "Rutracker API responded with an error: {}: {{ code: {}, text: {} }}",
    method,
    code,
    text
)]
struct ApiError {
    method: &'static str,
    code: u16,
    text: String,
}

/// Limit of request.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
struct Limit {
    limit: usize,
}

#[derive(Debug, Clone)]
pub struct TopicStat {
    pub seeders: usize,
    pub leechers: usize,
    pub seeder_last_seen: usize,
}

impl<'de> ::serde::Deserialize<'de> for TopicStat {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::serde::Deserialize::deserialize(deserializer).map(|stat: (usize, usize, usize)| {
            TopicStat {
                seeders: stat.0,
                leechers: stat.1,
                seeder_last_seen: stat.2,
            }
        })
    }
}

impl ::serde::Serialize for TopicStat {
    fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use serde::ser::SerializeTuple;
        let mut tup = serializer.serialize_tuple(3)?;
        tup.serialize_element(&self.seeders)?;
        tup.serialize_element(&self.leechers)?;
        tup.serialize_element(&self.seeder_last_seen)?;
        tup.end()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopicData {
    pub info_hash: String,
    pub forum_id: usize,
    pub poster_id: isize,
    pub size: f64,
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
pub struct RutrackerApi {
    url: Url,
    http_client: Client,
    limit: usize,
}

macro_rules! dynamic {
    ($name:ident, $arrayname:ident : $arraytype:ty, $key:ty, $value:ty) => {
        pub fn $name(&self, $arrayname: Vec<$arraytype>) -> Result<HashMap<$key, $value>>
        {
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
                debug!(concat!("RutrackerApi::",stringify!($name),"::url: {:?}"), url);
                let res: Response<HashMap<$key, Option<$value>>> = self.http_client.get(url).send()?.json()?;
                match res.error {
                    None => {
                        result.extend(res.result.into_iter().filter_map(|(k, v)| Some((k, v?))));
                    },
                    Some(err) => return Err(ApiError { method: stringify!($name), code: err.code, text: err.text }.into()),
                }
            }
            Ok(result)
        }
    }
}

impl RutrackerApi {
    pub fn new<S: IntoUrl>(url: S) -> Result<Self> {
        let url = url.into_url()?;
        let api = RutrackerApi {
            limit: RutrackerApi::get_limit(&url)?,
            url,
            http_client: Client::new(),
        };
        debug!("RutrackerApi::new::api: {:?}", api);
        Ok(api)
    }

    /// Get limit of request.
    fn get_limit(url: &Url) -> Result<usize> {
        let res: Response<Limit> = reqwest::get(url.join("v1/get_limit")?)?.json()?;
        match res.error {
            None => Ok(res.result.limit),
            Some(err) => Err(ApiError {
                method: "get_limit",
                code: err.code,
                text: err.text,
            }.into()),
        }
    }

    dynamic!(get_forum_name, forum_id: &usize, usize, String);
    dynamic!(get_user_name, user_id: &usize, usize, String);
    dynamic!(get_peer_stats, topic_id: &usize, usize, TopicStat);
    dynamic!(get_topic_id, hash: &str, String, usize);
    dynamic!(get_tor_topic_data, topic_id: &usize, usize, TopicData);

    pub fn forum_size(&self) -> Result<HashMap<usize, (usize, f64)>> {
        let url = self.url.join("v1/static/forum_size")?;
        let res: Response<HashMap<_, _>> = self.http_client.get(url).send()?.json()?;
        match res.error {
            None => Ok(res.result),
            Some(err) => Err(ApiError {
                method: "forum_size",
                code: err.code,
                text: err.text,
            }.into()),
        }
    }

    /// Get peer stats for all topics of the sub-forum
    pub fn pvc(&self, forum_id: usize) -> Result<HashMap<usize, TopicInfo>> {
        let url = self
            .url
            .join("v1/static/pvc/f/")?
            .join(forum_id.to_string().as_str())?;
        debug!("RutrackerApi::pvc::url: {}", url);
        let res: Response<HashMap<usize, OptionInfo>> = self.http_client.get(url).send()?.json()?;
        match res.error {
            None => Ok(res
                .result
                .into_iter()
                .filter_map(|(k, OptionInfo(v))| Some((k, v?)))
                .collect()),
            Some(err) => Err(ApiError {
                method: "pvc",
                code: err.code,
                text: err.text,
            }.into()),
        }
    }
}
