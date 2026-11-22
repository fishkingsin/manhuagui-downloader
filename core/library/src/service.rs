//! Source registry and the windowed browse engine (the 7/15/30-day core).

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{FixedOffset, NaiveDate, Utc};
use parking_lot::RwLock;
use source_api::{page_outside_window, ComicSummary, Source, SourceError, UpdateWindow};

/// Compiled-in registry of sources, looked up by [`Source::id`].
pub struct Library {
    sources: RwLock<HashMap<String, Arc<dyn Source>>>,
}

impl Library {
    pub fn new() -> Self {
        Self {
            sources: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_source(&self, source: Arc<dyn Source>) {
        self.sources.write().insert(source.id().to_string(), source);
    }

    pub fn source(&self, id: &str) -> Option<Arc<dyn Source>> {
        self.sources.read().get(id).cloned()
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}

/// "Today" in the site's timezone (Asia/Shanghai, UTC+8, no DST), computed once
/// per walk so the window cutoff never drifts between pages.
fn today_shanghai() -> NaiveDate {
    let offset = FixedOffset::east_opt(8 * 3600).expect("UTC+8 is a valid fixed offset");
    Utc::now().with_timezone(&offset).date_naive()
}

/// Browses pages `start_page..` sequentially, collecting items whose
/// `updated_on` falls inside `window` (inclusive cutoff).
///
/// - `UpdateWindow::All` fetches exactly `start_page` and returns its items
///   verbatim (no further paging).
/// - For `Days7`/`Days15`/`Days30`, each page's out-of-window and `None`-dated
///   items are dropped, and the walk stops as soon as a page is entirely
///   outside the window ([`page_outside_window`]), or the fetched page is the
///   last one, or `max_pages` pages have been fetched (hard cap).
pub async fn browse_within(
    source: &dyn Source,
    window: UpdateWindow,
    start_page: i64,
    max_pages: i64,
) -> Result<Vec<ComicSummary>, SourceError> {
    let today = today_shanghai();

    let days = match window {
        UpdateWindow::Days7 => Some(7),
        UpdateWindow::Days15 => Some(15),
        UpdateWindow::Days30 => Some(30),
        UpdateWindow::All => None,
    };

    let mut result = Vec::new();
    let mut page = start_page;
    let mut fetched = 0;

    loop {
        if fetched >= max_pages {
            break;
        }

        let browse = source.browse_latest(window, page, None).await?;
        fetched += 1;

        match days {
            None => {
                result.extend(browse.items);
                break;
            }
            Some(days) => {
                let cutoff = today - chrono::Duration::days(days);
                result.extend(
                    browse
                        .items
                        .iter()
                        .filter(|item| item.updated_on.is_some_and(|date| date >= cutoff))
                        .cloned(),
                );

                if page_outside_window(&browse.items, window, today)
                    || browse.page >= browse.total_pages
                {
                    break;
                }
            }
        }

        page += 1;
    }

    Ok(result)
}
