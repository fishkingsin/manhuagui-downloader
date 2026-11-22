use chrono::NaiveDate;
use source_api::{CoreConfig, Source};
use source_manhuagui::{browse_latest_url, parse_list_page, ManhuaguiClient, ManhuaguiSource};

#[test]
fn test_parse_list_page_japan() {
    let html = include_str!("fixtures/list_japan.html");
    let res = parse_list_page(html).expect("parse_list_page should succeed");

    assert_eq!(res.page, 1);
    assert_eq!(res.total_pages, 1263);
    assert!(
        res.items.len() >= 20,
        "expected >= 20 items, got {}",
        res.items.len()
    );

    let first = &res.items[0];
    assert_eq!(first.comic_id.source_id, "manhuagui");
    assert_eq!(first.comic_id.native_id, 24558);
    assert_eq!(first.title, "转生奇谭");
    assert_eq!(first.cover_url, "https://cf.mhgui.com/cpic/b/24558_94.jpg");
    assert_eq!(first.status_line, "更新至第70话机翻");
    assert_eq!(
        first.updated_on,
        Some(NaiveDate::from_ymd_opt(2026, 11, 22).unwrap())
    );
    assert_eq!(first.rating, Some(9.0));
}

#[test]
fn test_parse_list_page_japan_p2() {
    let html = include_str!("fixtures/list_japan_update_p2.html");
    let res = parse_list_page(html).expect("parse_list_page p2 should succeed");

    assert_eq!(res.page, 2);
    assert_eq!(res.total_pages, 1263);
    assert!(
        res.items.len() >= 20,
        "expected >= 20 items, got {}",
        res.items.len()
    );
}

#[test]
fn test_parse_list_page_missing_date() {
    let html = include_str!("fixtures/list_missing_date.html");
    let res =
        parse_list_page(html).expect("parse_list_page with missing/invalid date should succeed");

    assert_eq!(res.items.len(), 3);

    // First item has valid date and rating
    assert_eq!(
        res.items[0].updated_on,
        Some(NaiveDate::from_ymd_opt(2026, 11, 22).unwrap())
    );
    assert_eq!(res.items[0].rating, Some(9.0));

    // Second item has no updateon tag
    assert_eq!(res.items[1].updated_on, None);
    assert_eq!(res.items[1].rating, None);

    // Third item has invalid date text
    assert_eq!(res.items[2].updated_on, None);
    assert_eq!(res.items[2].rating, Some(6.0));
}

#[test]
fn test_manhuagui_source_id_and_trait() {
    let config = CoreConfig::default();
    let client = ManhuaguiClient::new(&config);
    let source = ManhuaguiSource::new(client);

    assert_eq!(source.id(), "manhuagui");

    let boxed: Box<dyn Source> = Box::new(source);
    assert_eq!(boxed.id(), "manhuagui");
}

#[test]
fn test_browse_latest_url_pattern() {
    assert_eq!(
        browse_latest_url(None, 1),
        "https://www.manhuagui.com/list/japan/update.html"
    );
    assert_eq!(
        browse_latest_url(None, 2),
        "https://www.manhuagui.com/list/japan/update_p2.html"
    );
    assert_eq!(
        browse_latest_url(Some("rexue"), 1),
        "https://www.manhuagui.com/list/japan_rexue/update.html"
    );
    assert_eq!(
        browse_latest_url(Some("rexue"), 3),
        "https://www.manhuagui.com/list/japan_rexue/update_p3.html"
    );
}

#[test]
fn test_map_eyre_to_source_error() {
    use source_api::SourceError;
    use source_manhuagui::map_eyre_to_source_error;

    let not_found_err = eyre::eyre!("404 Not Found: https://example.com");
    match map_eyre_to_source_error(not_found_err) {
        SourceError::NotFound { message } => assert!(message.contains("404")),
        other => panic!("expected NotFound, got {other:?}"),
    }

    let timeout_err = eyre::eyre!("网络连接超时");
    match map_eyre_to_source_error(timeout_err) {
        SourceError::Network { message } => assert!(message.contains("超时")),
        other => panic!("expected Network, got {other:?}"),
    }

    let parse_err = eyre::eyre!("无法解析漫画 ID");
    match map_eyre_to_source_error(parse_err) {
        SourceError::ParseFailed { message } => assert!(message.contains("解析")),
        other => panic!("expected ParseFailed, got {other:?}"),
    }
}
