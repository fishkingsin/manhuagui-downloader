use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreConfig {
    pub cookie: String,
    pub download_dir: PathBuf,
    pub export_dir: PathBuf,
    pub chapter_concurrency: usize,
    pub chapter_download_interval_sec: u64,
    pub img_concurrency: usize,
    pub img_download_interval_sec: u64,
    pub update_get_comic_interval_sec: u64,
    pub api_timeout_sec: u64,
    pub img_timeout_sec: u64,
    pub proxy_mode: ProxyMode,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub comic_dir_fmt: String,
    pub chapter_dir_fmt: String,
    pub create_pdf_concurrency: usize,
    pub enable_merge_pdf: bool,
    /// 导出跳过模式
    pub export_skip_mode: ExportSkipMode,
}

impl Default for CoreConfig {
    fn default() -> Self {
        let cpu_core_num = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1);

        CoreConfig {
            cookie: String::new(),
            download_dir: PathBuf::new(),
            export_dir: PathBuf::new(),
            chapter_concurrency: 1,
            chapter_download_interval_sec: 10,
            img_concurrency: 10,
            img_download_interval_sec: 0,
            update_get_comic_interval_sec: 0,
            api_timeout_sec: 3,
            img_timeout_sec: 3,
            proxy_mode: ProxyMode::System,
            proxy_host: "127.0.0.1".to_string(),
            proxy_port: 7890,
            comic_dir_fmt: "{comic_title}".to_string(),
            chapter_dir_fmt: "{group_name}/{order} {chapter_title}".to_string(),
            create_pdf_concurrency: cpu_core_num,
            enable_merge_pdf: true,
            export_skip_mode: ExportSkipMode::default(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProxyMode {
    #[default]
    System,
    NoProxy,
    Custom,
}

/// 导出跳过模式
#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExportSkipMode {
    /// 每次重新导出所有章节
    #[default]
    None,
    /// 跳过本地已存在的导出文件
    SkipExisting,
    /// 跳过曾导出过的章节(即使本地文件已删除)
    SkipExported,
}
