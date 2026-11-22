use std::{collections::HashMap, path::PathBuf};

use eyre::{OptionExt, WrapErr};
use scraper::{ElementRef, Html, Selector};
use tauri::AppHandle;
use tracing::instrument;

use crate::{extensions::ToEyre, utils};

#[allow(unused_imports)]
pub use source_api::{ComicInSearch, SearchResult, SearchResultComic};

pub trait SearchResultExt {
    fn from_html(app: &AppHandle, html: &str) -> eyre::Result<SearchResult>;
}

impl SearchResultExt for SearchResult {
    #[instrument(level = "error", skip_all)]
    fn from_html(app: &AppHandle, html: &str) -> eyre::Result<SearchResult> {
        let id_to_dir_map = utils::create_id_to_dir_map(app)?;

        let document = Html::parse_document(html);
        let book_result_selector = Selector::parse(".book-result .cf").to_eyre()?;

        let mut comics = Vec::new();
        for book_li in document.select(&book_result_selector) {
            let comic = comic_in_search_from_li(&book_li, &id_to_dir_map)?;
            comics.push(comic);
        }

        let current = match document
            .select(&Selector::parse(".current").to_eyre()?)
            .next()
        {
            Some(span) => span
                .text()
                .next()
                .ok_or_eyre("没有在当前页码的span中找到文本")?
                .parse::<i64>()
                .wrap_err("当前页码不是整数")?,
            None => 1,
        };

        let total = document
            .select(&Selector::parse(".result-count strong").to_eyre()?)
            .nth(1)
            .ok_or_eyre("没有找到总结果数的<strong>")?
            .text()
            .next()
            .ok_or_eyre("没有在总结果数的<strong>中找到文本")?
            .parse::<i64>()
            .wrap_err("总结果数不是整数")?;

        Ok(SearchResult {
            comics,
            current,
            total,
        })
    }
}

#[allow(dead_code)]
pub fn parse_search_result_from_html(app: &AppHandle, html: &str) -> eyre::Result<SearchResult> {
    SearchResult::from_html(app, html)
}

#[instrument(level = "error", skip_all)]
fn comic_in_search_from_li(
    li: &ElementRef,
    id_to_dir_map: &HashMap<i64, PathBuf>,
) -> eyre::Result<ComicInSearch> {
    let book_detail_div = li
        .select(&Selector::parse(".book-detail").to_eyre()?)
        .next()
        .ok_or_eyre("没有找到书籍详情的<div>")?;

    let dt = book_detail_div
        .select(&Selector::parse("dt").to_eyre()?)
        .next()
        .ok_or_eyre("没有找到漫画标题和链接的<dt>")?;
    let (id, title, subtitle) = get_id_and_title_and_subtitle(dt)?;

    let cover_src = li
        .select(&Selector::parse(".book-cover img").to_eyre()?)
        .next()
        .ok_or_eyre("没有找到封面的<img>")?
        .value()
        .attr("src")
        .ok_or_eyre("没有在封面的<img>中找到src属性")?;
    let cover = format!("https:{cover_src}");

    let dds = book_detail_div
        .select(&Selector::parse("dd").to_eyre()?)
        .collect::<Vec<_>>();

    let status_dd = dds.first().ok_or_eyre("没有找到漫画状态和更新时间的dd")?;
    let (status, update_time) = get_status_and_update_time(status_dd)?;

    let info_dd = dds.get(1).ok_or_eyre("没有找到年份、地区、类型的dd")?;
    let (year, region, genres) = get_year_and_region_and_genres(info_dd)?;

    let authors = dds
        .get(2)
        .ok_or_eyre("没有找到作者的<dd>")?
        .select(&Selector::parse("a").to_eyre()?)
        .filter_map(|a| a.value().attr("title").map(str::to_string))
        .collect::<Vec<_>>();

    let aliases = dds
        .get(3)
        .ok_or_eyre("没有找到别名的<dd>")?
        .select(&Selector::parse("a").to_eyre()?)
        .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
        .collect::<Vec<_>>();

    let intro = book_detail_div
        .select(&Selector::parse(".intro span").to_eyre()?)
        .next()
        .ok_or_eyre("没有找到简介的<span>")?
        .text()
        .nth(1)
        .ok_or_eyre("没有在简介的<span>中找到文本")?
        .trim()
        .trim_end_matches('[')
        .to_string();

    let mut comic = ComicInSearch {
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
        is_downloaded: false,
        comic_download_dir: PathBuf::new(),
    };

    comic.update_fields(id_to_dir_map);

    Ok(comic)
}

#[instrument(level = "error", skip_all)]
fn get_id_and_title_and_subtitle(dt: ElementRef) -> eyre::Result<(i64, String, Option<String>)> {
    let a = dt
        .select(&Selector::parse("dt > a").to_eyre()?)
        .next()
        .ok_or_eyre("没有找到标题和链接的<a>")?;

    let id = a
        .value()
        .attr("href")
        .ok_or_eyre("没有在标题和链接的<a>中找到href属性")?
        .trim_start_matches("/comic/")
        .trim_end_matches('/')
        .parse()
        .wrap_err("漫画id不是整数")?;

    let title = a
        .value()
        .attr("title")
        .ok_or_eyre("没有在标题和链接的<a>中找到title属性")?
        .to_string();

    let subtitle = dt
        .select(&Selector::parse("dt > small > a").to_eyre()?)
        .next()
        .and_then(|a| a.text().next())
        .map(|text| text.trim().to_string());

    Ok((id, title, subtitle))
}

#[instrument(level = "error", skip_all)]
fn get_status_and_update_time(status_dd: &ElementRef) -> eyre::Result<(String, String)> {
    let spans = status_dd
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

#[instrument(level = "error", skip_all)]
fn get_year_and_region_and_genres(
    info_dd: &ElementRef,
) -> eyre::Result<(i64, String, Vec<String>)> {
    let spans = info_dd
        .select(&Selector::parse("span").to_eyre()?)
        .collect::<Vec<_>>();

    let a_selector = Selector::parse("a").to_eyre()?;

    let year = spans
        .first()
        .ok_or_eyre("没有找到出版年份<span>")?
        .select(&a_selector)
        .next()
        .ok_or_eyre("没有找到出版年份<a>")?
        .text()
        .next()
        .ok_or_eyre("没有在出版年份<a>中找到文本")?
        .trim()
        .trim_end_matches('年')
        .parse()
        .wrap_err("出版年份不是整数")?;

    let region = spans
        .get(1)
        .ok_or_eyre("没有找到地区<span>")?
        .select(&a_selector)
        .next()
        .ok_or_eyre("没有找到地区<a>")?
        .value()
        .attr("title")
        .ok_or_eyre("没有在地区<a>中找到title属性")?
        .to_string();

    let genres = spans
        .get(2)
        .ok_or_eyre("没有找到类型<span>")?
        .select(&a_selector)
        .filter_map(|a| a.value().attr("title").map(str::to_string))
        .collect::<Vec<_>>();

    Ok((year, region, genres))
}
