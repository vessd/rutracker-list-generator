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
pub enum DatabaseName {
    KeepersLists,
    SubforumList,
    TorrentList,
    TorrentData,
}

#[derive(Debug)]
pub struct Database {
    env: lmdb::Environment,
    keepers_lists: lmdb::Database,
    subforum_list: lmdb::Database,
    torrent_list: lmdb::Database,
    torrent_data: lmdb::Database,
}

impl Database {
    pub fn new() -> Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_dbs(4)
            .open(Path::new("db"))?;
        let keepers_lists = env.create_db(Some("keepers_lists"), lmdb::DatabaseFlags::empty())?;
        let subforum_list = env.create_db(Some("subforum_list"), lmdb::DatabaseFlags::empty())?;
        let torrent_list = env.create_db(Some("torrent_list"), lmdb::DatabaseFlags::empty())?;
        let torrent_data = env.create_db(Some("torrent_data"), lmdb::DatabaseFlags::empty())?;
        Ok(Database {
            env,
            keepers_lists,
            subforum_list,
            torrent_list,
            torrent_data,
        })
    }

    fn get_db(&self, db_name: DatabaseName) -> lmdb::Database {
        match db_name {
            DatabaseName::KeepersLists => self.keepers_lists,
            DatabaseName::SubforumList => self.subforum_list,
            DatabaseName::TorrentList => self.torrent_list,
            DatabaseName::TorrentData => self.torrent_data,
        }
    }

    pub fn put<K, V>(&self, db_name: DatabaseName, map: &HashMap<K, V>) -> Result<()>
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

    pub fn get<K, V>(&self, db_name: DatabaseName, keys: Vec<K>) -> Result<HashMap<K, Option<V>>>
    where
        K: Hash + Eq + DeserializeOwned + Serialize,
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

    pub fn clear_db(&self, db_name: DatabaseName) -> Result<()> {
        let mut rw_txn = self.env.begin_rw_txn()?;
        rw_txn.clear_db(self.get_db(db_name))?;
        rw_txn.commit().map_err(From::from)
    }
}
