use bincode::{self, deserialize, serialize};
use lmdb::{self, Transaction};
use serde::{de::DeserializeOwned, Serialize};
use slog::Value;
use std::collections::HashMap;
use std::fmt::Debug;
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
    TopicId,
    SubforumList,
    TopicInfo,
    LocalList,
    TopicData,
    KeeperList,
}

#[derive(Debug)]
pub struct Database {
    env: lmdb::Environment,
    topic_id: lmdb::Database,
    subforum_list: lmdb::Database,
    topic_info: lmdb::Database,
    local_list: lmdb::Database,
    topic_data: lmdb::Database,
    keeper_list: lmdb::Database,
}

impl Database {
    pub fn new() -> Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_readers(1)
            .set_max_dbs(6)
            .set_map_size(10485760 * 10)
            .open(Path::new("db"))?;
        let empty = lmdb::DatabaseFlags::empty();
        let topic_id = env.create_db(Some("topic_id"), empty)?;
        let subforum_list = env.create_db(Some("subforum_list"), empty)?;
        let topic_info = env.create_db(Some("topic_info"), empty)?;
        let local_list = env.create_db(Some("local_list"), empty)?;
        let topic_data = env.create_db(Some("topic_data"), empty)?;
        let keeper_list = env.create_db(Some("keeper_list"), empty)?;

        Ok(Database {
            env,
            topic_id,
            subforum_list,
            topic_info,
            local_list,
            topic_data,
            keeper_list,
        })
    }

    fn get_db(&self, db_name: DBName) -> lmdb::Database {
        match db_name {
            DBName::TopicId => self.topic_id,
            DBName::SubforumList => self.subforum_list,
            DBName::TopicInfo => self.topic_info,
            DBName::LocalList => self.local_list,
            DBName::TopicData => self.topic_data,
            DBName::KeeperList => self.keeper_list,
        }
    }

    pub fn put<K, V>(&self, db_name: DBName, key: &K, value: &V) -> Result<()>
    where
        K: Serialize + Value,
        V: Serialize + Debug,
    {
        let mut rw_txn = self.env.begin_rw_txn()?;
        rw_txn.put(
            self.get_db(db_name),
            &serialize(key)?,
            &serialize(value)?,
            lmdb::WriteFlags::empty(),
        )?;
        rw_txn.commit()?;
        trace!("Database::put"; "value" => ?value, "key" => key);
        Ok(())
    }

    pub fn put_map<K, V>(&self, db_name: DBName, map: &HashMap<K, V>) -> Result<()>
    where
        K: Hash + Eq + Serialize + Value,
        V: Serialize + Debug,
    {
        let mut rw_txn = self.env.begin_rw_txn()?;
        for (key, val) in map {
            rw_txn.put(
                self.get_db(db_name),
                &serialize(key)?,
                &serialize(val)?,
                lmdb::WriteFlags::empty(),
            )?;
            trace!("Database::put_map"; "value" => ?val, "key" => key);
        }
        rw_txn.commit()?;
        Ok(())
    }

    pub fn get<K, V>(&self, db_name: DBName, key: &K) -> Result<V>
    where
        K: Serialize + Value,
        V: DeserializeOwned + Debug,
    {
        trace!("Database::get"; "key" => key);
        let ro_txn = self.env.begin_ro_txn()?;
        let res = match ro_txn.get(self.get_db(db_name), &serialize(key)?) {
            Ok(val) => deserialize(val)?,
            Err(err) => return Err(err.into()),
        };
        ro_txn.commit()?;
        trace!("Database::get"; "value" => ?&res, "key" => key);
        Ok(res)
    }

    pub fn get_map<K, V>(&self, db_name: DBName, keys: Vec<K>) -> Result<HashMap<K, Option<V>>>
    where
        K: Hash + Eq + Serialize + Value,
        V: DeserializeOwned + Debug,
    {
        let mut map: HashMap<K, Option<V>> = HashMap::with_capacity(keys.len());
        let ro_txn = self.env.begin_ro_txn()?;
        for key in keys {
            let val: Option<V> = match ro_txn.get(self.get_db(db_name), &serialize(&key)?) {
                Ok(val) => Some(deserialize(val)?),
                Err(lmdb::Error::NotFound) => None,
                Err(err) => return Err(err.into()),
            };
            trace!("Database::get_map"; "value" => ?&val, "key" => &key, "db_name" => ?db_name);
            map.insert(key, val);
        }
        ro_txn.commit()?;
        Ok(map)
    }

    pub fn clear_db(&self, db_name: DBName) -> Result<()> {
        let mut rw_txn = self.env.begin_rw_txn()?;
        rw_txn.clear_db(self.get_db(db_name))?;
        rw_txn.commit()?;
        Ok(())
    }
}
