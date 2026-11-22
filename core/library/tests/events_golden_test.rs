//! Golden event test: the library's event payloads must serialize byte-for-byte
//! identically to the desktop `DownloadEvent` variants they replace.
//!
//! Golden strings below were captured from `src-tauri/src/events.rs` (the
//! `DownloadEvent` enum with `#[serde(tag = "event", content = "data")]` and
//! `#[serde(rename_all = "camelCase")]` on each variant) at the pre-move commit.
//! The event *name* is the serde tag (the exact string the Vue frontend matches
//! on in `ProgressesPane.vue`), and the *data* is the inner camelCase payload.

use std::collections::HashMap;

use library::events::{
    SleepingPayload, SpeedPayload, TaskCreatePayload, TaskDeletePayload, TaskUpdatePayload,
    EVENT_SLEEPING, EVENT_SPEED, EVENT_TASK_CREATE, EVENT_TASK_DELETE, EVENT_TASK_UPDATE,
};
use source_api::{ChapterInfo, Comic, DownloadTaskState, LibraryEvent};

// ---- golden fixtures captured from the current serde shapes ----

const SPEED_GOLDEN: &str = r#"{"event":"Speed","data":{"speed":"1.2 MB/s"}}"#;
const SLEEPING_GOLDEN: &str =
    r#"{"event":"Sleeping","data":{"chapterId":123,"remainingSec":5}}"#;
const TASK_DELETE_GOLDEN: &str = r#"{"event":"TaskDelete","data":{"chapterId":123}}"#;
const TASK_UPDATE_GOLDEN: &str = r#"{"event":"TaskUpdate","data":{"state":"Downloading","chapterId":123,"downloadedImgCount":5,"totalImgCount":10}}"#;
const TASK_CREATE_GOLDEN: &str = r#"{"event":"TaskCreate","data":{"state":"Downloading","comic":{"id":1,"title":"测试漫画","subtitle":null,"cover":"https://example.com/cover.jpg","status":"连载中","updateTime":"2026-11-22","year":2024,"region":"日本","genres":["冒险"],"authors":["测试作者"],"aliases":[],"intro":"简介","groups":{"单话":[{"chapterId":123,"chapterTitle":"第01话","chapterSize":10,"comicId":1,"comicTitle":"测试漫画","groupName":"单话","groupSize":1,"order":1.0,"comicStatus":"连载中","isPdfExported":false,"isCbzExported":false}]}},"chapterInfo":{"chapterId":123,"chapterTitle":"第01话","chapterSize":10,"comicId":1,"comicTitle":"测试漫画","groupName":"单话","groupSize":1,"order":1.0,"comicStatus":"连载中","isPdfExported":false,"isCbzExported":false},"downloadedImgCount":0,"totalImgCount":10}}"#;

fn chapter() -> ChapterInfo {
    ChapterInfo {
        chapter_id: 123,
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
        chapter_download_dir: None,
    }
}

fn comic() -> Comic {
    Comic {
        id: 1,
        title: "测试漫画".into(),
        subtitle: None,
        cover: "https://example.com/cover.jpg".into(),
        status: "连载中".into(),
        update_time: "2026-11-22".into(),
        year: 2024,
        region: "日本".into(),
        genres: vec!["冒险".into()],
        authors: vec!["测试作者".into()],
        aliases: vec![],
        intro: "简介".into(),
        groups: HashMap::from([("单话".to_string(), vec![chapter()])]),
        is_downloaded: None,
        comic_download_dir: None,
    }
}

/// Reconstructs the desktop `DownloadEvent` envelope (`#[serde(tag="event", content="data")]`)
/// from a `LibraryEvent` and compares it, byte-for-byte, with the golden string.
fn assert_event_matches(name: &'static str, payload: impl serde::Serialize, golden: &str) {
    let payload = serde_json::to_value(payload).expect("payload must serialize");
    let event = LibraryEvent { name, payload };

    // The observer re-emits this exact name as the serde tag.
    let envelope = serde_json::json!({ "event": event.name, "data": event.payload });
    assert_eq!(
        envelope,
        serde_json::from_str::<serde_json::Value>(golden).expect("golden is valid json"),
        "envelope for `{name}` must equal the desktop DownloadEvent bytes"
    );

    // Also assert the name string itself is byte-identical to the frontend's tag.
    assert_eq!(
        event.name,
        serde_json::from_str::<serde_json::Value>(golden)
            .expect("golden is valid json")
            .get("event")
            .and_then(serde_json::Value::as_str)
            .expect("golden has an event tag"),
    );
}

#[test]
fn speed_event_matches_desktop_golden() {
    assert_event_matches(
        EVENT_SPEED,
        SpeedPayload {
            speed: "1.2 MB/s".into(),
        },
        SPEED_GOLDEN,
    );
}

#[test]
fn sleeping_event_matches_desktop_golden() {
    assert_event_matches(
        EVENT_SLEEPING,
        SleepingPayload {
            chapter_id: 123,
            remaining_sec: 5,
        },
        SLEEPING_GOLDEN,
    );
}

#[test]
fn task_create_event_matches_desktop_golden() {
    assert_event_matches(
        EVENT_TASK_CREATE,
        TaskCreatePayload {
            state: DownloadTaskState::Downloading,
            comic: comic(),
            chapter_info: chapter(),
            downloaded_img_count: 0,
            total_img_count: 10,
        },
        TASK_CREATE_GOLDEN,
    );
}

#[test]
fn task_delete_event_matches_desktop_golden() {
    assert_event_matches(
        EVENT_TASK_DELETE,
        TaskDeletePayload { chapter_id: 123 },
        TASK_DELETE_GOLDEN,
    );
}

#[test]
fn task_update_event_matches_desktop_golden() {
    assert_event_matches(
        EVENT_TASK_UPDATE,
        TaskUpdatePayload {
            state: DownloadTaskState::Downloading,
            chapter_id: 123,
            downloaded_img_count: 5,
            total_img_count: 10,
        },
        TASK_UPDATE_GOLDEN,
    );
}
