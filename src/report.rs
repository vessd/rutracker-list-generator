use chrono::Local;
use client::TorrentStatus;
use database::{DBName, Database};
use rutracker::forum::RutrackerForum;
use rutracker::{api::TopicData, RutrackerApi};
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
    size: f64,
    id: Vec<usize>,
}

impl Report {
    pub fn new(subforum: usize, api: &RutrackerApi, db: &Database) -> Result<Self> {
        let keys: Vec<usize> = db.get(DBName::SubforumList, &subforum)?;
        let id: Vec<usize> = db
            .get_map(DBName::LocalList, keys.into_iter().collect())?
            .into_iter()
            .filter(|(_, status)| *status == Some(TorrentStatus::Seeding))
            .map(|(id, _)| id)
            .collect();
        let size = api
            .get_tor_topic_data(id.iter().collect(), Some(DBName::TopicData))?
            .iter()
            .map(|(_, data)| data.size)
            .sum();
        Ok(Report { id, size })
    }

    fn convert_size(size: f64) -> String {
        if size < 1f64 {
            format!("{:.2} B", size)
        } else {
            match size.log2() as usize {
                0...9 => format!("{:.2} B", size),
                10...19 => format!("{:.2} KB", size / 1024f64),
                20...29 => format!("{:.2} МB", size / 1_048_576f64),
                _ => format!("{:.2} GB", size / 1_073_741_824f64),
            }
        }
    }

    pub fn to_bbcode(&self, db: &Database, max_message_len: usize) -> Result<Vec<String>> {
        let header_len = 200;
        let body_len = max_message_len - header_len;
        let date = Local::now().format("%d.%m.%Y");
        let mut message = String::with_capacity(max_message_len);
        message.push_str(format!("Актуально на: {}\n", date).as_str());
        message.push_str(
            format!(
                "Всего хранимых раздач в подразделе: {} шт. / {}\n",
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
        if num.is_empty() {
            message.push_str(format!("[spoiler=\"1 — {}\"][list=1]\n", self.id.len()).as_str());
            message.extend(item.into_iter());
            message.push_str("[/list][/spoiler]\n");
            vec.push(message);
        } else {
            message.push_str(format!("[spoiler=\"1 — {}\"][list=1]\n", num[0]).as_str());
            for (n, s) in item.iter().enumerate() {
                if n == num[0] {
                    break;
                }
                message.push_str(s);
            }
            message.push_str("[/list][/spoiler]\n");
            vec.push(message);
            for i in 0..num.len() {
                let mut message = String::with_capacity(max_message_len);
                message.push_str(
                    format!(
                        "[spoiler=\"{} — {}\"][list=1]\n",
                        num[i] + 1,
                        num.get(i + 1).unwrap_or(&self.id.len())
                    ).as_str(),
                );
                item[num[i]].insert_str(2, format!("={}", num[i] + 1).as_str());
                for (n, s) in item.iter().enumerate().skip(num[i]) {
                    if n == *num.get(i + 1).unwrap_or(&(self.id.len() + 1)) {
                        break;
                    }
                    message.push_str(s);
                }
                message.push_str("[/list][/spoiler]\n");
                vec.push(message);
            }
        }
        Ok(vec)
    }
}

#[derive(Debug)]
pub struct SummaryReport<'a> {
    report: HashMap<usize, Report>,
    db: &'a Database,
    forum: &'a RutrackerForum,
}

impl<'a> SummaryReport<'a> {
    pub fn new(db: &'a Database, forum: &'a RutrackerForum) -> Self {
        SummaryReport {
            report: HashMap::new(),
            db,
            forum,
        }
    }

    pub fn add_report(&mut self, id: usize, report: Report) {
        if self.report.insert(id, report).is_some() {
            warn!(
                "В сводный отчёт добавлен подфорум c id {}, \
                 но такой подфорум уже был добавлен ранее, \
                 возможно это баг",
                id
            );
        }
    }

    /* pub fn send(&self) -> Result<()> {
        let mut count = 0;
        let mut size = 0f64;
        for (id, report) in &self.report {
            count += report.id.len();
            size += report.size;
        }
        Ok(())
    } */
}
