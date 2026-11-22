//! Tests for the source registry (`Library`) and the windowed browse engine
//! (`browse_within`), over a `MockSource` backed by static fixture pages.
//! No network access; `ComicSummary` values are constructed directly.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use source_api::{
    BrowseResult, ChapterPages, Comic, ComicId, ComicSummary, SearchResult, Source, SourceError,
    UpdateWindow,
};

use library::service::{browse_within, Library};

fn summary(id: i64, updated_on: Option<NaiveDate>) -> ComicSummary {
    ComicSummary {
        comic_id: ComicId {
            source_id: "mock".to_string(),
            native_id: id,
        },
        title: format!("Comic {id}"),
        cover_url: format!("https://example.com/{id}.jpg"),
        status_line: "连载中".to_string(),
        updated_on,
        rating: Some(9.0),
    }
}

fn page(page: i64, total_pages: i64, items: Vec<ComicSummary>) -> BrowseResult {
    BrowseResult {
        items,
        page,
        total_pages,
    }
}

/// A date `n` days before "today". `n=1` is unambiguously inside a Days7
/// window and `n=20` unambiguously outside, regardless of whether "today" is
/// taken in UTC or Asia/Shanghai (the two differ by at most one calendar day).
fn days_ago(n: i64) -> NaiveDate {
    chrono::Utc::now().date_naive() - chrono::Duration::days(n)
}

/// A `Source` served from static fixture pages, with a fetch counter so tests
/// can assert exactly how many pages `browse_within` walked.
struct MockSource {
    pages: Vec<BrowseResult>,
    fetch_count: AtomicUsize,
}

impl MockSource {
    fn new(pages: Vec<BrowseResult>) -> Self {
        Self {
            pages,
            fetch_count: AtomicUsize::new(0),
        }
    }

    fn fetch_count(&self) -> usize {
        self.fetch_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Source for MockSource {
    fn id(&self) -> &'static str {
        "mock"
    }

    async fn search(&self, _keyword: &str, _page: i64) -> Result<SearchResult, SourceError> {
        unreachable!("search is not exercised by service tests")
    }

    async fn browse_latest(
        &self,
        _window: UpdateWindow,
        page: i64,
        _genre: Option<&str>,
    ) -> Result<BrowseResult, SourceError> {
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        let index = (page - 1) as usize;
        Ok(self.pages[index].clone())
    }

    async fn get_comic(&self, _native_id: i64) -> Result<Comic, SourceError> {
        unreachable!("get_comic is not exercised by service tests")
    }

    async fn get_chapter_pages(
        &self,
        _native_id: i64,
        _chapter_id: i64,
    ) -> Result<ChapterPages, SourceError> {
        unreachable!("get_chapter_pages is not exercised by service tests")
    }
}

#[test]
fn library_registers_and_looks_up_sources() {
    let library = Library::new();
    let mock: Arc<dyn Source> = Arc::new(MockSource::new(vec![]));

    library.register_source(mock);

    let found = library.source("mock").expect("source should be registered");
    assert_eq!(found.id(), "mock");
    assert!(library.source("missing").is_none());
}

#[tokio::test]
async fn browse_within_days7_returns_in_window_items_and_stops_outside() {
    // Page 1 is entirely inside the window; page 2 is entirely outside, so the
    // walk must stop after fetching exactly 2 pages and return only page 1's items.
    let p1 = page(
        1,
        10,
        vec![summary(1, Some(days_ago(1))), summary(2, Some(days_ago(2)))],
    );
    let p2 = page(
        2,
        10,
        vec![
            summary(3, Some(days_ago(20))),
            summary(4, Some(days_ago(21))),
        ],
    );
    let mock = MockSource::new(vec![p1, p2]);

    let result = browse_within(&mock, UpdateWindow::Days7, 1, 50)
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].comic_id.native_id, 1);
    assert_eq!(result[1].comic_id.native_id, 2);
    assert_eq!(mock.fetch_count(), 2);
}

#[tokio::test]
async fn browse_within_all_returns_single_page() {
    // `All` fetches exactly `start_page` and returns every item verbatim,
    // including None-dated entries and entries outside any Days window.
    let p1 = page(
        1,
        5,
        vec![
            summary(1, Some(days_ago(1))),
            summary(2, None),
            summary(3, Some(days_ago(20))),
        ],
    );
    let mock = MockSource::new(vec![p1]);

    let result = browse_within(&mock, UpdateWindow::All, 1, 50)
        .await
        .unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].comic_id.native_id, 1);
    assert_eq!(result[1].comic_id.native_id, 2);
    assert_eq!(result[2].comic_id.native_id, 3);
    assert_eq!(mock.fetch_count(), 1);
}

#[tokio::test]
async fn browse_within_days7_drops_none_dated_items() {
    // A None-dated item is outside the window and must be dropped from a
    // windowed result.
    let p1 = page(1, 1, vec![summary(1, Some(days_ago(1))), summary(2, None)]);
    let mock = MockSource::new(vec![p1]);

    let result = browse_within(&mock, UpdateWindow::Days7, 1, 50)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].comic_id.native_id, 1);
}

#[tokio::test]
async fn browse_within_honors_max_pages_cap() {
    // Every page is inside the window and none is the last, so only the hard
    // `max_pages` cap can stop the walk.
    let p1 = page(1, 100, vec![summary(1, Some(days_ago(1)))]);
    let p2 = page(2, 100, vec![summary(2, Some(days_ago(1)))]);
    let p3 = page(3, 100, vec![summary(3, Some(days_ago(1)))]);
    let mock = MockSource::new(vec![p1, p2, p3]);

    let result = browse_within(&mock, UpdateWindow::Days7, 1, 2)
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].comic_id.native_id, 1);
    assert_eq!(result[1].comic_id.native_id, 2);
    assert_eq!(mock.fetch_count(), 2);
}

#[tokio::test]
async fn browse_within_stops_at_last_page() {
    // Both pages are inside the window, but the walk must stop because page 2
    // is the last page (`total_pages == 2`).
    let p1 = page(1, 2, vec![summary(1, Some(days_ago(1)))]);
    let p2 = page(2, 2, vec![summary(2, Some(days_ago(1)))]);
    let mock = MockSource::new(vec![p1, p2]);

    let result = browse_within(&mock, UpdateWindow::Days7, 1, 50)
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].comic_id.native_id, 1);
    assert_eq!(result[1].comic_id.native_id, 2);
    assert_eq!(mock.fetch_count(), 2);
}
