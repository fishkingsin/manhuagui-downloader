mod commands;
mod config;
mod errors;
mod events;
mod extensions;
mod logger;
mod types;
mod utils;

use std::sync::Arc;

use config::Config;
use eyre::WrapErr;
use library::downloader::download_manager::DownloadManager;
use library::export::ComicExportLock;
use parking_lot::RwLock;
use source_manhuagui::ManhuaguiClient;
use tauri::{Manager, Wry};

use crate::{
    commands::*,
    errors::install_custom_eyre_handler,
    events::{
        CoreObserver, DownloadEvent, ExportCbzEvent, ExportPdfEvent, LogEvent,
        UpdateDownloadedComicsEvent,
    },
};

fn generate_context() -> tauri::Context<Wry> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_custom_eyre_handler().unwrap();

    let builder = tauri_specta::Builder::<Wry>::new()
        .commands(tauri_specta::collect_commands![
            greet,
            get_config,
            save_config,
            login,
            get_user_profile,
            search,
            get_comic,
            get_favorite,
            get_downloaded_comics,
            export_cbz,
            export_pdf,
            export_cbz_chapters,
            export_pdf_chapters,
            update_downloaded_comics,
            get_logs_dir_size,
            show_path_in_file_manager,
            create_download_task,
            pause_download_task,
            resume_download_task,
            delete_download_task,
            get_synced_comic,
            get_synced_comic_in_favorite,
            get_synced_comic_in_search,
            open_log_file,
        ])
        .events(tauri_specta::collect_events![
            DownloadEvent,
            ExportCbzEvent,
            ExportPdfEvent,
            UpdateDownloadedComicsEvent,
            LogEvent,
        ]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .formatter(specta_typescript::formatter::prettier)
                .header("// @ts-nocheck"), // 跳过检查
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let app_data_dir = app
                .path()
                .app_data_dir()
                .wrap_err("获取app_data_dir目录失败")?;

            std::fs::create_dir_all(&app_data_dir).wrap_err(format!(
                "创建app_data_dir目录`{}`失败",
                app_data_dir.display()
            ))?;

            let config = RwLock::new(Config::new(app.handle()).wrap_err("创建Config失败")?);
            let core_config = (&*config.read()).into();
            app.manage(config);

            let manhuagui_client = Arc::new(ManhuaguiClient::new(&core_config));

            let download_manager = DownloadManager::new(
                Arc::new(RwLock::new(core_config.clone())),
                manhuagui_client.clone() as Arc<dyn source_api::PageFetcher>,
                Arc::new(CoreObserver::new(app.handle().clone())),
            );
            app.manage(manhuagui_client);
            app.manage(download_manager);

            let export_lock = ComicExportLock::new();
            app.manage(export_lock);

            logger::init(app.handle()).wrap_err("初始化日志系统失败")?;

            Ok(())
        })
        .run(generate_context())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specta_export() {
        const BINDINGS_PATH: &str = "../src/bindings.ts";

        // Snapshot the committed bindings before regenerating so a wire-contract
        // drift is reported as a failure, never silently overwritten.
        let before = std::fs::read_to_string(BINDINGS_PATH)
            .expect("Failed to read committed typescript bindings before regenerating");

        let builder = tauri_specta::Builder::<tauri::Wry>::new()
            .commands(tauri_specta::collect_commands![
                greet,
                get_config,
                save_config,
                login,
                get_user_profile,
                search,
                get_comic,
                get_favorite,
                get_downloaded_comics,
                export_cbz,
                export_pdf,
                export_cbz_chapters,
                export_pdf_chapters,
                update_downloaded_comics,
                get_logs_dir_size,
                show_path_in_file_manager,
                create_download_task,
                pause_download_task,
                resume_download_task,
                delete_download_task,
                get_synced_comic,
                get_synced_comic_in_favorite,
                get_synced_comic_in_search,
                open_log_file,
            ])
            .events(tauri_specta::collect_events![
                DownloadEvent,
                ExportCbzEvent,
                ExportPdfEvent,
                UpdateDownloadedComicsEvent,
                LogEvent,
            ]);

        builder
            .export(
                specta_typescript::Typescript::default()
                    .bigint(specta_typescript::BigIntExportBehavior::Number)
                    .header("// @ts-nocheck"),
                BINDINGS_PATH,
            )
            .expect("Failed to export typescript bindings");

        let after = std::fs::read_to_string(BINDINGS_PATH)
            .expect("Failed to read regenerated typescript bindings");

        if before != after {
            panic!(
                "wire-contract violation: regenerating specta bindings changed `{BINDINGS_PATH}`. \
                 A Tauri command or event type changed shape without the committed frontend \
                 bindings being updated. Commit the regenerated file or fix the type drift."
            );
        }
    }
}
