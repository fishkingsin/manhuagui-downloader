use std::{collections::HashMap, io::Cursor, path::PathBuf};

use image::ImageReader;
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

#[instrument(level = "error", skip_all)]
pub fn get_dimensions(img_data: &[u8]) -> eyre::Result<(u32, u32)> {
    let reader = ImageReader::new(Cursor::new(&img_data)).with_guessed_format()?;
    let dimensions = reader.into_dimensions()?;
    Ok(dimensions)
}
