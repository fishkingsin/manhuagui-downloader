use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use bytes::Bytes;
use eyre::{eyre, OptionExt, WrapErr};
use parking_lot::RwLock;
use reqwest::{Response, StatusCode};
use reqwest_middleware::{ClientWithMiddleware, RequestBuilder};
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use source_api::{ChapterInfo, Comic, CoreConfig, PageRef, ProxyMode, SearchResult};
use tracing::instrument;

use crate::{decrypt, parse_comic, parse_search};

/// Constructs a `PageRef` for an image on manhuagui CDN.
/// Trims `.webp` extension from the URL if present and sets the `Referer` header.
pub fn img_url(path: &str, file: &str) -> PageRef {
    let raw = format!("https://i.hamreus.com{path}{file}");
    let url = raw.trim_end_matches(".webp").to_string();
    let mut headers = HashMap::new();
    headers.insert(
        "Referer".to_string(),
        "https://www.manhuagui.com/".to_string(),
    );
    PageRef { url, headers }
}

#[allow(async_fn_in_trait)]
pub trait SendWithTimeoutMsg {
    /// 发送请求并处理超时错误
    ///
    /// - 如果遇到超时错误，返回带有用户友好信息的错误
    /// - 否则返回原始错误
    async fn send_with_timeout_msg(self) -> eyre::Result<Response>;
}

impl SendWithTimeoutMsg for RequestBuilder {
    #[instrument(level = "error", skip_all)]
    async fn send_with_timeout_msg(self) -> eyre::Result<Response> {
        self.send().await.map_err(|e| {
            if e.is_timeout() || e.is_middleware() {
                eyre::Report::from(e).wrap_err(
                    "网络连接超时，可能是未使用代理或IP被封，请使用代理或切换代理线路后重试",
                )
            } else {
                eyre::Report::from(e)
            }
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub username: String,
    pub avatar: String,
}

impl UserProfile {
    #[instrument(level = "error", skip_all)]
    pub fn from_html(html: &str) -> eyre::Result<UserProfile> {
        let document = Html::parse_document(html);
        let avatar_box_sel = Selector::parse(".avatar-box").map_err(|e| eyre!("{e:?}"))?;
        let avatar_box = document
            .select(&avatar_box_sel)
            .next()
            .ok_or_eyre("没有找到`.avatar-box`的<div>")?;

        let h3_sel = Selector::parse("h3").map_err(|e| eyre!("{e:?}"))?;
        let username = avatar_box
            .select(&h3_sel)
            .next()
            .map(|h3| h3.text().collect::<String>().trim().to_string())
            .ok_or_eyre("没有找到用户名相关的<h3>")?;

        let img_sel = Selector::parse(".img-box img").map_err(|e| eyre!("{e:?}"))?;
        let avatar = avatar_box
            .select(&img_sel)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(|src| format!("https:{src}"))
            .ok_or_eyre("没有找到头像相关的<img>")?;

        let user_profile = UserProfile { username, avatar };
        Ok(user_profile)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct GetFavoriteResult {
    pub comics: Vec<ComicInFavorite>,
    pub current: i64,
    pub total: i64,
}

impl GetFavoriteResult {
    #[instrument(level = "error", skip_all)]
    pub fn from_html(html: &str) -> eyre::Result<GetFavoriteResult> {
        let empty_map = HashMap::new();
        Self::from_html_with_map(html, &empty_map)
    }

    #[instrument(level = "error", skip_all)]
    pub fn from_html_with_map(
        html: &str,
        id_to_dir_map: &HashMap<i64, PathBuf>,
    ) -> eyre::Result<GetFavoriteResult> {
        let document = Html::parse_document(html);
        let mut comics = Vec::new();
        let item_sel = Selector::parse(".dy_content_li").map_err(|e| eyre!("{e:?}"))?;
        for book_div in document.select(&item_sel) {
            let comic = ComicInFavorite::from_div(&book_div, id_to_dir_map)?;
            comics.push(comic);
        }

        let current_sel = Selector::parse(".current").map_err(|e| eyre!("{e:?}"))?;
        let current = match document.select(&current_sel).next() {
            Some(span) => span
                .text()
                .next()
                .ok_or_eyre("没有在当前页码的<span>中找到文本")?
                .parse::<i64>()
                .wrap_err("当前页码不是整数")?,
            None => 1,
        };

        let total_sel = Selector::parse(".flickr.right > span").map_err(|e| eyre!("{e:?}"))?;
        let total = match document.select(&total_sel).next() {
            Some(span) => span
                .text()
                .next()
                .ok_or_eyre("没有在总页数的<span>中找到文本")?
                .trim_start_matches("共")
                .trim_end_matches("记录")
                .parse::<i64>()
                .wrap_err("总页数不是整数")?,
            None => 1,
        };

        Ok(GetFavoriteResult {
            comics,
            current,
            total,
        })
    }

    pub fn update_fields(&mut self, id_to_dir_map: &HashMap<i64, PathBuf>) {
        for comic in &mut self.comics {
            comic.update_fields(id_to_dir_map);
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct ComicInFavorite {
    /// 漫画id
    pub id: i64,
    /// 漫画标题
    pub title: String,
    /// 漫画封面链接
    pub cover: String,
    /// 最近更新时间，两种格式
    /// - 2024-12-13
    /// - x分钟前
    pub last_update: String,
    /// 上次阅读时间，两种格式
    /// - 2024-12-13
    /// - x分钟前
    pub last_read: String,
    /// 是否已下载
    pub is_downloaded: bool,
    /// 漫画的下载目录
    pub comic_download_dir: PathBuf,
}

impl ComicInFavorite {
    #[instrument(level = "error", skip_all)]
    pub fn from_div(
        div: &ElementRef,
        id_to_dir_map: &HashMap<i64, PathBuf>,
    ) -> eyre::Result<ComicInFavorite> {
        let title_sel = Selector::parse(".dy_content_li h3 > a").map_err(|e| eyre!("{e:?}"))?;
        let a = div
            .select(&title_sel)
            .next()
            .ok_or_eyre("没有找到标题相关的<a>")?;

        let id = a
            .value()
            .attr("href")
            .ok_or_eyre("没有在标题和链接的<a>中找到href属性")?
            .trim_start_matches("/comic/")
            .trim_end_matches('/')
            .parse::<i64>()
            .wrap_err("漫画id不是整数")?;

        let title = a
            .text()
            .next()
            .ok_or_eyre("没有在标题和链接的<a>中找到文本")?
            .trim()
            .to_string();

        let img_sel = Selector::parse(".dy_img img").map_err(|e| eyre!("{e:?}"))?;
        let cover_src = div
            .select(&img_sel)
            .next()
            .ok_or_eyre("没有找到封面的<img>")?
            .value()
            .attr("src")
            .ok_or_eyre("没有在封面的<img>中找到src属性")?;
        let cover = format!("https:{cover_src}");

        let time_sel =
            Selector::parse(".dy_r > p > em:nth-child(2)").map_err(|e| eyre!("{e:?}"))?;
        let mut time_nodes = div.select(&time_sel);
        let last_update = time_nodes
            .next()
            .ok_or_eyre("没有找到最近更新时间<em>")?
            .text()
            .next()
            .ok_or_eyre("没有在最近更新时间<em>中找到文本")?
            .trim()
            .to_string();

        let last_read = time_nodes
            .next()
            .ok_or_eyre("没有找到上次阅读时间<em>")?
            .text()
            .next()
            .ok_or_eyre("没有在上次阅读时间<em>中找到文本")?
            .trim()
            .to_string();

        let mut comic = ComicInFavorite {
            id,
            title,
            cover,
            last_update,
            last_read,
            is_downloaded: false,
            comic_download_dir: PathBuf::new(),
        };

        comic.update_fields(id_to_dir_map);

        Ok(comic)
    }

    pub fn update_fields(&mut self, id_to_dir_map: &HashMap<i64, PathBuf>) {
        if let Some(comic_download_dir) = id_to_dir_map.get(&self.id) {
            self.comic_download_dir = comic_download_dir.clone();
            self.is_downloaded = true;
        }
    }
}

#[derive(Clone)]
pub struct ManhuaguiClient {
    config: Arc<RwLock<CoreConfig>>,
    api_client: Arc<RwLock<ClientWithMiddleware>>,
    img_client: Arc<RwLock<ClientWithMiddleware>>,
}

impl ManhuaguiClient {
    pub fn new(config: &CoreConfig) -> Self {
        let api_client = create_api_client(config);
        let api_client = Arc::new(RwLock::new(api_client));

        let img_client = create_img_client(config);
        let img_client = Arc::new(RwLock::new(img_client));

        Self {
            config: Arc::new(RwLock::new(config.clone())),
            api_client,
            img_client,
        }
    }

    #[instrument(level = "error", skip_all)]
    pub fn reload_client(&self, config: &CoreConfig) {
        *self.config.write() = config.clone();
        let api_client = create_api_client(config);
        *self.api_client.write() = api_client;
        let img_client = create_img_client(config);
        *self.img_client.write() = img_client;
    }

    #[instrument(level = "error", skip_all)]
    pub fn update_config(&self, config: &CoreConfig) {
        *self.config.write() = config.clone();
    }

    #[instrument(level = "error", skip_all, fields(url = url))]
    pub async fn get_html(&self, url: &str) -> eyre::Result<String> {
        let request = self.api_client.read().get(url);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == StatusCode::NOT_FOUND {
            return Err(eyre!("404 Not Found: {url}"));
        } else if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        Ok(body)
    }

    #[instrument(level = "error", skip_all)]
    pub async fn login(&self, username: &str, password: &str) -> eyre::Result<String> {
        let params = json!({"action": "user_login"});
        let form = json!({
            "txtUserName": username,
            "txtPassword": password,
        });
        let request = self
            .api_client
            .read()
            .get("https://www.manhuagui.com/tools/submit_ajax.ashx")
            .query(&params)
            .form(&form);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let headers = http_resp.headers().clone();
        let body = http_resp.text().await?;
        if status == StatusCode::FOUND {
            return Err(eyre!("cookie已过期或无效"));
        } else if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        let cookie = headers
            .get("set-cookie")
            .ok_or_eyre(format!("响应中没有set-cookie字段: {body}"))?
            .to_str()
            .wrap_err(format!("响应中的set-cookie字段不是utf-8字符串: {body}"))?
            .to_string();

        Ok(cookie)
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_user_profile(&self) -> eyre::Result<UserProfile> {
        let cookie = self.config.read().cookie.clone();
        let request = self
            .api_client
            .read()
            .get("https://www.manhuagui.com/user/center/index")
            .header("cookie", cookie);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == StatusCode::FOUND {
            return Err(eyre!("未登录、cookie已过期或cookie无效"));
        } else if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }

        let user_profile = UserProfile::from_html(&body).wrap_err("将body转换为UserProfile失败")?;
        Ok(user_profile)
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(keyword = keyword, page_num = page_num)
    )]
    pub async fn search(&self, keyword: &str, page_num: i64) -> eyre::Result<SearchResult> {
        let url = format!("https://www.manhuagui.com/s/{keyword}_p{page_num}.html");
        let request = self.api_client.read().get(url);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        let mut search_result =
            parse_search(&body, page_num).wrap_err("将body转换为SearchResult失败")?;
        let id_to_dir_map = source_api::create_id_to_dir_map(&self.config.read().download_dir)?;
        for comic in &mut search_result.comics {
            comic.update_fields(&id_to_dir_map);
        }
        Ok(search_result)
    }

    #[instrument(level = "error", skip_all, fields(comic_id = id))]
    pub async fn get_comic(&self, id: i64) -> eyre::Result<Comic> {
        let request = self
            .api_client
            .read()
            .get(format!("https://www.manhuagui.com/comic/{id}/"));
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        let mut comic = parse_comic(&body).wrap_err("将body转换为Comic失败")?;
        let id_to_dir_map = source_api::create_id_to_dir_map(&self.config.read().download_dir)?;
        // TODO: 这是为了兼容v0.4.2及之前的版本，后续需要移除，计划在v0.6.0之后移除
        if let Some(comic_download_dir) = id_to_dir_map.get(&comic.id) {
            comic
                .create_chapter_metadata_for_old_version(comic_download_dir)
                .wrap_err("为旧版本创建章节元数据失败")?;
        }
        comic.update_fields(&id_to_dir_map)?;

        Ok(comic)
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_id = chapter_info.comic_id,
            comic_title = chapter_info.comic_title,
            group_name = chapter_info.group_name,
            chapter_id = chapter_info.chapter_id
        )
    )]
    pub async fn get_img_urls(&self, chapter_info: &ChapterInfo) -> eyre::Result<Vec<PageRef>> {
        let comic_id = chapter_info.comic_id;
        let chapter_id = chapter_info.chapter_id;

        let url = format!("https://www.manhuagui.com/comic/{comic_id}/{chapter_id}.html");
        let request = self.api_client.read().get(url);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }

        let decrypt_result = decrypt(&body).wrap_err("解密失败")?;

        let pages = decrypt_result
            .files
            .iter()
            .map(|file| img_url(&decrypt_result.path, file))
            .collect();

        Ok(pages)
    }

    #[instrument(level = "error", skip_all, fields(url = url))]
    pub async fn get_img_bytes(&self, url: &str) -> eyre::Result<Bytes> {
        let request = self
            .img_client
            .read()
            .get(url)
            .header("referer", "https://www.manhuagui.com/");
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        let image_data = http_resp.bytes().await?;

        Ok(image_data)
    }

    #[instrument(level = "error", skip_all, fields(page_num = page_num))]
    pub async fn get_favorite(&self, page_num: i64) -> eyre::Result<GetFavoriteResult> {
        let cookie = self.config.read().cookie.clone();
        let url = format!("https://www.manhuagui.com/user/book/shelf/{page_num}");
        let request = self.api_client.read().get(url).header("cookie", cookie);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        let mut get_favorite_result =
            GetFavoriteResult::from_html(&body).wrap_err("将body转换为GetFavoriteResult失败")?;
        let id_to_dir_map = source_api::create_id_to_dir_map(&self.config.read().download_dir)?;
        get_favorite_result.update_fields(&id_to_dir_map);
        Ok(get_favorite_result)
    }
}

fn create_api_client(config: &CoreConfig) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1) // 指数为1，保证重试间隔为1秒不变
        .jitter(Jitter::Bounded) // 重试间隔在1秒左右波动
        .build_with_total_retry_duration(Duration::from_secs(5)); // 重试总时长为5秒

    let client_builder = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(config.api_timeout_sec))
        .redirect(reqwest::redirect::Policy::none());
    let client = set_proxy(client_builder, config, "api_client")
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn create_img_client(config: &CoreConfig) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    let client_builder =
        reqwest::ClientBuilder::new().timeout(Duration::from_secs(config.img_timeout_sec));
    let client = set_proxy(client_builder, config, "img_client")
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn set_proxy(
    builder: reqwest::ClientBuilder,
    config: &CoreConfig,
    client_name: &str,
) -> reqwest::ClientBuilder {
    match config.proxy_mode {
        ProxyMode::System => builder,
        ProxyMode::NoProxy => builder.no_proxy(),
        ProxyMode::Custom => {
            let proxy_host = &config.proxy_host;
            let proxy_port = &config.proxy_port;
            let proxy_url = format!("http://{proxy_host}:{proxy_port}");

            match reqwest::Proxy::all(&proxy_url).map_err(eyre::Report::from) {
                Ok(proxy) => builder.proxy(proxy),
                Err(err) => {
                    tracing::error!("{client_name}将`{proxy_url}`设为代理失败，将直连: {err:?}");
                    builder.no_proxy()
                }
            }
        }
    }
}
