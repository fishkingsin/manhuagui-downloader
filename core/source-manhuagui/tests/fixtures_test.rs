#[test]
fn test_decrypt_chapter() {
    let html = include_str!("fixtures/chapter.html");
    let res = source_manhuagui::decrypt(html).expect("decrypt should succeed");
    assert_eq!(res.bid, 22992);
    assert_eq!(res.cid, 286036);
    assert_eq!(res.bname, "爆发吋前");
    assert_eq!(res.cname, "第01回");
    assert_eq!(res.files.len(), 13);
    assert!(!res.path.is_empty());
}

#[test]
fn test_parse_comic() {
    let html = include_str!("fixtures/comic.html");
    let comic = source_manhuagui::parse_comic(html).expect("parse_comic should succeed");
    assert_eq!(comic.id, 22992);
    assert_eq!(comic.title, "爆发吋前");
    assert_eq!(comic.status, "连载中");
    assert_eq!(comic.year, 2017);
    assert_eq!(comic.region, "日本漫画");
    assert_eq!(comic.authors, vec!["永远的菜鸟"]);
    assert_eq!(comic.genres, vec!["搞笑"]);

    let group = comic.groups.get("单话").expect("missing 单话 group");
    assert_eq!(group.len(), 1);
    assert_eq!(group[0].chapter_id, 286036);
    assert_eq!(group[0].chapter_title, "第01回");
}

#[test]
fn test_parse_search() {
    let html = include_str!("fixtures/search.html");
    let search = source_manhuagui::parse_search(html, 1).expect("parse_search should succeed");
    assert_eq!(search.total, 12);
    assert_eq!(search.current, 1);
    assert_eq!(search.comics.len(), 10);
    assert_eq!(search.comics[0].id, 1128);
    assert_eq!(search.comics[0].title, "ONE PIECE航海王");
    assert_eq!(search.comics[0].subtitle.as_deref(), Some("海贼王"));
    assert_eq!(search.comics[0].year, 1997);
    assert_eq!(search.comics[0].region, "日本漫画");
    assert_eq!(search.comics[0].authors, vec!["尾田荣一郎"]);
    assert_eq!(
        search.comics[0].genres,
        vec![
            "热血".to_string(),
            "冒险".to_string(),
            "格斗".to_string()
        ]
    );
}

#[test]
fn test_parse_list_page() {
    let html = include_str!("fixtures/list_japan.html");
    let res = source_manhuagui::parse_list_page(html).expect("parse_list_page should succeed");
    assert!(res.items.len() >= 20, "expected >= 20 items, got {}", res.items.len());

    let first = &res.items[0];
    assert_eq!(first.comic_id.native_id, 24558);
    assert_eq!(first.title, "转生奇谭");
    assert!(first.cover_url.starts_with("https:"));
    assert_eq!(first.status_line, "更新至第70话机翻");
    assert_eq!(
        first.updated_on,
        Some(chrono::NaiveDate::from_ymd_opt(2026, 11, 22).unwrap())
    );
    assert_eq!(first.rating, Some(9.0));
    assert!(res.total_pages > 1000);
}

#[test]
fn test_parse_list_page_missing_date() {
    let html = include_str!("fixtures/list_missing_date.html");
    let res = source_manhuagui::parse_list_page(html).expect("parse_list_page should succeed");
    assert_eq!(res.items.len(), 3);

    // First item has valid date
    assert_eq!(
        res.items[0].updated_on,
        Some(chrono::NaiveDate::from_ymd_opt(2026, 11, 22).unwrap())
    );
    // Second item has missing date
    assert_eq!(res.items[1].updated_on, None);
    // Third item has unparseable date
    assert_eq!(res.items[2].updated_on, None);
    assert_eq!(res.items[2].rating, Some(6.0));
}

