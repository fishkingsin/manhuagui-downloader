use std::{collections::HashMap, path::PathBuf};

use eyre::{OptionExt, WrapErr};
use scraper::{ElementRef, Html, Selector};
use tracing::instrument;

use source_api::{filename_filter, ChapterInfo, Comic, ComicInSearch, SearchResult};

trait ToEyre<T> {
    fn to_eyre(self) -> eyre::Result<T>;
}

impl<T> ToEyre<T> for Result<T, scraper::error::SelectorErrorKind<'_>> {
    fn to_eyre(self) -> eyre::Result<T> {
        self.map_err(|e| eyre::eyre!(e.to_string()))
    }
}

#[instrument(level = "error", skip_all)]
pub fn parse_comic(html: &str) -> eyre::Result<Comic> {
    let document = Html::parse_document(html);

    let hidden_fragment = match document
        .select(&Selector::parse("#__VIEWSTATE").to_eyre()?)
        .next()
    {
        Some(hidden_input) => {
            let compressed_data = hidden_input
                .value()
                .attr("value")
                .ok_or_eyre("没有在包含隐藏数据的<input>中找到value属性")?;

            let decompressed_data =
                lz_str::decompress_from_base64(compressed_data).ok_or_eyre("lzstring解压缩失败")?;

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

    let comic = Comic {
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

    Ok(comic)
}

#[instrument(level = "error", skip_all)]
pub fn parse_search(html: &str, page_num: i64) -> eyre::Result<SearchResult> {
    let document = Html::parse_document(html);
    let book_result_selector = Selector::parse(".book-result .cf").to_eyre()?;

    let mut comics = Vec::new();
    for book_li in document.select(&book_result_selector) {
        let comic = comic_in_search_from_li(&book_li)?;
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
            .unwrap_or(page_num),
        None => page_num,
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

#[instrument(level = "error", skip_all)]
fn comic_in_search_from_li(li: &ElementRef) -> eyre::Result<ComicInSearch> {
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

    let comic = ComicInSearch {
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
