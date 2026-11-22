# manhuagui e-reader for BOOX — design

**Date:** 2026-11-22
**Status:** pending approval
**Repos affected:** fork of `lanyeeee/manhuagui-downloader` (core extraction) + new `manhuagui-android` app repo

## Intent

Read manhuagui.com's Japanese manga on a BOOX Leaf 5 / 5C (and other Android e-ink readers) with an e-ink-first experience: discrete page turns, physical buttons, black-and-white UI, no animation, instant turns via prefetch. Stream pages live as the primary mode; download chapters for offline as a per-chapter user preference; bookmark comics locally and get a notification when they update; export downloaded content to CBZ/PDF. The desktop downloader's scraper/downloader/exporter becomes a shared Rust core so site fixes land once for both apps. The core is a **source-agnostic service with adapters**: manhuagui is the first adapter, and another manga source can replace or join it without touching the app UI or the service layer.

## Why

- The user reads on a BOOX Leaf 5/5C. The site's mobile web experience is hostile to e-ink: scroll-heavy, animated, ad-laden.
- The desktop repo already solved the hard parts — HTML scraping, chapter image decryption (p.a.c.k.e.r unpack), concurrent downloading, CBZ/PDF export. Reimplementing in Kotlin invites drift; sharing a core keeps one source of truth for site quirks.
- The user browses manga by "what updated recently in the Japan list" — hence first-class 7/15/30-day update windows.

## Goals

1. **Browse**: Japanese comics (`/list/japan/`), sorted by latest update, with client-side filter "updated within last 7 / 15 / 30 days". Genre filters are **not in the v1 UI** (window + search cover discovery; `browse_latest` carries an optional genre parameter so adding chips later touches no core code).
2. **Search**: keyword search (`/s/{kw}_p{n}.html`).
3. **Comic detail**: cover, metadata, chapter list, bookmark toggle, download controls.
4. **E-ink reader**: one page at a time, RTL page order by default (Japanese manga) with LTR toggle; tap zones for next/prev/menu; pinch and double-tap zoom with pan; physical page-turn buttons and volume keys; keep-screen-on while reading; prefetch upcoming pages; resume where you left off.
5. **Bookmarks**: local only (no site login). Periodic background check for new chapters; Android notification when any bookmarked comic updated; tapping it opens the bookmarks/updates screen.
6. **Streaming primary, offline optional**: pages stream live from `i.hamreus.com` (Referer header). Any chapter can be downloaded for offline; the reader transparently uses local files when present, network otherwise.
7. **Export**: downloaded chapters/comics export to CBZ or PDF via the Rust exporter, saved through the Storage Access Framework.
8. **Desktop app keeps working** off the same extracted core.

## Non-goals (v1)

- Site login / server-side favorites sync (bookmarks are local)
- iOS, or a reading UI on desktop
- Sources other than manhuagui
- Automatic downloading of new chapters on update (notify only)
- Webtoon / vertical-continuous reader mode
- Image tone processing (contrast boost etc.) — BOOX system controls cover it
- Proxy support in the Android UI (core retains the capability; surface later if wanted)
- Dynamic, runtime-loaded source plugins — sources are compiled-in adapters; adding one is a code change
- A second source implementation in v1 — the seam exists but stays unexercised until a real second source arrives

## Anti-goals

- Anything scroll- or animation-dependent — they ghost and lag on E Ink
- Requiring network to read downloaded chapters
- Two divergent scrapers (desktop vs Android) — all scraping lives in the shared core
- Any account requirement to use the app

## Constraints

- **Hardware**: BOOX Leaf 5 (7" 300ppi mono, physical page-turn buttons, ~3GB RAM, Android 12) and Leaf 5C (Kaleido 3 color). Must also run acceptably on ordinary Android phones/tablets. `minSdk 29`, `targetSdk 35`.
- **Images**: `i.hamreus.com` requires `Referer: https://www.manhuagui.com/` or it 403s.
- **No native date filter**: the site has no "updated within N days" filter. The app pages `/list/japan/update.html` (descending update date) and stops paging when entries leave the window. Dates come from `更新于：YYYY-MM-DD` spans.
- **Memory**: ~3GB device; decoded manga pages are 10–20MB each. Bitmap cache must be screen-sized, bounded, and drop distant pages.
- **Tauri coupling**: `AppHandle` (config access) and `.emit()` (progress events) appear across ~16 files; both must be replaced with plain Rust (config struct, callback trait) inside the core, with the desktop Tauri shell adapting.
- **Language**: UI English; site content (titles, chapter names) displayed as-is (Chinese/Japanese).
- **Rate limiting**: bookmark update checks must fetch comic pages sequentially with small delays, not in a burst.

## Approaches considered

1. **A — Pure native (Kotlin + Compose), port the scraper.** Full e-ink control, no FFI complexity, but duplicates ~1,200 lines of scraper/decrypt and ~700 lines of downloader/exporter logic; site fixes applied twice forever.
2. **B — Tauri 2 mobile (reuse all Rust, WebView UI).** Maximal reuse, but every e-ink-critical feature (physical key events, background polling + notifications, per-request Referer, bitmap memory control) needs Kotlin Android glue around a WebView that fights e-ink.
3. **A+B hybrid — native Compose UI + Rust core via FFI (chosen).** UI/reader/notifications/keys in Kotlin where the platform demands it; scraping/decrypt/downloader/exporter in shared Rust where the proven logic lives. Costs: one-time refactor to strip Tauri from the core, permanent two-toolchain build (cargo-ndk + Gradle), FFI debugging. Rejected alternatives because: A wastes the proven logic and drifts; B forces the hard parts through IPC into a WebView.

## Decisions (made with the user)

- Hybrid A+B, as above.
- Repo layout: **fork** `lanyeeee/manhuagui-downloader` (user actively uses the desktop app), extract `core/`, keep the Tauri shell working on top of it. New repo `manhuagui-android` for the app, depending on the fork's core by git tag. (Within the fork, the desktop shell depends on `core/` by path — that's the diagram's "path dep" label; the Android repo's dependency is the git tag.)
- Streaming primary; offline is a per-chapter download action (user preference).
- E-ink optimization is a first-class requirement, not a stretch goal.
- Local bookmarks, no login, ever (v1 at least).
- Reader default RTL, toggle to LTR.
- English UI.
- Export both CBZ and PDF.
- **Rust core is a service behind a source-agnostic interface; manhuagui is an adapter.** Static registry of compiled-in sources; the app and the downloader/exporter never see manhuagui-specific types.
- Downloads are durable **while the app is alive**: a foreground service keeps them running with screen off or app switched away; swiping the app away stops them; interrupted chapters re-download on next open (no partial resume, matching desktop behavior). No persisted queue in v1.
- Reader has pinch/double-tap zoom + pan from v1; pages decode at full source resolution.

## Architecture

```
┌──────────────────────────────────┐    ┌───────────────────────────────┐
│ manhuagui-android (Kotlin)       │    │ manhuagui-downloader fork     │
│                                  │    │ (desktop Tauri shell)         │
│ UI: Compose (no animations)      │    │ commands.rs adapts to core    │
│  Latest / Search / Detail /      │    └──────────────┬────────────────┘ │
│  Reader / Bookmarks / Downloads/ │                   │ path dep         │
│  Settings                        │                   ▼                  │
│ Coil+OkHttp (per-page headers,   │    ┌───────────────────────────────┐
│  cache)                          │    │ core/source-api (Rust traits) │
│ Room (bookmarks, progress,       │    │ core/source-manhuagui         │
│  download index), DataStore      │    │  (adapter: client, decrypt,   │
│ WorkManager (update checks →     │    │   list, types)                │
│  notifications)                  │    │ core/library (registry,       │
│ UniFFI LibraryService ───────────┼───▶│  downloader, exporter)        │
└──────────────────────────────────┘git │                               │
                                      dep└───────────────────────────────┘
```

### Core extraction (fork repo)

The core is split by role, not by site:

- **`core/source-api`** — the service layer: domain types (`ComicId = {sourceId, nativeId}`, `ComicSummary`, `ComicDetail`, `Chapter`, `PageRef = {url, headers}`, `UpdateWindow`, errors) and the `Source` trait: `search`, `browse_latest(window, page)`, `get_comic`, `get_chapter_pages(chapterId)`. No site specifics. Header requirements (Referer etc.) travel inside `PageRef` so the downloader and the Kotlin image loader stay source-agnostic. Errors are a structured enum in `source-api` (`Network | RateLimited | ParseFailed | NotFound | Cancelled` + message): the desktop shell renders `message` exactly as today (behavior unchanged); Android maps the kind to English text with the message as detail.
- **`core/source-manhuagui`** — the adapter: everything site-specific moves here from `src-tauri/src/`: `manhuagui_client.rs` (minus `AppHandle` — takes a plain config struct with cookie/proxy/UA), `decrypt.rs` (p.a.c.k.e.r unpack; already Tauri-free), the needed subset of `types/*` (comic, chapter, search), plus **new** `list.rs` parsing `/list/japan/` and `/list/japan/update.html` (id, title, cover URL, status line, `更新于` date, rating) with client-side window filtering. Implements `Source`.
- **`core/library`** — source registry (compiled-in list, looked up by `sourceId`), the downloader (moved from `downloader/*`, operating on `PageRef`s; progress `emit` → `trait DownloadObserver`), and the exporter (`export/*`: CBZ, PDF — already source-agnostic). The downloader keeps its fmt-driven directory layout and `元数据.json`/`章节元数据.json` metadata files **unchanged** (desktop-identical behavior); Android passes fmt strings that yield `files/downloaded/{comicId}/{chapterId}/`. Those metadata files are the filesystem source of truth for download state; Room holds a query-friendly index reconciled by a scan on app open (app-specific storage isn't user-editable on Android 11+, so drift is internal and detectable). No storage abstraction layer in v1.
- Desktop `src-tauri` keeps `commands.rs`, `config.rs`, `logger.rs`, login/favorites types, Tauri glue; implements `DownloadObserver` by re-emitting Tauri events, so desktop behavior is unchanged.
- Exposed to Android via **UniFFI** as one `LibraryService` object: methods take a `sourceId` (v1: always `"manhuagui"`), plus async methods (search, browse, get_comic, get_chapter_pages, download_chapter, export, cancel) and an observer callback interface for progress. The Kotlin side never learns which site it's talking to.

### Android app (`manhuagui-android`)

- **Build**: Gradle + `cargo-ndk` (or the `rust` Gradle plugin), targets `aarch64-linux-android` (+ `x86_64` for emulator). Kotlin 2.x, Compose (BOM current), single `:app` module plus `:core-rust` glue. minSdk 29, targetSdk 35.
- **Network (streaming)**: Coil `ImageLoader` over OkHttp; each `PageRef` carries its own headers (Referer for manhuagui) applied per-request via Coil. Memory cache ≈ 25% of app heap; disk cache bounded (default 512MB, configurable). Reader pages decode at full source resolution (zoom needs it); covers and thumbnails downsample to view size. Network parameters (timeouts, retries, concurrency, inter-request pacing) live in the core config struct: Android keeps the desktop's anti-ban pacing verbatim but raises timeouts for mobile networks (~15s connect / ~30s read vs the desktop's 3s). No network knobs in v1 Settings.
- **Reader**: `HorizontalPager` with snap, `userScrollEnabled` on (swipe) but zero animation/crossfade; tap zones overlay (prev 40% / next 40% / center 20% → chrome menu), mirrored automatically in RTL; pinch + double-tap zoom with pan (full-res decode, memory bounded by dropping distant pages); `dispatchKeyEvent` handles `KEYCODE_PAGE_UP/DOWN`, `KEYCODE_VOLUME_UP/DOWN`, and BOOX page-turn keycodes; `FLAG_KEEP_SCREEN_ON` while reader open; prefetch next 3 pages (direction-aware) and drop pages >2 away; progress (comicId, chapterId, pageIndex) saved to Room on every page turn; end-of-chapter → "next chapter" affordance using `nextId` from the decrypted payload.
- **Browse**: Latest screen = `/list/japan/update.html` paged until dates exit the chosen window (7/15/30 chips + "all"); search screen; detail screen with chapter list (reverse order toggle), bookmark star, download button per chapter (+ "download all unread").
- **Bookmarks & notifications**: Room table keyed by `(sourceId, comicId)` (title, cover, lastKnownChapterIds, lastChecked, updatedAt). WorkManager periodic check (default every 6h, configurable 1/6/12/24h, plus on-app-open) → for each bookmark sequentially (1–2s jitter) fetch comic page via core → diff chapter ids → update Room → if new chapters: post one summary notification ("3 manga updated — One Piece ch. 1101…"), tap → Bookmarks screen with update badges. In-app hint if battery optimization is killing background work.
- **Offline**: Rust downloader fetches page images into app files dir (`files/downloaded/{comicId}/{chapterId}/`), indexed in Room and reconciled against the core's metadata files on app open; reader prefers local files; download UI shows per-chapter progress via observer; delete per chapter/comic. Downloads run under a **foreground service** (notification with progress): they survive screen-off and app switching, stop when the app is swiped away, and interrupted chapters restart from scratch on next open — no persisted queue in v1.
- **Export**: pick comic or chapters → Rust exporter writes CBZ/PDF into app cache → Kotlin streams the copy to the user-picked SAF `content://` Uri. The exporter stays path-based (no sink refactor in v1); accepted cost: transient ~2× disk during export. Android always exports fresh to the picked destination — the desktop's skip modes and `is_exported` flags are desktop-only.
- **Settings** (DataStore): reading direction (RTL default), window default (7/15/30/all), check interval, offline-only mode toggle (reader warns/asks before streaming), prefetch count, cache size.
- **E-ink style**: pure black-on-white, 1px black borders, no elevation/shadows/ripples/crossfades; dark mode = white-on-black via palette swap; system font; large tap targets; no splash animation.

## Desktop → Android adaptation map

The desktop app is downloader-centric (scrape → files on disk → export CBZ → read elsewhere); the Android app composes the same engine reader-first (scrape → render on screen; disk optional). Concretely:

| Desktop (today) | Android (adapts to) |
|---|---|
| `commands.rs` Tauri commands (search, get_comic, get_img_urls…) | Same command surface re-exposed as `LibraryService` via UniFFI (the repo already generates TS bindings via tauri-specta; UniFFI is the Kotlin analogue) |
| `get_img_urls` (scrape chapter + p.a.c.k.e.r decrypt) | Unchanged, moved to `source-manhuagui`, returns `PageRef{url, headers}` |
| `get_img_bytes` — every page through Rust to disk | Split: streaming pages never touch Rust (Coil/OkHttp + PageRef headers, screen-sized decode, cache); only offline downloads use the Rust downloader |
| `downloader/` + `emit` speed/progress | Same engine; `DownloadObserver` trait → Kotlin Flow (desktop re-emits Tauri events) |
| `export/cbz.rs`, `pdf.rs` → chosen folder | Same code; SAF destination; reuses downloaded files or downloads-then-exports |
| `config.rs` (Tauri store) | Plain `CoreConfig` struct, persisted in DataStore on Android |
| Login + site favorites | Dropped — local bookmarks in Room |
| — | New: `/list/` browse + update windows (`list.rs`), reading-position memory, WorkManager checks + notifications |

## Phasing

0. **Core extraction** — fork, extract `core/`, desktop app builds and behaves identically (manual QA checklist: search, chapter decrypt+download, export, favorites login, settings).
1. **Android skeleton** — Gradle+cargo-ndk, UniFFI object, Latest (window-filtered), Search, Detail, minimal Reader (tap zones, streaming, RTL).
2. **Reader & bookmarks** — keys/buttons, prefetch, progress resume, keep-screen-on; bookmarks + WorkManager checks + notifications.
3. **Offline & export** — Rust downloader wired to UI, offline-first reader path, export CBZ/PDF, settings complete.

Each phase ends in a runnable, verifiable state.

## Risks

- **Refactor regressions in the desktop app**: downloader/exporter event plumbing changes. Mitigate: keep observer events shape-identical to current Tauri events; manual QA checklist per release; core unit tests for decrypt (fixture from a real chapter HTML) and list parsing before the Android work starts.
- **UniFFI async/callback ergonomics**: long-running downloads with progress over FFI. Mitigate: observer callback interface, throttle progress to ~4Hz; run blocking core calls on Dispatchers.IO.
- **BOOX background restrictions**: aggressive battery management can kill WorkManager. Mitigate: onboarding step requesting exemption + docs; on-open refresh as fallback.
- **CDN client discrimination**: `i.hamreus.com` might treat OkHttp's TLS/client profile differently than desktop reqwest. Verify in phase 1; escape hatch: route streaming through the core (Rust fetches bytes, Coil decodes).
- **Site changes**: single core fix; but images CDN or p.a.c.k.e.r format changes break both apps simultaneously until fixed (accepted trade of sharing).
- **Over-abstraction**: the `Source` trait is designed from one implementation. Mitigate: keep the trait minimal (the four operations the app needs), expect to reshape it when a real second source arrives, and never let site specifics leak into `source-api`/`library`.
- **RAM pressure on 3GB devices**: bounded cache, page dropping, full-res decode only for the visible reader page; test with 200-page chapters.

## What's left to the builder

- Exact UniFFI interface signatures and error mapping (core `eyre` errors → sealed Kotlin result types)
- `Source` trait shape in Rust (async trait objects) and how `PageRef` headers flow through Coil per-request
- Room schema DDL and DAOs (with `sourceId` in every content key)
- Compose navigation graph and per-screen state holders
- `/list/` page parsing specifics (`list.rs`): selector paths for cover/title/status/date/rating, pagination termination
- Update-date parsing (`更新于：YYYY-MM-DD`, site timezone assumption = Asia/Shanghai)
- Gradle/NDK/UniFFI build wiring and CI (GitHub Actions: Rust test + assembleDebug + desktop `cargo build` on the fork)
- On-device QA checklist for Leaf 5 (buttons, ghosting, RAM, battery) — user runs this
- App name/icon, notification channel design
