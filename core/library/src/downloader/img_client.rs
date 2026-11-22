use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use eyre::eyre;
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use source_api::{CoreConfig, PageRef, ProxyMode, SendWithTimeoutMsg};

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
