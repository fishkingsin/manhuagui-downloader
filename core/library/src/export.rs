mod cbz;
mod pdf;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

pub use cbz::{cbz, cbz_chapters};
use eyre::WrapErr;
use parking_lot::Mutex;
pub use pdf::{pdf, pdf_chapters};
use tracing::instrument;

use source_api::{ChapterInfo, EventObserver, LibraryEvent};

/// 导出互斥锁管理器，确保同一漫画的导出操作串行执行
#[derive(Debug, Clone, Default)]
pub struct ComicExportLock {
    locked_comic_ids: Arc<Mutex<HashSet<i64>>>,
}

impl ComicExportLock {
    pub fn new() -> Self {
        Self {
            locked_comic_ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// 尝试获取漫画导出锁，返回是否成功(如果该漫画正在导出则返回 false)
    pub fn try_acquire(&self, comic_id: i64) -> bool {
        let mut locked = self.locked_comic_ids.lock();
        if locked.contains(&comic_id) {
            return false;
        }

        locked.insert(comic_id);
        true
    }

    /// 释放漫画导出锁
    pub fn release(&self, comic_id: i64) {
        self.locked_comic_ids.lock().remove(&comic_id);
    }
}

/// RAII guard that releases a [`ComicExportLock`] on drop. The shell acquires
/// the lock around a library export call and holds this guard for its duration.
pub struct ComicExportLockGuard {
    lock: ComicExportLock,
    comic_id: i64,
}

impl ComicExportLockGuard {
    pub fn new(lock: ComicExportLock, comic_id: i64) -> Self {
        Self { lock, comic_id }
    }
}

impl Drop for ComicExportLockGuard {
    fn drop(&mut self) {
        self.lock.release(self.comic_id);
    }
}

/// Emits a serialized payload as a [`LibraryEvent`] on the observer.
pub(crate) fn emit_event(
    observer: &dyn EventObserver,
    name: &'static str,
    payload: impl serde::Serialize,
) {
    if let Ok(payload) = serde_json::to_value(payload) {
        observer.on_event(&LibraryEvent { name, payload });
    }
}

enum ExportFormat {
    Pdf,
    Cbz,
}

impl ExportFormat {
    fn extension(&self) -> &str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Cbz => "cbz",
        }
    }
}

trait PathIsImg {
    /// 判断路径是否为图片文件
    fn is_img(&self) -> bool;
}

impl PathIsImg for Path {
    fn is_img(&self) -> bool {
        self.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_lowercase)
            .is_some_and(|ext| matches!(ext.as_str(), "jpg"))
    }
}

/// 获取已下载的章节
fn get_downloaded_chapters(groups: &HashMap<String, Vec<ChapterInfo>>) -> Vec<ChapterInfo> {
    groups
        .values()
        .flatten()
        .filter(|chapter| chapter.is_downloaded.unwrap_or(false))
        .cloned()
        .collect()
}

/// 根据章节ID列表获取已下载的章节
fn get_downloaded_chapters_by_ids(
    groups: &HashMap<String, Vec<ChapterInfo>>,
    chapter_ids: &[i64],
) -> Vec<ChapterInfo> {
    let id_set: HashSet<_> = chapter_ids.iter().copied().collect();

    groups
        .values()
        .flatten()
        .filter(|chapter| {
            chapter.is_downloaded.unwrap_or(false) && id_set.contains(&chapter.chapter_id)
        })
        .cloned()
        .collect()
}

#[instrument(level = "error", skip_all, fields(images_dir = %images_dir.display()))]
fn get_image_paths(images_dir: &Path) -> eyre::Result<Vec<PathBuf>> {
    let mut image_paths: Vec<PathBuf> = std::fs::read_dir(images_dir)
        .wrap_err(format!("读取目录`{}`失败", images_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_img())
        .collect();
    image_paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(image_paths)
}
