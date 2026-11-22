use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use eyre::{eyre, WrapErr};
use parking_lot::RwLock;
use reqwest_middleware::ClientWithMiddleware;
use tokio::sync::Semaphore;
use tracing::instrument;

use source_api::{Comic, CoreConfig, EventObserver, LibraryEvent, PageRef};

use crate::{
    downloader::{
        download_task::DownloadTask,
        download_task_state::DownloadTaskState,
        img_client::create_img_client,
        DownloadDeps,
    },
    events::{SpeedPayload, EVENT_SPEED},
};

pub struct DownloadManager {
    pub config: Arc<RwLock<CoreConfig>>,
    pub observer: Arc<dyn EventObserver>,
    pub img_client: Arc<RwLock<ClientWithMiddleware>>,
    pub chapter_sem: Arc<Semaphore>,
    pub img_sem: Arc<Semaphore>,
    pub byte_per_sec: Arc<AtomicU64>,
    download_tasks: RwLock<HashMap<i64, Arc<DownloadTask>>>,
}

impl DownloadManager {
    pub fn new(config: Arc<RwLock<CoreConfig>>, observer: Arc<dyn EventObserver>) -> Self {
        let (chapter_concurrency, img_concurrency) = {
            let config = config.read();
            (config.chapter_concurrency, config.img_concurrency)
        };

        let img_client = create_img_client(&config.read());

        let manager = DownloadManager {
            img_client: Arc::new(RwLock::new(img_client)),
            config,
            observer,
            chapter_sem: Arc::new(Semaphore::new(chapter_concurrency)),
            img_sem: Arc::new(Semaphore::new(img_concurrency)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            download_tasks: RwLock::new(HashMap::new()),
        };

        crate::downloader::spawn(Self::emit_download_speed_loop(
            manager.observer.clone(),
            manager.byte_per_sec.clone(),
        ));

        manager
    }

    /// Updates the live config snapshot (proxy/timeout/interval changes that do
    /// not require rebuilding the HTTP client).
    pub fn update_config(&self, config: &CoreConfig) {
        *self.config.write() = config.clone();
    }

    /// Updates the live config snapshot and rebuilds the image HTTP client
    /// (needed when proxy settings change).
    pub fn reload_client(&self, config: &CoreConfig) {
        *self.config.write() = config.clone();
        *self.img_client.write() = create_img_client(config);
    }

    fn deps(&self) -> DownloadDeps {
        DownloadDeps {
            config: self.config.clone(),
            observer: self.observer.clone(),
            chapter_sem: self.chapter_sem.clone(),
            img_sem: self.img_sem.clone(),
            byte_per_sec: self.byte_per_sec.clone(),
            img_client: self.img_client.clone(),
        }
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = comic.id, comic_title = comic.title, chapter_id = chapter_id)
    )]
    pub fn create_download_task(
        &self,
        comic: Comic,
        chapter_id: i64,
        pages: Vec<PageRef>,
    ) -> eyre::Result<()> {
        use DownloadTaskState::{Downloading, Paused, Pending};

        let mut tasks = self.download_tasks.write();

        if let Some(task) = tasks.get(&chapter_id) {
            let state = *task.state_sender.borrow();
            if matches!(state, Pending | Downloading | Paused) {
                return Err(eyre!("章节ID对应的下载任务已存在"));
            }
        }

        if let Some(task) = tasks.remove(&chapter_id) {
            task.delete_sender
                .send(())
                .wrap_err("通知旧下载任务删除失败")?;
        }

        let task = DownloadTask::new(self.deps(), comic, chapter_id, pages)?;
        tasks.insert(chapter_id, task);
        Ok(())
    }

    #[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
    pub fn pause_download_task(&self, chapter_id: i64) -> eyre::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&chapter_id) else {
            return Err(eyre!("未找到章节ID对应的下载任务"));
        };
        task.set_state(DownloadTaskState::Paused);
        Ok(())
    }

    #[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
    pub fn resume_download_task(&self, chapter_id: i64) -> eyre::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&chapter_id) else {
            return Err(eyre!("未找到章节ID对应的下载任务"));
        };
        task.set_state(DownloadTaskState::Pending);
        Ok(())
    }

    #[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
    pub fn delete_download_task(&self, chapter_id: i64) -> eyre::Result<()> {
        let mut tasks = self.download_tasks.write();
        let Some(task) = tasks.remove(&chapter_id) else {
            return Err(eyre!("未找到章节ID对应的下载任务"));
        };
        task.delete_sender
            .send(())
            .wrap_err("通知下载任务删除失败")?;
        Ok(())
    }

    async fn emit_download_speed_loop(observer: Arc<dyn EventObserver>, byte_per_sec: Arc<AtomicU64>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let byte_per_sec = byte_per_sec.swap(0, Ordering::Relaxed);
            #[allow(clippy::cast_precision_loss)]
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2} MB/s");
            if let Ok(payload) = serde_json::to_value(SpeedPayload { speed }) {
                observer.on_event(&LibraryEvent {
                    name: EVENT_SPEED,
                    payload,
                });
            }
        }
    }
}
