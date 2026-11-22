use std::{
    io::Write,
    sync::{atomic::AtomicU32, Arc},
};

use eyre::{eyre, OptionExt, WrapErr};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use yaserde::YaSerialize;
use zip::{write::SimpleFileOptions, ZipWriter};

use source_api::{ChapterInfo, Comic, CoreConfig, EventObserver, ExportSkipMode};

use super::{
    emit_event, get_downloaded_chapters, get_downloaded_chapters_by_ids, get_image_paths,
    ExportFormat,
};
use crate::events::{
    ExportEndPayload, ExportErrorPayload, ExportProgressPayload, ExportStartPayload, EVENT_CBZ_END,
    EVENT_CBZ_ERROR, EVENT_CBZ_PROGRESS, EVENT_CBZ_START,
};

struct CbzErrorEventGuard<'a> {
    uuid: String,
    observer: &'a dyn EventObserver,
    success: bool,
}

impl Drop for CbzErrorEventGuard<'_> {
    fn drop(&mut self) {
        if self.success {
            return;
        }

        let uuid = self.uuid.clone();
        emit_event(self.observer, EVENT_CBZ_ERROR, ExportErrorPayload { uuid });
    }
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic.id, comic_title = comic.title)
)]
pub fn cbz(config: &CoreConfig, observer: &dyn EventObserver, comic: &Comic) -> eyre::Result<()> {
    // 获取配置
    let skip_mode = config.export_skip_mode;
    // 获取已下载章节
    let downloaded_chapters = get_downloaded_chapters(&comic.groups);
    // 调用内部实现
    cbz_internal(config, observer, comic, downloaded_chapters, skip_mode)
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic.id, comic_title = comic.title)
)]
pub fn cbz_chapters(
    config: &CoreConfig,
    observer: &dyn EventObserver,
    comic: &Comic,
    chapter_ids: Vec<i64>,
) -> eyre::Result<()> {
    // 获取指定章节
    let downloaded_chapters = get_downloaded_chapters_by_ids(&comic.groups, &chapter_ids);

    // 调用内部实现(用户主动选择，所以不跳过)
    cbz_internal(
        config,
        observer,
        comic,
        downloaded_chapters,
        ExportSkipMode::None,
    )
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
#[instrument(level = "error", skip_all, fields(skip_mode = ?skip_mode))]
fn cbz_internal(
    config: &CoreConfig,
    observer: &dyn EventObserver,
    comic: &Comic,
    downloaded_chapters: Vec<ChapterInfo>,
    skip_mode: ExportSkipMode,
) -> eyre::Result<()> {
    // 用于生成格式化的xml
    let xml_cfg = yaserde::ser::Config {
        perform_indent: true,
        ..Default::default()
    };
    let event_uuid = uuid::Uuid::new_v4().to_string();

    // 发送开始导出cbz事件
    emit_event(
        observer,
        EVENT_CBZ_START,
        ExportStartPayload {
            uuid: event_uuid.clone(),
            comic_title: comic.title.clone(),
            total: downloaded_chapters.len() as u32,
        },
    );

    // 如果success为false，drop时发送Error事件
    let mut error_event_guard = CbzErrorEventGuard {
        uuid: event_uuid.clone(),
        observer,
        success: false,
    };

    // 用来记录导出进度
    let current = Arc::new(AtomicU32::new(0));

    let extension = ExportFormat::Cbz.extension();
    let comic_export_dir = comic
        .get_comic_export_dir(config)
        .wrap_err("获取导出目录失败")?;
    let cbz_export_dir = comic_export_dir.join(extension);

    // 并发处理
    let current_span = tracing::Span::current();
    let downloaded_chapters = downloaded_chapters.into_par_iter();
    downloaded_chapters.try_for_each(|mut chapter_info| -> eyre::Result<()> {
        let _enter = current_span.enter();
        let span = tracing::error_span!(
            "export_cbz_rayon",
            group_name = chapter_info.group_name,
            chapter_title = chapter_info.chapter_title,
            chapter_id = chapter_info.chapter_id,
            order = chapter_info.order
        );
        let _enter = span.enter();

        // 获取导出路径
        let chapter_download_dir = chapter_info
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
        let chapter_relative_dir = chapter_info
            .get_chapter_relative_dir(comic)
            .wrap_err("获取章节相对目录失败")?;
        let chapter_relative_dir_parent = chapter_relative_dir
            .parent()
            .ok_or_eyre(format!("`{}`没有父目录", chapter_relative_dir.display()))?;
        let chapter_export_dir = cbz_export_dir.join(chapter_relative_dir_parent);

        // 保证导出目录存在
        std::fs::create_dir_all(&chapter_export_dir)
            .wrap_err(format!("创建目录`{}`失败", chapter_export_dir.display()))?;

        let zip_path = chapter_export_dir.join(format!("{chapter_download_dir_name}.{extension}"));

        // 跳过逻辑
        let should_skip = match skip_mode {
            ExportSkipMode::SkipExported if chapter_info.is_cbz_exported => true,
            ExportSkipMode::SkipExisting if zip_path.exists() => true,
            _ => false,
        };

        if should_skip {
            // 更新进度
            let current = current.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            emit_event(
                observer,
                EVENT_CBZ_PROGRESS,
                ExportProgressPayload {
                    uuid: event_uuid.clone(),
                    current,
                },
            );
            return Ok(());
        }

        // 生成ComicInfo
        let comic_info = ComicInfo::from(comic, &chapter_info);
        // 序列化ComicInfo为xml
        let comic_info_xml = yaserde::ser::to_string_with_config(&comic_info, &xml_cfg)
            .map_err(|err_msg| eyre!("序列化`ComicInfo.xml`失败: {err_msg}"))?;

        // 创建cbz文件
        let zip_file = std::fs::File::create(&zip_path)
            .wrap_err(format!("创建文件`{}`失败", zip_path.display()))?;
        let mut zip_writer = ZipWriter::new(zip_file);

        // 把ComicInfo.xml写入cbz
        zip_writer
            .start_file("ComicInfo.xml", SimpleFileOptions::default())
            .wrap_err(format!("在`{}`创建`ComicInfo.xml`失败", zip_path.display()))?;
        zip_writer
            .write_all(comic_info_xml.as_bytes())
            .wrap_err("写入`ComicInfo.xml`失败")?;

        let image_paths = get_image_paths(chapter_download_dir).wrap_err(format!(
            "获取`{}`中的图片失败",
            chapter_download_dir.display()
        ))?;

        for image_path in image_paths {
            let filename = image_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_eyre(format!("获取`{}`的文件名失败", image_path.display()))?;
            // 将文件写入cbz
            zip_writer
                .start_file(filename, SimpleFileOptions::default())
                .wrap_err(format!("在`{}`创建`{filename:?}`失败", zip_path.display()))?;
            let mut file = std::fs::File::open(&image_path)
                .wrap_err(format!("打开`{}`失败", image_path.display()))?;
            std::io::copy(&mut file, &mut zip_writer).wrap_err(format!(
                "将`{}`写入`{}`失败",
                image_path.display(),
                zip_path.display()
            ))?;
        }

        zip_writer
            .finish()
            .wrap_err(format!("关闭`{}`失败", zip_path.display()))?;

        // 更新章节导出状态
        chapter_info.is_cbz_exported = true;
        chapter_info.save_metadata()?;

        // 更新导出cbz的进度
        let current = current.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        // 发送导出cbz进度事件
        emit_event(
            observer,
            EVENT_CBZ_PROGRESS,
            ExportProgressPayload {
                uuid: event_uuid.clone(),
                current,
            },
        );

        Ok(())
    })?;

    // 标记为成功，后面drop时就不会发送Error事件
    error_event_guard.success = true;
    emit_event(
        observer,
        EVENT_CBZ_END,
        ExportEndPayload {
            uuid: event_uuid,
            comic_id: comic.id,
            chapter_export_dir: cbz_export_dir,
        },
    );

    Ok(())
}

/// 主要参考了[Kavita的文档](https://wiki.kavitareader.com/guides/metadata/comics/)
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, YaSerialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicInfo {
    #[yaserde(rename = "Manga")]
    pub manga: String,
    /// 漫画名
    #[yaserde(rename = "Series")]
    pub series: String,
    /// 出版社
    #[yaserde(rename = "Publisher")]
    pub publisher: String,
    /// 作者
    #[yaserde(rename = "Writer")]
    pub writer: String,
    /// 漫画类型
    #[yaserde(rename = "Genre")]
    pub genre: String,
    #[yaserde(rename = "Summary")]
    pub summary: String,
    /// 章节名
    #[yaserde(rename = "Title")]
    pub title: String,
    /// 普通章节序号
    #[yaserde(rename = "Number")]
    pub number: Option<String>,
    /// 卷序号
    #[yaserde(rename = "Volume")]
    pub volume: Option<String>,
    /// 如果值为Special，则该章节会被Kavita视为特刊
    #[yaserde(rename = "Format")]
    pub format: Option<String>,
    /// 该章节的有多少页
    #[yaserde(rename = "PageCount")]
    pub page_count: i64,
    /// 章节总数
    /// - `0` => Ongoing
    /// - `非零`且与`Number`或`Volume`一致 => Completed
    /// - `其他非零值` => Ended
    #[yaserde(rename = "Count")]
    pub count: i64,
}
impl ComicInfo {
    #[allow(clippy::cast_possible_wrap)]
    pub fn from(comic: &Comic, chapter_info: &ChapterInfo) -> ComicInfo {
        let order = Some(chapter_info.order.to_string());
        let (number, volume, format) = match chapter_info.group_name.as_str() {
            "单话" => (order, None, None),
            "单行本" => (None, order, None),
            _ => (order, None, Some("Special".to_string())),
        };

        let count = match chapter_info.comic_status.as_ref() {
            "连载中" => 0,
            _ => chapter_info.group_size,
        };

        ComicInfo {
            manga: "Yes".to_string(),
            series: chapter_info.comic_title.clone(),
            publisher: "漫画柜".to_string(),
            writer: comic.authors.join(", "),
            genre: comic.genres.join(", "),
            summary: comic.intro.clone(),
            title: chapter_info.chapter_title.clone(),
            number,
            volume,
            format,
            page_count: chapter_info.chapter_size,
            count,
        }
    }
}
