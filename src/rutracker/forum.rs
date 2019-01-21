use crate::config::ForumConfig;
use cookie;
use encoding_rs::WINDOWS_1251;
use kuchiki::traits::TendrilSink;
use kuchiki::{self, ElementData, NodeDataRef, NodeRef};
use reqwest::header::{HeaderMap, CONTENT_TYPE, COOKIE, SET_COOKIE};
use reqwest::{Client, ClientBuilder, Proxy, RedirectPolicy, StatusCode};
use std::ops::Deref;
use std::rc::Rc;
use url::form_urlencoded;

pub type Result<T> = ::std::result::Result<T, ::failure::Error>;

pub const MESSAGE_LEN: usize = 120_000;

#[derive(Debug, Fail)]
enum ForumError {
    #[fail(display = "failed to get cookie from header")]
    CookieNotFound,
    #[fail(display = "failed to get token from page")]
    TokenNotFound,
    #[fail(display = "failed to get keys from page")]
    KeysNotFound,
    #[fail(display = "message length exceeded")]
    MessageLengthExceeded,
    #[fail(display = "unexpected status code: {}", status)]
    UnexpectedStatus { status: ::reqwest::StatusCode },
}

#[derive(Debug)]
struct IterPage<'a> {
    url: &'a str,
    href: Option<String>,
    client: &'a Client,
}

impl<'a> Iterator for IterPage<'a> {
    type Item = Result<NodeRef>;
    fn next(&mut self) -> Option<Result<NodeRef>> {
        let url = format!("{}{}", self.url, self.href.take()?);
        let mut response = match self.client.get(url.as_str()).send() {
            Ok(r) => r,
            Err(err) => return Some(Err(err.into())),
        };
        let response = match response.text() {
            Ok(r) => r,
            Err(err) => return Some(Err(err.into())),
        };
        let document = kuchiki::parse_html().one(response);
        if let Some(pg) = document.select(".pg").unwrap().last() {
            if pg.text_contents() == "След." {
                if let Some(element) = pg.as_node().as_element() {
                    if let Some(href) = element.attributes.borrow().get("href") {
                        self.href = Some(href.to_owned());
                    }
                }
            }
        }
        Some(Ok(document))
    }
}

#[derive(Debug)]
pub struct User {
    pub id: usize,
    pub name: String,
    pub bt: String,
    pub api: String,
    pub cookies: HeaderMap,
    pub form_token: String,
}

impl User {
    pub fn new(config: &ForumConfig) -> Result<Self> {
        let client = if let Some(p) = config.proxy.as_ref() {
            ClientBuilder::new()
                .proxy(Proxy::all(p)?)
                .redirect(RedirectPolicy::none())
                .build()?
        } else {
            ClientBuilder::new()
                .redirect(RedirectPolicy::none())
                .build()?
        };
        let name = config.user.name.clone();
        let url = config.url.clone();
        let cookies = User::get_cookie(config, &client)?;
        let page = client
            .get((url + "profile.php").as_str())
            .headers(cookies.clone())
            .query(&[("mode", "viewprofile"), ("u", &name)])
            .send()?
            .text()?;
        let (bt, api, id) = User::get_keys(&page).ok_or(ForumError::KeysNotFound)?;
        let form_token = User::get_form_token(&page).ok_or(ForumError::TokenNotFound)?;
        Ok(User {
            id,
            name,
            bt,
            api,
            cookies,
            form_token,
        })
    }

    // https://github.com/seanmonstar/reqwest/issues/14
    fn get_cookie(config: &ForumConfig, client: &Client) -> Result<HeaderMap> {
        let resp = client
            .post((config.url.clone() + "login.php").as_str())
            .form(&[
                ("login_username", config.user.name.as_str()),
                ("login_password", config.user.password.as_str()),
                ("login", "Вход"),
            ])
            .send()?;
        let mut cookies = String::new();
        for c in resp.headers().get_all(SET_COOKIE).iter() {
            let c = cookie::Cookie::parse(c.to_str()?)?;
            if !cookies.is_empty() {
                cookies.push_str("; ");
            }
            cookies.push_str(format!("{}={}", c.name(), c.value()).as_str());
        }
        if cookies.is_empty() {
            Err(ForumError::CookieNotFound.into())
        } else {
            let mut map = HeaderMap::new();
            map.insert(COOKIE, cookies.parse()?);
            Ok(map)
        }
    }

    fn get_keys(page: &str) -> Option<(String, String, usize)> {
        let document = kuchiki::parse_html().one(page);
        let keys: Vec<String> = document
            .select(".med")
            .expect("select keys")
            .map(|node| node.text_contents())
            .find(|s| s.starts_with("bt: "))?
            .split_whitespace()
            .filter(|s| !s.ends_with(':'))
            .map(str::to_owned)
            .collect();
        Some((
            keys.get(0)?.clone(),
            keys.get(1)?.clone(),
            keys.get(2)?.parse().ok()?,
        ))
    }

    fn get_form_token(page: &str) -> Option<String> {
        Some(
            page.lines()
                .find(|l| l.contains("form_token"))?
                .split_terminator('\'')
                .nth(1)?
                .to_owned(),
        )
    }
}

#[derive(Debug)]
pub struct Post {
    pub id: i32,
    pub author: String,
    pub body: NodeDataRef<ElementData>,
    topic: Rc<TopicData>,
}

impl Post {
    fn from_node(node: &NodeRef, topic: &Topic) -> Option<Post> {
        Some(Post {
            id: RutrackerForum::get_id(node, "id").or_else(|| {
                RutrackerForum::get_id(node.select_first(".small").ok()?.as_node(), "href")
            })?,
            author: RutrackerForum::get_text(node, ".nick")?,
            body: node.select_first(".post_body").ok()?,
            topic: Rc::clone(&topic.0),
        })
    }

    pub fn get_stored_torrents(&self) -> Vec<i32> {
        self.body
            .as_node()
            .select(".postLink")
            .unwrap()
            .filter_map(|link| RutrackerForum::get_id(link.as_node(), "href"))
            .collect()
    }

    pub fn edit(&self, message: &str) -> Result<()> {
        if message.len() > MESSAGE_LEN {
            return Err(ForumError::MessageLengthExceeded.into());
        }
        if self.topic.forum.rutracker.dry_run {
            info!(
                "Сообщение id {} будет изменено:\n{}",
                self.id, message
            );
            return Ok(());
        }
        let url = format!(
            "{}posting.php?mode=editpost&p={}",
            self.topic.forum.rutracker.url, self.id
        );
        let params = RutrackerForum::encode(&[
            ("mode", "editpost"),
            ("f", self.topic.forum.id.to_string().as_str()),
            ("t", self.topic.id.to_string().as_str()),
            ("p", self.id.to_string().as_str()),
            ("subject", self.topic.title.as_str()),
            ("fontFace", "-1"),
            ("codeColor", "black"),
            ("codeSize", "12"),
            ("align", "-1"),
            ("codeUrl2", ""),
            ("message", message),
            ("submit_mode", "submit"),
            ("decflag", "2"),
            ("update_post_time", "on"),
            (
                "form_token",
                self.topic.forum.rutracker.user.form_token.as_str(),
            ),
        ]);
        let resp = self
            .topic
            .forum
            .rutracker
            .client
            .post(url.as_str())
            .body(params)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .send()?;
        match resp.status() {
            StatusCode::OK => Ok(()),
            _ => Err(ForumError::UnexpectedStatus {
                status: resp.status(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TopicData {
    pub id: i32,
    pub author: String,
    pub title: String,
    forum: Rc<ForumData>,
}

#[derive(Debug)]
pub struct Topic(pub Rc<TopicData>);

impl Deref for Topic {
    type Target = TopicData;
    #[inline]
    fn deref(&self) -> &TopicData {
        &*self.0
    }
}

impl Topic {
    fn iter(&self) -> IterPage<'_> {
        IterPage {
            url: self.forum.rutracker.url.as_str(),
            href: Some(format!("viewtopic.php?t={}", self.id)),
            client: &self.forum.rutracker.client,
        }
    }

    fn from_node(node: &NodeRef, forum: &Forum) -> Option<Topic> {
        Some(Topic(Rc::new(TopicData {
            id: RutrackerForum::get_id(node, "data-topic_id")?,
            author: RutrackerForum::get_text(node, ".vf-col-author")?,
            title: RutrackerForum::get_text(node, ".tt-text")?,
            forum: Rc::clone(&forum.0),
        })))
    }

    pub fn get_posts(&self) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        for page in self.iter() {
            match page?.select_first("#topic_main") {
                Ok(topic) => posts.extend(
                    topic
                        .as_node()
                        .select(".row1,.row2")
                        .unwrap()
                        .filter_map(|d| Post::from_node(d.as_node(), self)),
                ),
                Err(()) => break,
            };
        }
        Ok(posts)
    }

    pub fn get_user_posts(&self) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        let iter = IterPage {
            url: self.forum.rutracker.url.as_str(),
            href: Some(format!(
                "search.php?uid={}&t={}&dm=1",
                self.forum.rutracker.user.id, self.id
            )),
            client: &self.forum.rutracker.client,
        };
        for page in iter {
            let document = page?;
            posts.extend(
                document
                    .select(".row1,.row2")
                    .unwrap()
                    .filter_map(|d| Post::from_node(d.as_node(), self)),
            );
        }
        posts.reverse();
        Ok(posts)
    }

    pub fn reply(&self, message: &str) -> Result<Option<i32>> {
        if message.len() > MESSAGE_LEN {
            return Err(ForumError::MessageLengthExceeded.into());
        }
        if self.forum.rutracker.dry_run {
            info!(
                "В тему {} будет добавлено сообщение:\n {}",
                self.title, message
            );
            return Ok(None);
        }
        let url = format!(
            "{}posting.php?mode=reply&t={}",
            self.forum.rutracker.url, self.id
        );
        let params = RutrackerForum::encode(&[
            ("mode", "reply"),
            ("t", self.id.to_string().as_str()),
            ("fontFace", "-1"),
            ("codeColor", "black"),
            ("codeSize", "12"),
            ("align", "-1"),
            ("codeUrl2", ""),
            ("message", message),
            ("submit_mode", "submit"),
            ("form_token", self.forum.rutracker.user.form_token.as_str()),
        ]);
        let mut resp = self
            .forum
            .rutracker
            .client
            .post(url.as_str())
            .body(params)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(ForumError::UnexpectedStatus {
                status: resp.status(),
            }
            .into());
        }
        let document = kuchiki::parse_html().one(resp.text()?);
        let post_id = match document.select_first(".mrg_16 a") {
            Ok(data) => RutrackerForum::get_id(data.as_node(), "href"),
            Err(()) => None,
        };
        Ok(post_id)
    }
}

#[derive(Debug)]
pub struct ForumData {
    pub id: usize,
    pub title: String,
    rutracker: Rc<RutrackerForumData>,
}

#[derive(Debug)]
pub struct Forum(pub Rc<ForumData>);

impl Deref for Forum {
    type Target = ForumData;
    #[inline]
    fn deref(&self) -> &ForumData {
        &*self.0
    }
}

impl Forum {
    fn iter(&self) -> IterPage<'_> {
        IterPage {
            url: self.rutracker.url.as_str(),
            href: Some(format!("viewforum.php?f={}", self.id)),
            client: &self.0.rutracker.client,
        }
    }

    pub fn get_topic<T: Into<String>>(&self, id: i32, author: T, title: T) -> Topic {
        Topic(Rc::new(TopicData {
            id,
            author: author.into(),
            title: title.into(),
            forum: Rc::clone(&self.0),
        }))
    }

    pub fn get_topics(&self) -> Result<Vec<Topic>> {
        let mut topics = Vec::new();
        for page in self.iter() {
            let document = page?;
            topics.extend(
                document
                    .select(".hl-tr")
                    .unwrap()
                    .filter_map(|d| Topic::from_node(d.as_node(), self)),
            );
        }
        Ok(topics)
    }
}

#[derive(Debug)]
pub struct RutrackerForumData {
    pub user: User,
    client: Client,
    url: String,
    dry_run: bool,
}

#[derive(Debug)]
pub struct RutrackerForum(pub Rc<RutrackerForumData>);

impl Deref for RutrackerForum {
    type Target = RutrackerForumData;
    #[inline]
    fn deref(&self) -> &RutrackerForumData {
        &*self.0
    }
}

impl RutrackerForum {
    pub fn new(config: &ForumConfig, dry_run: bool) -> Result<Self> {
        let user = User::new(config)?;
        let url = config.url.clone();
        let proxy = config.proxy.as_ref();
        let client = if let Some(p) = proxy {
            ClientBuilder::new()
                .proxy(Proxy::all(p)?)
                .default_headers(user.cookies.clone())
                .build()?
        } else {
            ClientBuilder::new()
                .default_headers(user.cookies.clone())
                .build()?
        };
        Ok(RutrackerForum(Rc::new(RutrackerForumData {
            client,
            url,
            user,
            dry_run,
        })))
    }

    pub fn get_forum<T: Into<String>>(&self, id: usize, title: T) -> Forum {
        Forum(Rc::new(ForumData {
            id,
            title: title.into(),
            rutracker: Rc::clone(&self.0),
        }))
    }

    pub fn get_keepers_forum(&self) -> Forum {
        self.get_forum(2156, "Группа \"Хранители\"")
    }

    pub fn get_keepers_working_forum(&self) -> Forum {
        self.get_forum(
            1584,
            "\"Хранители\" (рабочий подфорум)",
        )
    }

    fn get_text(node: &NodeRef, selectors: &str) -> Option<String> {
        Some(
            node.select_first(selectors)
                .ok()?
                .text_contents()
                .trim()
                .to_owned(),
        )
    }

    fn get_id(node: &NodeRef, attribute: &str) -> Option<i32> {
        node.as_element()?
            .attributes
            .borrow()
            .get(attribute)?
            .split(|c: char| !c.is_ascii_digit())
            .find(|s| !s.is_empty())?
            .parse::<i32>()
            .ok()
    }

    pub fn encode(vec: &[(&str, &str)]) -> String {
        form_urlencoded::Serializer::new(String::new())
            .custom_encoding_override(|s| WINDOWS_1251.encode(s).0)
            .extend_pairs(vec)
            .finish()
    }
}
