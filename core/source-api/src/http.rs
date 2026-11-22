use reqwest::Response;
use reqwest_middleware::RequestBuilder;
use tracing::instrument;

/// Sends a request and maps timeout/middleware errors to a user-friendly
/// message, leaving other errors untouched.
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
