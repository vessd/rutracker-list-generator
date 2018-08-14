use chrono::Local;
use database::{DBName, Database};
use rutracker::forum::{Post, RutrackerForum, Topic, MESSAGE_LEN};

pub type Result<T> = ::std::result::Result<T, ::failure::Error>;

#[derive(Debug)]
pub struct List {
    forum: usize,
    size: f64,
    id: Vec<usize>,
}

impl List {
    pub fn new(db: &Database, forum: usize, id: Vec<usize>) -> Result<Self> {
        let size = db
            .get_tor_topic_data(&id)?
            .iter()
            .map(|(_, data)| data.size)
            .sum();
        Ok(Self { forum, id, size })
    }

    fn convert_size(size: f64) -> String {
        if size < 1f64 {
            format!("{:.2} B", size)
        } else {
            match size.log2() as usize {
                0...9 => format!("{:.2} B", size),
                10...19 => format!("{:.2} KB", size / 10f64.exp2()),
                20...29 => format!("{:.2} МB", size / 20f64.exp2()),
                _ => format!("{:.2} GB", size / 30f64.exp2()),
            }
        }
    }

    pub fn to_bbcode(&self, db: &Database, date: &str, max_len: usize) -> Result<Vec<String>> {
        let header_len = 200;
        let body_len = max_len - header_len;
        let mut message = String::with_capacity(max_len);
        message.push_str(
            format!(
                "Актуально на: [color=darkblue]{}[/color]\n\
                 Всего хранимых раздач в подразделе: {} шт. / {}\n",
                date,
                self.id.len(),
                List::convert_size(self.size)
            ).as_str(),
        );
        let mut item: Vec<(usize, String, f64)> = db
            .get_tor_topic_data(&self.id)?
            .into_iter()
            .map(|(id, data)| (id, data.topic_title, data.size))
            .collect();
        item.sort_unstable_by(|a, b| a.1.as_str().cmp(b.1.as_str()));
        let mut item: Vec<String> = item
            .into_iter()
            .map(|(id, topic_title, size)| {
                format!(
                    "[*][url=viewtopic.php?t={}]{}[/url] {}\n",
                    id,
                    topic_title,
                    List::convert_size(size)
                )
            })
            .collect();
        let mut num = Vec::new();
        let mut len = 0;
        for (n, s) in item.iter().enumerate() {
            if len + s.len() > body_len {
                len = 0;
                num.push(n);
            } else {
                len += s.len();
            }
        }
        debug!("List::to_bbcode::num {:?}", num);
        let mut vec = Vec::with_capacity(num.len() + 1);
        message.push_str(
            format!(
                "[spoiler=\"№№ 1 — {}\"][list=1]\n",
                num.get(0).unwrap_or(&self.id.len())
            ).as_str(),
        );
        item.iter()
            .take(*num.get(0).unwrap_or(&self.id.len()))
            .for_each(|i| message.push_str(i));
        message.push_str("[/list][/spoiler]\n");
        vec.push(message);
        for i in 0..num.len() {
            let mut message = String::with_capacity(max_len);
            message.push_str(
                format!(
                    "[spoiler=\"№№ {} — {}\"][list=1]\n",
                    num[i] + 1,
                    num.get(i + 1).unwrap_or(&self.id.len())
                ).as_str(),
            );
            item[num[i]].insert_str(2, format!("={}", num[i] + 1).as_str());
            item.iter()
                .skip(num[i])
                .take(*num.get(i + 1).unwrap_or(&(self.id.len() + 1)) - num[i])
                .for_each(|i| message.push_str(i));
            message.push_str("[/list][/spoiler]\n");
            vec.push(message);
        }
        Ok(vec)
    }
}

#[derive(Debug)]
pub struct Report<'a> {
    list: Vec<List>,
    date: String,
    db: &'a Database,
    forum: &'a RutrackerForum,
}

impl<'a> Report<'a> {
    pub fn new(db: &'a Database, forum: &'a RutrackerForum) -> Self {
        Self {
            list: Vec::new(),
            date: Local::now().format("%d.%m.%Y").to_string(),
            db,
            forum,
        }
    }

    pub fn add_list(&mut self, list: List) {
        self.list.push(list);
    }

    pub fn send_list_header(&self, forum_id: usize, topic: &Topic, post: &Post) -> Result<()> {
        info!("Формирование статиски подраздела...");
        let forum_size = self.db.get_forum_size(forum_id)?.unwrap_or((0, 0f64));
        let (keeper, torrent_id) = topic.get_stored_torrents()?;
        for (i, k) in keeper.iter().enumerate() {
            self.db.put(DBName::KeeperList, k, &torrent_id[i])?;
        }
        let mut torrent: Vec<usize> = torrent_id.iter().flat_map(|id| id).cloned().collect();
        torrent.sort_unstable();
        torrent.dedup();
        let torrent_info = self.db.get_tor_topic_data(torrent)?;
        let count = torrent_info.len();
        let size = torrent_info.values().map(|d| d.size).sum();
        let mut message = format!(
            "[url=viewforum.php?f={}][u][color=#006699]{}[/u][/color][/url] \
             | [url=tracker.php?f={0}&tm=-1&o=10&s=1&oop=1][color=indigo]\
             [u]Проверка сидов[/u][/color][/url]\n\n\
             Актуально на: [color=darkblue]{}[/color]\n\
             Всего раздач в подразделе: {} шт. / {}\n\
             Всего хранимых раздач в подразделе: \
             {} шт. / {}\n\
             Количество хранителей: {}\n[hr]\n",
            forum_id,
            topic.title.split(" » ").last().unwrap_or(""),
            self.date,
            forum_size.0,
            List::convert_size(forum_size.1),
            count,
            List::convert_size(size),
            keeper.len()
        );
        for (num, name) in keeper.iter().enumerate() {
            message.push_str(
                format!(
                    "Хранитель {}: \
                     [url=profile.php?mode=viewprofile&{}&name=1]\
                     [u][color=#006699]{}[/u][/color][/url] \
                     [color=gray]~>[/color] {} шт. [color=gray]~>[/color] {}\n",
                    num + 1,
                    RutrackerForum::encode(&[("u", name)]),
                    name,
                    torrent_id[num].len(),
                    List::convert_size(
                        torrent_id[num]
                            .iter()
                            .filter_map(|id| torrent_info.get(id))
                            .map(|d| d.size)
                            .sum()
                    )
                ).as_str(),
            );
        }
        post.edit(message.as_str())?;
        Ok(())
    }

    pub fn send_list(&self, list: &List, topic: &Topic) -> Result<Option<usize>> {
        let messages = list.to_bbcode(self.db, &self.date, MESSAGE_LEN)?;
        let posts = topic.get_user_posts()?;
        let name = self.forum.user.name.as_str();
        let post_id = {
            let mut message = messages.iter();
            let mut post = posts.iter().skip(if topic.author == name { 1 } else { 0 });
            let post_id = match (post.next(), message.next()) {
                (Some(post), Some(message)) => {
                    post.edit(message)?;
                    Some(post.id)
                }
                (None, Some(message)) => topic.reply(message)?,
                _ => unreachable!(),
            };
            loop {
                match (post.next(), message.next()) {
                    (Some(post), Some(message)) => post.edit(message)?,
                    (Some(post), None) => post.edit("резерв")?,
                    (None, Some(message)) => topic.reply(message).map(|_| ())?,
                    (None, None) => break,
                }
            }
            post_id
        };
        if topic.author == name {
            self.send_list_header(list.forum, topic, &posts[0])?;
        }
        Ok(post_id)
    }

    pub fn send_all_list(&self) -> Result<Vec<(Option<usize>, String, usize, f64)>> {
        let keeper_working_forum = self.forum.get_keepers_working_forum();
        let mut topics = self.db.get_topic_with_subforum_list(
            &keeper_working_forum,
            &self.list.iter().map(|r| r.forum).collect::<Vec<usize>>(),
        )?;
        let mut vec = Vec::with_capacity(topics.len());
        for list in &self.list {
            if let Some((id, author, title)) = topics.remove(&list.forum) {
                let topic = keeper_working_forum.get_topic(id, author, title);
                vec.push((
                    self.send_list(list, &topic)?,
                    topic.title,
                    list.id.len(),
                    list.size,
                ));
            }
        }
        Ok(vec)
    }

    pub fn send_all(&self) -> Result<()> {
        let mut vec = self.send_all_list()?;
        let (count, size) = vec.iter().fold((0, 0f64), |(count, size), (_, _, c, s)| {
            (count + c, size + s)
        });
        let mut message = format!(
            "Актуально на: {}\n\
             Общее количество хранимых раздач: {} шт.\n\
             Общий вес хранимых раздач: {}\n[hr]",
            self.date,
            count,
            List::convert_size(size),
        );
        vec.sort_unstable_by(|a, b| a.1.as_str().cmp(b.1.as_str()));
        for (id, title, count, size) in vec {
            if let Some(id) = id {
                message.push_str(
                    format!(
                        "[url=viewtopic.php?p={}#{0}][u]{}[/u][/url] — {} шт. ({})",
                        id,
                        title.trim_left_matches("[Список] "),
                        count,
                        List::convert_size(size)
                    ).as_str(),
                );
            } else {
                message.push_str(
                    format!(
                        "[u]{}[/u] — {} шт. ({})",
                        title.trim_left_matches("[Список] "),
                        count,
                        List::convert_size(size)
                    ).as_str(),
                );
            }
        }
        let keepers_forum = self.forum.get_keepers_forum();
        let summary_report = keepers_forum.get_topic(
            4275633,
            "Tokuchi_Toua",
            "Сводные отчеты работы в группе (публикация)",
        );
        let posts = summary_report.get_user_posts()?;
        if let Some(post) = posts.get(0) {
            post.edit(message.as_str())?;
            if posts.len() > 1 {
                warn!("В теме сводных отчётов должно быть не больше одного сообщения");
            }
        } else {
            summary_report.reply(message.as_str())?;
        }
        Ok(())
    }
}
