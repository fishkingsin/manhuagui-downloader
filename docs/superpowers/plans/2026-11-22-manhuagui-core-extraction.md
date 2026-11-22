# manhuagui Core Extraction Implementation Plan (Phase 0)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the desktop app's scraper/decrypt/downloader/exporter into a layered, source-agnostic Rust core (`core/source-api`, `core/source-manhuagui`, `core/library`) behind a `Source` trait with a `LibraryService` facade, with the desktop Tauri shell behavior byte-identical.

**Architecture:** Three new workspace crates under `core/`. `source-api` holds domain types, `CoreConfig`, the `Source` trait, structured errors, and an `EventObserver`; `source-manhuagui` holds all site knowledge (client, p.a.c.k.e.r decrypt, HTML parsers, new `/list/` browsing) and implements `Source`; `library` holds the downloader, exporters, source registry, and `LibraryService`. The desktop shell (`src-tauri`) keeps Tauri commands, its `Config` (frontend shape), tauri-specta events, and login/favorites glue, now calling into core.

**Tech Stack:** Rust 2021, tokio, reqwest(+retry/middleware), scraper, serde/specta (feature-gated), lz_str, chrono, zip/pdf crates as moved; Tauri 2 + Vue shell unchanged.

**Spec:** `docs/superpowers/specs/2026-11-22-manhuagui-android-ereader-design.md`

## Global Constraints

- Desktop behavior identical: same Tauri command names/signatures, same event names + JSON payloads (camelCase), same `元数据.json`/`章节元数据.json` file format, same default config values (e.g. `chapter_download_interval_sec: 10`, `img_concurrency: 10`, `comic_dir_fmt: "{comic_title}"`, `chapter_dir_fmt: "{group_name}/{order} {chapter_title}"`).
- Core crates must not depend on `tauri`, `tauri-specta`, or `AppHandle` — only serde + optionally `specta` behind a `specta` feature flag (`#[cfg_attr(feature = "specta", derive(specta::Type))]`) so desktop TS bindings stay generatable.
- Layering: `source-api` ← `source-manhuagui`; `source-api` ← `library`. `library` never imports `source-manhuagui`. All HTML parsing lives in `source-manhuagui`.
- Anti-ban pacing values pass through unchanged; timeouts stay 3s on desktop (mobile overrides are the Android phase's business).
- One deviation from spec §Core extraction, documented here: login/favorites/user-profile types and client methods stay **in `source-manhuagui`** (they are site knowledge) but off the `Source` trait; only the desktop shell calls them. The spec's "leave in shell" intent is preserved: the trait and Android never see them.
- Workspace root `Cargo.toml` gains `[workspace] members = ["src-tauri", "core/source-api", "core/source-manhuagui", "core/library"]` with shared `[workspace.dependencies]`; `src-tauri` keeps its package name `manhuagui-downloader`.
- Every task ends with `cargo build --workspace` green and, where a GUI behavior changed path, the listed manual QA checkpoint.

## Review Focus

1. **Old metadata files must still load** — users' existing `元数据.json`/`章节元数据.json` (written by pre-refactor builds) feed `Comic::from_metadata`. Test: golden fixture captured in Task 3 before the move, round-trip test after.
2. **Event payloads byte-identical on the wire** — the Vue frontend matches on event names and camelCase payload fields. Test: golden JSON per event variant (Task 7).
3. **Missing/malformed `更新于` dates on list pages** — entry must still appear (with `updated_on: None`), not crash parsing. Test in Task 6.
4. **Window boundary + timezone** — `更新于：2026-11-22` is an Asia/Shanghai date; a comic updated exactly N days ago is inside a `Days(N)` window (inclusive). Test in Task 6.
5. **Frontend type shape stability** — `Comic`/`ChapterInfo` serde field names (camelCase) must not drift; TS bindings regenerate identical. Test: serde snapshot vs golden JSON (Task 3).

---

### Task 1: Fork, branch, workspace scaffold

**Files:**
- Create: `core/source-api/Cargo.toml`, `core/source-api/src/lib.rs` (empty)
- Create: `core/source-manhuagui/Cargo.toml`, `core/source-manhuagui/src/lib.rs` (empty)
- Create: `core/library/Cargo.toml`, `core/library/src/lib.rs` (empty)
- Modify: `Cargo.toml` at repo root (new, workspace) — move `src-tauri/Cargo.toml` package config stays; root gets workspace + shared deps

**Interfaces:**
- Produces: workspace where `cargo build --workspace` and `cargo test --workspace` pass; crates named `source-api`, `source-manhuagui`, `library` with `edition 2021`.

- [ ] **Step 1: Fork + branch**

```bash
cd /Users/james/Development/manhuagui-downloader
gh repo fork --remote-name fork   # creates fork under user's GitHub, adds remote "fork"; if gh unavailable, ask user to fork on github.com and `git remote add fork <url>`
git checkout -b core-extraction
git add docs/ && git commit -m "docs: android e-reader design spec"
```

- [ ] **Step 2: Create root workspace `Cargo.toml`**

Root `Cargo.toml`: `[package]` empty — workspace only: members as in Global Constraints, `[workspace.dependencies]` for `tokio`, `serde`, `serde_json`, `eyre`, `reqwest`, `reqwest-middleware`, `reqwest-retry`, `scraper`, `regex`, `lz_str`, `chrono`, `specta`, `tracing`, `bytes`, `parking_lot`. Versions: copy verbatim from `src-tauri/Cargo.toml`. Add `async-trait = "0.1"`.
`src-tauri/Cargo.toml` switches those deps to `{ workspace = true }`.

- [ ] **Step 3: Empty crates; verify build**

Each `core/*/Cargo.toml`: name, `edition 2021`, lib path default, deps empty for now. Run `cargo build --workspace` → OK. Run `pnpm tauri build` is NOT needed yet; run `cargo check -p manhuagui-downloader` → OK.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "chore: cargo workspace with core crates scaffold"
```

### Task 2: `CoreConfig` + enums in source-api

**Files:**
- Create: `core/source-api/src/config.rs`, `core/source-api/src/error.rs`
- Modify: `core/source-api/src/lib.rs` (re-exports), `core/source-api/Cargo.toml` (serde, serde_json, chrono, eyre, specta optional)

**Interfaces:**
- Produces:
  - `pub struct CoreConfig` — fields exactly `src-tauri/src/config.rs`'s `Config` minus `enable_file_logger`, plus `pub api_timeout_sec: u64`, `pub img_timeout_sec: u64` (default 3/3 to match today's hardcoded behavior). Includes `cookie`, `download_dir: PathBuf`, `export_dir: PathBuf`, `chapter_concurrency`, `chapter_download_interval_sec`, `img_concurrency`, `img_download_interval_sec`, `update_get_comic_interval_sec`, `proxy_mode: ProxyMode`, `proxy_host`, `proxy_port`, `comic_dir_fmt`, `chapter_dir_fmt`, `create_pdf_concurrency`, `enable_merge_pdf`, `export_skip_mode: ExportSkipMode`.
  - `pub enum ProxyMode { System, NoProxy, Custom }`, `pub enum ExportSkipMode { None, SkipExisting, SkipExported }` (serde derives, defaults System/None).
  - `impl Default for CoreConfig` with desktop-default values verbatim (dirs = `PathBuf::new()` placeholders; fmt strings as in Global Constraints).
  - `pub enum SourceError { Network { message: String }, RateLimited { message: String }, ParseFailed { message: String }, NotFound { message: String }, Cancelled { message: String } }` with `impl From<SourceError> for eyre::Report` (message passthrough).
  - Later tasks rely on: `source_api::{CoreConfig, ProxyMode, ExportSkipMode, SourceError}`.

- [ ] **Step 1: Write failing tests** (`core/source-api/tests/config_test.rs`): assert `CoreConfig::default()` equals every pinned default from Global Constraints (interval 10, concurrency 10/1, fmt strings, timeouts 3/3, `ProxyMode::System`, `ExportSkipMode::None`); assert `SourceError::ParseFailed { .. }` converts to an eyre Report whose `to_string()` contains the original message.

- [ ] **Step 2: Run** `cargo test -p source-api` → FAIL (types missing).

- [ ] **Step 3: Implement** `config.rs` + `error.rs`; wire `lib.rs`. No choices open — copy field names/defaults verbatim from `src-tauri/src/config.rs`.

- [ ] **Step 4: Run** `cargo test -p source-api` → PASS. `cargo build --workspace` → OK.

- [ ] **Step 5: Commit** `git commit -am "feat(core): CoreConfig and SourceError in source-api"`

### Task 3: Move domain types into source-api (structs only)

**Files:**
- Create: `core/source-api/src/types.rs` (moved structs)
- Modify: `src-tauri/src/types.rs` and `src-tauri/src/types/*` (delete moved, re-export from `source_api`)
- Test: `core/source-api/tests/types_golden_test.rs`; fixture `core/source-api/tests/fixtures/pre_move_comic.json`

**Interfaces:**
- Moves verbatim (fields unchanged, serde attrs unchanged, `specta::Type` behind `specta` feature): `Comic`, `ComicDirFmtParams`, `ChapterInfo`, `SearchResultComic`, `SearchResult`, `DownloadTaskState` (from `downloader/download_task_state.rs`), plus new `pub struct ComicId { pub source_id: String, pub native_id: i64 }`, `pub enum UpdateWindow { Days7, Days15, Days30, All }`, `pub struct ComicSummary { pub comic_id: ComicId, pub title: String, pub cover_url: String, pub status_line: String, pub updated_on: Option<chrono::NaiveDate>, pub rating: Option<f32> }`, `pub struct BrowseResult { pub items: Vec<ComicSummary>, pub page: i64, pub total_pages: i64 }`, `pub struct PageRef { pub url: String, pub headers: std::collections::HashMap<String, String> }`, `pub struct ChapterPages { pub pages: Vec<PageRef>, pub prev_chapter_id: Option<i64>, pub next_chapter_id: Option<i64> }`.
- Methods that take `AppHandle`/parse HTML do NOT move; they become adapter functions (Task 5). `Comic::from_metadata`, `save_metadata`, `update_fields`, fmt-dir helpers move with the struct but take `&CoreConfig` instead of `&AppHandle` (their bodies only read config fields — mechanical).
- Desktop keeps its TS bindings: enable `specta = { workspace = true, optional = true }`, `specta` feature on in src-tauri dep.

- [ ] **Step 1: Capture golden fixtures with the CURRENT build** — before moving anything:

```bash
mkdir -p core/source-api/tests/fixtures
# generate a sample Comic JSON via a tiny one-off test in src-tauri (construct Comic { id: 12345, title: "测试漫画", .. } with representative fields, serde_json::to_string_pretty) and write pre_move_comic.json; same for ChapterInfo → pre_move_chapter_info.json
```

Expected: fixture files committed, generated by pre-move code.

- [ ] **Step 2: Write failing golden test** in `types_golden_test.rs`: `serde_json::from_str::<Comic>(fixture)` round-trips to identical pretty JSON; same for `ChapterInfo`; `DownloadTaskState` serializes to the exact strings the frontend sees today (`"Paused"` etc. — copy current enum serde output).

- [ ] **Step 3: Run** → FAIL.

- [ ] **Step 4: Move structs** per Interfaces; update `src-tauri` to `pub use source_api::{Comic, ChapterInfo, ...};` everywhere (imports change, no logic changes). Add `specta` feature wiring.

- [ ] **Step 5: Run** `cargo test -p source-api` → PASS; `cargo build --workspace` → OK; `pnpm build` (frontend typecheck + vite; bindings.ts must regenerate without diff — run `git diff --exit-code src/bindings.ts` after `pnpm tauri dev`-less generation path used by repo, else skip with note).

- [ ] **Step 6: Commit** `git commit -am "refactor(core): move Comic/ChapterInfo/SearchResult/DownloadTaskState to source-api"`

### Task 4: decrypt + parsers → source-manhuagui

**Files:**
- Create: `core/source-manhuagui/src/decrypt.rs` (moved), `src/parse.rs` (moved `from_html` bodies), `tests/fixtures/chapter_*.html` (real chapter pages)
- Delete: `src-tauri/src/decrypt.rs`; `types/*` parsing code

**Interfaces:**
- Produces: `pub fn parse_comic(html: &str) -> eyre::Result<Comic>` (body = old `Comic::from_html` minus AppHandle config reads — config-driven bits like dir fmt application move to callers or take `&CoreConfig`), `pub fn parse_search(html: &str, page_num: i64) -> eyre::Result<SearchResult>`, `pub fn parse_chapter_list(...)` (whatever `ChapterInfo::from_html*` exists — move all), `pub fn decrypt(html: &str) -> eyre::Result<DecryptResult>` with `DecryptResult` moved here unchanged.

- [ ] **Step 1: Fetch fixtures** (network once, then offline):

```bash
curl -sL -A "Mozilla/5.0" "https://www.manhuagui.com/s/海贼王_p1.html" > core/source-manhuagui/tests/fixtures/search.html
curl -sL -A "Mozilla/5.0" "https://www.manhuagui.com/comic/22992/" > core/source-manhuagui/tests/fixtures/comic.html
# pick a chapter from comic.html, fetch it:
curl -sL -A "Mozilla/5.0" "https://www.manhuagui.com/comic/22992/<chapterId>.html" > core/source-manhuagui/tests/fixtures/chapter.html
```

Trim ad `<script>` blocks if huge; keep the packed `SMH.utils` script intact.

- [ ] **Step 2: Write failing tests**: `decrypt::decrypt(include_str!("fixtures/chapter.html"))` returns expected `bid`/`cid`/non-empty `files`/`path`; `parse_comic` on comic.html returns expected title/chapter count; `parse_search` returns items. Assert exact values read off the fixture.

- [ ] **Step 3: Run** → FAIL (crate empty). **Step 4: Move code** (decrypt.rs verbatim; from_html bodies into free functions; replace `app.get_config()` reads with `&CoreConfig` params). **Step 5:** tests PASS + workspace builds. **Step 6: Commit** `refactor(core): move decrypt and html parsers to source-manhuagui`

### Task 5: ManhuaguiClient → source-manhuagui (AppHandle-free)

**Files:**
- Create: `core/source-manhuagui/src/client.rs`; move `types/{comic_info? remaining}` as needed
- Modify: `src-tauri/src/manhuagui_client.rs` → delete; `src-tauri/src/commands.rs`, `downloader/*`, `export/*` call sites switch to `source_manhuagui::ManhuaguiClient` (see Interfaces)

**Interfaces:**
- `pub struct ManhuaguiClient { api_client: Arc<RwLock<ClientWithMiddleware>>, img_client: Arc<RwLock<ClientWithMiddleware>> }`
- `ManhuaguiClient::new(config: &CoreConfig)`; `reload_client(&self, config: &CoreConfig)` (replaces `reload_client(&self, app)`)
- Methods verbatim except: no `AppHandle`; `get_img_urls(&self, chapter_info: &ChapterInfo) -> eyre::Result<Vec<PageRef>>` (builds `https://i.hamreus.com{path}{file}` + `headers: {"Referer": "https://www.manhuagui.com/"}`); keeps `login`, `get_user_profile`, `get_favorite` (site knowledge; desktop-only) with their result types moved here.
- Timeout values read from `CoreConfig.api_timeout_sec`/`img_timeout_sec` (callers pass desktop 3/3 → behavior unchanged).
- Shell holds `Arc<ManhuaguiClient>` in Tauri state, rebuilt on config save (same as today's `reload_client`).

- [ ] **Step 1: Failing test** — `get_img_urls` against chapter fixture: mock transport not available in repo; test the URL assembly as a pure fn `pub fn img_url(path: &str, file: &str) -> PageRef` → exact url + referer header. 
- [ ] **Step 2: Run** → FAIL. **Step 3: Move client**; add `img_url` helper used by `get_img_urls`. Rewire shell call sites (`commands.rs`, `downloader`, `export`): they obtain client from Tauri state; `app.get_config()` stays in shell, pass `&config` into client fns. **Step 4: `cargo test -p source-manhuagui` PASS; `cargo build --workspace` OK.** 
- [ ] **Step 5: Desktop QA checkpoint [human]**: run `pnpm tauri dev` — login, search, comic detail, favorites list still work. **Step 6: Commit** `refactor(core): ManhuaguiClient moves to source-manhuagui`

### Task 6: list.rs + `Source` impl (browse, windows)

**Files:**
- Create: `core/source-api/src/source.rs` (trait), `core/source-manhuagui/src/list.rs`, `src/list_source.rs` (ManhuaguiSource)
- Test: `core/source-manhuagui/tests/list_test.rs`, fixtures `list_japan.html`, `list_japan_update.html`, `list_japan_update_p2.html`

**Interfaces:**
- `source-api`: `#[async_trait::async_trait] pub trait Source: Send + Sync { fn id(&self) -> &'static str; async fn search(&self, keyword: &str, page: i64) -> Result<SearchResult, SourceError>; async fn browse_latest(&self, window: UpdateWindow, page: i64, genre: Option<&str>) -> Result<BrowseResult, SourceError>; async fn get_comic(&self, native_id: i64) -> Result<Comic, SourceError>; async fn get_chapter_pages(&self, native_id: i64, chapter_id: i64) -> Result<ChapterPages, SourceError>; }`
- `list.rs`: `pub fn parse_list_page(html: &str) -> eyre::Result<BrowseResult>` — selector: `ul#contList > li`; per li: comic id from `a.bcover[href="/comic/{id}/"]`, title from `a[title]`, cover from `img[src|data-src]` (`//cf.mhgui.com/...` → prefix `https:`), status_line from `span.tt`, `updated_on` from `span.updateon` text `更新于：YYYY-MM-DD` (Asia/Shanghai naive date; missing → None), rating from `span.updateon > em` (parse f32; non-numeric → None); total pages from pager `.pager` last page link `index_p{n}.html`.
- `ManhuaguiSource { client: ManhuaguiClient }` implements `Source`; `browse_latest` URL: genre `None` → `/list/japan/update.html` (p1) / `/list/japan/update_p{n}.html`? — verify real pattern from fixture pager links (`index_p{n}.html` suffix with `/list/japan/` base); genre `Some(g)` → `/list/japan_{g}/update.html`. Window filtering is the caller's job (LibraryService, Task 9) — `browse_latest` returns the raw page; BUT it must also mark when a page contains only entries outside a window: implement `pub fn page_outside_window(items: &[ComicSummary], window: UpdateWindow, today: NaiveDate) -> bool` (true when ALL items' `updated_on` are older than window cutoff, `None` treated as outside).
- Adapter maps its eyre errors to `SourceError`: HTTP status 404 → NotFound; timeout/connect → Network; else message passthrough (RateLimited not distinguishable today — map via future need).

- [ ] **Step 1: Fetch fixtures**: `/list/japan/`, `/list/japan/update.html`, page 2 of update list (follow its pager link).
- [ ] **Step 2: Failing tests**: `parse_list_page` on list_japan.html → ≥ 20 items, first item id/title/status exact, updated_on == fixture's first date, rating parsed, total_pages == 1263-ish (read from fixture); entry with missing `更新于` (hand-trim one in a fixture copy) → `updated_on: None`, no error; `page_outside_window`: cutoff inclusive (date == today - 7 days is INSIDE Days7); `All` never outside; all-None page outside any Days window.
- [ ] **Step 3: Run → FAIL.** **Step 4: Implement trait + list.rs + ManhuaguiSource** (`get_chapter_pages` = decrypt + `img_url` mapping, prev/next from `DecryptResult.prev_id/next_id`, `0` → `None`). **Step 5: Tests PASS; workspace builds.** **Step 6: Commit** `feat(core): /list/ browsing, UpdateWindow, Source impl for manhuagui`

### Task 7: EventObserver + downloader → library

**Files:**
- Create: `core/source-api/src/events.rs`, `core/library/src/downloader.rs` (moved manager/task/img_task), `core/library/src/lib.rs` wiring
- Modify: `src-tauri/src/events.rs` (From impls), `src-tauri/src/commands.rs` (rewire), delete `src-tauri/src/downloader/`
- Test: `core/library/tests/events_golden_test.rs`

**Interfaces:**
- `source-api`: `pub struct LibraryEvent { pub name: &'static str, pub payload: serde_json::Value }` and `pub trait EventObserver: Send + Sync { fn on_event(&self, event: &LibraryEvent); }`. Names: `download-speed`, `download-sleeping`, `download-task-create`, `download-task-delete`, `download-task-update` — **use the exact event-name strings the tauri_specta `Event` derive produces today** (read them from the generated `bindings.ts` / tauri-specta docs before coding; if the derive uses the Rust variant path, copy that verbatim).
- Moved `DownloadManager::new(config: Arc<RwLock<CoreConfig>>, client: Arc<ManhuaguiClient>, observer: Arc<dyn EventObserver>)`; `create_download_task(&self, comic: Comic, chapter_id: i64)` — internally fetches pages via client then proceeds as today. All `ev.emit(&self.app)` → `self.observer.on_event(&LibraryEvent { name, payload: serde_json::to_value(payload)? })`.
- Desktop `events.rs` keeps every tauri_specta type; adds `pub struct CoreObserver(AppHandle)` implementing `EventObserver` by `match name { ... }` → deserialize payload → `event.emit(&app)`. Payload structs identical ⇒ wire bytes identical.
- `DownloadTask`/`DownloadImgTask` keep their logic; their `AppHandle` field becomes `{ config: Arc<RwLock<CoreConfig>>, client, observer }`.

- [ ] **Step 1: Golden event test (failing)** — construct each `LibraryEvent` variant the way the moved code will (Speed `{"speed":"1.2 MB/s"}`, TaskUpdate `{"state":"Downloading","chapterId":123,...}` camelCase), assert `serde_json::to_string(&desktop_event_after_from)` equals the golden strings captured today from `bindings.ts`/events.rs serde attrs. Capture goldens first as fixtures (`events_golden_test.rs` constants).
- [ ] **Step 2: Run → FAIL.** **Step 3: Move downloader** with the two mechanical replacements; rewire `commands.rs` (create/pause/resume/delete/get_downloaded_comics) — shell now constructs `DownloadManager` in Tauri state with `CoreObserver(app.clone())`. **Step 4: `cargo test -p source-api -p library` PASS; workspace builds; `cargo check -p manhuagui-downloader` clean.**
- [ ] **Step 5: Desktop QA checkpoint [human]**: download a chapter (pause, resume, delete mid-download; watch speed events in UI); verify downloaded dir has `元数据.json` + `章节元数据.json` + images; `DownloadedPane` shows it; `UpdateDownloadedComics` button works.
- [ ] **Step 6: Commit** `refactor(core): downloader to library with EventObserver`

### Task 8: Exporters → library

**Files:**
- Create: `core/library/src/export.rs`, `core/library/src/export/{cbz,pdf}.rs` (moved)
- Modify: `src-tauri/src/commands.rs` export commands rewire; delete `src-tauri/src/export*`

**Interfaces:**
- `pub fn cbz(config: &CoreConfig, observer: &dyn EventObserver, comic: &Comic) -> eyre::Result<()>`; `cbz_chapters(..., chapter_ids: Vec<i64>)`; `pdf`, `pdf_chapters` likewise. `ComicExportLock` moves verbatim. Export events (cbz/pdf start/progress/error/end, pdf merge*) flow through the same `LibraryEvent` mechanism with today's names.
- Everything dir/export-skip/merge-pdf reads `CoreConfig` (shell converts via `From<&Config> for CoreConfig` — add this impl in src-tauri now).

- [ ] **Step 1: Failing test** — none feasible without golden images; instead test `From<&Config> for CoreConfig` maps every field (property test over a filled sample). 
- [ ] **Step 2: FAIL.** **Step 3: Move + rewire; add From impl.** **Step 4: Tests PASS; workspace builds.**
- [ ] **Step 5: Desktop QA [human]**: export one downloaded comic to CBZ and PDF (incl. merge-pdf on, skip modes), files open correctly.
- [ ] **Step 6: Commit** `refactor(core): exporters to library`

### Task 9: Registry + LibraryService + CI

**Files:**
- Create: `core/library/src/service.rs`, `core/library/tests/service_test.rs` (MockSource), `.github/workflows/core-ci.yml`
- Modify: `README.md` (one paragraph: core layout), root workspace docs

**Interfaces:**
- `pub struct Library { sources: parking_lot::RwLock<HashMap<String, Arc<dyn Source>>> }` with `register_source(&self, s: Arc<dyn Source>)`, `source(&self, id: &str) -> Option<Arc<dyn Source>>`.
- `pub async fn browse_within(source: &dyn Source, window: UpdateWindow, start_page: i64, max_pages: i64) -> Result<Vec<ComicSummary>, SourceError>` — pages `start_page..` sequentially until `page_outside_window` or `max_pages` (default cap 50) or last page; concatenates items; `All` → single page result. This is the 7/15/30-day engine Android calls.
- `MockSource` in tests implements `Source` over static fixture pages; test: window Days7 with fixtures where p1 inside, p2 outside → returns p1 items only, fetched exactly 2 pages; `All` returns one page; missing-date items dropped from windowed results but kept in `All`.

- [ ] **Step 1: Failing tests** (above). **Step 2: FAIL.** **Step 3: Implement `Library` + `browse_within`.** **Step 4: PASS; workspace builds; `cargo clippy --workspace` clean.**
- [ ] **Step 5: CI** — GitHub Actions on push/PR: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `cargo build -p manhuagui-downloader`. Note in workflow: tests are network-free (fixtures only).
- [ ] **Step 6: Tag + commit**: `git commit -am "feat(core): Library registry and windowed browse; CI"`, `git tag v0.1.0-core`, push branch + tag to `fork`.

## Post-plan (not in scope here)

Android phases 1–3 (per spec §Phasing) get their own plans against the `v0.1.0-core` tag: UniFFI surface over `Library`/`ManhuaguiClient`, then app skeleton, reader, bookmarks, offline, export.
