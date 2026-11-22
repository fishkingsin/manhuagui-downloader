use source_api::{CoreConfig, ExportSkipMode, ProxyMode, SourceError};

#[test]
fn core_config_default_matches_pinned_defaults() {
    let cfg = CoreConfig::default();

    assert_eq!(cfg.cookie, "");
    assert_eq!(cfg.chapter_concurrency, 1);
    assert_eq!(cfg.chapter_download_interval_sec, 10);
    assert_eq!(cfg.img_concurrency, 10);
    assert_eq!(cfg.img_download_interval_sec, 0);
    assert_eq!(cfg.update_get_comic_interval_sec, 0);
    assert_eq!(cfg.api_timeout_sec, 3);
    assert_eq!(cfg.img_timeout_sec, 3);
    assert_eq!(cfg.proxy_mode, ProxyMode::System);
    assert_eq!(cfg.proxy_host, "127.0.0.1");
    assert_eq!(cfg.proxy_port, 7890);
    assert_eq!(cfg.comic_dir_fmt, "{comic_title}");
    assert_eq!(cfg.chapter_dir_fmt, "{group_name}/{order} {chapter_title}");
    assert!(cfg.create_pdf_concurrency >= 1);
    assert!(cfg.enable_merge_pdf);
    assert_eq!(cfg.export_skip_mode, ExportSkipMode::None);
}

#[test]
fn parse_failed_converts_to_eyre_report_keeping_message() {
    let err = SourceError::ParseFailed {
        message: "missing chapter list".to_string(),
    };

    let report: eyre::Report = err.into();

    assert!(report.to_string().contains("missing chapter list"));
}
