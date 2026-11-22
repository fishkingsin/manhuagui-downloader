use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;

use library::events::{
    SleepingPayload, SpeedPayload, TaskCreatePayload, TaskDeletePayload, TaskUpdatePayload,
    EVENT_SLEEPING, EVENT_SPEED, EVENT_TASK_CREATE, EVENT_TASK_DELETE, EVENT_TASK_UPDATE,
};
use source_api::{ChapterInfo, Comic, DownloadTaskState, EventObserver, LibraryEvent};

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Speed { speed: String },

    #[serde(rename_all = "camelCase")]
    Sleeping { chapter_id: i64, remaining_sec: u64 },

    #[serde(rename_all = "camelCase")]
    TaskCreate {
        state: DownloadTaskState,
        comic: Box<Comic>,
        chapter_info: Box<ChapterInfo>,
        downloaded_img_count: u32,
        total_img_count: u32,
    },

    #[serde(rename_all = "camelCase")]
    TaskDelete { chapter_id: i64 },

    #[serde(rename_all = "camelCase")]
    TaskUpdate {
        state: DownloadTaskState,
        chapter_id: i64,
        downloaded_img_count: u32,
        total_img_count: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum ExportCbzEvent {
    #[serde(rename_all = "camelCase")]
    Start {
        uuid: String,
        comic_title: String,
        total: u32,
    },

    #[serde(rename_all = "camelCase")]
    Progress { uuid: String, current: u32 },

    #[serde(rename_all = "camelCase")]
    Error { uuid: String },

    #[serde(rename_all = "camelCase")]
    End {
        uuid: String,
        comic_id: i64,
        chapter_export_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum ExportPdfEvent {
    #[serde(rename_all = "camelCase")]
    CreateStart {
        uuid: String,
        comic_title: String,
        total: u32,
    },

    #[serde(rename_all = "camelCase")]
    CreateProgress { uuid: String, current: u32 },

    #[serde(rename_all = "camelCase")]
    CreateError { uuid: String },

    #[serde(rename_all = "camelCase")]
    CreateEnd {
        uuid: String,
        comic_id: i64,
        chapter_export_dir: PathBuf,
    },

    #[serde(rename_all = "camelCase")]
    MergeStart {
        uuid: String,
        comic_title: String,
        total: u32,
    },

    #[serde(rename_all = "camelCase")]
    MergeProgress { uuid: String, current: u32 },

    #[serde(rename_all = "camelCase")]
    MergeError { uuid: String },

    #[serde(rename_all = "camelCase")]
    MergeEnd {
        uuid: String,
        comic_id: i64,
        chapter_export_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum UpdateDownloadedComicsEvent {
    #[serde(rename_all = "camelCase")]
    GetComicStart { total: i64 },

    #[serde(rename_all = "camelCase")]
    GetComicProgress { current: i64, total: i64 },

    #[serde(rename_all = "camelCase")]
    CreateDownloadTasksStart {
        comic_id: i64,
        comic_title: String,
        current: i64,
        total: i64,
    },

    #[serde(rename_all = "camelCase")]
    CreateDownloadTaskProgress { comic_id: i64, current: i64 },

    #[serde(rename_all = "camelCase")]
    CreateDownloadTasksEnd { comic_id: i64 },

    #[serde(rename_all = "camelCase")]
    GetComicEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub json_raw: String,
}

/// Bridges library [`LibraryEvent`]s to the desktop tauri_specta events the
/// Vue frontend listens on. The event name is the `DownloadEvent` variant tag.
pub struct CoreObserver(AppHandle);

impl CoreObserver {
    pub fn new(app: AppHandle) -> Self {
        Self(app)
    }
}

impl EventObserver for CoreObserver {
    fn on_event(&self, event: &LibraryEvent) {
        let app = &self.0;
        match event.name {
            EVENT_SPEED => {
                if let Ok(payload) = serde_json::from_value::<SpeedPayload>(event.payload.clone()) {
                    let _ = DownloadEvent::Speed { speed: payload.speed }.emit(app);
                }
            }
            EVENT_SLEEPING => {
                if let Ok(payload) =
                    serde_json::from_value::<SleepingPayload>(event.payload.clone())
                {
                    let _ = DownloadEvent::Sleeping {
                        chapter_id: payload.chapter_id,
                        remaining_sec: payload.remaining_sec,
                    }
                    .emit(app);
                }
            }
            EVENT_TASK_CREATE => {
                if let Ok(payload) =
                    serde_json::from_value::<TaskCreatePayload>(event.payload.clone())
                {
                    let _ = DownloadEvent::TaskCreate {
                        state: payload.state,
                        comic: Box::new(payload.comic),
                        chapter_info: Box::new(payload.chapter_info),
                        downloaded_img_count: payload.downloaded_img_count,
                        total_img_count: payload.total_img_count,
                    }
                    .emit(app);
                }
            }
            EVENT_TASK_DELETE => {
                if let Ok(payload) =
                    serde_json::from_value::<TaskDeletePayload>(event.payload.clone())
                {
                    let _ = DownloadEvent::TaskDelete {
                        chapter_id: payload.chapter_id,
                    }
                    .emit(app);
                }
            }
            EVENT_TASK_UPDATE => {
                if let Ok(payload) =
                    serde_json::from_value::<TaskUpdatePayload>(event.payload.clone())
                {
                    let _ = DownloadEvent::TaskUpdate {
                        state: payload.state,
                        chapter_id: payload.chapter_id,
                        downloaded_img_count: payload.downloaded_img_count,
                        total_img_count: payload.total_img_count,
                    }
                    .emit(app);
                }
            }
            _ => {}
        }
    }
}
