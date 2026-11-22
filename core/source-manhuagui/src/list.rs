use std::sync::LazyLock;

use chrono::NaiveDate;
use eyre::{OptionExt, WrapErr};
use regex::Regex;
use scraper::{Html, Selector};
use source_api::{BrowseResult, ComicId, ComicSummary};

static LIST_ITEM_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("ul#contList > li").expect("valid selector"));
static BCOVER_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a.bcover").expect("valid selector"));
static TITLE_LINK_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("p.ell a").expect("valid selector"));
static IMG_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("img").expect("valid selector"));
static STATUS_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.tt").expect("valid selector"));
static UPDATEON_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.updateon").expect("valid selector"));
static RATING_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.updateon em, em").expect("valid selector"));
static PAGER_CURRENT_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".pager span.current").expect("valid selector"));
static PAGER_LINKS_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".pager a").expect("valid selector"));
static RESULT_COUNT_STRONG_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".result-count strong").expect("valid selector"));

static DATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4}-\d{2}-\d{2})").expect("valid regex"));
static PAGE_NUM_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:index|update)_p(\d+)\.html").expect("valid regex"));

/// Constructs the list page URL for manhuagui given an optional genre and page number.
pub fn browse_latest_url(genre: Option<&str>, page: i64) -> String {
    let base = match genre {
        Some(g) if !g.trim().is_empty() => format!("japan_{}", g.trim().trim_matches('/')),
        _ => "japan".to_string(),
    };
    let path = if page <= 1 {
        format!("/list/{base}/update.html")
    } else {
        format!("/list/{base}/update_p{page}.html")
    };
    format!("https://www.manhuagui.com{path}")
}

/// Parses an HTML list/browse page from manhuagui into a `BrowseResult`.
pub fn parse_list_page(html: &str) -> eyre::Result<BrowseResult> {
    let document = Html::parse_document(html);

    let mut items = Vec::new();
    for li in document.select(&LIST_ITEM_SEL) {
        let bcover = match li.select(&BCOVER_SEL).next() {
            Some(el) => el,
            None => continue,
        };

        let href = bcover
            .value()
            .attr("href")
            .ok_or_eyre("缺少 a.bcover 的 href 属性")?;

        let native_id = href
            .trim_start_matches("/comic/")
            .trim_end_matches('/')
            .parse::<i64>()
            .wrap_err_with(|| format!("无法解析漫画 ID: {href}"))?;

        let title = bcover
            .value()
            .attr("title")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                li.select(&TITLE_LINK_SEL)
                    .next()
                    .and_then(|a| a.value().attr("title").or_else(|| a.text().next()))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_default();

        let cover_url = li
            .select(&IMG_SEL)
            .next()
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("src"))
            })
            .map(|src| {
                if src.starts_with("//") {
                    format!("https:{src}")
                } else {
                    src.to_string()
                }
            })
            .unwrap_or_default();

        let status_line = li
            .select(&STATUS_SEL)
            .next()
            .map(|span| span.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let (updated_on, rating) = if let Some(updateon_el) = li.select(&UPDATEON_SEL).next() {
            let updateon_text = updateon_el.text().collect::<String>();
            let date = DATE_REGEX
                .captures(&updateon_text)
                .and_then(|caps| caps.get(1))
                .and_then(|m| NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok());

            let rating = updateon_el
                .select(&RATING_SEL)
                .next()
                .map(|em| em.text().collect::<String>().trim().to_string())
                .and_then(|s| s.parse::<f32>().ok());

            (date, rating)
        } else {
            (None, None)
        };

        items.push(ComicSummary {
            comic_id: ComicId {
                source_id: "manhuagui".to_string(),
                native_id,
            },
            title,
            cover_url,
            status_line,
            updated_on,
            rating,
        });
    }

    let current_page = document
        .select(&PAGER_CURRENT_SEL)
        .next()
        .and_then(|span| span.text().collect::<String>().trim().parse::<i64>().ok())
        .or_else(|| {
            document
                .select(&RESULT_COUNT_STRONG_SEL)
                .next()
                .and_then(|s| s.text().collect::<String>().trim().parse::<i64>().ok())
        })
        .unwrap_or(1);

    let mut total_pages = 1;

    for a in document.select(&PAGER_LINKS_SEL) {
        if let Some(href) = a.value().attr("href") {
            if let Some(caps) = PAGE_NUM_REGEX.captures(href) {
                if let Some(m) = caps.get(1) {
                    if let Ok(p) = m.as_str().parse::<i64>() {
                        if p > total_pages {
                            total_pages = p;
                        }
                    }
                }
            }
        }
    }

    if total_pages == 1 {
        if let Some(strong) = document.select(&RESULT_COUNT_STRONG_SEL).nth(1) {
            if let Ok(p) = strong.text().collect::<String>().trim().parse::<i64>() {
                if p > total_pages {
                    total_pages = p;
                }
            }
        }
    }

    if total_pages < current_page {
        total_pages = current_page;
    }

    Ok(BrowseResult {
        items,
        page: current_page,
        total_pages,
    })
}
