use source_manhuagui::img_url;

#[test]
fn test_img_url_construction() {
    let path = "/ps3/b/bfcq_ydcn/第01回/";
    let file = "001.jpg.webp";
    let page_ref = img_url(path, file);
    assert_eq!(
        page_ref.url,
        "https://i.hamreus.com/ps3/b/bfcq_ydcn/第01回/001.jpg"
    );
    assert_eq!(
        page_ref.headers.get("Referer").map(|s| s.as_str()),
        Some("https://www.manhuagui.com/")
    );
}
