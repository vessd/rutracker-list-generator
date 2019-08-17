use crate::config::ForumConfig;
use encoding_rs::WINDOWS_1251;
use failure::Fail;
use reqwest::{
    header::{HeaderMap, CONTENT_TYPE, COOKIE},
    Client, ClientBuilder, Proxy, RedirectPolicy, StatusCode,
};
use scraper::{element_ref::ElementRef, Html, Selector};
use std::{ops::Deref, rc::Rc};
use url::form_urlencoded;

pub type Result<T> = std::result::Result<T, failure::Error>;

pub const MESSAGE_LEN: usize = 120_000;

#[derive(Debug, Clone, Fail)]
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
    UnexpectedStatus { status: reqwest::StatusCode },
}

fn selector(selectors: &str) -> Selector {
    Selector::parse(selectors).expect("css selector parse error")
}

#[derive(Debug, Clone)]
struct IterPage<'a> {
    url: &'a str,
    href: Option<String>,
    client: &'a Client,
}

impl<'a> Iterator for IterPage<'a> {
    type Item = Result<Html>;

    fn next(&mut self) -> Option<Result<Html>> {
        let url = format!("{}{}", self.url, self.href.take()?);
        let mut response = match self.client.get(url.as_str()).send() {
            Ok(r) => r,
            Err(err) => return Some(Err(err.into())),
        };
        let response = match response.text() {
            Ok(r) => r,
            Err(err) => return Some(Err(err.into())),
        };
        let document = Html::parse_document(response.as_str());
        if let Some(pg) = document.select(&selector(".pg")).last() {
            if let Some(text) = pg.text().last() {
                if text == "След." {
                    if let Some(href) = pg.value().attr("href") {
                        self.href = Some(href.to_owned());
                    }
                }
            }
        }
        Some(Ok(document))
    }
}

#[derive(Debug, Clone)]
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
        let cookies = Self::get_cookie(config, &client)?;
        let page = client
            .get((url + "profile.php").as_str())
            .headers(cookies.clone())
            .query(&[("mode", "viewprofile"), ("u", &name)])
            .send()?
            .text()?;
        let (bt, api, id) = Self::get_keys(&page).ok_or(ForumError::KeysNotFound)?;
        let form_token = Self::get_form_token(&page).ok_or(ForumError::TokenNotFound)?;
        Ok(Self {
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
        for c in resp.cookies() {
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
        let document = Html::parse_document(page);
        let selector = selector(".med");
        let mut keys = document
            .select(&selector)
            .flat_map(|element| element.text())
            .skip_while(|s| !s.starts_with("bt:"))
            .filter(|s| !s.contains(':'))
            .take(3)
            .map(|s| s.trim());
        Some((
            keys.next()?.to_owned(),
            keys.next()?.to_owned(),
            keys.next()?.parse().ok()?,
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

#[derive(Debug, Clone)]
pub struct Post {
    pub id: i32,
    pub author: String,
    pub stored_torrents: Vec<i32>,
    topic: Rc<TopicData>,
}

impl Post {
    fn from_element(element: ElementRef<'_>, topic: &Topic) -> Option<Self> {
        Some(Self {
            id: RutrackerForum::get_id(element, "id").or_else(|| {
                RutrackerForum::get_id(element.select(&selector(".small")).next()?, "href")
            })?,
            author: RutrackerForum::get_text(element, ".nick")?,
            stored_torrents: element
                .select(&selector(".post_body .postLink"))
                .filter_map(|link| RutrackerForum::get_id(link, "href"))
                .collect(),
            topic: Rc::clone(&topic.0),
        })
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

#[derive(Debug, Clone)]
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

    fn from_element(element: ElementRef<'_>, forum: &Forum) -> Option<Self> {
        Some(Self(Rc::new(TopicData {
            id: RutrackerForum::get_id(element, "data-topic_id")?,
            author: RutrackerForum::get_text(element, ".vf-col-author")?,
            title: RutrackerForum::get_text(element, ".tt-text")?,
            forum: Rc::clone(&forum.0),
        })))
    }

    pub fn get_posts(&self) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        for page in self.iter() {
            page?.select(&selector("#topic_main")).for_each(|t| {
                posts.extend(
                    t.select(&selector(".row1,.row2"))
                        .filter_map(|e| Post::from_element(e, self)),
                )
            });
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
        for p in iter {
            posts.extend(
                p?.select(&selector(".row1,.row2"))
                    .filter_map(|e| Post::from_element(e, self)),
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
        let document = Html::parse_document(resp.text()?.as_str());
        let post_id = document
            .select(&selector(".mrg_16 a"))
            .next()
            .and_then(|d| RutrackerForum::get_id(d, "href"));
        Ok(post_id)
    }
}

#[derive(Debug, Clone)]
pub struct ForumData {
    pub id: usize,
    pub title: String,
    rutracker: Rc<RutrackerForumData>,
}

#[derive(Debug, Clone)]
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
        for p in self.iter() {
            topics.extend(
                p?.select(&selector(".hl-tr"))
                    .filter_map(|e| Topic::from_element(e, self)),
            );
        }
        Ok(topics)
    }
}

#[derive(Debug, Clone)]
pub struct RutrackerForumData {
    pub user: User,
    client: Client,
    url: String,
    dry_run: bool,
}

#[derive(Debug, Clone)]
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
        Ok(Self(Rc::new(RutrackerForumData {
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

    fn get_text(element: ElementRef<'_>, selectors: &str) -> Option<String> {
        Some(
            element
                .select(&selector(selectors))
                .next()?
                .text()
                .collect::<String>()
                .trim()
                .to_owned(),
        )
    }

    fn get_id(element: ElementRef<'_>, attribute: &str) -> Option<i32> {
        element
            .value()
            .attr(attribute)?
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

#[cfg(test)]
mod tests {
    use super::*;

    fn forum_document() -> Html {
        let page = r#"<table class="vf-table vf-gen forumline forum"><tr id="tr-4243634" class="hl-tr" data-topic_id="4243634">
            <td id="4243634" class="vf-col-icon vf-topic-icon-cell">
                <img class="topic_icon" src="https://static.t-ru.org/templates/v1/images/folder.gif" alt="">
                    </td>
            <td class="vf-col-t-title tt">
            <span class="topictitle">
                <a id="tt-4243634" href="viewtopic.php?t=4243634" class="topictitle tt-text">[Список] Hi-Res форматы, оцифровки » Hi-Res stereo и многоканальн<wbr>ая музыка » Джаз и Блюз (многоканаль<wbr>ная музыка)</a>
            </span>
            </td>
            <td class="vf-col-replies tCenter med">0</td>
            <td class="vf-col-author tCenter med">
                <a href="profile.php?mode=viewprofile&amp;u=1" rel="nofollow">Zhyvana</a></td>
            <td class="vf-col-last-post tCenter nowrap small" style="padding: 1px 6px 2px;">
                <p>2012-11-08 18:08</p>
                <p>
                    <a href="profile.php?mode=viewprofile&amp;u=1" rel="nofollow">Zhyvana</a>
                    <a href="viewtopic.php?p=56209547#56209547" rel="nofollow">
                        <img src="https://static.t-ru.org/templates/v1/images/icon_latest_reply.gif" class="icon2" alt="&raquo;">
                    </a>
                </p>
            </td>
            </tr></table>"#;
        Html::parse_document(page)
    }

    fn topic_document() -> Html {
        let page = r##"<table class="topic" id="topic_main"><tbody id="post_74823806" class="row2">
                    <tr>
                        <td class="poster_info td1 hide-for-print">
                            <a id="74823806"></a>
                            <p class="nick " title="Вставить выделенный кусок сообщения" onclick="bbcode.onclickPoster('TestUser3', '74823806');">
                                <a href="#" onclick="return false;">TestUser3</a>
                            </p>
                            <p class="rank_img"><img src="https://static.t-ru.org/ranks/hraniteli.gif" alt="Хранитель"></p>
                            <p class="joined"><em>Стаж:</em> 10 лет 1 месяц</p>
                            <p class="posts"><em>Сообщений:</em> 13</p>
                            <p class="flag"><img src="https://static.t-ru.org/flags/143.gif" class="poster-flag" alt="flag" title="Россия"></p>
                        </td>
                        <td class="message td2" rowspan="2">
                            <div class="post_head">
                                <p class="post-time">
                                    <span class="hl-scrolled-to-wrap">
                                        <span class="show-for-print bold">TestUser3 &middot; </span>
                                        <img src="https://static.t-ru.org/templates/v1/images/icon_minipost.gif" class="icon1 hide-for-print"
                                            alt="">
                                        <a class="p-link small" href="viewtopic.php?p=74823806#74823806">27-Дек-18 10:54</a>
                                    </span>
                                    <span class="posted_since hide-for-print">(спустя 11 месяцев)</span>
                                </p>
                                <p style="float: right; padding: 3px 2px 4px;" class="hide-for-print">
                                    <a class="txtb" href="posting.php?mode=quote&amp;p=74823806">[Цитировать]</a>&nbsp;
                                </p>
                                <div class="clear"></div>
                            </div>
                            <div class="post_wrap" id="p-42077832-4">
                                <div class="post_body" id="p-74823806">
                                    Актуально на <span class="post-b">27.12.2018</span><br>
                                    Количество сидируемых раздач: <span class="post-b">11</span><br>
                                    Общий объём: <span class="post-b">65 GB</span>
                                    <div class="sp-wrap">
                                        <div class="sp-head folded"><span>Раздачи</span></div>
                                        <div class="sp-body">
                                            <ol type="1">
                                                <li><a href="viewtopic.php?t=3257068" class="postLink">Взаперти / Locked Down (Дэниэл Зирилли / Daniel
                                                        Zirilli) [1080p] [2010, боевик, BDRip] DVO</a> · <span class="post-b">8.26 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=3256971" class="postLink">Взаперти / Locked Down (Дэниэл Зирилли / Daniel
                                                        Zirilli) [720p] [2010, боевик, BDRip] DVO</a> · <span class="post-b">4.68 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=3112723" class="postLink">Гончие ада / Воины Эллады / Hellhounds (Рик
                                                        Шродер / Rick Schroder) [720p] [2009, ужасы, BDRip] MVOx2</a> · <span class="post-b">4.71 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=3658846" class="postLink">Записки Лазаря / The Lazarus Papers (Джереми
                                                        Хандли / Jeremiah Hundley, Дэниэл Зирилли / Daniel Zirilli) [2010, США, фантастика, боевик, драма,
                                                        BDRip 1080p] DVO [канал ДТВ] + original eng</a> · <span class="post-b">6.67 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=3658103" class="postLink">Записки Лазаря / The Lazarus Papers (Джереми
                                                        Хандли / Jeremiah Hundley, Дэниэл Зирилли / Daniel Zirilli) [2010, США, фантастика, боевик, драма,
                                                        BDRip 720p] DVO [канал ДТВ] + original eng</a> · <span class="post-b">4.49 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=4494260" class="postLink">Корона и дракон / The Crown and the Dragon (Энн
                                                        К. Блэк / Anne K. Black) [2013, США, фэнтези, приключения, BDRip 1080p] VO (VANO) + Original Eng +
                                                        Sub Eng</a> · <span class="post-b">9.24 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=3163695" class="postLink">Круг боли / Circle of Pain (Дэниэл Зирилли /
                                                        Daniel Zirilli) [1080p] [2010, боевик, BDRip] DVO</a> · <span class="post-b">6.83 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=4154537" class="postLink">Круг боли / Circle of Pain (Дэниэл Зирилли /
                                                        Daniel Zirilli) [2010, США, боевик, драма, BDRip 720p] MVO(НТВ+) + Sub Eng + Original Eng</a> ·
                                                    <span class="post-b">4.47 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=3851508" class="postLink">Призраки Салема / A Haunting in Salem (Шэйн Ван
                                                        Дайк / Shane Van Dyke) [2011, США, Триллер, ужасы BDRip 1080p] VO Original Eng</a> · <span class="post-b">6.82 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=3838012" class="postLink">Призраки Салема / A Haunting in Salem (Шэйн Ван
                                                        Дайк / Shane Van Dyke) [2011, США, Триллер, ужасы BDRip 720p] VO Original Eng</a> · <span class="post-b">4.63 GB</span><br></li>
                                                <li><a href="viewtopic.php?t=1934628" class="postLink">Слияние с зомби / Automaton transfusion (Стивен
                                                        Миллер / Steven Miller) [720p] [2006, ужасы, BDRip]</a> · <span class="post-b">4.54 GB</span></li>
                                            </ol>
                                        </div>
                                    </div>
                                </div>
                                <!--/post_body-->
                            </div>
                            <!--/post_wrap-->
                        </td>
                    </tr>
                    <tr>
                        <td class="poster_btn td3 hide-for-print">
                            <div style="padding: 2px 6px 4px;" class="post_btn_2">
                                <a class="txtb" href="profile.php?mode=viewprofile&amp;u=1654861">[Профиль]</a>&nbsp;
                                <a class="txtb" href="privmsg.php?mode=post&amp;u=1654861">[ЛС]</a>&nbsp;
                            </div>
                        </td>
                    </tr>
                </tbody>
            </table>"##;
        Html::parse_document(page)
    }

    #[test]
    fn user_get_keys() {
        let page = r#"<table class="user_details borderless w100"><tr><th>Хранительские ключи:</th>
            <td class="med">bt: <b>s5hSHCn7QZ</b> api: <b>hjw7SmAkgC</b>
            id: <b>69166419</b></td></tr></table>"#;
        let keys = User::get_keys(page);
        let bt = String::from("s5hSHCn7QZ");
        let api = String::from("hjw7SmAkgC");
        let id = 69166419;
        assert_eq!(Some((bt, api, id)), keys);
    }

    #[test]
    fn user_get_form_token() {
        let page = r#"<script>
            window.BB = {
                cur_domain: location.hostname.replace(/.*?([^.]+\.[^.]+)$/, '$1'),
                // keep space " : " for TLO
                form_token: 'f59bb89sc9b72ff261e1ba2ce960098d',
                begun_iframe_src: "https://rutrk.org/iframe/begun-1.html",
                IS_GUEST: !!'',
                BB_SCRIPT: 'profile',
                IMG_URL: 'https://static.t-ru.org/templates/v1/images',
                SMILES_URL: 'https://static.t-ru.org/smiles',
                FORUM_ID: '',
                };
            BB.cookie_defaults = {
                domain: '.' + BB.cur_domain,
                path: "/forum/",
            };
            </script>"#;
        let form_token = User::get_form_token(page);
        let token = String::from("f59bb89sc9b72ff261e1ba2ce960098d");
        assert_eq!(Some(token), form_token);
    }

    #[test]
    fn forum_get_id() {
        let document = forum_document();
        let element = document.select(&selector(".hl-tr")).next().unwrap();
        let id = RutrackerForum::get_id(element, "data-topic_id");
        assert_eq!(id, Some(4243634));
    }

    #[test]
    fn forum_get_text() {
        let document = forum_document();
        let element = document.select(&selector(".hl-tr")).next().unwrap();

        let author = RutrackerForum::get_text(element, ".vf-col-author");
        assert_eq!(author.as_ref().map(|s| s.as_ref()), Some("Zhyvana"));

        let title = RutrackerForum::get_text(element, ".tt-text");
        assert_eq!(
            title.as_ref().map(|s| s.as_ref()),
            Some("[Список] Hi-Res форматы, оцифровки » Hi-Res stereo и многоканальная музыка » Джаз и Блюз (многоканальная музыка)")
        );
    }

    #[test]
    fn topic_get_id() {
        let document = topic_document();
        let element = document
            .select(&selector("#topic_main"))
            .next()
            .unwrap()
            .select(&selector(".row1,.row2"))
            .next()
            .unwrap();
        let id = RutrackerForum::get_id(element, "id");
        assert_eq!(id, Some(74823806));

        let vec: Vec<_> = element
            .select(&selector(".postLink"))
            .filter_map(|link| RutrackerForum::get_id(link, "href"))
            .collect();
        let id = vec![
            3257068, 3256971, 3112723, 3658846, 3658103, 4494260, 3163695, 4154537, 3851508,
            3838012, 1934628,
        ];
        assert_eq!(vec, id);
    }

    #[test]
    fn topic_get_text() {
        let document = topic_document();
        let element = document
            .select(&selector("#topic_main"))
            .next()
            .unwrap()
            .select(&selector(".row1,.row2"))
            .next()
            .unwrap();
        let author = RutrackerForum::get_text(element, ".nick");
        assert_eq!(author.as_ref().map(|s| s.as_ref()), Some("TestUser3"));
    }
}
