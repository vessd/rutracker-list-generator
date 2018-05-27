use cookie;
use encoding::all::WINDOWS_1251;
use encoding::{DecoderTrap, Encoding};
use kuchiki::traits::TendrilSink;
use kuchiki::{self, ElementData, NodeDataRef, NodeRef};
use reqwest::header::{Cookie, Headers, SetCookie};
use reqwest::{Client, ClientBuilder, Proxy, RedirectPolicy, StatusCode};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;
use Config;

pub type Result<T> = ::std::result::Result<T, Error>;
pub type StoredTorrents = HashMap<String, Vec<usize>>;

const MESSAGE_LEN: usize = 120_000;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Api(err: super::api::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Cookie {
            description("cann't get cookie")
            display("cann't get cookie from Rutracker forum")
        }
        Token {
            description("cann't get token")
            display("cann't get token from Rutracker forum")
        }
        Message {
            description("message length exceeded")
            display("message length exceeded")
        }
        Keys {
            description("keys not found")
            display("keys not found")
        }
        User
        Parse(err: ::std::num::ParseIntError) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Unexpected {
            description("unexpected error")
            display("unexpected error")
        }
        UnexpectedResponse(status: StatusCode) {
            description("unexpected response")
            display("unexpected response from the rutracker forum: {}", status)
        }
        Reqwest(err: ::reqwest::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Io(err: ::std::io::Error) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        Decode(s: Cow<'static, str>) {
            description("decoder error")
            display("{}", s)
        }
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
    pub fn new(config: &Config) -> Result<Self> {
        if let Some(ref user) = config.user {
            let url = config.forum_url.as_ref();
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
            let name = user.name.clone();
            let cookie = User::get_cookie(&user.name, &user.password, url, &client)?;
            let page = client
                .get((url.to_owned() + "profile.php").as_str())
                .header(cookie.clone())
                .query(&[("mode", "viewprofile"), ("u", &user.name)])
                .send()?
                .text()?;
            let (bt, api, id) = User::get_keys(&page).ok_or(Error::Keys)?;
            let form_token = User::get_form_token(&page).ok_or(Error::Token)?;
            Ok(User {
                id,
                name,
                bt,
                api,
                cookie,
                form_token,
            })
        } else {
            Err(Error::User)
        }
    }

    // https://github.com/seanmonstar/reqwest/issues/14
    fn get_cookie(user: &str, pass: &str, url: &str, client: &Client) -> Result<Cookie> {
        let resp = client
            .post((url.to_owned() + "login.php").as_str())
            .form(&[
                ("login_username", user),
                ("login_password", pass),
                ("login", "Вход"),
            ])
            .send()?;

        let mut cookie = Cookie::new();
        resp.headers()
            .get::<SetCookie>()
            .ok_or(Error::Cookie)?
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
pub struct Post {
    id: Option<usize>,
    author: Option<String>,
    post_body: NodeDataRef<ElementData>,
}

impl Post {
    fn get_id(node: &NodeRef) -> Option<usize> {
        node.as_element()?
            .attributes
            .borrow()
            .get("id")?
            .get(5..)?
            .parse::<usize>()
            .ok()
    }

    fn get_author(node: &NodeRef) -> Option<String> {
        Some(
            node.select_first(".nick")
                .ok()?
                .text_contents()
                .trim()
                .to_string(),
        )
    }

    fn get_t_ids(&self) -> Vec<usize> {
        self.post_body
            .as_node()
            .select(".postLink")
            .unwrap()
            .filter_map(|link| {
                link.as_node()
                    .as_element()?
                    .attributes
                    .borrow()
                    .get("href")?
                    .get(16..)?
                    .parse()
                    .ok()
            })
            .collect()
    }

    fn from_node(node: &NodeRef) -> Option<Post> {
        Some(Post {
            id: Post::get_id(node),
            author: Post::get_author(node),
            post_body: node.select_first(".post_body").ok()?,
        })
    }
}

#[derive(Debug)]
pub struct Topic {
    pub posts: Vec<Post>,
}

impl Topic {
    pub fn get_stored_torrents(&self) -> StoredTorrents {
        let mut map = HashMap::new();
        for p in self.posts.iter().skip(1) {
            let author = p.author.clone().unwrap_or_else(|| "unknow".to_owned());
            let keeper = map.entry(author).or_insert_with(Vec::new);
            keeper.append(&mut p.get_t_ids());
        }
        map
    }
}

#[derive(Debug)]
pub struct Forum<'a> {
    pub id: usize,
    pub name: String,
    rutracker: &'a RutrackerForum,
}

#[derive(Debug)]
pub struct RutrackerForum {
    client: Client,
    url: String,
    user: User,
}

impl RutrackerForum {
    pub fn new(user: User, config: &Config) -> Result<Self> {
        let url = config.forum_url.clone();
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
        Ok(RutrackerForum { client, url, user })
    }

    fn decode<R: Read>(source: &mut R) -> Result<String> {
        let mut buf = Vec::new();
        source.read_to_end(&mut buf)?;
        match WINDOWS_1251.decode(&buf, DecoderTrap::Replace) {
            Ok(s) => Ok(s),
            Err(err) => Err(Error::Decode(err)),
        }
    }

    fn get_posts(&self, mut url: String) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        loop {
            let mut resp = self.client.get(url.as_str()).send()?;
            let resp = RutrackerForum::decode(&mut resp)?;
            let document = kuchiki::parse_html().one(resp);
            let topic_main = match document.select_first("#topic_main") {
                Ok(topic) => topic,
                Err(()) => break,
            };
            posts.append(&mut topic_main
                .as_node()
                .select(".row1,.row2")
                .unwrap()
                .filter_map(|d| Post::from_node(d.as_node()))
                .collect());
            if let Some(pg) = document.select(".pg").unwrap().last() {
                if pg.text_contents() == "След." {
                    if let Some(element) = pg.as_node().as_element() {
                        if let Some(href) = element.attributes.borrow().get("href") {
                            url = href.to_owned();
                            url.insert_str(0, self.url.as_str());
                            continue;
                        }
                    }
                }
            }
            break;
        }
        Ok(posts)
    }

    pub fn get_topic(&self, id: usize) -> Result<Topic> {
        let mut url = id.to_string();
        url.insert_str(0, "viewtopic.php?t=");
        url.insert_str(0, self.url.as_str());
        let posts = self.get_posts(url)?;
        Ok(Topic { posts })
    }

    pub fn reply_topic(&self, id: usize, message: &str) -> Result<()> {
        if message.len() > MESSAGE_LEN {
            return Err(Error::Message);
        }
        let mut url = id.to_string();
        url.insert_str(0, "posting.php?mode=reply&t=");
        url.insert_str(0, self.url.as_str());
        let reply = self
            .client
            .post(url.as_str())
            .form(&[
                ("mode", "reply"),
                ("t", id.to_string().as_str()),
                ("fontFace", "-1"),
                ("codeColor", "black"),
                ("codeSize", "12"),
                ("align", "-1"),
                ("codeUrl2", ""),
                ("message", message),
                ("submit_mode", "submit"),
                ("form_token", self.user.form_token.as_str()),
            ])
            .send()?;
        match reply.status() {
            StatusCode::Ok => Ok(()),
            _ => Err(Error::UnexpectedResponse(reply.status())),
        }
    }

    pub fn edit_post(&self, f_id: usize, t_id: usize, p_id: usize, message: &str) -> Result<()> {
        if message.len() > MESSAGE_LEN {
            return Err(Error::Message);
        }
        let mut url = p_id.to_string();
        url.insert_str(0, "posting.php?mode=editpost&p=");
        url.insert_str(0, self.url.as_str());
        let reply = self
            .client
            .post(url.as_str())
            .form(&[
                ("mode", "editpost"),
                ("f", f_id.to_string().as_str()),
                ("t", t_id.to_string().as_str()),
                ("p", p_id.to_string().as_str()),
                ("fontFace", "-1"),
                ("codeColor", "black"),
                ("codeSize", "12"),
                ("align", "-1"),
                ("codeUrl2", ""),
                ("message", message),
                ("submit_mode", "submit"),
                ("decflag", "2"),
                ("update_post_time", "on"),
                ("form_token", self.user.form_token.as_str()),
            ])
            .send()?;
        match reply.status() {
            StatusCode::Ok => Ok(()),
            _ => Err(Error::UnexpectedResponse(reply.status())),
        }
    }

    //pub fn get_forum(&self, id: usize, name: String) -> Result<Forum> {}
}
