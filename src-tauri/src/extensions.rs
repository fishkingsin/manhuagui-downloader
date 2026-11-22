use std::sync::Arc;

use eyre::eyre;
use parking_lot::RwLock;
use scraper::error::SelectorErrorKind;
use source_manhuagui::ManhuaguiClient;
use tauri::{Manager, State};

use library::downloader::download_manager::DownloadManager;

use crate::{config::Config, export::ComicExportLock};

pub trait EyreReportToMessage {
    fn to_message(&self) -> String;
}

impl EyreReportToMessage for eyre::Report {
    fn to_message(&self) -> String {
        format!("{self:?}")
    }
}

#[allow(dead_code)]
pub trait ToEyre<T> {
    fn to_eyre(self) -> eyre::Result<T>;
}

impl<T> ToEyre<T> for Result<T, SelectorErrorKind<'_>> {
    fn to_eyre(self) -> eyre::Result<T> {
        self.map_err(|e| eyre!(e.to_string()))
    }
}

pub trait AppHandleExt {
    fn get_config(&self) -> State<'_, RwLock<Config>>;
    fn get_core_config(&self) -> source_api::CoreConfig;
    fn get_manhuagui_client(&self) -> State<'_, Arc<ManhuaguiClient>>;
    fn get_download_manager(&self) -> State<'_, DownloadManager>;
    fn get_export_lock(&self) -> State<'_, ComicExportLock>;
}

impl AppHandleExt for tauri::AppHandle {
    fn get_config(&self) -> State<'_, RwLock<Config>> {
        self.state::<RwLock<Config>>()
    }
    fn get_core_config(&self) -> source_api::CoreConfig {
        let config = self.get_config().read().clone();
        (&config).into()
    }
    fn get_manhuagui_client(&self) -> State<'_, Arc<ManhuaguiClient>> {
        self.state::<Arc<ManhuaguiClient>>()
    }
    fn get_download_manager(&self) -> State<'_, DownloadManager> {
        self.state::<DownloadManager>()
    }
    fn get_export_lock(&self) -> State<'_, ComicExportLock> {
        self.state::<ComicExportLock>()
    }
}

pub trait WalkDirEntryExt {
    fn is_comic_metadata(&self) -> bool;
    #[allow(dead_code)]
    fn is_chapter_metadata(&self) -> bool;
}
impl WalkDirEntryExt for walkdir::DirEntry {
    fn is_comic_metadata(&self) -> bool {
        if !self.file_type().is_file() {
            return false;
        }
        if self.file_name() != "元数据.json" {
            return false;
        }

        true
    }

    fn is_chapter_metadata(&self) -> bool {
        if !self.file_type().is_file() {
            return false;
        }
        if self.file_name() != "章节元数据.json" {
            return false;
        }

        true
    }
}

pub trait PathIsImg {
    /// 判断路径是否为图片文件
    fn is_img(&self) -> bool;
}

impl PathIsImg for std::path::Path {
    fn is_img(&self) -> bool {
        self.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_lowercase)
            .is_some_and(|ext| matches!(ext.as_str(), "jpg"))
    }
}
