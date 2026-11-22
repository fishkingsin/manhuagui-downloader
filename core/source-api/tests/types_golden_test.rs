use std::collections::HashMap;
use std::path::PathBuf;
use source_api::{
    BrowseResult, ChapterDirFmtParams, ChapterInfo, ChapterPages, Comic, ComicDirFmtParams,
    ComicId, ComicInSearch, ComicSummary, DownloadTaskState, PageRef, SearchResult,
    SearchResultComic, UpdateWindow,
};

#[test]
fn test_comic_golden_roundtrip() {
    let comic_json = include_str!("fixtures/pre_move_comic.json");
    let comic: Comic = serde_json::from_str(comic_json).expect("failed to deserialize pre_move_comic.json");

    assert_eq!(comic.id, 67890);
    assert_eq!(comic.title, "测试漫画");
    assert_eq!(comic.subtitle.as_deref(), Some("测试副标题"));
    assert_eq!(comic.year, 2024);
    assert_eq!(comic.status, "连载中");
    assert_eq!(comic.is_downloaded, Some(true));
    assert_eq!(comic.comic_download_dir, Some(PathBuf::from("/path/to/comic")));

    let reserialized = serde_json::to_string_pretty(&comic).expect("failed to serialize Comic");
    let v1: serde_json::Value = serde_json::from_str(comic_json).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(v1, v2, "Comic serialization must match pre-move golden fixture exactly");
}

#[test]
fn test_comic_metadata_golden_roundtrip() {
    let comic_json = include_str!("fixtures/pre_move_comic_metadata.json");
    let comic: Comic = serde_json::from_str(comic_json).expect("failed to deserialize pre_move_comic_metadata.json");

    assert_eq!(comic.id, 67890);
    assert_eq!(comic.is_downloaded, None);

    let reserialized = serde_json::to_string_pretty(&comic).expect("failed to serialize Comic metadata");
    let v1: serde_json::Value = serde_json::from_str(comic_json).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(v1, v2, "Comic metadata serialization must match pre-move golden fixture exactly");
}

#[test]
fn test_chapter_info_golden_roundtrip() {
    let chapter_json = include_str!("fixtures/pre_move_chapter_info.json");
    let chapter: ChapterInfo = serde_json::from_str(chapter_json).expect("failed to deserialize pre_move_chapter_info.json");

    assert_eq!(chapter.chapter_id, 12345);
    assert_eq!(chapter.chapter_title, "第01话");
    assert_eq!(chapter.chapter_size, 25);
    assert_eq!(chapter.comic_id, 67890);
    assert_eq!(chapter.is_downloaded, Some(true));
    assert!(chapter.is_cbz_exported);
    assert!(!chapter.is_pdf_exported);
    assert_eq!(chapter.chapter_download_dir, Some(PathBuf::from("/path/to/chapter")));

    let reserialized = serde_json::to_string_pretty(&chapter).expect("failed to serialize ChapterInfo");
    let v1: serde_json::Value = serde_json::from_str(chapter_json).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(v1, v2, "ChapterInfo serialization must match pre-move golden fixture exactly");
}

#[test]
fn test_chapter_metadata_golden_roundtrip() {
    let chapter_json = include_str!("fixtures/pre_move_chapter_metadata.json");
    let chapter: ChapterInfo = serde_json::from_str(chapter_json).expect("failed to deserialize pre_move_chapter_metadata.json");

    assert_eq!(chapter.chapter_id, 12345);
    assert_eq!(chapter.is_downloaded, None);
    assert_eq!(chapter.chapter_download_dir, None);

    let reserialized = serde_json::to_string_pretty(&chapter).expect("failed to serialize ChapterInfo metadata");
    let v1: serde_json::Value = serde_json::from_str(chapter_json).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(v1, v2, "ChapterInfo metadata serialization must match pre-move golden fixture exactly");
}

#[test]
fn test_download_task_state_serialization() {
    let states = vec![
        (DownloadTaskState::Pending, "\"Pending\""),
        (DownloadTaskState::Downloading, "\"Downloading\""),
        (DownloadTaskState::Paused, "\"Paused\""),
        (DownloadTaskState::Completed, "\"Completed\""),
        (DownloadTaskState::Failed, "\"Failed\""),
    ];

    for (state, json) in states {
        assert_eq!(serde_json::to_string(&state).unwrap(), json);
        let deserialized: DownloadTaskState = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, state);
    }
}

#[test]
fn test_search_result_and_comic_in_search() {
    let comic = ComicInSearch {
        id: 101,
        title: "搜索漫画".to_string(),
        subtitle: Some("副标题".to_string()),
        cover: "https://example.com/cover.jpg".to_string(),
        status: "连载中".to_string(),
        update_time: "2026-11-22".to_string(),
        year: 2023,
        region: "日本".to_string(),
        genres: vec!["冒险".to_string()],
        authors: vec!["作者".to_string()],
        aliases: vec!["别名".to_string()],
        intro: "简介".to_string(),
        is_downloaded: true,
        comic_download_dir: PathBuf::from("/download/dir"),
    };

    let result = SearchResult {
        comics: vec![comic.clone()],
        current: 1,
        total: 100,
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: SearchResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, result);

    // Test SearchResultComic alias
    let comic_alias: SearchResultComic = comic.clone();
    assert_eq!(comic_alias.id, 101);
}

#[test]
fn test_dir_fmt_params() {
    let comic_fmt = ComicDirFmtParams {
        comic_id: 12345,
        comic_title: "测试漫画".to_string(),
        comic_subtitle: "副标题".to_string(),
        pub_year: 2023,
        region: "日本".to_string(),
        author: "作者".to_string(),
    };
    let json_comic = serde_json::to_string(&comic_fmt).unwrap();
    let des_comic: ComicDirFmtParams = serde_json::from_str(&json_comic).unwrap();
    assert_eq!(comic_fmt, des_comic);

    let chapter_fmt = ChapterDirFmtParams {
        comic_id: 12345,
        comic_title: "测试漫画".to_string(),
        comic_subtitle: "副标题".to_string(),
        pub_year: 2023,
        region: "日本".to_string(),
        author: "作者".to_string(),
        group_name: "单话".to_string(),
        chapter_id: 67890,
        chapter_title: "第1话".to_string(),
        order: 1.0,
    };
    let json_chap = serde_json::to_string(&chapter_fmt).unwrap();
    let des_chap: ChapterDirFmtParams = serde_json::from_str(&json_chap).unwrap();
    assert_eq!(chapter_fmt, des_chap);
}

#[test]
fn test_new_domain_types() {
    let comic_id = ComicId {
        source_id: "manhuagui".to_string(),
        native_id: 12345,
    };
    let json_id = serde_json::to_string(&comic_id).unwrap();
    let des_id: ComicId = serde_json::from_str(&json_id).unwrap();
    assert_eq!(des_id, comic_id);

    let window = UpdateWindow::Days7;
    let json_window = serde_json::to_string(&window).unwrap();
    let des_window: UpdateWindow = serde_json::from_str(&json_window).unwrap();
    assert_eq!(des_window, window);

    let summary = ComicSummary {
        comic_id: comic_id.clone(),
        title: "概要漫画".to_string(),
        cover_url: "https://example.com/cover.jpg".to_string(),
        status_line: "连载至10话".to_string(),
        updated_on: chrono::NaiveDate::from_ymd_opt(2026, 11, 22),
        rating: Some(9.5),
    };
    let json_summary = serde_json::to_string(&summary).unwrap();
    let des_summary: ComicSummary = serde_json::from_str(&json_summary).unwrap();
    assert_eq!(des_summary, summary);

    let browse = BrowseResult {
        items: vec![summary],
        page: 1,
        total_pages: 5,
    };
    let json_browse = serde_json::to_string(&browse).unwrap();
    let des_browse: BrowseResult = serde_json::from_str(&json_browse).unwrap();
    assert_eq!(des_browse, browse);

    let mut headers = HashMap::new();
    headers.insert("Referer".to_string(), "https://www.manhuagui.com/".to_string());
    let page_ref = PageRef {
        url: "https://img.manhuagui.com/001.jpg".to_string(),
        headers,
    };
    let chapter_pages = ChapterPages {
        pages: vec![page_ref],
        prev_chapter_id: None,
        next_chapter_id: Some(12346),
    };
    let json_pages = serde_json::to_string(&chapter_pages).unwrap();
    let des_pages: ChapterPages = serde_json::from_str(&json_pages).unwrap();
    assert_eq!(des_pages, chapter_pages);
}
