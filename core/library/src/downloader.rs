pub mod download_img_task;
pub mod download_manager;
pub mod download_task;
pub mod download_task_state;
mod img_client;

use std::sync::{atomic::AtomicU64, Arc, OnceLock};

use parking_lot::RwLock;
use reqwest_middleware::ClientWithMiddleware;
use tokio::sync::Semaphore;

use source_api::{CoreConfig, EventObserver};

/// Returns a handle to the current Tokio runtime, or a library-owned fallback
/// runtime when the caller is not inside one (e.g. during desktop app setup).
///
/// `tokio::spawn` panics outside a runtime context; the original code used
/// `tauri::async_runtime::spawn`, which explicitly enters a runtime. The
/// library cannot depend on Tauri, so we mirror that behavior here.
fn runtime_handle() -> tokio::runtime::Handle {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        return handle;
    }

    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME
        .get_or_init(|| tokio::runtime::Runtime::new().expect("failed to create tokio runtime"))
        .handle()
        .clone()
}

pub(crate) fn spawn<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    runtime_handle().spawn(future);
}

/// Shared dependencies the downloader tasks carry in place of a desktop
/// `AppHandle`. The library never touches Tauri or the manhuagui source crate.
#[derive(Clone)]
pub struct DownloadDeps {
    pub config: Arc<RwLock<CoreConfig>>,
    pub observer: Arc<dyn EventObserver>,
    pub chapter_sem: Arc<Semaphore>,
    pub img_sem: Arc<Semaphore>,
    pub byte_per_sec: Arc<AtomicU64>,
    pub img_client: Arc<RwLock<ClientWithMiddleware>>,
}

pub trait EyreReportToMessage {
    fn to_message(&self) -> String;
}

impl EyreReportToMessage for eyre::Report {
    fn to_message(&self) -> String {
        format!("{self:?}")
    }
}
