use chrono::NaiveDate;
use source_api::{
    page_outside_window, BrowseResult, ChapterPages, Comic, ComicId, ComicSummary, SearchResult,
    Source, SourceError, UpdateWindow,
};

fn make_summary(id: i64, updated_on: Option<NaiveDate>) -> ComicSummary {
    ComicSummary {
        comic_id: ComicId {
            source_id: "test".to_string(),
            native_id: id,
        },
        title: format!("Comic {id}"),
        cover_url: format!("https://example.com/{id}.jpg"),
        status_line: "连载中".to_string(),
        updated_on,
        rating: Some(9.0),
    }
}

#[test]
fn test_page_outside_window_days7() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();

    // today is inside
    let item_today = make_summary(1, Some(today));
    assert!(!page_outside_window(&[item_today], UpdateWindow::Days7, today));

    // today - 6 days is inside
    let d6 = today - chrono::Duration::days(6);
    let item_d6 = make_summary(2, Some(d6));
    assert!(!page_outside_window(&[item_d6], UpdateWindow::Days7, today));

    // today - 7 days is inclusive cutoff (inside)
    let d7 = today - chrono::Duration::days(7);
    let item_d7 = make_summary(3, Some(d7));
    assert!(!page_outside_window(&[item_d7], UpdateWindow::Days7, today));

    // today - 8 days is outside
    let d8 = today - chrono::Duration::days(8);
    let item_d8 = make_summary(4, Some(d8));
    assert!(page_outside_window(&[item_d8], UpdateWindow::Days7, today));

    // None is treated as outside
    let item_none = make_summary(5, None);
    assert!(page_outside_window(&[item_none], UpdateWindow::Days7, today));

    // Mixed: one inside, one outside -> not all outside -> false
    let item_d5 = make_summary(6, Some(today - chrono::Duration::days(5)));
    let item_d10 = make_summary(7, Some(today - chrono::Duration::days(10)));
    assert!(!page_outside_window(
        &[item_d5, item_d10],
        UpdateWindow::Days7,
        today
    ));

    // Mixed: all outside (one old, one None) -> all outside -> true
    let item_d12 = make_summary(8, Some(today - chrono::Duration::days(12)));
    let item_none2 = make_summary(9, None);
    assert!(page_outside_window(
        &[item_d12, item_none2],
        UpdateWindow::Days7,
        today
    ));
}

#[test]
fn test_page_outside_window_days15_and_days30() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();

    let d15 = today - chrono::Duration::days(15);
    let item_d15 = make_summary(1, Some(d15));
    // d15 is inside Days15 (inclusive cutoff)
    assert!(!page_outside_window(&[item_d15.clone()], UpdateWindow::Days15, today));
    // d15 is outside Days7
    assert!(page_outside_window(&[item_d15], UpdateWindow::Days7, today));

    let d16 = today - chrono::Duration::days(16);
    let item_d16 = make_summary(2, Some(d16));
    assert!(page_outside_window(&[item_d16], UpdateWindow::Days15, today));

    let d30 = today - chrono::Duration::days(30);
    let item_d30 = make_summary(3, Some(d30));
    // d30 is inside Days30
    assert!(!page_outside_window(&[item_d30.clone()], UpdateWindow::Days30, today));
    // d30 is outside Days15
    assert!(page_outside_window(&[item_d30], UpdateWindow::Days15, today));

    let d31 = today - chrono::Duration::days(31);
    let item_d31 = make_summary(4, Some(d31));
    assert!(page_outside_window(&[item_d31], UpdateWindow::Days30, today));
}

#[test]
fn test_page_outside_window_all() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();

    // Ancient item
    let ancient = today - chrono::Duration::days(3650);
    let item_ancient = make_summary(1, Some(ancient));
    assert!(!page_outside_window(&[item_ancient], UpdateWindow::All, today));

    // Item with None
    let item_none = make_summary(2, None);
    assert!(!page_outside_window(&[item_none], UpdateWindow::All, today));

    // Empty list
    assert!(!page_outside_window(&[], UpdateWindow::All, today));
}

#[test]
fn test_page_outside_window_all_none() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();
    let items = vec![make_summary(1, None), make_summary(2, None)];

    assert!(page_outside_window(&items, UpdateWindow::Days7, today));
    assert!(page_outside_window(&items, UpdateWindow::Days15, today));
    assert!(page_outside_window(&items, UpdateWindow::Days30, today));
    assert!(!page_outside_window(&items, UpdateWindow::All, today));
}

struct DummySource;

#[async_trait::async_trait]
impl Source for DummySource {
    fn id(&self) -> &'static str {
        "dummy"
    }

    async fn search(&self, _keyword: &str, _page: i64) -> Result<SearchResult, SourceError> {
        Ok(SearchResult {
            comics: vec![],
            current: 1,
            total: 0,
        })
    }

    async fn browse_latest(
        &self,
        _window: UpdateWindow,
        _page: i64,
        _genre: Option<&str>,
    ) -> Result<BrowseResult, SourceError> {
        Ok(BrowseResult {
            items: vec![],
            page: 1,
            total_pages: 1,
        })
    }

    async fn get_comic(&self, _native_id: i64) -> Result<Comic, SourceError> {
        Err(SourceError::NotFound {
            message: "not found".to_string(),
        })
    }

    async fn get_chapter_pages(
        &self,
        _native_id: i64,
        _chapter_id: i64,
    ) -> Result<ChapterPages, SourceError> {
        Err(SourceError::NotFound {
            message: "not found".to_string(),
        })
    }
}

#[tokio::test]
async fn test_source_trait_object_safety() {
    let source: Box<dyn Source> = Box::new(DummySource);
    assert_eq!(source.id(), "dummy");
    let res = source.browse_latest(UpdateWindow::Days7, 1, None).await;
    assert!(res.is_ok());
}
