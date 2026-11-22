use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use eyre::eyre;
use parking_lot::RwLock;
use reqwest::{Response, StatusCode};
use reqwest_middleware::{ClientWithMiddleware, RequestBuilder};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use source_api::{CoreConfig, PageRef, ProxyMode};

#[allow(async_fn_in_trait)]
pub trait SendWithTimeoutMsg {
    /// Sends a request and maps timeout errors to a user-friendly message.
    async fn send_with_timeout_msg(self) -> eyre::Result<Response>;
}

impl SendWithTimeoutMsg for RequestBuilder {
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

/// Downloads the raw bytes of a single page image.
pub async fn fetch_page_bytes(
    img_client: Arc<RwLock<ClientWithMiddleware>>,
    page: &PageRef,
) -> eyre::Result<Bytes> {
    // Build the request first so the lock guard is dropped before the await.
    let request = {
        let client = img_client.read();
        let mut request = client.get(&page.url);
        for (key, value) in &page.headers {
            request = request.header(key.as_str(), value.as_str());
        }
        request
    };

    let http_resp = request.send_with_timeout_msg().await?;
    let status = http_resp.status();
    if status != StatusCode::OK {
        let body = http_resp.text().await?;
        return Err(eyre!("预料之外的状态码({status}): {body}"));
    }

    let image_data = http_resp.bytes().await?;
    Ok(image_data)
}

pub fn create_img_client(config: &CoreConfig) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    let client_builder = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(config.img_timeout_sec));
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
