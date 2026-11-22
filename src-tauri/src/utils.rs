use std::{collections::HashMap, path::PathBuf};

use tauri::AppHandle;
use tracing::instrument;

use crate::{extensions::AppHandleExt, types::Comic};

#[allow(unused_imports)]
pub use source_api::filename_filter;

#[instrument(level = "error", skip_all, fields(comic_id = id))]
pub async fn get_comic(app: &AppHandle, id: i64) -> eyre::Result<Comic> {
    let manhuagui_client = app.get_manhuagui_client();

    let comic = manhuagui_client.get_comic(id).await?;

    Ok(comic)
}

#[instrument(level = "error", skip_all)]
pub fn create_id_to_dir_map(app: &AppHandle) -> eyre::Result<HashMap<i64, PathBuf>> {
    let download_dir = app.get_config().read().download_dir.clone();
    source_api::create_id_to_dir_map(&download_dir)
}
