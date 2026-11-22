//! Regression tests for `DownloadManager::create_download_task`.
//!
//! The desktop contract is: creating a download task is synchronous and
//! performs the duplicate-task check *before* any network work; page fetching
//! happens inside the task's background `process()`, so a fetch failure marks
//! the task `Failed` (it still appears in the UI as a task first).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use eyre::eyre;
use library::downloader::download_manager::DownloadManager;
use library::events::{EVENT_TASK_CREATE, EVENT_TASK_UPDATE};
use source_api::{
    ChapterInfo, Comic, CoreConfig, EventObserver, LibraryEvent, PageFetcher, PageRef,
};

/// A page fetcher that records how many times it was called and can be made to
/// fail, so tests can prove whether fetching happens before the duplicate check
/// and how a fetch error surfaces.
struct MockFetcher {
    calls: AtomicUsize,
    fail: bool,
}

impl MockFetcher {
    fn ok() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail: true,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl PageFetcher for MockFetcher {
    async fn fetch_pages(&self, _chapter_info: &ChapterInfo) -> eyre::Result<Vec<PageRef>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(eyre!("模拟获取图片链接失败"))
        } else {
            Ok(vec![])
        }
    }
}

/// An observer that records every emitted event so tests can assert the
/// order/content of the `TaskCreate` → `Failed` flow.
#[derive(Default)]
struct RecordingObserver {
    events: Mutex<Vec<LibraryEvent>>,
}

impl RecordingObserver {
    fn events(&self) -> Vec<LibraryEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .map(|e| LibraryEvent {
                name: e.name,
                payload: e.payload.clone(),
            })
            .collect()
    }
}

impl EventObserver for RecordingObserver {
    fn on_event(&self, event: &LibraryEvent) {
        self.events.lock().unwrap().push(LibraryEvent {
            name: event.name,
            payload: event.payload.clone(),
        });
    }
}

/// A minimal comic/chapter whose download-dir fields are pre-populated, so
/// `ensure_download_dir_fields` short-circuits (no `strfmt` resolution) and
/// `save_metadata` writes into the given scratch directory.
fn comic_with_dir(dir: &std::path::Path) -> (Comic, i64) {
    let chapter_id = 123;
    let chapter = ChapterInfo {
        chapter_id,
        chapter_title: "第01话".into(),
        chapter_size: 10,
        comic_id: 1,
        comic_title: "测试漫画".into(),
        group_name: "单话".into(),
        group_size: 1,
        order: 1.0,
        comic_status: "连载中".into(),
        is_pdf_exported: false,
        is_cbz_exported: false,
        is_downloaded: None,
        chapter_download_dir: Some(dir.to_path_buf()),
    };
    let comic = Comic {
        id: 1,
        title: "测试漫画".into(),
        subtitle: None,
        cover: String::new(),
        status: "连载中".into(),
        update_time: String::new(),
        year: 2024,
        region: String::new(),
        genres: vec![],
        authors: vec![],
        aliases: vec![],
        intro: String::new(),
        groups: HashMap::from([("单话".to_string(), vec![chapter])]),
        is_downloaded: None,
        comic_download_dir: Some(dir.to_path_buf()),
    };
    (comic, chapter_id)
}

/// The duplicate-task check must run synchronously, before any page fetch. A
/// `#[tokio::test]` (current_thread) never polls the spawned background task,
/// so if `create_download_task` performed a fetch first the counter would move.
#[tokio::test]
async fn create_download_task_rejects_duplicate_without_fetching() {
    let fetcher = Arc::new(MockFetcher::ok());
    let manager = DownloadManager::new(
        Arc::new(parking_lot::RwLock::new(CoreConfig::default())),
        fetcher.clone(),
        Arc::new(RecordingObserver::default()),
    );

    let dir = std::env::temp_dir().join("mhg-core-test-dup");
    let (comic, chapter_id) = comic_with_dir(&dir);

    manager
        .create_download_task(comic.clone(), chapter_id)
        .unwrap();

    let err = manager.create_download_task(comic, chapter_id).unwrap_err();
    assert_eq!(err.to_string(), "章节ID对应的下载任务已存在");
    assert_eq!(
        fetcher.calls(),
        0,
        "the duplicate check must not fetch pages"
    );
}

/// A fetch failure must still surface a `TaskCreate` event first, then mark the
/// task `Failed` (matching the desktop behavior where the task appears in the
/// UI and then fails), rather than being swallowed before task creation.
#[tokio::test(flavor = "multi_thread")]
async fn fetch_failure_marks_task_failed_after_create() {
    let observer = Arc::new(RecordingObserver::default());
    let manager = DownloadManager::new(
        Arc::new(parking_lot::RwLock::new(CoreConfig::default())),
        Arc::new(MockFetcher::failing()),
        observer.clone(),
    );

    let dir = std::env::temp_dir().join(format!("mhg-core-test-fail-{}", std::process::id()));
    let (comic, chapter_id) = comic_with_dir(&dir);

    manager.create_download_task(comic, chapter_id).unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let events = observer.events();
        let saw_create = events.iter().any(|e| e.name == EVENT_TASK_CREATE);
        let saw_failed = events.iter().any(|e| {
            e.name == EVENT_TASK_UPDATE
                && e.payload.get("state").and_then(|v| v.as_str()) == Some("Failed")
        });

        if saw_create && saw_failed {
            break;
        }

        assert!(
            Instant::now() < deadline,
            "task did not emit TaskCreate then Failed within the timeout"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let _ = std::fs::remove_dir_all(&dir);
}
