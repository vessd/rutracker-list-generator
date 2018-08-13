use bincode::{deserialize, serialize};
use client::TorrentStatus;
use lmdb::{self, Cursor, Transaction};
use rutracker::api::{RutrackerApi, TopicData, TopicInfo};
use rutracker::forum::Forum;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::{cmp::Eq, hash::Hash};

pub type Result<T> = ::std::result::Result<T, ::failure::Error>;

#[derive(Debug, Clone, Copy)]
pub enum DBName {
    ForumName,
    ForumSize,
    ForumStat,
    ForumList,
    TopicId,
    TopicInfo,
    TopicData,
    LocalList,
    KeeperList,
}

#[derive(Debug)]
pub struct Database {
    api: RutrackerApi,
    env: lmdb::Environment,
    forum_name: lmdb::Database,
    forum_size: lmdb::Database,
    forum_stat: lmdb::Database,
    forum_list: lmdb::Database,
    topic_id: lmdb::Database,
    topic_info: lmdb::Database,
    topic_data: lmdb::Database,
    local_list: lmdb::Database,
    keeper_list: lmdb::Database,
}

impl Database {
    pub fn new(api: RutrackerApi) -> Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_readers(1)
            .set_max_dbs(9)
            .set_map_size(10485760 * 10)
            .open(Path::new("db"))?;
        let forum_name = env.create_db(Some("forum_name"), lmdb::DatabaseFlags::empty())?;
        let forum_size = env.create_db(Some("forum_size"), lmdb::DatabaseFlags::empty())?;
        let forum_stat = env.create_db(Some("forum_stat"), lmdb::DatabaseFlags::empty())?;
        let forum_list = env.create_db(Some("forum_list"), lmdb::DatabaseFlags::empty())?;
        let topic_id = env.create_db(Some("topic_id"), lmdb::DatabaseFlags::empty())?;
        let topic_info = env.create_db(Some("topic_info"), lmdb::DatabaseFlags::empty())?;
        let topic_data = env.create_db(Some("topic_data"), lmdb::DatabaseFlags::empty())?;
        let local_list = env.create_db(Some("local_list"), lmdb::DatabaseFlags::empty())?;
        let keeper_list = env.create_db(Some("keeper_list"), lmdb::DatabaseFlags::empty())?;

        let database = Database {
            api,
            env,
            forum_name,
            forum_size,
            forum_stat,
            forum_list,
            topic_id,
            topic_info,
            topic_data,
            local_list,
            keeper_list,
        };

        database.clear_db(DBName::ForumSize)?;
        database.clear_db(DBName::ForumList)?;
        database.clear_db(DBName::TopicInfo)?;
        database.clear_db(DBName::LocalList)?;

        Ok(database)
    }

    fn get_db(&self, db_name: DBName) -> lmdb::Database {
        match db_name {
            DBName::ForumName => self.forum_name,
            DBName::ForumSize => self.forum_size,
            DBName::ForumStat => self.forum_stat,
            DBName::ForumList => self.forum_list,
            DBName::TopicId => self.topic_id,
            DBName::TopicInfo => self.topic_info,
            DBName::TopicData => self.topic_data,
            DBName::LocalList => self.local_list,
            DBName::KeeperList => self.keeper_list,
        }
    }

    pub fn put<K, V>(&self, db_name: DBName, key: &K, value: &V) -> Result<()>
    where
        K: Serialize,
        V: Serialize,
    {
        let mut rw_txn = self.env.begin_rw_txn()?;
        rw_txn.put(
            self.get_db(db_name),
            &serialize(key)?,
            &serialize(value)?,
            lmdb::WriteFlags::empty(),
        )?;
        rw_txn.commit()?;
        Ok(())
    }

    pub fn put_map<K, V>(&self, db_name: DBName, map: &HashMap<K, V>) -> Result<()>
    where
        K: Hash + Eq + Serialize,
        V: Serialize,
    {
        let mut rw_txn = self.env.begin_rw_txn()?;
        for (key, val) in map {
            rw_txn.put(
                self.get_db(db_name),
                &serialize(key)?,
                &serialize(val)?,
                lmdb::WriteFlags::empty(),
            )?;
        }
        rw_txn.commit()?;
        Ok(())
    }

    pub fn get<K, V>(&self, db_name: DBName, key: &K) -> Result<Option<V>>
    where
        K: Serialize,
        V: DeserializeOwned,
    {
        let ro_txn = self.env.begin_ro_txn()?;
        let res = match ro_txn.get(self.get_db(db_name), &serialize(key)?) {
            Ok(val) => Some(deserialize(val)?),
            Err(lmdb::Error::NotFound) => None,
            Err(err) => return Err(err.into()),
        };
        ro_txn.commit()?;
        Ok(res)
    }

    pub fn get_map<K, V>(&self, db_name: DBName, keys: &[K]) -> Result<HashMap<K, Option<V>>>
    where
        K: Hash + Eq + Serialize + Clone,
        V: DeserializeOwned,
    {
        let mut map = HashMap::with_capacity(keys.len());
        let ro_txn = self.env.begin_ro_txn()?;
        for key in keys {
            let val: Option<V> = match ro_txn.get(self.get_db(db_name), &serialize(key)?) {
                Ok(val) => Some(deserialize(val)?),
                Err(lmdb::Error::NotFound) => None,
                Err(err) => return Err(err.into()),
            };
            map.insert(key.clone(), val);
        }
        ro_txn.commit()?;
        Ok(map)
    }

    pub fn get_by_filter<K, V, P>(&self, db_name: DBName, filter: P) -> Result<HashMap<K, V>>
    where
        K: Hash + Eq + DeserializeOwned,
        V: DeserializeOwned,
        P: Fn(&K, &V) -> bool,
    {
        let ro_txn = self.env.begin_ro_txn()?;
        let map = ro_txn
            .open_ro_cursor(self.get_db(db_name))?
            .iter()
            .filter_map(|(k, v)| {
                let k = deserialize(k).ok()?;
                let v = deserialize(v).ok()?;
                if filter(&k, &v) {
                    Some((k, v))
                } else {
                    None
                }
            })
            .collect();
        ro_txn.commit()?;
        Ok(map)
    }

    pub fn clear_db(&self, db_name: DBName) -> Result<()> {
        let mut rw_txn = self.env.begin_rw_txn()?;
        rw_txn.clear_db(self.get_db(db_name))?;
        rw_txn.commit()?;
        Ok(())
    }

    pub fn get_forum_name<T>(&self, forum_id: T) -> Result<HashMap<usize, String>>
    where
        T: AsRef<[usize]>,
    {
        let buf = self.get_map(DBName::ForumName, forum_id.as_ref())?;
        let mut map = self.api.get_forum_name(
            buf.iter()
                .filter(|(_, v)| v.is_none())
                .map(|(k, _)| k)
                .collect(),
        )?;
        self.put_map(DBName::ForumName, &map)?;
        map.extend(buf.into_iter().filter_map(|(k, v)| Some((k, v?))));
        Ok(map)
    }

    pub fn get_forum_size(&self, forum_id: usize) -> Result<Option<(usize, f64)>> {
        let mut buf = self.get(DBName::ForumSize, &forum_id)?;
        if buf.is_none() {
            let mut map = self.api.forum_size()?;
            self.put_map(DBName::ForumSize, &map)?;
            buf = map.remove(&forum_id);
        }
        Ok(buf)
    }

    pub fn get_topic_id<'a, T>(&self, hash: T) -> Result<HashMap<String, usize>>
    where
        T: AsRef<[&'a str]>,
    {
        let buf = self.get_map(DBName::TopicId, hash.as_ref())?;
        let mut map = self.api.get_topic_id(
            buf.iter()
                .filter(|(_, v)| v.is_none())
                .map(|(k, _)| *k)
                .collect(),
        )?;
        self.put_map(DBName::TopicId, &map)?;
        map.extend(
            buf.into_iter()
                .filter_map(|(k, v)| Some((k.to_owned(), v?))),
        );
        Ok(map)
    }

    pub fn get_tor_topic_data<T>(&self, topic_id: T) -> Result<HashMap<usize, TopicData>>
    where
        T: AsRef<[usize]>,
    {
        let buf = self.get_map(DBName::TopicData, topic_id.as_ref())?;
        let mut map = self.api.get_tor_topic_data(
            buf.iter()
                .filter(|(_, v)| v.is_none())
                .map(|(k, _)| k)
                .collect(),
        )?;
        self.put_map(DBName::TopicData, &map)?;
        map.extend(buf.into_iter().filter_map(|(k, v)| Some((k, v?))));
        Ok(map)
    }

    pub fn pvc<T>(&self, forum_id: usize, topic_id: Option<T>) -> Result<HashMap<usize, TopicInfo>>
    where
        T: AsRef<[usize]>,
    {
        let id: Vec<usize> = if let Some(topic_id) = topic_id.as_ref() {
            self.get_map(DBName::ForumList, topic_id.as_ref())?
                .into_iter()
                .filter(|(_, v)| *v == Some(forum_id))
                .map(|(k, _)| k)
                .collect()
        } else {
            self.get_by_filter(DBName::ForumList, |_, v: &usize| *v == forum_id)?
                .into_iter()
                .map(|(k, _)| k)
                .collect()
        };

        if !id.is_empty() {
            Ok(self
                .get_map(DBName::TopicInfo, &id)?
                .into_iter()
                .filter_map(|(k, v)| Some((k, v?)))
                .collect())
        } else {
            let mut map = self.api.pvc(forum_id)?;
            self.put_map(DBName::TopicInfo, &map)?;
            self.put_map(
                DBName::ForumList,
                &map.keys().map(|id| (*id, forum_id)).collect(),
            )?;
            if let Some(topic_id) = topic_id {
                Ok(topic_id
                    .as_ref()
                    .iter()
                    .filter_map(|id| Some((*id, map.remove(id)?)))
                    .collect())
            } else {
                Ok(map)
            }
        }
    }

    pub fn get_local_by_forum(&self, forum_id: usize) -> Result<HashMap<usize, TorrentStatus>> {
        let mut map = self.get_by_filter(DBName::LocalList, |_, _| true)?;
        let buf = self.get_map(
            DBName::ForumList,
            &map.keys().cloned().collect::<Vec<usize>>(),
        )?;
        map.retain(|k, _| buf.get(k) == Some(&Some(forum_id)));
        Ok(map)
    }

    pub fn get_topic_with_subforum_list<'a>(
        &self,
        forum: &'a Forum,
        forum_id: &[usize],
    ) -> Result<HashMap<usize, (usize, String, String)>> {
        let buf: HashMap<usize, Option<(usize, String, String)>> =
            self.get_map(DBName::ForumStat, forum_id)?;
        if buf.values().any(|v| v.is_none()) {
            let forum_name = self.get_forum_name(forum_id)?;
            let mut forum_name: HashMap<&str, usize> =
                forum_name.iter().map(|(k, v)| (v.as_str(), *k)).collect();
            let map: HashMap<usize, (usize, String, String)> = forum
                .get_topics()?
                .into_iter()
                .filter_map(|t| {
                    let forum_id = forum_name.remove(t.title.split(" » ").last()?)?;
                    Some((forum_id, (t.id, t.author, t.title)))
                })
                .collect();
            self.put_map(DBName::ForumStat, &map)?;
            Ok(map)
        } else {
            Ok(buf
                .into_iter()
                .map(|(forum_id, v)| {
                    let (id, author, title) = v.unwrap();
                    (forum_id, (id, author, title))
                })
                .collect())
        }
    }
}
