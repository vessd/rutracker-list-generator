use chrono::Local;
use database::{DBName, Database};
use rutracker::forum::{Forum, RutrackerForum, Topic, MESSAGE_LEN};
use rutracker::{api::TopicData, RutrackerApi as Api};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Api(err: ::rutracker::api::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Forum(err: ::rutracker::forum::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Database(err: ::database::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
    }
}

#[derive(Debug)]
pub struct Report {
    forum: usize,
    size: f64,
    id: Vec<usize>,
}

impl Report {
    pub fn new(api: &Api, forum: usize, id: Vec<usize>) -> Result<Self> {
        for id in &id {
            trace!("Report::new"; "id" => id);
        }
        let size = api
            .get_tor_topic_data(id.iter().collect(), Some(DBName::TopicData))?
            .iter()
            .map(|(_, data)| data.size)
            .sum();
        Ok(Report { forum, id, size })
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

    pub fn to_bbcode(&self, db: &Database, max_message_len: usize) -> Result<Vec<String>> {
        let header_len = 200;
        let body_len = max_message_len - header_len;
        let date = Local::now().format("%d.%m.%Y");
        let mut message = String::with_capacity(max_message_len);
        message.push_str(
            format!(
                "Актуально на: [color=darkblue]{}[/color]\n\
                 Всего хранимых раздач в подразделе: {} шт. / {}\n",
                date,
                self.id.len(),
                Report::convert_size(self.size)
            ).as_str(),
        );
        let mut item: Vec<(usize, String, f64)> = db
            .get_map(DBName::TopicData, self.id.to_vec())?
            .into_iter()
            .filter_map(|(id, data): (usize, Option<TopicData>)| {
                let data = data?;
                Some((id, data.topic_title, data.size))
            })
            .collect();
        item.sort_unstable_by(|a, b| a.1.as_str().cmp(b.1.as_str()));
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
        debug!("Report::to_bbcode::num {:?}", num);
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
            let mut message = String::with_capacity(max_message_len);
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
pub struct SummaryReport<'a> {
    report: Vec<Report>,
    date: String,
    db: &'a Database,
    api: &'a Api<'a>,
    forum: &'a RutrackerForum,
}

impl<'a> SummaryReport<'a> {
    pub fn new(db: &'a Database, api: &'a Api, forum: &'a RutrackerForum) -> Self {
        SummaryReport {
            report: Vec::new(),
            date: Local::now().format("%d.%m.%Y").to_string(),
            db,
            api,
            forum,
        }
    }

    pub fn add_report(&mut self, report: Report) {
        self.report.push(report);
    }

    fn get_topics(&self, forum: &'a Forum, id: Vec<&usize>) -> Result<HashMap<usize, Topic<'a>>> {
        let forum_name = self.api.get_forum_name(id, None)?;
        let mut forum_name: HashMap<&str, usize> =
            forum_name.iter().map(|(k, v)| (v.as_str(), *k)).collect();
        Ok(forum
            .get_topics()?
            .into_iter()
            .filter_map(|t| Some((forum_name.remove(t.title.split(" » ").last()?)?, t)))
            .collect())
    }

    pub fn send_report(&self, report: &Report, topic: &Topic) -> Result<Option<usize>> {
        let messages = report.to_bbcode(self.db, MESSAGE_LEN)?;
        let posts = topic.get_user_posts()?;
        trace!("{:?}", posts);
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
            info!("Формирование статиски подраздела...");
            let forum_size = self
                .api
                .forum_size()?
                .remove(&report.forum)
                .unwrap_or((0, 0f64));
            let posts = topic.get_posts()?;
            let mut reports_info = HashMap::new();
            for post in posts.iter().skip(1) {
                if let Entry::Vacant(v) = reports_info.entry(post.author.clone()) {
                    v.insert(post.get_stored_torrents_info().unwrap_or_else(|| {
                        let id = posts
                            .iter()
                            .skip(1)
                            .filter(|p| post.author == p.author)
                            .map(|p| p.get_stored_torrents())
                            .flatten()
                            .collect();
                        Report::new(self.api, report.forum, id)
                            .map(|r| (r.id.len(), r.size))
                            .unwrap_or((0, 0f64))
                    }));
                }
            }
            let (count, size) = reports_info
                .values()
                .fold((0, 0f64), |(count, size), (c, s)| (count + c, size + s));
            let mut message = format!(
                "[url=viewforum.php?f={}][u][color=#006699]{}[/u][/color][/url] \
                 | [url=tracker.php?f={0}&tm=-1&o=10&s=1&oop=1][color=indigo]\
                 [u]Проверка сидов[/u][/color][/url]\n\n\
                 Актуально на: [color=darkblue]{}[/color]\n\
                 Всего раздач в подразделе: {} шт. / {}\n\
                 Всего хранимых раздач в подразделе: \
                 {} шт. / {}\n\
                 Количество хранителей: {}\n[hr]\n",
                report.forum,
                topic.title.split(" » ").last().unwrap_or(""),
                self.date,
                forum_size.0,
                Report::convert_size(forum_size.1),
                count,
                Report::convert_size(size),
                reports_info.len()
            );
            for (num, (name, report)) in reports_info.iter().enumerate() {
                message.push_str(
                    format!(
                        "Хранитель {}: \
                         [url=profile.php?mode=viewprofile&u={}&name=1]\
                         [u][color=#006699]{1}[/u][/color][/url] \
                         [color=gray]~>[/color] {} шт. [color=gray]~>[/color] {}\n",
                        num + 1,
                        name,
                        report.0,
                        Report::convert_size(report.1)
                    ).as_str(),
                );
            }
            trace!("{}", message);
            posts[0].edit(message.as_str())?;
        }
        Ok(post_id)
    }

    pub fn send(&self) -> Result<()> {
        let keeper_forum = self.forum.get_forum(
            1584,
            "\"Хранители\" (рабочий подфорум)",
        );
        let tpocis = self.get_topics(
            &keeper_forum,
            self.report.iter().map(|r| &r.forum).collect(),
        )?;
        let mut vec = Vec::with_capacity(tpocis.len());
        for report in &self.report {
            if let Some(topic) = tpocis.get(&report.forum) {
                trace!("{:?}", topic);
                vec.push((
                    self.send_report(report, topic)?,
                    topic.title.as_str(),
                    report.id.len(),
                    report.size,
                ));
            }
        }
        vec.sort_unstable_by(|a, b| a.1.cmp(b.1));
        let (count, size) = vec.iter().fold((0, 0f64), |(count, size), (_, _, c, s)| {
            (count + c, size + s)
        });
        let mut message = format!(
            "Актуально на: {}\n\
             Общее количество хранимых раздач: {} шт.\n\
             Общий вес хранимых раздач: {}\n[hr]",
            self.date,
            count,
            Report::convert_size(size),
        );
        for (id, title, count, size) in vec {
            if let Some(id) = id {
                message.push_str(
                    format!(
                        "[url=viewtopic.php?p={}#{0}][u]{}[/u][/url] — {} шт. ({})",
                        id,
                        title,
                        count,
                        Report::convert_size(size)
                    ).as_str(),
                );
            } else {
                message.push_str(
                    format!(
                        "[u]{}[/u] — {} шт. ({})",
                        title,
                        count,
                        Report::convert_size(size)
                    ).as_str(),
                );
            }
        }
        let keeper_forum = self
            .forum
            .get_forum(2156, "Группа \"Хранители\"");
        let reports_topic = keeper_forum.get_topic(
            4275633,
            "Tokuchi_Toua",
            "Сводные отчеты работы в группе (публикация)",
        );
        let posts = reports_topic.get_user_posts()?;
        if posts.is_empty() {
            reports_topic.reply(message.as_str())?;
        } else {
            posts[0].edit(message.as_str())?;
            if posts.len() > 1 {
                warn!("В теме сводных отчётов не должно быть больше одного сообщения");
            }
        }
        Ok(())
    }
}
