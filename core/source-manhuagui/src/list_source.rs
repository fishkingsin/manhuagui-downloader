use async_trait::async_trait;
use source_api::{
    BrowseResult, ChapterPages, Comic, SearchResult, Source, SourceError, UpdateWindow,
};

use crate::{
    client::{img_url, ManhuaguiClient},
    decrypt::decrypt,
    list::{browse_latest_url, parse_list_page},
};

pub struct ManhuaguiSource {
    client: ManhuaguiClient,
}

impl ManhuaguiSource {
    pub fn new(client: ManhuaguiClient) -> Self {
        Self { client }
    }

    pub fn client(&self) -> &ManhuaguiClient {
        &self.client
    }
}

pub fn map_eyre_to_source_error(err: eyre::Report) -> SourceError {
    if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
        if req_err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
            return SourceError::NotFound {
                message: err.to_string(),
            };
        }
        if req_err.is_timeout() || req_err.is_connect() {
            return SourceError::Network {
                message: err.to_string(),
            };
        }
    }
    if let Some(mw_err) = err.downcast_ref::<reqwest_middleware::Error>() {
        match mw_err {
            reqwest_middleware::Error::Reqwest(e) => {
                if e.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                    return SourceError::NotFound {
                        message: err.to_string(),
                    };
                }
                if e.is_timeout() || e.is_connect() {
                    return SourceError::Network {
                        message: err.to_string(),
                    };
                }
            }
            reqwest_middleware::Error::Middleware(_) => {}
        }
    }
    let msg = err.to_string();
    if msg.contains("404") || msg.contains("NotFound") || msg.contains("未找到") {
        SourceError::NotFound { message: msg }
    } else if msg.contains("timeout")
        || msg.contains("超时")
        || msg.contains("connect")
        || msg.contains("dns")
        || msg.contains("网络")
    {
        SourceError::Network { message: msg }
    } else if msg.contains("parse")
        || msg.contains("解析")
        || msg.contains("解密")
        || msg.contains("Selector")
    {
        SourceError::ParseFailed { message: msg }
    } else {
        SourceError::Network { message: msg }
    }
}

#[async_trait]
impl Source for ManhuaguiSource {
    fn id(&self) -> &'static str {
        "manhuagui"
    }

    async fn search(&self, keyword: &str, page: i64) -> Result<SearchResult, SourceError> {
        self.client
            .search(keyword, page)
            .await
            .map_err(map_eyre_to_source_error)
    }

    async fn browse_latest(
        &self,
        _window: UpdateWindow,
        page: i64,
        genre: Option<&str>,
    ) -> Result<BrowseResult, SourceError> {
        let url = browse_latest_url(genre, page);
        let body = self
            .client
            .get_html(&url)
            .await
            .map_err(map_eyre_to_source_error)?;
        parse_list_page(&body).map_err(map_eyre_to_source_error)
    }

    async fn get_comic(&self, native_id: i64) -> Result<Comic, SourceError> {
        self.client
            .get_comic(native_id)
            .await
            .map_err(map_eyre_to_source_error)
    }

    async fn get_chapter_pages(
        &self,
        native_id: i64,
        chapter_id: i64,
    ) -> Result<ChapterPages, SourceError> {
        let url = format!("https://www.manhuagui.com/comic/{native_id}/{chapter_id}.html");
        let body = self
            .client
            .get_html(&url)
            .await
            .map_err(map_eyre_to_source_error)?;
        let decrypt_result = decrypt(&body).map_err(map_eyre_to_source_error)?;
        let pages = decrypt_result
            .files
            .iter()
            .map(|file| img_url(&decrypt_result.path, file))
            .collect();
        let prev_chapter_id = if decrypt_result.prev_id == 0 {
            None
        } else {
            Some(decrypt_result.prev_id)
        };
        let next_chapter_id = if decrypt_result.next_id == 0 {
            None
        } else {
            Some(decrypt_result.next_id)
        };

        Ok(ChapterPages {
            pages,
            prev_chapter_id,
            next_chapter_id,
        })
    }
}
