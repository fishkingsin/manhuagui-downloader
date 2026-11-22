use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub cookie: String,
    pub download_dir: PathBuf,
    pub export_dir: PathBuf,
    pub enable_file_logger: bool,
    pub chapter_concurrency: usize,
    pub chapter_download_interval_sec: u64,
    pub img_concurrency: usize,
    pub img_download_interval_sec: u64,
    pub update_get_comic_interval_sec: u64,
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

impl Config {
    #[instrument(level = "error", skip_all)]
    pub fn new(app: &AppHandle) -> eyre::Result<Config> {
        let app_data_dir = app.path().app_data_dir()?;
        let config_path = app_data_dir.join("config.json");

        let config = if config_path.exists() {
            let config_string = std::fs::read_to_string(config_path)?;
            match serde_json::from_str(&config_string) {
                // 如果能够直接解析为Config，则直接返回
                Ok(config) => config,
                // 否则，将默认配置与文件中已有的配置合并
                // 以免新版本添加了新的配置项，用户升级到新版本后，所有配置项都被重置
                Err(_) => Config::merge_config(&config_string, &app_data_dir),
            }
        } else {
            Config::default(&app_data_dir)
        };
        config.save(app)?;
        Ok(config)
    }

    #[instrument(level = "error", skip_all)]
    pub fn save(&self, app: &AppHandle) -> eyre::Result<()> {
        let app_data_dir = app.path().app_data_dir()?;
        let config_path = app_data_dir.join("config.json");
        let config_string = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, config_string)?;
        Ok(())
    }

    fn merge_config(config_string: &str, app_data_dir: &Path) -> Config {
        let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(config_string) else {
            return Config::default(app_data_dir);
        };
        let serde_json::Value::Object(ref mut map) = json_value else {
            return Config::default(app_data_dir);
        };
        let Ok(default_config_value) = serde_json::to_value(Config::default(app_data_dir)) else {
            return Config::default(app_data_dir);
        };
        let serde_json::Value::Object(default_map) = default_config_value else {
            return Config::default(app_data_dir);
        };
        for (key, value) in default_map {
            map.entry(key).or_insert(value);
        }
        let Ok(config) = serde_json::from_value(json_value) else {
            return Config::default(app_data_dir);
        };
        config
    }

    fn default(app_data_dir: &Path) -> Config {
        let cpu_core_num = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1);

        Config {
            cookie: String::new(),
            download_dir: app_data_dir.join("漫画下载"),
            export_dir: app_data_dir.join("漫画导出"),
            enable_file_logger: true,
            chapter_concurrency: 1,
            chapter_download_interval_sec: 10,
            img_concurrency: 10,
            img_download_interval_sec: 0,
            update_get_comic_interval_sec: 0,
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub enum ProxyMode {
    #[default]
    System,
    NoProxy,
    Custom,
}

impl From<ProxyMode> for source_api::ProxyMode {
    fn from(m: ProxyMode) -> Self {
        match m {
            ProxyMode::System => source_api::ProxyMode::System,
            ProxyMode::NoProxy => source_api::ProxyMode::NoProxy,
            ProxyMode::Custom => source_api::ProxyMode::Custom,
        }
    }
}

/// 导出跳过模式
#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub enum ExportSkipMode {
    /// 每次重新导出所有章节
    #[default]
    None,
    /// 跳过本地已存在的导出文件
    SkipExisting,
    /// 跳过曾导出过的章节(即使本地文件已删除)
    SkipExported,
}

impl From<ExportSkipMode> for source_api::ExportSkipMode {
    fn from(m: ExportSkipMode) -> Self {
        match m {
            ExportSkipMode::None => source_api::ExportSkipMode::None,
            ExportSkipMode::SkipExisting => source_api::ExportSkipMode::SkipExisting,
            ExportSkipMode::SkipExported => source_api::ExportSkipMode::SkipExported,
        }
    }
}

impl From<&Config> for source_api::CoreConfig {
    fn from(c: &Config) -> Self {
        source_api::CoreConfig {
            cookie: c.cookie.clone(),
            download_dir: c.download_dir.clone(),
            export_dir: c.export_dir.clone(),
            chapter_concurrency: c.chapter_concurrency,
            chapter_download_interval_sec: c.chapter_download_interval_sec,
            img_concurrency: c.img_concurrency,
            img_download_interval_sec: c.img_download_interval_sec,
            update_get_comic_interval_sec: c.update_get_comic_interval_sec,
            api_timeout_sec: 3,
            img_timeout_sec: 3,
            proxy_mode: c.proxy_mode.into(),
            proxy_host: c.proxy_host.clone(),
            proxy_port: c.proxy_port,
            comic_dir_fmt: c.comic_dir_fmt.clone(),
            chapter_dir_fmt: c.chapter_dir_fmt.clone(),
            create_pdf_concurrency: c.create_pdf_concurrency,
            enable_merge_pdf: c.enable_merge_pdf,
            export_skip_mode: c.export_skip_mode.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled_config() -> Config {
        Config {
            cookie: "session-cookie".to_string(),
            download_dir: PathBuf::from("/data/downloads"),
            export_dir: PathBuf::from("/data/exports"),
            enable_file_logger: false,
            chapter_concurrency: 7,
            chapter_download_interval_sec: 11,
            img_concurrency: 13,
            img_download_interval_sec: 17,
            update_get_comic_interval_sec: 19,
            proxy_mode: ProxyMode::Custom,
            proxy_host: "proxy.example.com".to_string(),
            proxy_port: 3128,
            comic_dir_fmt: "{comic_title} - {author}".to_string(),
            chapter_dir_fmt: "{order} {chapter_title}".to_string(),
            create_pdf_concurrency: 23,
            enable_merge_pdf: false,
            export_skip_mode: ExportSkipMode::SkipExisting,
        }
    }

    #[test]
    fn from_config_maps_every_field_to_core_config() {
        let config = filled_config();
        let core: source_api::CoreConfig = (&config).into();

        assert_eq!(core.cookie, config.cookie);
        assert_eq!(core.download_dir, config.download_dir);
        assert_eq!(core.export_dir, config.export_dir);
        assert_eq!(core.chapter_concurrency, config.chapter_concurrency);
        assert_eq!(
            core.chapter_download_interval_sec,
            config.chapter_download_interval_sec
        );
        assert_eq!(core.img_concurrency, config.img_concurrency);
        assert_eq!(
            core.img_download_interval_sec,
            config.img_download_interval_sec
        );
        assert_eq!(
            core.update_get_comic_interval_sec,
            config.update_get_comic_interval_sec
        );
        assert_eq!(core.proxy_host, config.proxy_host);
        assert_eq!(core.proxy_port, config.proxy_port);
        assert_eq!(core.comic_dir_fmt, config.comic_dir_fmt);
        assert_eq!(core.chapter_dir_fmt, config.chapter_dir_fmt);
        assert_eq!(core.create_pdf_concurrency, config.create_pdf_concurrency);
        assert_eq!(core.enable_merge_pdf, config.enable_merge_pdf);

        assert_eq!(core.proxy_mode, source_api::ProxyMode::Custom);
        assert_eq!(
            core.export_skip_mode,
            source_api::ExportSkipMode::SkipExisting
        );

        // These two fields have no counterpart on the shell `Config`; the From
        // impl pins them to the historical default of 3 seconds.
        assert_eq!(core.api_timeout_sec, 3);
        assert_eq!(core.img_timeout_sec, 3);
    }
}
