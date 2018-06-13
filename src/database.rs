use bincode::{self, deserialize, serialize};
use lmdb::{self, Transaction};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::{cmp::Eq, hash::Hash};

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Lmdb(err: lmdb::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Bincode(err: bincode::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DBName {
    ForumList,
    TopicInfo,
    TopicData,
    KeepersLists,
}

#[derive(Debug)]
pub struct Database {
    env: lmdb::Environment,
    forum_list: lmdb::Database,
    topic_info: lmdb::Database,
    topic_data: lmdb::Database,
    keepers_lists: lmdb::Database,
}

impl Database {
    pub fn new() -> Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_dbs(4)
            .open(Path::new("db"))?;
        let empty = lmdb::DatabaseFlags::empty();
        let forum_list = env.create_db(Some("ForumList"), empty)?;
        let topic_info = env.create_db(Some("TopicInfo"), empty)?;
        let topic_data = env.create_db(Some("TopicData"), empty)?;
        let keepers_lists = env.create_db(Some("KeepersLists"), empty)?;
        Ok(Database {
            env,
            forum_list,
            topic_info,
            topic_data,
            keepers_lists,
        })
    }

    fn get_db(&self, db_name: DBName) -> lmdb::Database {
        match db_name {
            DBName::ForumList => self.forum_list,
            DBName::TopicInfo => self.topic_info,
            DBName::TopicData => self.topic_data,
            DBName::KeepersLists => self.keepers_lists,
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
        rw_txn.commit().map_err(From::from)
    }

    pub fn put_map<K, V>(&self, db_name: DBName, map: &HashMap<K, V>) -> Result<()>
    where
        K: Hash + Eq + Serialize,
        V: Serialize,
    {
        let mut rw_txn = self.env.begin_rw_txn()?;
        for (key, data) in map {
            rw_txn.put(
                self.get_db(db_name),
                &serialize(key)?,
                &serialize(data)?,
                lmdb::WriteFlags::empty(),
            )?
        }
        rw_txn.commit().map_err(From::from)
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

    pub fn get_map<K, V>(&self, db_name: DBName, keys: Vec<K>) -> Result<HashMap<K, Option<V>>>
    where
        K: Hash + Eq + Serialize,
        V: DeserializeOwned,
    {
        let mut map: HashMap<K, Option<V>> = HashMap::new();
        let ro_txn = self.env.begin_ro_txn()?;
        for key in keys {
            let val: Option<V> = match ro_txn.get(self.get_db(db_name), &serialize(&key)?) {
                Ok(val) => Some(deserialize(val)?),
                Err(lmdb::Error::NotFound) => None,
                Err(err) => return Err(err.into()),
            };
            map.insert(key, val);
        }
        ro_txn.commit()?;
        Ok(map)
    }

    pub fn clear_db(&self, db_name: DBName) -> Result<()> {
        let mut rw_txn = self.env.begin_rw_txn()?;
        rw_txn.clear_db(self.get_db(db_name))?;
        rw_txn.commit().map_err(From::from)
    }
}
