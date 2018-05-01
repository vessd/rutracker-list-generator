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
        Cookie {
            description("cann't get cookie")
            display("cann't get cookie from Rutracker forum")
        }
        Token {
            description("cann't get token")
            display("cann't get token from Rutracker forum")
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
        Message {
            description("message length exceeded")
            display("message length exceeded")
        }
        UnexpectedResponse(status: StatusCode) {
            description("unexpected response")
            display("unexpected response from the transmission server: {}", status)
        }
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
        Some(node.select_first(".nick").ok()?.text_contents().trim().to_string())
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
pub struct RutrackerForum {
    client: Client,
    base_url: String,
    form_token: String,
}

impl RutrackerForum {
    pub fn new(user: &str, password: &str, config: &Config) -> Result<Self> {
        let base_url = config.forum_url.clone();
        let proxy = config.https_proxy.as_ref();
        let headers = RutrackerForum::get_headers(user, password, &base_url, proxy)?;
        println!("{}", headers);
        let client = if let Some(p) = proxy {
            ClientBuilder::new().proxy(Proxy::all(p)?).default_headers(headers).build()?
        } else {
            ClientBuilder::new().default_headers(headers).build()?
        };
        let form_token = RutrackerForum::get_form_token(base_url.clone(), &client)?;
        Ok(RutrackerForum {
            client,
            base_url,
            form_token,
        })
    }

    // https://github.com/seanmonstar/reqwest/issues/14
    fn get_headers(user: &str, password: &str, base_url: &str, proxy: Option<&String>) -> Result<Headers> {
        let client = if let Some(p) = proxy {
            ClientBuilder::new()
                .proxy(Proxy::all(p)?)
                .redirect(RedirectPolicy::none())
                .build()?
        } else {
            ClientBuilder::new().redirect(RedirectPolicy::none()).build()?
        };
        let resp = client
            .post((base_url.to_owned() + "login.php").as_str())
            .form(&[("login_username", user), ("login_password", password), ("login", "")])
            .send()?;

        let mut cookie = Cookie::new();
        resp.headers().get::<SetCookie>().ok_or(Error::Cookie)?.iter().for_each(|c| {
            let co = cookie::Cookie::parse(c.as_str()).unwrap();
            cookie.append(co.name().to_owned(), co.value().to_owned());
        });
        let mut headers = Headers::new();
        headers.set(cookie);
        Ok(headers)
    }

    fn get_form_token(base_url: String, client: &Client) -> Result<String> {
        let resp = client.get((base_url + "index.php").as_str()).send()?.text()?;
        Ok(resp.lines()
            .find(|l| l.contains("form_token"))
            .ok_or(Error::Token)?
            .split_terminator('\'')
            .nth(1)
            .ok_or(Error::Token)?
            .to_owned())
    }

    fn decode<R>(source: &mut R) -> Result<String>
    where
        R: Read,
    {
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
                            url.insert_str(0, self.base_url.as_str());
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
        url.insert_str(0, self.base_url.as_str());
        let posts = self.get_posts(url)?;
        Ok(Topic { posts })
    }

    pub fn reply_topic(&self, id: usize, message: &str) -> Result<()> {
        if message.len() > MESSAGE_LEN {
            return Err(Error::Message);
        }
        let mut url = id.to_string();
        url.insert_str(0, "posting.php?mode=reply&t=");
        url.insert_str(0, self.base_url.as_str());
        let reply = self.client
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
                ("form_token", self.form_token.as_str()),
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
        url.insert_str(0, self.base_url.as_str());
        let reply = self.client
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
                ("form_token", self.form_token.as_str()),
            ])
            .send()?;
        match reply.status() {
            StatusCode::Ok => Ok(()),
            _ => Err(Error::UnexpectedResponse(reply.status())),
        }
    }
}
