use std::collections::HashMap;

use eyre::{OptionExt, WrapErr};
use scraper::{ElementRef, Html, Selector};
use tauri::AppHandle;
use tracing::instrument;

use crate::{extensions::ToEyre, types::ChapterInfo, utils};

#[allow(unused_imports)]
pub use source_api::{filename_filter, Comic, ComicDirFmtParams};

pub trait ComicExt {
    fn from_html(app: &AppHandle, html: &str) -> eyre::Result<Comic>;
}

impl ComicExt for Comic {
    #[allow(clippy::too_many_lines)]
    #[instrument(level = "error", skip_all)]
    fn from_html(app: &AppHandle, html: &str) -> eyre::Result<Comic> {
        let document = Html::parse_document(html);

        let hidden_fragment = match document
            .select(&Selector::parse("#__VIEWSTATE").to_eyre()?)
            .next()
        {
            Some(hidden_input) => {
                // 有隐藏数据
                let compressed_data = hidden_input
                    .value()
                    .attr("value")
                    .ok_or_eyre("没有在包含隐藏数据的<input>中找到value属性")?;

                let decompressed_data = lz_str::decompress_from_base64(compressed_data)
                    .ok_or_eyre("lzstring解压缩失败")?;

                let hidden_html = String::from_utf16(&decompressed_data)
                    .wrap_err("lzstring解压缩后的数据不是utf-16字符串")?;

                Some(Html::parse_fragment(&hidden_html))
            }
            None => None,
        };

        let book_detail_div = document
            .select(&Selector::parse(".book-detail").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到漫画详情的<div>")?;

        let id = document
            .select(&Selector::parse(".crumb > a:nth-last-child(1)").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到漫画链接的<a>")?
            .value()
            .attr("href")
            .ok_or_eyre("没有在漫画链接的<a>中找到href属性")?
            .trim_start_matches("/comic/")
            .trim_end_matches('/')
            .parse::<i64>()
            .wrap_err("漫画id不是整数")?;

        let (title, subtitle) = get_title_and_subtitle(&book_detail_div)?;

        let cover_src = document
            .select(&Selector::parse(".hcover img").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到封面的<img>")?
            .value()
            .attr("src")
            .ok_or_eyre("没有在封面的<img>中找到src属性")?
            .to_string();
        let cover = format!("https:{cover_src}");

        let detail_lis = book_detail_div
            .select(&Selector::parse(".detail-list > li").to_eyre()?)
            .collect::<Vec<_>>();

        let li = detail_lis
            .first()
            .ok_or_eyre("没有找到出版年份和地区的<li>")?;
        let (year, region) = get_year_and_region(li)?;

        let li = detail_lis
            .get(1)
            .ok_or_eyre("没有找到漫画类型和作者的<li>")?;
        let (genres, authors) = get_genres_and_authors(li)?;

        let li = detail_lis.get(2).ok_or_eyre("没有找到别名的<li>")?;
        let aliases = li
            .select(&Selector::parse("span > a").to_eyre()?)
            .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
            .collect::<Vec<_>>();

        let li = detail_lis
            .get(3)
            .ok_or_eyre("没有找到状态和更新时间的<li>")?;
        let (status, update_time) = get_status_and_update_time(li)?;

        let intro = book_detail_div
            .select(&Selector::parse("#intro-cut").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到简介的<div>")?
            .text()
            .next()
            .ok_or_eyre("没有在简介的<div>中找到文本")?
            .trim()
            .to_string();

        let groups = if let Some(fragment) = hidden_fragment {
            get_groups(&fragment.root_element(), id, &title, &status)?
        } else {
            let chapter_div = document
                .select(&Selector::parse(".chapter").to_eyre()?)
                .next()
                .ok_or_eyre("没有找到章节列表的<div>")?;

            get_groups(&chapter_div, id, &title, &status)?
        };

        let mut comic = Comic {
            id,
            title,
            subtitle,
            cover,
            status,
            update_time,
            year,
            region,
            genres,
            authors,
            aliases,
            intro,
            groups,
            is_downloaded: None,
            comic_download_dir: None,
        };

        let id_to_dir_map = utils::create_id_to_dir_map(app)?;

        // TODO: 这是为了兼容v0.4.2及之前的版本，后续需要移除，计划在v0.6.0之后移除
        if let Some(comic_download_dir) = id_to_dir_map.get(&comic.id) {
            comic
                .create_chapter_metadata_for_old_version(comic_download_dir)
                .wrap_err("为旧版本创建章节元数据失败")?;
        }

        comic.update_fields(&id_to_dir_map)?;

        Ok(comic)
    }
}

#[allow(dead_code)]
pub fn parse_comic_from_html(app: &AppHandle, html: &str) -> eyre::Result<Comic> {
    Comic::from_html(app, html)
}

#[instrument(level = "error", skip_all)]
fn get_title_and_subtitle(book_detail_div: &ElementRef) -> eyre::Result<(String, Option<String>)> {
    let title = book_detail_div
        .select(&Selector::parse(".book-title h1").to_eyre()?)
        .next()
        .ok_or_eyre("没有找到漫画标题的<h1>")?
        .text()
        .next()
        .ok_or_eyre("没有在漫画标题的<h1>中找到文本")?
        .trim()
        .to_string();
    let title = filename_filter(&title);

    let subtitle = book_detail_div
        .select(&Selector::parse(".book-title h2").to_eyre()?)
        .next()
        .and_then(|h2| h2.text().next())
        .map(|text| text.trim().to_string());

    Ok((title, subtitle))
}

#[instrument(level = "error", skip_all)]
fn get_year_and_region(li: &ElementRef) -> eyre::Result<(i64, String)> {
    let spans = li
        .select(&Selector::parse("span").to_eyre()?)
        .collect::<Vec<_>>();
    let a_selector = Selector::parse("a").to_eyre()?;

    let year = spans
        .first()
        .ok_or_eyre("没有找到出版年份的<span>")?
        .select(&a_selector)
        .next()
        .ok_or_eyre("没有找到出版年份的<a>")?
        .text()
        .next()
        .ok_or_eyre("没有在出版年份的<a>中找到文本")?
        .trim()
        .trim_end_matches('年')
        .parse::<i64>()
        .wrap_err("出版年份不是整数")?;

    let region = spans
        .get(1)
        .ok_or_eyre("没有找到地区的<span>")?
        .select(&a_selector)
        .next()
        .ok_or_eyre("没有找到地区的<a>")?
        .value()
        .attr("title")
        .ok_or_eyre("没有在地区的<a>中找到title属性")?
        .to_string();

    Ok((year, region))
}

#[instrument(level = "error", skip_all)]
fn get_genres_and_authors(li: &ElementRef) -> eyre::Result<(Vec<String>, Vec<String>)> {
    let spans = li
        .select(&Selector::parse("span").to_eyre()?)
        .collect::<Vec<_>>();
    let a_selector = Selector::parse("a").to_eyre()?;

    let genres = spans
        .first()
        .ok_or_eyre("没有找到漫画类型的<span>")?
        .select(&a_selector)
        .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
        .collect::<Vec<_>>();

    let authors = spans
        .get(1)
        .ok_or_eyre("没有找到作者的<span>")?
        .select(&a_selector)
        .filter_map(|a| a.value().attr("title").map(str::to_string))
        .collect::<Vec<_>>();

    Ok((genres, authors))
}

#[instrument(level = "error", skip_all)]
fn get_status_and_update_time(li: &ElementRef) -> eyre::Result<(String, String)> {
    let spans = li
        .select(&Selector::parse("span > span").to_eyre()?)
        .collect::<Vec<_>>();

    let status = spans
        .first()
        .ok_or_eyre("没有找到漫画状态的<span>")?
        .text()
        .next()
        .ok_or_eyre("没有在漫画状态的<span>中找到文本")?
        .trim()
        .to_string();

    let update_time = spans
        .get(1)
        .ok_or_eyre("没有找到更新时间的<span>")?
        .text()
        .next()
        .ok_or_eyre("没有在更新时间的<span>中找到文本")?
        .trim()
        .to_string();

    Ok((status, update_time))
}

#[allow(clippy::cast_possible_wrap)]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic_id, comic_title = comic_title, comic_status = comic_status)
)]
fn get_groups(
    chapter_div: &ElementRef,
    comic_id: i64,
    comic_title: &str,
    comic_status: &str,
) -> eyre::Result<HashMap<String, Vec<ChapterInfo>>> {
    let mut group_names = chapter_div
        .select(&Selector::parse("h4").to_eyre()?)
        .map(|h4| h4.text().next().unwrap_or_default().trim().to_string())
        .collect::<Vec<_>>();

    let chapter_divs = chapter_div
        .select(&Selector::parse(".chapter-list").to_eyre()?)
        .collect::<Vec<_>>();

    // 保证 group_names.len() == chapter_divs.len()
    while group_names.len() < chapter_divs.len() {
        group_names.push(String::new());
    }

    let empty_count = group_names.iter().filter(|s| s.is_empty()).count();
    let mut empty_index = 0;

    let mut group_name_and_chapter_divs = group_names
        .into_iter()
        .zip(chapter_divs)
        .collect::<Vec<_>>();

    for (group_name, _) in &mut group_name_and_chapter_divs {
        if !group_name.is_empty() {
            continue;
        }
        // 处理没有group_name的情况
        empty_index += 1;
        if empty_count == 1 {
            *group_name = "其他".to_string();
        } else {
            *group_name = format!("其他{empty_index}");
        }
    }

    let mut groups = HashMap::new();
    for (group_name, chapter_list_div) in group_name_and_chapter_divs {
        let group_name = filename_filter(&group_name);

        let uls = chapter_list_div
            .select(&Selector::parse("ul").to_eyre()?)
            .collect::<Vec<_>>();

        let mut order = 0.0;
        // 统计一共有多少个li
        let group_size = chapter_list_div
            .select(&Selector::parse("li").to_eyre()?)
            .count() as i64;

        let mut chapter_infos = Vec::new();
        for ul in uls {
            let mut lis = ul
                .select(&Selector::parse("li").to_eyre()?)
                .collect::<Vec<_>>();
            lis.reverse();

            for li in lis {
                order += 1.0;
                let a = li
                    .select(&Selector::parse("a").to_eyre()?)
                    .next()
                    .ok_or_eyre("没有找到章节的<a>")?;

                let chapter_id = a
                    .value()
                    .attr("href")
                    .ok_or_eyre("没有在章节的<a>中找到href属性")?
                    .trim_start_matches(&format!("/comic/{comic_id}/"))
                    .trim_end_matches(".html")
                    .parse::<i64>()
                    .wrap_err("章节id不是整数")?;

                let chapter_title = a
                    .value()
                    .attr("title")
                    .ok_or_eyre("没有在章节的<a>中找到title属性")?
                    .to_string();
                let chapter_title = filename_filter(&chapter_title);

                let chapter_size = a
                    .select(&Selector::parse("span > i").to_eyre()?)
                    .next()
                    .ok_or_eyre("没有找到章节的<i>")?
                    .text()
                    .next()
                    .ok_or_eyre("没有在章节的<i>中找到文本")?
                    .trim()
                    .trim_end_matches('p')
                    .parse::<i64>()
                    .wrap_err("章节页数不是整数")?;

                chapter_infos.push(ChapterInfo {
                    chapter_id,
                    chapter_title,
                    chapter_size,
                    comic_id,
                    comic_title: comic_title.to_string(),
                    group_name: group_name.clone(),
                    group_size,
                    order,
                    comic_status: comic_status.to_string(),
                    is_pdf_exported: false,
                    is_cbz_exported: false,
                    is_downloaded: None,
                    chapter_download_dir: None,
                });
            }
        }

        groups.insert(group_name, chapter_infos);
    }

    Ok(groups)
}
