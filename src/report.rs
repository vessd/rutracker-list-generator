use chrono::Local;
use crate::database::Database;
use crate::rutracker::forum::{Post, RutrackerForum, Topic, MESSAGE_LEN};
use std::collections::HashMap;

pub type Result<T> = ::std::result::Result<T, ::failure::Error>;

#[derive(Debug)]
pub struct Report<'a> {
    forum_id: Vec<i16>,
    date: String,
    db: &'a Database,
}

impl<'a> Report<'a> {
    pub fn new(db: &'a Database, forum_id: Vec<i16>) -> Self {
        let date = Local::now().format("%d.%m.%Y").to_string();
        Self { forum_id, date, db }
    }

    fn convert_size(size: f64) -> String {
        if size < 1f64 {
            format!("{:.2} B", size)
        } else {
            match size.log2() as usize {
                0..=9 => format!("{:.2} B", size),
                10..=19 => format!("{:.2} KB", size / 10f64.exp2()),
                20..=29 => format!("{:.2} МB", size / 20f64.exp2()),
                _ => format!("{:.2} GB", size / 30f64.exp2()),
            }
        }
    }

    pub fn get_bbcode_message(&self, forum_id: i16, max_len: usize) -> Result<Vec<String>> {
        let header_len = 200;
        let body_len = max_len - header_len;
        let mut message = String::with_capacity(max_len);
        let mut item = self.db.get_local_tor_by_forum(forum_id)?;
        item.sort_unstable_by(|a, b| a.1.as_str().cmp(b.1.as_str()));
        let count = item.len();
        let size = Report::convert_size(item.iter().map(|(_, _, size)| size).sum());
        message.push_str(
            format!(
                "Актуально на: [color=darkblue]{}[/color]\n\
                 Всего хранимых раздач в подразделе: {} шт. / {}\n",
                self.date, count, size
            )
            .as_str(),
        );
        let mut item: Vec<String> = item
            .into_iter()
            .map(|(id, topic_title, size)| {
                format!(
                    "[*][url=viewtopic.php?t={}]{}[/url] {}\n",
                    id,
                    topic_title,
                    Report::convert_size(size)
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
        debug!("Report::get_bbcode_message::num {:?}", num);
        let mut vec = Vec::with_capacity(num.len() + 1);
        message.push_str(
            format!(
                "[spoiler=\"№№ 1 — {}\"][list=1]\n",
                num.get(0).unwrap_or(&count)
            )
            .as_str(),
        );
        item.iter()
            .take(*num.get(0).unwrap_or(&count))
            .for_each(|i| message.push_str(i));
        message.push_str("[/list][/spoiler]\n");
        vec.push(message);
        for i in 0..num.len() {
            let mut message = String::with_capacity(max_len);
            message.push_str(
                format!(
                    "[spoiler=\"№№ {} — {}\"][list=1]\n",
                    num[i] + 1,
                    num.get(i + 1).unwrap_or(&count)
                )
                .as_str(),
            );
            item[num[i]].insert_str(2, format!("={}", num[i] + 1).as_str());
            item.iter()
                .skip(num[i])
                .take(*num.get(i + 1).unwrap_or(&(count + 1)) - num[i])
                .for_each(|i| message.push_str(i));
            message.push_str("[/list][/spoiler]\n");
            vec.push(message);
        }
        Ok(vec)
    }

    pub fn send_list_header(&self, forum_id: i16, topic_title: &str, post: &Post) -> Result<()> {
        info!(
            "Формирование статиски подраздела {}...",
            forum_id
        );
        let forum_size = self.db.get_forum_size(forum_id)?;
        let keepres_list_size = self.db.get_keepres_list_size(forum_id)?;
        let count: i32 = keepres_list_size.iter().map(|s| s.1).sum();
        let size = keepres_list_size.iter().map(|s| s.2).sum();
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
            topic_title.split(" » ").last().unwrap_or(""),
            self.date,
            forum_size.0,
            Report::convert_size(forum_size.1),
            count,
            Report::convert_size(size),
            keepres_list_size.len()
        );
        for (num, (name, count, size)) in keepres_list_size.iter().enumerate() {
            message.push_str(
                format!(
                    "Хранитель {}: \
                     [url=profile.php?mode=viewprofile&{}&name=1]\
                     [u][color=#006699]{}[/u][/color][/url] \
                     [color=gray]~>[/color] {} шт. [color=gray]~>[/color] {}\n",
                    num + 1,
                    RutrackerForum::encode(&[("u", name)]),
                    name,
                    count,
                    Report::convert_size(*size)
                )
                .as_str(),
            );
        }
        post.edit(message.as_str())?;
        Ok(())
    }

    pub fn send_list(&self, forum_id: i16, topic: &Topic) -> Result<Option<i32>> {
        let messages = self.get_bbcode_message(forum_id, MESSAGE_LEN)?;
        let posts = topic.get_user_posts()?;
        let name = self.db.forum.user.name.as_str();
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
            self.send_list_header(forum_id, topic.title.as_str(), &posts[0])?;
        }
        Ok(post_id)
    }

    pub fn send_all_list(&self) -> Result<HashMap<i16, Option<i32>>> {
        let topics = self.db.get_topics(&self.forum_id)?;
        let mut map = HashMap::with_capacity(topics.len());
        for (id, topic) in topics {
            map.insert(id, self.send_list(id, &topic)?);
        }
        Ok(map)
    }

    pub fn send_all(&self) -> Result<()> {
        let map = self.send_all_list()?;
        let mut local_list_size = self
            .db
            .get_local_list_size(&map.keys().cloned().collect::<Vec<i16>>())?;
        let count: i32 = local_list_size.iter().map(|l| l.2).sum();
        let size = local_list_size.iter().map(|l| l.3).sum();
        let mut message = format!(
            "Актуально на: {}\n\
             Общее количество хранимых раздач: {} шт.\n\
             Общий вес хранимых раздач: {}\n[hr]",
            self.date,
            count,
            Report::convert_size(size),
        );
        local_list_size.sort_unstable_by(|a, b| a.1.as_str().cmp(b.1.as_str()));
        for (f_id, title, count, size) in local_list_size {
            if let Some(p_id) = map[&f_id] {
                message.push_str(
                    format!(
                        "[url=viewtopic.php?p={}#{0}][u]{}[/u][/url] — {} шт. ({})",
                        p_id,
                        title.trim_start_matches("[Список] "),
                        count,
                        Report::convert_size(size)
                    )
                    .as_str(),
                );
            } else {
                message.push_str(
                    format!(
                        "[u]{}[/u] — {} шт. ({})",
                        title.trim_start_matches("[Список] "),
                        count,
                        Report::convert_size(size)
                    )
                    .as_str(),
                );
            }
        }
        let keepers_forum = self.db.forum.get_keepers_forum();
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
