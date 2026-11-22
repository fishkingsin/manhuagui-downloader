use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use eyre::{eyre, OptionExt, WrapErr};
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use walkdir::WalkDir;

use crate::{config::CoreConfig, utils::filename_filter};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct ComicId {
    pub source_id: String,
    pub native_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub enum UpdateWindow {
    Days7,
    Days15,
    Days30,
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct ComicSummary {
    pub comic_id: ComicId,
    pub title: String,
    pub cover_url: String,
    pub status_line: String,
    pub updated_on: Option<chrono::NaiveDate>,
    pub rating: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct BrowseResult {
    pub items: Vec<ComicSummary>,
    pub page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct PageRef {
    pub url: String,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct ChapterPages {
    pub pages: Vec<PageRef>,
    pub prev_chapter_id: Option<i64>,
    pub next_chapter_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
pub enum DownloadTaskState {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
pub struct ChapterDirFmtParams {
    pub comic_id: i64,
    pub comic_title: String,
    pub comic_subtitle: String,
    pub pub_year: i64,
    pub region: String,
    pub author: String,
    pub group_name: String,
    pub chapter_id: i64,
    pub chapter_title: String,
    pub order: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    /// 章节id
    pub chapter_id: i64,
    /// 章节标题
    pub chapter_title: String,
    /// 此章节有多少页
    pub chapter_size: i64,
    /// 漫画id
    pub comic_id: i64,
    /// 漫画标题
    pub comic_title: String,
    /// 组名(单话、单行本、番外篇)
    pub group_name: String,
    /// 此章节对应的group有多少章节
    pub group_size: i64,
    /// 此章节在group中的顺序
    pub order: f64,
    /// 漫画状态(连载中/已完结)
    pub comic_status: String,
    /// 是否曾导出过PDF
    #[serde(default)]
    pub is_pdf_exported: bool,
    /// 是否曾导出过CBZ
    #[serde(default)]
    pub is_cbz_exported: bool,
    /// 是否已下载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_download_dir: Option<PathBuf>,
}

impl ChapterInfo {
    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_id = self.comic_id,
            comic_title = self.comic_title,
            group_name = self.group_name,
            chapter_id = self.chapter_id,
            order = self.order
        )
    )]
    pub fn save_metadata(&self) -> eyre::Result<()> {
        let mut chapter_info = self.clone();
        // 将is_downloaded和chapter_download_dir字段设置为None
        // 这样能使这些字段在序列化时被忽略
        chapter_info.is_downloaded = None;
        chapter_info.chapter_download_dir = None;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;
        let metadata_path = chapter_download_dir.join("章节元数据.json");

        std::fs::create_dir_all(chapter_download_dir)
            .wrap_err(format!("创建目录`{}`失败", chapter_download_dir.display()))?;

        let chapter_json = serde_json::to_string_pretty(&chapter_info)
            .wrap_err("将ChapterInfo序列化为json失败")?;

        std::fs::write(&metadata_path, chapter_json)
            .wrap_err(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_id = self.comic_id,
            comic_title = self.comic_title,
            group_name = self.group_name,
            chapter_id = self.chapter_id,
            order = self.order
        )
    )]
    pub fn get_temp_download_dir(&self) -> eyre::Result<PathBuf> {
        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        let chapter_download_dir_name = chapter_download_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_eyre(format!(
                "获取`{}`的目录名失败",
                chapter_download_dir.display()
            ))?;

        let parent = chapter_download_dir.parent().ok_or_eyre(format!(
            "`{}`的父目录不存在",
            chapter_download_dir.display()
        ))?;

        let temp_download_dir = parent.join(format!(".下载中-{chapter_download_dir_name}"));
        Ok(temp_download_dir)
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_id = comic.id,
            comic_title = comic.title,
            group_name = self.group_name,
            chapter_id = self.chapter_id,
            order = self.order
        )
    )]
    pub fn get_chapter_relative_dir(&self, comic: &Comic) -> eyre::Result<PathBuf> {
        let comic_download_dir = comic
            .comic_download_dir
            .as_ref()
            .ok_or_eyre("`comic_download_dir`字段为`None`")?;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        let relative_dir = chapter_download_dir
            .strip_prefix(comic_download_dir)
            .wrap_err(format!(
                "无法从路径`{}`中移除前缀`{}`",
                chapter_download_dir.display(),
                comic_download_dir.display()
            ))?;

        Ok(relative_dir.to_path_buf())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_id = fmt_params.comic_id,
            comic_title = fmt_params.comic_title,
            author = fmt_params.author,
            group_name = fmt_params.group_name,
            chapter_id = fmt_params.chapter_id,
            chapter_title = fmt_params.chapter_title,
            order = fmt_params.order
        )
    )]
    pub fn get_chapter_download_dir_by_fmt(
        config: &CoreConfig,
        comic_download_dir: &Path,
        fmt_params: &ChapterDirFmtParams,
    ) -> eyre::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .wrap_err("将ChapterDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .ok_or_eyre("ChapterDirFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();

        let mut chapter_dir_fmt = config.chapter_dir_fmt.clone();
        Self::preprocess_order_placeholder(&mut chapter_dir_fmt, &vars)?;

        let dir_fmt_parts: Vec<&str> = chapter_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).wrap_err("格式化目录名失败")?;
            let dir_name = filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }

        let mut chapter_download_dir = comic_download_dir.to_path_buf();
        for dir_name in dir_names {
            chapter_download_dir = chapter_download_dir.join(dir_name);
        }

        Ok(chapter_download_dir)
    }

    #[instrument(level = "error", skip_all, fields(fmt = fmt))]
    pub fn preprocess_order_placeholder(
        fmt: &mut String,
        vars: &HashMap<String, String>,
    ) -> eyre::Result<()> {
        use strfmt::strfmt;

        let Some(order_str) = vars.get("order") else {
            return Ok(());
        };

        let (int_part, frac_part) = match order_str.split_once('.') {
            Some((i, f)) => (i, f),
            None => (order_str.as_str(), ""),
        };
        let should_append_frac = !frac_part.is_empty() && frac_part != "0";

        let re = Regex::new(r"(\{\{)|(\}\})|(\{order(?::(.*?))?\})")?;

        let new_fmt = re.replace_all(fmt, |caps: &Captures| {
            if caps.get(1).is_some() {
                return "{{".to_string();
            }
            if caps.get(2).is_some() {
                return "}}".to_string();
            }

            let fmt_spec = caps.get(4).map_or("", |m| m.as_str());
            let int_fmt = format!("{{v:{fmt_spec}}}");
            let mut temp_vars = HashMap::new();
            temp_vars.insert("v".to_string(), int_part.to_string());

            let formatted_int = strfmt(&int_fmt, &temp_vars).unwrap_or(int_part.to_string());

            if should_append_frac {
                format!("{formatted_int}.{frac_part}")
            } else {
                formatted_int
            }
        });

        *fmt = new_fmt.to_string();

        Ok(())
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
pub struct ComicDirFmtParams {
    pub comic_id: i64,
    pub comic_title: String,
    pub comic_subtitle: String,
    pub pub_year: i64,
    pub region: String,
    pub author: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct Comic {
    /// 漫画id
    pub id: i64,
    /// 漫画标题
    pub title: String,
    /// 漫画副标题
    pub subtitle: Option<String>,
    /// 封面链接
    pub cover: String,
    /// 漫画状态(连载中/已完结)
    pub status: String,
    /// 上次更新时间
    pub update_time: String,
    /// 出版年份
    pub year: i64,
    /// 地区
    pub region: String,
    /// 类型
    pub genres: Vec<String>,
    /// 作者
    pub authors: Vec<String>,
    /// 漫画别名
    pub aliases: Vec<String>,
    /// 简介
    pub intro: String,
    /// 组名(单话、单行本...)->章节信息
    pub groups: HashMap<String, Vec<ChapterInfo>>,
    /// 是否已下载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comic_download_dir: Option<PathBuf>,
}

impl Comic {
    #[instrument(level = "error", skip_all, fields(metadata_path = %metadata_path.display()))]
    pub fn from_metadata(metadata_path: &Path) -> eyre::Result<Comic> {
        let comic_json = std::fs::read_to_string(metadata_path)?;
        let mut comic = serde_json::from_str::<Comic>(&comic_json)
            .wrap_err("将元数据文件反序列化为Comic失败")?;
        let parent = metadata_path
            .parent()
            .ok_or_eyre(format!("`{}`没有父目录", metadata_path.display()))?;
        let comic_download_dir = parent.to_path_buf();

        // 为旧版本创建章节元数据
        comic
            .create_chapter_metadata_for_old_version(&comic_download_dir)
            .wrap_err("为旧版本创建章节元数据失败")?;

        comic.comic_download_dir = Some(comic_download_dir);
        comic.is_downloaded = Some(true);

        // 来自元数据的章节信息没有`chapter_download_dir`和`is_downloaded`字段，需要更新
        comic.update_chapter_infos_fields()?;

        Ok(comic)
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = self.id, comic_title = self.title)
    )]
    pub fn update_fields(&mut self, id_to_dir_map: &HashMap<i64, PathBuf>) -> eyre::Result<()> {
        if let Some(comic_download_dir) = id_to_dir_map.get(&self.id) {
            self.comic_download_dir = Some(comic_download_dir.clone());
            self.is_downloaded = Some(true);

            self.update_chapter_infos_fields()?;
        }

        Ok(())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = self.id, comic_title = self.title)
    )]
    pub fn update_chapter_infos_fields(&mut self) -> eyre::Result<()> {
        let Some(comic_download_dir) = &self.comic_download_dir else {
            return Err(eyre!("`comic_download_dir`字段为`None`"));
        };

        if !comic_download_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(comic_download_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() || entry.file_name() != "章节元数据.json" {
                continue;
            }

            let metadata_path = entry.path();

            let metadata_str = std::fs::read_to_string(metadata_path)
                .wrap_err(format!("读取`{}`失败", metadata_path.display()))?;

            let chapter_json: serde_json::Value =
                serde_json::from_str(&metadata_str).wrap_err(format!(
                    "将`{}`反序列化为serde_json::Value失败",
                    metadata_path.display()
                ))?;

            let chapter_id = chapter_json
                .get("chapterId")
                .and_then(serde_json::Value::as_i64)
                .ok_or_eyre(format!("`{}`没有`chapterId`字段", metadata_path.display()))?;

            let group_name = chapter_json
                .get("groupName")
                .and_then(|word| word.as_str())
                .ok_or_eyre(format!("`{}`没有`groupName`字段", metadata_path.display()))?
                .to_string();

            let Some(group) = self.groups.get_mut(&group_name) else {
                continue;
            };

            if let Some(chapter_info) = group
                .iter_mut()
                .find(|chapter| chapter.chapter_id == chapter_id)
            {
                let parent = metadata_path
                    .parent()
                    .ok_or_eyre(format!("`{}`没有父目录", metadata_path.display()))?;
                chapter_info.chapter_download_dir = Some(parent.to_path_buf());
                chapter_info.is_downloaded = Some(true);
                chapter_info.is_pdf_exported = chapter_json
                    .get("isPdfExported")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                chapter_info.is_cbz_exported = chapter_json
                    .get("isCbzExported")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
            }
        }

        Ok(())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = self.id, comic_title = self.title)
    )]
    pub fn save_metadata(&self) -> eyre::Result<()> {
        let mut comic = self.clone();
        // 将所有的is_downloaded字段设置为None，这样能使is_downloaded字段在序列化时被忽略
        comic.is_downloaded = None;
        for chapter_infos in comic.groups.values_mut() {
            for chapter_info in chapter_infos.iter_mut() {
                chapter_info.is_downloaded = None;
            }
        }

        let comic_download_dir = self
            .comic_download_dir
            .as_ref()
            .ok_or_eyre("`comic_download_dir`字段为`None`")?;
        let metadata_path = comic_download_dir.join("元数据.json");

        std::fs::create_dir_all(comic_download_dir)
            .wrap_err(format!("创建目录`{}`失败", comic_download_dir.display()))?;

        let comic_json =
            serde_json::to_string_pretty(&comic).wrap_err("将Comic序列化为json失败")?;

        std::fs::write(&metadata_path, comic_json)
            .wrap_err(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = self.id, comic_title = self.title)
    )]
    pub fn get_comic_export_dir(&self, config: &CoreConfig) -> eyre::Result<PathBuf> {
        let download_dir = config.download_dir.clone();
        let export_dir = config.export_dir.clone();

        let Some(comic_download_dir) = self.comic_download_dir.clone() else {
            return Err(eyre!("`comic_download_dir`字段为`None`"));
        };

        let relative_dir = comic_download_dir
            .strip_prefix(&download_dir)
            .wrap_err(format!(
                "无法从路径`{}`中移除前缀`{}`",
                comic_download_dir.display(),
                download_dir.display()
            ))?;

        let comic_export_dir = export_dir.join(relative_dir);
        Ok(comic_export_dir)
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = self.id, comic_title = self.title)
    )]
    pub fn ensure_download_dir_fields(&mut self, config: &CoreConfig) -> eyre::Result<()> {
        if self.has_download_dir_fields() {
            return Ok(());
        }

        self.update_download_dir_fields_by_fmt(config)
    }

    pub fn has_download_dir_fields(&self) -> bool {
        let comic_download_dir_ready = self.comic_download_dir.is_some();
        let chapter_download_dir_ready = self
            .groups
            .values()
            .flatten()
            .all(|chapter| chapter.chapter_download_dir.is_some());

        comic_download_dir_ready && chapter_download_dir_ready
    }

    /// 根据fmt更新`comic_download_dir`和`chapter_infos.chapter_download_dir`字段
    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = self.id, comic_title = self.title)
    )]
    pub fn update_download_dir_fields_by_fmt(&mut self, config: &CoreConfig) -> eyre::Result<()> {
        let comic_id = self.id;
        let comic_title = self.title.clone();
        let comic_subtitle = self.subtitle.clone().unwrap_or_default();
        let author = self.authors.join(", ");

        let comic_dir_fmt_params = ComicDirFmtParams {
            comic_id,
            comic_title: comic_title.clone(),
            comic_subtitle: comic_subtitle.clone(),
            pub_year: self.year,
            region: self.region.clone(),
            author: author.clone(),
        };
        let comic_download_dir =
            Comic::get_comic_download_dir_by_fmt(config, &comic_dir_fmt_params)?;
        self.comic_download_dir = Some(comic_download_dir.clone());

        for chapter_info in &mut self.groups.values_mut().flatten() {
            let chapter_dir_fmt_params = ChapterDirFmtParams {
                comic_id,
                comic_title: comic_title.clone(),
                comic_subtitle: comic_subtitle.clone(),
                pub_year: self.year,
                region: self.region.clone(),
                author: author.clone(),
                group_name: chapter_info.group_name.clone(),
                chapter_id: chapter_info.chapter_id,
                chapter_title: chapter_info.chapter_title.clone(),
                order: chapter_info.order,
            };
            let chapter_download_dir = ChapterInfo::get_chapter_download_dir_by_fmt(
                config,
                &comic_download_dir,
                &chapter_dir_fmt_params,
            )?;
            chapter_info.chapter_download_dir = Some(chapter_download_dir);
        }

        Ok(())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_id = fmt_params.comic_id,
            comic_title = fmt_params.comic_title,
            author = fmt_params.author
        )
    )]
    pub fn get_comic_download_dir_by_fmt(
        config: &CoreConfig,
        fmt_params: &ComicDirFmtParams,
    ) -> eyre::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .wrap_err("将ComicDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .ok_or_eyre("ComicDirFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();

        let download_dir = config.download_dir.clone();
        let comic_dir_fmt = config.comic_dir_fmt.clone();

        let dir_fmt_parts: Vec<&str> = comic_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).wrap_err("格式化目录名失败")?;
            let dir_name = filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }

        let mut comic_download_dir = download_dir;
        for dir_name in dir_names {
            comic_download_dir = comic_download_dir.join(dir_name);
        }

        Ok(comic_download_dir)
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = self.id, comic_title = self.title)
    )]
    pub fn create_chapter_metadata_for_old_version(
        &self,
        comic_download_dir: &Path,
    ) -> eyre::Result<()> {
        let mut chapter_dirs = HashSet::new();
        let Ok(read_dir) = std::fs::read_dir(comic_download_dir) else {
            return Ok(());
        };
        for group_entry in read_dir.filter_map(Result::ok) {
            let Ok(file_type) = group_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }

            let Ok(inner_read_dir) = std::fs::read_dir(group_entry.path()) else {
                continue;
            };
            for chapter_entry in inner_read_dir.filter_map(Result::ok) {
                let Ok(file_type) = chapter_entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                chapter_dirs.insert(chapter_entry.path());
            }
        }

        for chapter_info in self.groups.values().flatten() {
            let group_title = filename_filter(&chapter_info.group_name);
            let chapter_title = filename_filter(&chapter_info.chapter_title);
            let order = chapter_info.order;
            let prefixed_chapter_title = format!("{order} {chapter_title}");

            let old_chapter_dir = comic_download_dir
                .join(&group_title)
                .join(&prefixed_chapter_title);

            let old_chapter_dir_exists = chapter_dirs.contains(&old_chapter_dir);
            let old_chapter_metadata_exists = old_chapter_dir.join("章节元数据.json").exists();

            if old_chapter_dir_exists && !old_chapter_metadata_exists {
                // 如果旧版本的章节目录存在，但没有元数据文件，就创建一个
                let mut info = chapter_info.clone();
                info.chapter_download_dir = Some(old_chapter_dir);
                info.is_downloaded = Some(true);
                info.save_metadata()?;
            }
        }

        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct ComicInSearch {
    /// 漫画id
    pub id: i64,
    /// 漫画标题
    pub title: String,
    /// 漫画副标题
    pub subtitle: Option<String>,
    /// 封面链接
    pub cover: String,
    /// 漫画状态(连载中/已完结)
    pub status: String,
    /// 上次更新时间
    pub update_time: String,
    /// 出版年份
    pub year: i64,
    /// 地区
    pub region: String,
    /// 类型
    pub genres: Vec<String>,
    /// 作者
    pub authors: Vec<String>,
    /// 漫画别名
    pub aliases: Vec<String>,
    /// 简介
    pub intro: String,
    /// 是否已下载
    pub is_downloaded: bool,
    /// 漫画的下载目录
    pub comic_download_dir: PathBuf,
}

pub type SearchResultComic = ComicInSearch;

impl ComicInSearch {
    pub fn update_fields(&mut self, id_to_dir_map: &HashMap<i64, PathBuf>) {
        if let Some(comic_download_dir) = id_to_dir_map.get(&self.id) {
            self.comic_download_dir = comic_download_dir.clone();
            self.is_downloaded = true;
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(export = false))]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub comics: Vec<ComicInSearch>,
    pub current: i64,
    pub total: i64,
}
