use serde::{Deserialize, Serialize};

use source_api::{ChapterInfo, Comic, DownloadTaskState};

/// Event name (serde tag) for each download event. These mirror the exact
/// strings the Vue frontend dispatches on (`ProgressesPane.vue`), which are the
/// tauri_specta `DownloadEvent` variant tags.
pub const EVENT_SPEED: &str = "Speed";
pub const EVENT_SLEEPING: &str = "Sleeping";
pub const EVENT_TASK_CREATE: &str = "TaskCreate";
pub const EVENT_TASK_DELETE: &str = "TaskDelete";
pub const EVENT_TASK_UPDATE: &str = "TaskUpdate";

/// Payload of the `Speed` download event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedPayload {
    pub speed: String,
}

/// Payload of the `Sleeping` download event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepingPayload {
    pub chapter_id: i64,
    pub remaining_sec: u64,
}

/// Payload of the `TaskCreate` download event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreatePayload {
    pub state: DownloadTaskState,
    pub comic: Comic,
    pub chapter_info: ChapterInfo,
    pub downloaded_img_count: u32,
    pub total_img_count: u32,
}

/// Payload of the `TaskDelete` download event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDeletePayload {
    pub chapter_id: i64,
}

/// Payload of the `TaskUpdate` download event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdatePayload {
    pub state: DownloadTaskState,
    pub chapter_id: i64,
    pub downloaded_img_count: u32,
    pub total_img_count: u32,
}
