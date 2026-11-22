use chrono::NaiveDate;

use crate::{
    error::SourceError,
    types::{
        BrowseResult, ChapterInfo, ChapterPages, Comic, ComicSummary, PageRef, SearchResult,
        UpdateWindow,
    },
};

#[async_trait::async_trait]
pub trait Source: Send + Sync {
    fn id(&self) -> &'static str;

    /// "Today" in the source's local timezone, used as the window reference
    /// date. Sources whose site dates are in a non-local timezone override
    /// this (see `ManhuaguiSource`, which uses Asia/Shanghai).
    fn today(&self) -> NaiveDate {
        chrono::Local::now().date_naive()
    }

    async fn search(&self, keyword: &str, page: i64) -> Result<SearchResult, SourceError>;
    async fn browse_latest(
        &self,
        window: UpdateWindow,
        page: i64,
        genre: Option<&str>,
    ) -> Result<BrowseResult, SourceError>;
    async fn get_comic(&self, native_id: i64) -> Result<Comic, SourceError>;
    async fn get_chapter_pages(
        &self,
        native_id: i64,
        chapter_id: i64,
    ) -> Result<ChapterPages, SourceError>;
}

/// Fetches the page image references for a chapter (network + decrypt), in
/// display order.
///
/// The downloader injects this so it can fetch pages *inside* a task's
/// background processing — after the duplicate-task check and the `TaskCreate`
/// event — without importing the site adapter crate.
#[async_trait::async_trait]
pub trait PageFetcher: Send + Sync {
    async fn fetch_pages(&self, chapter_info: &ChapterInfo) -> eyre::Result<Vec<PageRef>>;
}

/// Returns true when ALL items' `updated_on` are older than the window cutoff.
///
/// - Items with `updated_on: None` are treated as outside the window.
/// - The cutoff date is inclusive (`updated_on == today - window_days` is inside the window).
/// - `UpdateWindow::All` is never outside (`false`).
pub fn page_outside_window(items: &[ComicSummary], window: UpdateWindow, today: NaiveDate) -> bool {
    let days = match window {
        UpdateWindow::Days7 => 7,
        UpdateWindow::Days15 => 15,
        UpdateWindow::Days30 => 30,
        UpdateWindow::All => return false,
    };

    let cutoff = today - chrono::Duration::days(days);
    items.iter().all(|item| match item.updated_on {
        Some(date) => date < cutoff,
        None => true,
    })
}
