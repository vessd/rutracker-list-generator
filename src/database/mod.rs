mod models;
mod schema;

use self::models::{Forum, KeeperTorrent, LocalTorrent, Topic, Torrent};
use self::schema::{forums, keeper_torrents, local_torrents, topics, torrents};
use crate::client;
use crate::rutracker::forum::Topic as RutrackerTopic;
use crate::rutracker::{RutrackerApi, RutrackerForum};
use diesel::dsl::{delete, insert_into, insert_or_ignore_into, replace_into, sql, update};
use diesel::prelude::{
    Connection, ExpressionMethods, GroupByDsl, JoinOnDsl, OptionalExtension, QueryDsl, QueryResult,
    RunQueryDsl, SqliteConnection,
};
use diesel::sql_types::{Double, Integer};
use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

type Result<T> = ::std::result::Result<T, ::failure::Error>;

pub struct Database {
    pub api: RutrackerApi,
    pub forum: RutrackerForum,
    sqlite: SqliteConnection,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Database {{ api: {:?}, forum: {:?}, sqlite: SqliteConnection }}",
            self.api, self.forum
        )
    }
}

impl Database {
    pub fn new(api: RutrackerApi, forum: RutrackerForum) -> Result<Self> {
        let sqlite = SqliteConnection::establish("rlg.db")?;
        delete(local_torrents::table).execute(&sqlite)?;
        delete(keeper_torrents::table).execute(&sqlite)?;
        Ok(Self { api, forum, sqlite })
    }

    pub fn delete_by_hash(&self, hash: &[String]) -> Result<()> {
        delete(local_torrents::table)
            .filter(local_torrents::hash.eq_any(hash))
            .execute(&self.sqlite)?;
        Ok(())
    }

    pub fn get_forum_size(&self, forum_id: i16) -> Result<(i32, f64)> {
        if let Some(forum_size) = forums::table
            .filter(forums::id.eq(forum_id))
            .select((forums::tor_count, forums::tor_size_bytes))
            .get_result(&self.sqlite)
            .optional()?
        {
            Ok(forum_size)
        } else {
            self.update_subforum_info()?;
            Ok(forums::table
                .filter(forums::id.eq(forum_id))
                .select((forums::tor_count, forums::tor_size_bytes))
                .get_result(&self.sqlite)
                .optional()?
                .unwrap_or((0, 0f64)))
        }
    }

    pub fn get_keepres_list_size(&self, forum_id: i16) -> Result<Vec<(String, i32, f64)>> {
        let forum = self.forum.get_keepers_working_forum();
        let topic = forums::table
            .inner_join(topics::table)
            .select((topics::id, topics::author, topics::title))
            .filter(forums::id.eq(forum_id))
            .get_result::<(i32, String, String)>(&self.sqlite)?;
        let topic = forum.get_topic(topic.0, topic.1, topic.2);
        let posts = topic.get_posts()?;
        let mut keeper = HashMap::new();
        let mut i = 0;
        for p in posts.iter().skip(1) {
            if let Entry::Vacant(v) = keeper.entry(p.author.as_str()) {
                v.insert(i);
                i += 1;
            }
            let torrents: Vec<_> = p
                .get_stored_torrents()
                .into_iter()
                .map(|id| KeeperTorrent {
                    keeper: Cow::from(p.author.as_str()),
                    topic_id: id,
                })
                .collect();
            insert_or_ignore_into(keeper_torrents::table)
                .values(&torrents)
                .execute(&self.sqlite)?;
        }
        let tor_for_update = keeper_torrents::table
            .select(keeper_torrents::topic_id)
            .filter(keeper_torrents::topic_id.ne_all(torrents::table.select(torrents::topic_id)))
            .get_results(&self.sqlite)?;
        self.update_torrent_data(tor_for_update)?;
        let buf = keeper_torrents::table
            .inner_join(torrents::table)
            .select((
                keeper_torrents::keeper,
                sql::<Integer>("count(keeper_torrents.topic_id)"),
                sql::<Double>("sum(size)"),
            ))
            .group_by(keeper_torrents::keeper)
            .filter(torrents::forum_id.eq(forum_id))
            .get_results::<(String, i32, f64)>(&self.sqlite)?;
        let mut vec: Vec<(String, i32, f64)> = vec![("".to_owned(), 0, 0f64); buf.len()];
        for v in buf {
            let i = keeper[v.0.as_str()];
            vec[i] = v;
        }
        Ok(vec)
    }

    pub fn get_local_list_size(&self, forum_id: &[i16]) -> Result<Vec<(i16, String, i32, f64)>> {
        Ok(torrents::table
            .inner_join(local_torrents::table.on(local_torrents::hash.eq(torrents::hash)))
            .inner_join(forums::table.inner_join(topics::table))
            .select((
                forums::id,
                topics::title,
                sql::<Integer>("count(torrents.topic_id)"),
                sql::<Double>("sum(size)"),
            ))
            .group_by(forums::id)
            .filter(torrents::forum_id.eq_any(forum_id))
            .filter(local_torrents::status.eq(client::TorrentStatus::Seeding as i16))
            .get_results(&self.sqlite)?)
    }

    pub fn get_local_tor_by_forum(&self, forum_id: i16) -> Result<Vec<(i32, String, f64)>> {
        Ok(torrents::table
            .inner_join(local_torrents::table.on(local_torrents::hash.eq(torrents::hash)))
            .filter(torrents::forum_id.eq(forum_id))
            .filter(local_torrents::status.eq(client::TorrentStatus::Seeding as i16))
            .select((torrents::topic_id, torrents::title, torrents::size))
            .load(&self.sqlite)?)
    }

    pub fn get_topic_id(&self, hash: &[String]) -> Result<Vec<i32>> {
        Ok(torrents::table
            .select(torrents::topic_id)
            .filter(torrents::hash.eq_any(hash))
            .get_results(&self.sqlite)?)
    }

    pub fn get_topics(&self, forum_id: &[i16]) -> Result<HashMap<i16, RutrackerTopic>> {
        let forum = self.forum.get_keepers_working_forum();
        let map: HashMap<i16, RutrackerTopic> = forums::table
            .inner_join(topics::table)
            .select((forums::id, topics::id, topics::author, topics::title))
            .filter(forums::id.eq_any(forum_id))
            .get_results::<(i16, i32, String, String)>(&self.sqlite)?
            .into_iter()
            .map(|(forum_id, topic_id, author, title)| {
                (forum_id, forum.get_topic(topic_id, author, title))
            })
            .collect();
        if map.len() == forum_id.len() {
            Ok(map)
        } else {
            self.update_subforum_info()?;
            Ok(forums::table
                .inner_join(topics::table)
                .select((forums::id, topics::id, topics::author, topics::title))
                .filter(forums::id.eq_any(forum_id))
                .get_results::<(i16, i32, String, String)>(&self.sqlite)?
                .into_iter()
                .map(|(forum_id, topic_id, author, title)| {
                    (forum_id, forum.get_topic(topic_id, author, title))
                })
                .collect())
        }
    }

    pub fn get_torrents_for_change(
        &self, url: &str, forum_id: i16, seeders: (i16, i16), status: &[i16],
    ) -> Result<Vec<String>> {
        Ok(torrents::table
            .inner_join(local_torrents::table.on(local_torrents::hash.eq(torrents::hash)))
            .select(torrents::hash)
            .filter(torrents::forum_id.eq(forum_id))
            .filter(local_torrents::url.eq(url))
            .filter(torrents::seeders.between(seeders.0, seeders.1 - 1))
            .filter(local_torrents::status.eq_any(status))
            .get_results(&self.sqlite)?)
    }

    pub fn save_torrent(&self, torrent: Vec<client::Torrent>, url: &str) -> Result<()> {
        let local: Vec<LocalTorrent<'_>> = torrent
            .into_iter()
            .map(|t| LocalTorrent {
                hash: t.hash,
                status: t.status as i16,
                url: Cow::from(url),
            })
            .collect();
        insert_into(local_torrents::table)
            .values(&local)
            .execute(&self.sqlite)?;
        let unavailable = local_torrents::table
            .select(local_torrents::hash)
            .filter(local_torrents::hash.ne_all(torrents::table.select(torrents::hash)))
            .load::<String>(&self.sqlite)?;
        if !unavailable.is_empty() {
            let topic_id: Vec<_> = self
                .api
                .get_topic_id(unavailable)?
                .into_iter()
                .map(|(_, v)| v)
                .collect();
            self.update_torrent_data(topic_id)?;
        }
        Ok(())
    }

    pub fn set_status_by_hash(&self, status: i16, hash: &[String]) -> Result<()> {
        update(local_torrents::table)
            .filter(local_torrents::hash.eq_any(hash))
            .set(local_torrents::status.eq(status))
            .execute(&self.sqlite)?;
        Ok(())
    }

    pub fn set_status_by_id(&self, status: i16, topic_id: &[i32]) -> Result<()> {
        self.set_status_by_hash(
            status,
            &torrents::table
                .select(torrents::hash)
                .filter(torrents::topic_id.eq_any(topic_id))
                .get_results(&self.sqlite)?,
        )?;
        Ok(())
    }

    pub fn update_subforum_info(&self) -> Result<()> {
        let keepers_working_forum = self.forum.get_keepers_working_forum();
        let topics: Vec<Topic> = keepers_working_forum
            .get_topics()?
            .into_iter()
            .map(|t| {
                let t = Rc::try_unwrap(t.0).unwrap_or_else(|e| (*e).clone());
                Topic {
                    id: t.id,
                    author: t.author,
                    title: t.title,
                }
            })
            .collect();
        replace_into(topics::table)
            .values(&topics)
            .execute(&self.sqlite)?;

        let mut forum_size = self.api.forum_size()?;
        let mut forum_name: HashMap<String, i16> = self
            .api
            .get_forum_name(forum_size.keys().cloned().collect())?
            .into_iter()
            .map(|(k, v)| (v, k))
            .collect();

        let forums: Vec<Forum> = topics
            .iter()
            .filter_map(|t| {
                let name = t.title.split(" » ").last()?;
                let id = forum_name.remove(name)?;
                let (tor_count, tor_size_bytes) = forum_size.remove(&id)?;
                Some(Forum {
                    id,
                    name: name.to_owned(),
                    tor_count,
                    tor_size_bytes,
                    topic_id: t.id,
                })
            })
            .collect();
        replace_into(forums::table)
            .values(&forums)
            .execute(&self.sqlite)?;
        Ok(())
    }

    pub fn update_torrent_data(&self, topic_id: Vec<i32>) -> Result<()> {
        if !topic_id.is_empty() {
            let tor_data: Vec<Torrent> = self
                .api
                .get_tor_topic_data(topic_id)?
                .into_iter()
                .map(|(id, data)| Torrent {
                    topic_id: id,
                    forum_id: data.forum_id,
                    poster_id: data.poster_id,
                    title: data.topic_title,
                    hash: data.info_hash,
                    size: data.size,
                    reg_time: data.reg_time,
                    status: data.tor_status,
                    seeders: data.seeders,
                })
                .collect();
            replace_into(torrents::table)
                .values(&tor_data)
                .execute(&self.sqlite)?;
        }
        Ok(())
    }

    pub fn update_torrent_info(&self, forum_id: i16) -> Result<()> {
        let tor_info = self.api.pvc(forum_id)?;
        self.sqlite.transaction::<_, ::failure::Error, _>(|| {
            for (id, info) in &tor_info {
                update(torrents::table)
                    .filter(torrents::topic_id.eq(id))
                    .filter(torrents::reg_time.eq(info.reg_time))
                    .set((
                        torrents::status.eq(info.tor_status),
                        torrents::seeders.eq(info.seeders),
                    ))
                    .execute(&self.sqlite)?;
            }
            Ok(())
        })?;
        let tor_for_update = self
            .sqlite
            .transaction::<Vec<i32>, ::failure::Error, _>(|| {
                Ok(tor_info
                    .iter()
                    .map(|(id, info)| {
                        torrents::table
                            .select(torrents::topic_id)
                            .filter(torrents::topic_id.eq(id))
                            .filter(torrents::reg_time.ne(info.reg_time))
                            .get_result::<i32>(&self.sqlite)
                            .optional()
                    })
                    .collect::<QueryResult<Vec<Option<i32>>>>()?
                    .into_iter()
                    .filter_map(|v| v)
                    .collect())
            })?;
        self.update_torrent_data(tor_for_update)?;
        Ok(())
    }
}
