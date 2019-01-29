use crate::client::{TorrentClient, TorrentStatus};
use crate::config::Subforum;
use crate::database::Database;

pub type Result<T> = std::result::Result<T, failure::Error>;

#[derive(Debug)]
pub struct Control<'a> {
    clients: Vec<Box<dyn TorrentClient>>,
    db: &'a Database,
    dry_run: bool,
}

impl<'a> Control<'a> {
    pub fn new(db: &'a Database, dry_run: bool) -> Self {
        Control {
            clients: Vec::new(),
            db,
            dry_run,
        }
    }

    pub fn add_client(&mut self, client: Box<dyn TorrentClient>) -> Result<()> {
        self.db.save_torrent(client.list()?, client.url())?;
        self.clients.push(client);
        Ok(())
    }

    pub fn start(&mut self, forum_id: i16, stop: i16) {
        let range = (0, stop);
        let status_vec = &[TorrentStatus::Stopped as i16];
        let mut count = 0;
        for client in &mut self.clients {
            let hash = error_try!(
                    self.db
                        .get_torrents_for_change(client.url(), forum_id, range, status_vec),
                    continue,
                    "Не удалось получить список раздач для запуска: {}"
                );
            if self.dry_run {
                error_try!(
                    self.db.get_topic_id(&hash),
                    continue,
                    "Не удалось получить id раздач: {}"
                )
                .iter()
                .for_each(|id| info!("Раздача с id {} будет запущена", id));
            } else {
                error_try!(
                    client.start(&hash),
                    continue,
                    "Не удалось запустить раздачи: {}"
                );
                count += hash.len();
                error_try!(
                    self.db
                        .set_status_by_hash(TorrentStatus::Seeding as i16, &hash),
                    continue,
                    "Не удалось изменить статус раздач в базе данных: {}"
                );
            }
        }
        info!("Запущено раздач: {}", count);
    }

    pub fn stop(&mut self, forum_id: i16, stop: i16, remove: i16) {
        let range = (stop, remove);
        let status_vec = &[TorrentStatus::Seeding as i16];
        let mut count = 0;
        for client in &mut self.clients {
            let hash = error_try!(
                    self.db
                        .get_torrents_for_change(client.url(), forum_id, range, status_vec),
                    continue,
                    "Не удалось получить список раздач для остановки: {}"
                );
            if self.dry_run {
                error_try!(
                    self.db.get_topic_id(&hash),
                    continue,
                    "Не удалось получить id раздач: {}"
                )
                .iter()
                .for_each(|id| {
                    info!(
                        "Раздача с id {} будет остановлена",
                        id
                    )
                });
            } else {
                error_try!(
                    client.stop(&hash),
                    continue,
                    "Не удалось остановить раздачи: {}"
                );
                count += hash.len();
                error_try!(
                    self.db
                        .set_status_by_hash(TorrentStatus::Stopped as i16, &hash),
                    continue,
                    "Не удалось изменить статус раздач в базе данных: {}"
                );
            }
        }
        info!("Остановлено раздач: {}", count);
    }

    pub fn remove(&mut self, forum_id: i16, remove: i16) {
        let range = (remove, i16::max_value());
        let status_vec = &[TorrentStatus::Seeding as i16, TorrentStatus::Stopped as i16];
        let mut count = 0;
        for client in &mut self.clients {
            let hash = error_try!(
                    self.db
                        .get_torrents_for_change(client.url(), forum_id, range, status_vec),
                    continue,
                    "Не удалось получить список раздач для удаления: {}"
                );
            if self.dry_run {
                error_try!(
                    self.db.get_topic_id(&hash),
                    continue,
                    "Не удалось получить id раздач: {}"
                )
                .iter()
                .for_each(|id| info!("Раздача с id {} будет удалена", id));
            } else {
                error_try!(
                    client.remove(&hash, true),
                    continue,
                    "Не удалось удалить раздачи: {}"
                );
                count += hash.len();
                error_try!(
                    self.db.delete_by_hash(&hash),
                    continue,
                    "Не удалось удалить раздачи из базы данных: {}"
                );
            }
        }
        info!("Удалено раздач: {}", count);
    }

    pub fn apply_config(&mut self, forum: &Subforum) {
        for id in forum.id.iter().cloned() {
            error_try!(
                self.db.update_torrent_info(id),
                continue,
                "Не удалось обновить информацию о раздачах: {}"
            );
            self.remove(id, forum.remove);
            self.stop(id, forum.stop, forum.remove);
            self.start(id, forum.download);
        }
    }
}
