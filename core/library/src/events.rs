use std::path::PathBuf;

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

// Export event names. CBZ uses `Start`/`Progress`/`Error`/`End` and PDF uses
// `Create*`/`Merge*`, so the tags are already disjoint and need no namespace;
// each name equals the tauri_specta variant tag of `ExportCbzEvent`/`ExportPdfEvent`.
pub const EVENT_CBZ_START: &str = "Start";
pub const EVENT_CBZ_PROGRESS: &str = "Progress";
pub const EVENT_CBZ_ERROR: &str = "Error";
pub const EVENT_CBZ_END: &str = "End";
pub const EVENT_PDF_CREATE_START: &str = "CreateStart";
pub const EVENT_PDF_CREATE_PROGRESS: &str = "CreateProgress";
pub const EVENT_PDF_CREATE_ERROR: &str = "CreateError";
pub const EVENT_PDF_CREATE_END: &str = "CreateEnd";
pub const EVENT_PDF_MERGE_START: &str = "MergeStart";
pub const EVENT_PDF_MERGE_PROGRESS: &str = "MergeProgress";
pub const EVENT_PDF_MERGE_ERROR: &str = "MergeError";
pub const EVENT_PDF_MERGE_END: &str = "MergeEnd";

/// Payload of the CBZ `Start` / PDF `CreateStart` / `MergeStart` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStartPayload {
    pub uuid: String,
    pub comic_title: String,
    pub total: u32,
}

/// Payload of the CBZ `Progress` / PDF `CreateProgress` / `MergeProgress` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgressPayload {
    pub uuid: String,
    pub current: u32,
}

/// Payload of the CBZ `Error` / PDF `CreateError` / `MergeError` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportErrorPayload {
    pub uuid: String,
}

/// Payload of the CBZ `End` / PDF `CreateEnd` / `MergeEnd` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEndPayload {
    pub uuid: String,
    pub comic_id: i64,
    pub chapter_export_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn export_event_names_match_wire_tags() {
        assert_eq!(EVENT_CBZ_START, "Start");
        assert_eq!(EVENT_CBZ_PROGRESS, "Progress");
        assert_eq!(EVENT_CBZ_ERROR, "Error");
        assert_eq!(EVENT_CBZ_END, "End");
        assert_eq!(EVENT_PDF_CREATE_START, "CreateStart");
        assert_eq!(EVENT_PDF_CREATE_PROGRESS, "CreateProgress");
        assert_eq!(EVENT_PDF_CREATE_ERROR, "CreateError");
        assert_eq!(EVENT_PDF_CREATE_END, "CreateEnd");
        assert_eq!(EVENT_PDF_MERGE_START, "MergeStart");
        assert_eq!(EVENT_PDF_MERGE_PROGRESS, "MergeProgress");
        assert_eq!(EVENT_PDF_MERGE_ERROR, "MergeError");
        assert_eq!(EVENT_PDF_MERGE_END, "MergeEnd");
    }

    #[test]
    fn export_event_payloads_serialize_camel_case() {
        let start = serde_json::to_value(ExportStartPayload {
            uuid: "u".to_string(),
            comic_title: "t".to_string(),
            total: 3,
        })
        .unwrap();
        assert_eq!(
            start,
            json!({ "uuid": "u", "comicTitle": "t", "total": 3 })
        );

        let progress = serde_json::to_value(ExportProgressPayload {
            uuid: "u".to_string(),
            current: 2,
        })
        .unwrap();
        assert_eq!(progress, json!({ "uuid": "u", "current": 2 }));

        let error = serde_json::to_value(ExportErrorPayload {
            uuid: "u".to_string(),
        })
        .unwrap();
        assert_eq!(error, json!({ "uuid": "u" }));

        let end = serde_json::to_value(ExportEndPayload {
            uuid: "u".to_string(),
            comic_id: 1,
            chapter_export_dir: PathBuf::from("/x/y"),
        })
        .unwrap();
        assert_eq!(
            end,
            json!({ "uuid": "u", "comicId": 1, "chapterExportDir": "/x/y" })
        );
    }
}
