use config::ForumConfig;
use cookie;
use encoding_rs::WINDOWS_1251;
use kuchiki::traits::TendrilSink;
use kuchiki::{self, ElementData, NodeDataRef, NodeRef};
use reqwest::header::{ContentType, Cookie, Headers, SetCookie};
use reqwest::{Client, ClientBuilder, Proxy, RedirectPolicy, StatusCode};
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
    url: String,
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
    pub cookie: Cookie,
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
        let cookie = User::get_cookie(config, &client)?;
        let page = client
            .get((url + "profile.php").as_str())
            .header(cookie.clone())
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
            cookie,
            form_token,
        })
    }

    // https://github.com/seanmonstar/reqwest/issues/14
    fn get_cookie(config: &ForumConfig, client: &Client) -> Result<Cookie> {
        let resp = client
            .post((config.url.clone() + "login.php").as_str())
            .form(&[
                ("login_username", config.user.name.as_str()),
                ("login_password", config.user.password.as_str()),
                ("login", "Вход"),
            ])
            .send()?;

        let mut cookie = Cookie::new();
        resp.headers()
            .get::<SetCookie>()
            .ok_or(ForumError::CookieNotFound)?
            .iter()
            .for_each(|c| {
                let co = cookie::Cookie::parse(c.as_str()).unwrap();
                cookie.append(co.name().to_owned(), co.value().to_owned());
            });
        Ok(cookie)
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
pub struct Post<'a> {
    pub id: usize,
    pub author: String,
    pub body: NodeDataRef<ElementData>,
    topic: &'a Topic<'a>,
}

impl<'a> Post<'a> {
    fn from_node(node: &NodeRef, topic: &'a Topic) -> Option<Post<'a>> {
        Some(Post {
            id: RutrackerForum::get_id(node, "id").or_else(|| {
                RutrackerForum::get_id(node.select_first(".small").ok()?.as_node(), "href")
            })?,
            author: RutrackerForum::get_text(node, ".nick")?,
            body: node.select_first(".post_body").ok()?,
            topic,
        })
    }

    pub fn get_stored_torrents(&self) -> Vec<usize> {
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
            .header(ContentType::form_url_encoded())
            .send()?;
        match resp.status() {
            StatusCode::Ok => Ok(()),
            _ => Err(ForumError::UnexpectedStatus {
                status: resp.status(),
            }.into()),
        }
    }
}

#[derive(Debug)]
pub struct Topic<'a> {
    pub id: usize,
    pub author: String,
    pub title: String,
    forum: &'a Forum<'a>,
}

impl<'a> Topic<'a> {
    fn iter(&self) -> IterPage<'a> {
        IterPage {
            url: self.forum.rutracker.url.clone(),
            href: Some(format!("viewtopic.php?t={}", self.id)),
            client: &self.forum.rutracker.client,
        }
    }

    fn from_node(node: &NodeRef, forum: &'a Forum) -> Option<Topic<'a>> {
        Some(Topic {
            id: RutrackerForum::get_id(node, "data-topic_id")?,
            author: RutrackerForum::get_text(node, ".vf-col-author")?,
            title: RutrackerForum::get_text(node, ".tt-text")?,
            forum,
        })
    }

    pub fn get_stored_torrents(&self) -> Result<(Vec<String>, Vec<Vec<usize>>)> {
        let posts = self.get_posts()?;
        let mut keeper = Vec::new();
        let mut torrent_id: Vec<Vec<usize>> = Vec::new();
        for p in posts.iter().skip(1) {
            if let Some(i) = keeper.iter().position(|k| *k == p.author) {
                torrent_id[i].extend(p.get_stored_torrents().into_iter());
            } else {
                keeper.push(p.author.clone());
                torrent_id.push(p.get_stored_torrents());
            }
        }
        Ok((keeper, torrent_id))
    }

    pub fn get_posts(&self) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        for page in self.iter() {
            let document = page?;
            let topic_main = match document.select_first("#topic_main") {
                Ok(topic) => topic,
                Err(()) => break,
            };
            posts.extend(
                topic_main
                    .as_node()
                    .select(".row1,.row2")
                    .unwrap()
                    .filter_map(|d| Post::from_node(d.as_node(), self)),
            );
        }
        Ok(posts)
    }

    pub fn get_user_posts(&self) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        let iter = IterPage {
            url: self.forum.rutracker.url.clone(),
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

    pub fn reply(&self, message: &str) -> Result<Option<usize>> {
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
            .header(ContentType::form_url_encoded())
            .send()?;
        if resp.status() != StatusCode::Ok {
            return Err(ForumError::UnexpectedStatus {
                status: resp.status(),
            }.into());
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
pub struct Forum<'a> {
    pub id: usize,
    pub title: String,
    rutracker: &'a RutrackerForum,
}

impl<'a> Forum<'a> {
    fn iter(&self) -> IterPage<'a> {
        IterPage {
            url: self.rutracker.url.clone(),
            href: Some(format!("viewforum.php?f={}", self.id)),
            client: &self.rutracker.client,
        }
    }

    pub fn get_topic<T: Into<String>>(&self, id: usize, author: T, title: T) -> Topic {
        Topic {
            id,
            author: author.into(),
            title: title.into(),
            forum: self,
        }
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
pub struct RutrackerForum {
    pub user: User,
    client: Client,
    url: String,
    dry_run: bool,
}

impl RutrackerForum {
    pub fn new(config: &ForumConfig, dry_run: bool) -> Result<Self> {
        let user = User::new(config)?;
        let url = config.url.clone();
        let proxy = config.proxy.as_ref();
        let mut headers = Headers::new();
        headers.set(user.cookie.clone());
        let client = if let Some(p) = proxy {
            ClientBuilder::new()
                .proxy(Proxy::all(p)?)
                .default_headers(headers)
                .build()?
        } else {
            ClientBuilder::new().default_headers(headers).build()?
        };
        Ok(RutrackerForum {
            client,
            url,
            user,
            dry_run,
        })
    }

    pub fn get_forum<T: Into<String>>(&self, id: usize, title: T) -> Forum {
        Forum {
            id,
            title: title.into(),
            rutracker: self,
        }
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

    fn get_id(node: &NodeRef, attribute: &str) -> Option<usize> {
        node.as_element()?
            .attributes
            .borrow()
            .get(attribute)?
            .split(|c: char| !c.is_ascii_digit())
            .find(|s| !s.is_empty())?
            .parse::<usize>()
            .ok()
    }

    pub fn encode(vec: &[(&str, &str)]) -> String {
        form_urlencoded::Serializer::new(String::new())
            .custom_encoding_override(|s| WINDOWS_1251.encode(s).0)
            .extend_pairs(vec)
            .finish()
    }
}
