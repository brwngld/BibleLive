use crate::content::import;
use crate::content::model::{ContentItem, ContentSummary, ItemType, LabeledSection, SearchHit};
use crate::content::ContentStore;
use serde::{Deserialize, Serialize};
use tauri::State;

use rusqlite::params;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}! Welcome to BibleLive.")
}

// ---- Status ---------------------------------------------------------------

#[derive(Serialize)]
pub struct AppStatus {
    pub version: String,
    pub build: String,
    pub edition: String,
    pub database_ready: bool,
    pub engines: EnginesStatus,
}

#[derive(Serialize)]
pub struct EnginesStatus {
    pub content: &'static str,
    pub display: &'static str,
    pub audio: &'static str,
    pub intelligence: &'static str,
}

#[tauri::command]
pub fn app_status(store: State<'_, ContentStore>) -> AppStatus {
    AppStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: crate::fmt_utc(env!("BL_BUILD_SECS").parse().unwrap_or(0)),
        edition: "community-pro".to_string(),
        database_ready: store.is_ready(),
        engines: EnginesStatus {
            content: "ready",
            display: "planned (M2)",
            audio: "planned (M3)",
            intelligence: "planned (M4)",
        },
    }
}

/// Open the data folder (%APPDATA%\BibleLive) in Windows Explorer —
/// support shortcut for crash.log and the database.
#[tauri::command]
pub fn open_data_folder() -> Result<(), String> {
    let dir = crate::content::data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ---- Library --------------------------------------------------------------

#[tauri::command]
pub async fn library_stats(
    store: State<'_, ContentStore>,
) -> Result<serde_json::Value, crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || s.stats())
        .await
        .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub item_type: Option<String>,
    pub title_query: Option<String>,
    /// "ot" | "nt" — applies to Bible items only.
    pub testament: Option<String>,
    /// "title-asc" (default) | "title-desc" | "added-desc" | "added-asc" | "canonical"
    pub sort: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    200
}

#[tauri::command]
pub async fn list_content(
    store: State<'_, ContentStore>,
    filter: ListFilter,
) -> Result<Vec<ContentSummary>, crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let item_type = filter.item_type.as_deref().and_then(parse_item_type);
        s.list_items(
            item_type.as_ref(),
            filter.title_query.as_deref(),
            filter.testament.as_deref(),
            filter.sort.as_deref().unwrap_or("title-asc"),
            filter.limit,
            filter.offset,
        )
    })
    .await
    .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

fn parse_item_type(s: &str) -> Option<ItemType> {
    match s {
        "bible" => Some(ItemType::Bible),
        "song" => Some(ItemType::Song),
        "hymn" => Some(ItemType::Hymn),
        "book" => Some(ItemType::Book),
        "document" => Some(ItemType::Document),
        _ => None,
    }
}

#[tauri::command]
pub async fn get_content(
    store: State<'_, ContentStore>,
    id: String,
) -> Result<ContentItem, crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        s.get_item(&id)?
            .ok_or_else(|| crate::content::ContentError::Other(format!("item {} not found", id)))
    })
    .await
    .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn create_content(
    store: State<'_, ContentStore>,
    item: ContentItem,
) -> Result<(), crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || s.insert_item(&item))
        .await
        .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn update_content(
    store: State<'_, ContentStore>,
    item: ContentItem,
) -> Result<(), crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || s.update_item(&item))
        .await
        .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn delete_content(
    store: State<'_, ContentStore>,
    id: String,
) -> Result<(), crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || s.delete_item(&id))
        .await
        .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn search_content(
    store: State<'_, ContentStore>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SearchHit>, crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || s.search(&query, limit.unwrap_or(50)))
        .await
        .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

// ---- Import ---------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextImportInput {
    pub title: String,
    pub item_type: String,
    pub text: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_license")]
    pub license: String,
}

fn default_language() -> String {
    "en".to_string()
}
fn default_license() -> String {
    "unknown".to_string()
}

#[tauri::command]
pub async fn import_text_content(
    store: State<'_, ContentStore>,
    input: TextImportInput,
) -> Result<ContentItem, crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let item_type = parse_item_type(&input.item_type)
            .filter(|t| *t != ItemType::Bible)
            .unwrap_or(ItemType::Document);
        let item = import::import_text(
            new_id(&input.item_type),
            input.title,
            item_type,
            &input.text,
            &input.language,
            &input.license,
        )?;
        s.insert_item(&item)?;
        Ok(item)
    })
    .await
    .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileImportInput {
    pub path: String,
    pub title: Option<String>,
    pub item_type: Option<String>,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_license")]
    pub license: String,
}

#[tauri::command]
pub async fn import_file_content(
    store: State<'_, ContentStore>,
    input: FileImportInput,
) -> Result<ContentItem, crate::content::ContentError> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::PathBuf::from(&input.path);
        let file_title = input.title.clone().unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported content")
                .to_string()
        });
        let item_type = input
            .item_type
            .as_deref()
            .and_then(parse_item_type)
            .filter(|t| *t != ItemType::Bible)
            .unwrap_or(ItemType::Document);
        let item = import::import_file(
            new_id(&item_type.as_str()),
            file_title,
            item_type,
            &path,
            &input.language,
            &input.license,
        )?;
        s.insert_item(&item)?;
        Ok(item)
    })
    .await
    .map_err(|e| crate::content::ContentError::Other(e.to_string()))?
}

// ---- Display Engine ---------------------------------------------------------

use crate::display::{self, DisplayManager, DisplayMode, RenderKind, SectionedContent};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub is_primary: bool,
}

#[tauri::command]
pub fn list_monitors(app: tauri::AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let mut out = Vec::new();
    for (i, m) in app
        .available_monitors()
        .map_err(|e| e.to_string())?
        .iter()
        .enumerate()
    {
        out.push(MonitorInfo {
            name: m
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("Monitor {}", i + 1)),
            width: m.size().width,
            height: m.size().height,
            x: m.position().x,
            y: m.position().y,
            is_primary: app
                .primary_monitor()
                .ok()
                .flatten()
                .map(|p| p.name() == m.name())
                .unwrap_or(i == 0),
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn get_display_slots(app: tauri::AppHandle, mgr: State<'_, DisplayManager>) -> Vec<display::SlotView> {
    let mut views = mgr.slots();
    for v in &mut views {
        v.window_open = app
            .get_webview_window(&format!("display-{}", v.slot))
            .is_some();
    }
    views
}

#[tauri::command]
pub fn set_slot_monitor(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    slot: u8,
    monitor: Option<String>,
) -> Result<(), String> {
    if !(1..=5).contains(&slot) {
        return Err("slot must be 1-5".into());
    }
    mgr.set_monitor(slot, monitor);
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

#[tauri::command]
pub fn set_slot_mode(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    slot: u8,
    mode: String,
) -> Result<(), String> {
    let mode = match mode.as_str() {
        "auto" => DisplayMode::Auto,
        "manual" => DisplayMode::Manual,
        "lock" => DisplayMode::Lock,
        other => return Err(format!("unknown mode: {other}")),
    };
    mgr.set_mode(slot, mode);
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

#[tauri::command]
pub fn set_slot_blank(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    slot: u8,
    blank: bool,
) -> Result<(), String> {
    mgr.set_blank(slot, blank);
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

/// Open (or re-attach) the fullscreen output window for a slot, positioned
/// on its target monitor. Falls back to the primary monitor if the target
/// is missing — the service keeps running either way.
///
/// MUST be async: creating a webview window from a sync command deadlocks
/// on Windows (WebView2 initialization blocks the main thread).
#[tauri::command]
pub async fn open_slot_output(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    slot: u8,
) -> Result<(), String> {
    if !(1..=5).contains(&slot) {
        return Err("slot must be 1-5".into());
    }
    eprintln!("[display] open_slot_output slot={slot}");
    let label = format!("display-{slot}");

    if let Some(existing) = app.get_webview_window(&label) {
        eprintln!("[display] window exists, showing");
        let _ = existing.show();
        let _ = existing.set_focus();
        display::emit_slot(&app, slot, &mgr);
        return Ok(());
    }

    // Resolve the target monitor (fallback: primary).
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let target_name = mgr.slots()[(slot - 1) as usize].monitor.clone();
    let mut degraded = false;
    let target = target_name
        .as_ref()
        .and_then(|name| monitors.iter().find(|m| m.name().map(|n| n == name).unwrap_or(false)));
    let monitor = match target {
        Some(m) => m.clone(),
        None => {
            degraded = target_name.is_some(); // had a target, but it's gone
            app.primary_monitor()
                .ok()
                .flatten()
                .or_else(|| monitors.first().cloned())
                .ok_or("no monitors found")?
        }
    };
    eprintln!("[display] target monitor: {:?}", monitor.name());

    let pos = monitor.position();
    let size = monitor.size();

    // Create hidden first: an immediately-fullscreened visible window can
    // hit a WebView2 composition bug that paints white forever. Showing
    // after layout forces a clean first paint.
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title(format!("BibleLive Display {slot}"))
    .decorations(false)
    .resizable(false)
    .visible(false)
    .position(pos.x as f64, pos.y as f64)
    .inner_size(size.width as f64, size.height as f64)
    .build()
    .map_err(|e| format!("create output window: {e}"))?;

    let _ = window.set_fullscreen(true);
    let _ = window.show();
    let _ = window.set_focus();
    eprintln!("[display] window created, fullscreen + shown");
    // Nudge the compositor so the first frame actually paints.
    let _ = window.set_size(tauri::PhysicalSize::new(size.width, size.height));

    // Remember degradation for the operator UI.
    mgr.mark_degraded(slot, degraded);
    // Persist slot config (monitor mapping) so profiles include reality.
    let _ = store; // reserved: auto-save slot config later

    // Push content several times — a freshly created webview may still be
    // loading when the first push fires, so it can miss single events.
    let app2 = app.clone();
    let mgr2 = mgr.inner().clone();
    std::thread::spawn(move || {
        for delay in [300u64, 800, 1500, 3000] {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            display::emit_slot(&app2, slot, &mgr2);
        }
    });
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

#[tauri::command]
pub fn close_slot_output(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    slot: u8,
) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(&format!("display-{slot}")) {
        let _ = w.close();
    }
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

/// Move keyboard focus between open fullscreen outputs. Tab = next,
/// Shift+Tab = previous (the output window sends dir ±1). With one output
/// open this is a no-op.
#[tauri::command]
pub async fn cycle_output_focus(
    app: tauri::AppHandle,
    from_slot: u8,
    dir: i32,
) -> Result<(), String> {
    let open: Vec<u8> = (1..=5u8)
        .filter(|s| app.get_webview_window(&format!("display-{s}")).is_some())
        .collect();
    eprintln!("[focus] cycle from slot {from_slot} dir {dir}, open={open:?}");
    if open.len() < 2 {
        return Ok(());
    }
    let idx = open.iter().position(|&s| s == from_slot).unwrap_or(0);
    let next = open[((idx as i32 + dir).rem_euclid(open.len() as i32)) as usize];
    eprintln!("[focus] focusing display-{next}");
    if let Some(w) = app.get_webview_window(&format!("display-{next}")) {
        let focused = w.set_focus();
        eprintln!("[focus] set_focus result: {focused:?}");
    }
    Ok(())
}

/// Move an open output window to the next connected monitor (wraps around),
/// updating the slot's monitor assignment to match. Lets the operator send
/// a display to the other projector with one key press (M in fullscreen).
#[tauri::command]
pub async fn move_slot_output_to_next_monitor(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    slot: u8,
) -> Result<(), String> {
    let window = app
        .get_webview_window(&format!("display-{slot}"))
        .ok_or("output is not open")?;
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if monitors.len() < 2 {
        return Err("only one screen is connected".into());
    }

    // Which monitor currently holds the window?
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let current = monitors
        .iter()
        .position(|m| {
            let mp = m.position();
            let ms = m.size();
            pos.x >= mp.x && pos.x < mp.x + ms.width as i32 && pos.y >= mp.y && pos.y < mp.y + ms.height as i32
        })
        .unwrap_or(0);
    let next = &monitors[(current + 1) % monitors.len()];
    let npos = next.position();
    let nsize = next.size();

    // Re-fullscreen onto the new monitor (position first, then fullscreen).
    let _ = window.set_fullscreen(false);
    let _ = window.set_position(tauri::PhysicalPosition::new(npos.x, npos.y));
    let _ = window.set_size(tauri::PhysicalSize::new(nsize.width, nsize.height));
    let _ = window.set_fullscreen(true);

    // Keep the slot's monitor assignment in sync (profiles stay accurate).
    mgr.set_monitor(slot, next.name().map(|n| n.to_string()));
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptureInput {
    pub slot: u8,
    pub item_id: String,
    pub keys: Vec<String>,
}

/// Shared scripture projection: loads the WHOLE chapter of the first
/// requested key (so Next/Prev walks the chapter), index at the picked
/// verse. Used by the manual command and by Automatic-mode auto-show.
/// Load a bible book's sections for the given keys: the WHOLE chapter of
/// the first key that resolves (so Next/Prev walks the chapter), else the
/// exact keys. Returns (title, sections, index).
async fn load_scripture_sections(
    store: &ContentStore,
    item_id: &str,
    keys: &[String],
) -> Result<(String, Vec<LabeledSection>, usize), String> {
    let s = store.clone();
    let item_id = item_id.to_string();
    let keys = keys.to_vec();
    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded: Option<(usize, Vec<LabeledSection>)> = None;
        for k in &keys {
            if let Some(found) = s.get_chapter_sections(&item_id, k)? {
                loaded = Some(found);
                break;
            }
        }
        let (index, sections) = match loaded {
            Some(x) => x,
            None => {
                let v = s.get_sections_by_keys(&item_id, &keys)?;
                if v.is_empty() {
                    return Err(crate::content::ContentError::Other(
                        "no verses matched those keys".into(),
                    ));
                }
                (0, v)
            }
        };
        let title = s
            .get_item(&item_id)?
            .map(|i| i.title)
            .unwrap_or_else(|| "Scripture".into());
        Ok((title, sections, index))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: crate::content::ContentError| e.to_string())
}

/// The same book in the other translation: "bible-kjv-1-corinthians" +
/// "asv" → "bible-asv-1-corinthians". The same translation, or a
/// non-Bible item, does not pair.
fn companion_bible_id(item_id: &str, version: &str) -> Option<String> {
    for from in ["kjv", "asv"] {
        if let Some(book) = item_id.strip_prefix(&format!("bible-{from}-")) {
            return if from == version {
                None
            } else {
                Some(format!("bible-{version}-{book}"))
            };
        }
    }
    None
}

/// Resolve and attach the companion scripture for a slot's pairing, from
/// the primary's item id and the keys it was projected with (verse keys
/// are identical across translations, so the companion lines up
/// verse-for-verse). No-op when the slot isn't paired or the translation
/// lacks the book — the display silently stays single-column.
async fn attach_pair(
    store: &ContentStore,
    mgr: &DisplayManager,
    slot: u8,
    primary_item_id: &str,
    keys: &[String],
) {
    let Some(version) = mgr.pair_version_of(slot) else { return };
    let Some(pair_id) = companion_bible_id(primary_item_id, &version) else { return };
    let Ok((ptitle, psections, pindex)) = load_scripture_sections(store, &pair_id, keys).await
    else {
        return;
    };
    if psections.is_empty() {
        return;
    }
    mgr.set_pair(
        slot,
        Some(SectionedContent::with_index(
            pair_id,
            RenderKind::Scripture,
            ptitle,
            psections,
            pindex,
        )),
    );
}

async fn scripture_to_slot(
    app: &tauri::AppHandle,
    mgr: &DisplayManager,
    store: &ContentStore,
    service: &ServiceState,
    slot: u8,
    item_id: String,
    keys: Vec<String>,
) -> Result<(), String> {
    let (title, sections, index) = load_scripture_sections(store, &item_id, &keys).await?;

    if sections.is_empty() {
        return Err("no verses matched those keys".into());
    }

    let label = sections
        .get(index.min(sections.len() - 1))
        .map(|sec| sec.label.clone())
        .unwrap_or_default();
    let rec_title = title.clone();
    mgr.set_sections(
        slot,
        SectionedContent::with_index(item_id.clone(), RenderKind::Scripture, title, sections, index),
    );
    attach_pair(store, mgr, slot, &item_id, &keys).await;
    record_item(store, service, slot, "scripture", &rec_title, &label);
    display::emit_slot(app, slot, mgr);
    Ok(())
}

/// Put scripture on a slot. Loads the WHOLE chapter of the first requested
/// verse (so Next/Prev walks the chapter); index starts at the picked verse.
#[tauri::command]
pub async fn set_slot_scripture(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    service: State<'_, Arc<ServiceState>>,
    input: ScriptureInput,
) -> Result<(), String> {
    scripture_to_slot(
        &app,
        mgr.inner(),
        store.inner(),
        service.inner(),
        input.slot,
        input.item_id,
        input.keys,
    )
    .await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairInput {
    pub slot: u8,
    pub version: Option<String>,
}

/// Turn two-version display on/off for a slot ("kjv" | "asv" | null).
/// When scripture is already on the slot, the second column appears (or
/// disappears) immediately at the current verse.
#[tauri::command]
pub async fn set_slot_pair(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    input: PairInput,
) -> Result<(), String> {
    match input.version.as_deref() {
        None | Some("kjv") | Some("asv") => {}
        Some(other) => return Err(format!("unknown version: {other}")),
    }
    mgr.set_pair_version(input.slot, input.version.clone());
    let store_c = store.inner().clone();
    let key = format!("display_pair_{}", input.slot);
    let val = input.version.clone().unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        store_c.set_setting(&key, &val).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    // Re-resolve against what's on the slot right now, so the operator
    // sees the change without re-picking the verse.
    let current = mgr.current_scripture(input.slot);
    mgr.set_pair(input.slot, None);
    if let Some((item_id, verse_key)) = current {
        attach_pair(store.inner(), mgr.inner(), input.slot, &item_id, &[verse_key]).await;
    }
    display::emit_slot(&app, input.slot, mgr.inner());
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionInput {
    pub slot: u8,
    pub item_id: String,
    pub key: String,
}

/// Put a song/hymn section (stanza/chorus) on a slot. Loads ALL sections of
/// the item so next/prev walks the whole song.
#[tauri::command]
pub async fn set_slot_section(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    service: State<'_, Arc<ServiceState>>,
    input: SectionInput,
) -> Result<(), String> {
    let s = store.inner().clone();
    let item_id = input.item_id.clone();
    let key = input.key.clone();
    let found = tauri::async_runtime::spawn_blocking(move || s.get_song_sections(&item_id, &key))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let Some((index, sections)) = found else {
        return Err("no sections matched that key".into());
    };
    let item = store.inner().get_item(&input.item_id).ok().flatten();
    let title = item
        .as_ref()
        .map(|i| i.title.clone())
        .unwrap_or_else(|| "Lyrics".into());
    // Slides render scripture-style (one big centered block per slide);
    // songs/hymns render lyric-style (listed lines).
    let render_kind = match item.as_ref().map(|i| i.item_type) {
        Some(ItemType::Slide) => RenderKind::Scripture,
        _ => RenderKind::Lyrics,
    };

    let label = sections
        .get(index.min(sections.len() - 1))
        .map(|sec| sec.label.clone())
        .unwrap_or_default();
    let rec_title = title.clone();
    mgr.set_sections(
        input.slot,
        SectionedContent::with_index(input.item_id.clone(), render_kind, title, sections, index),
    );
    record_item(
        &store,
        &service,
        input.slot,
        if render_kind == RenderKind::Scripture { "slide" } else { "lyrics" },
        &rec_title,
        &label,
    );
    display::emit_slot(&app, input.slot, &mgr);
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInput {
    pub slot: u8,
    pub title: Option<String>,
    pub image_path: Option<String>,
    pub video_path: Option<String>,
}

#[tauri::command]
pub fn set_slot_media(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    service: State<'_, Arc<ServiceState>>,
    input: MediaInput,
) -> Result<(), String> {
    if input.image_path.is_none() && input.video_path.is_none() {
        return Err("provide image_path or video_path".into());
    }
    let title = input
        .title
        .clone()
        .unwrap_or_else(|| "Media".into());
    let kind = if input.video_path.is_some() { "video" } else { "image" };
    record_item(&store, &service, input.slot, kind, &title, "");
    mgr.set_media(input.slot, title, input.image_path, input.video_path);
    display::emit_slot(&app, input.slot, &mgr);
    Ok(())
}

#[tauri::command]
pub fn slot_step(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    slot: u8,
    delta: i32,
) -> Result<(), String> {
    mgr.step(slot, delta);
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

// ---- Service Control --------------------------------------------------------

use crate::session::SessionMeta;

fn new_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sess-{:x}", nanos ^ (nanos >> 32) ^ std::process::id() as u128)
}

/// Record a displayed-content item into the active service session, if any.
fn record_item(
    store: &ContentStore,
    service: &ServiceState,
    slot: u8,
    kind: &str,
    title: &str,
    label: &str,
) {
    if let Some(sess) = service.current_session() {
        let _ = store.add_session_item(&sess.id, slot, kind, title, label);
    }
}

#[tauri::command]
pub async fn start_service_session(
    store: State<'_, ContentStore>,
    service: State<'_, Arc<ServiceState>>,
    name: String,
) -> Result<SessionMeta, String> {
    let s = store.inner().clone();
    let svc = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let meta = SessionMeta {
            id: new_session_id(),
            name: if name.trim().is_empty() {
                format!("Service — {}", time_now_human())
            } else {
                name.trim().to_string()
            },
            started_at: String::new(), // DB sets the timestamp
            ended_at: None,
        };
        s.insert_session(&meta.id, &meta.name).map_err(|e| e.to_string())?;
        // Read back the DB timestamp so the UI timer is accurate.
        let started = list_sessions(&s)?
            .into_iter()
            .find(|m| m.id == meta.id)
            .map(|m| m.started_at)
            .unwrap_or_default();
        let meta = SessionMeta { started_at: started, ..meta };
        svc.start_session(meta.clone());
        Ok(meta)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn time_now_human() -> String {
    // Cheap human-readable local date for default session names.
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-Date -Format 'ddd d MMM yyyy'"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

#[tauri::command]
pub async fn end_service_session(
    store: State<'_, ContentStore>,
    service: State<'_, Arc<ServiceState>>,
) -> Result<Option<SessionMeta>, String> {
    let s = store.inner().clone();
    let svc = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(mut meta) = svc.end_session() else {
            return Ok(None);
        };
        s.end_session(&meta.id).map_err(|e| e.to_string())?;
        meta.ended_at = Some(String::new());
        // Read back the ended timestamp.
        meta.ended_at = list_sessions(&s)?
            .into_iter()
            .find(|m| m.id == meta.id)
            .and_then(|m| m.ended_at);
        Ok(Some(meta))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionItemRow {
    pub at: String,
    pub slot: u8,
    pub kind: String,
    pub title: String,
    pub label: String,
}

pub fn list_sessions(store: &ContentStore) -> Result<Vec<SessionMeta>, String> {
    let conn = store.conn_handle();
    let conn = conn.lock();
    let mut stmt = conn
        .prepare("SELECT id, name, started_at, ended_at FROM service_sessions ORDER BY started_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SessionMeta {
                id: r.get(0)?,
                name: r.get(1)?,
                started_at: r.get(2)?,
                ended_at: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn session_items(store: &ContentStore, id: &str) -> Result<Vec<SessionItemRow>, String> {
    let conn = store.conn_handle();
    let conn = conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT at, slot, kind, title, label FROM session_items
             WHERE session_id = ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![id], |r| {
            Ok(SessionItemRow {
                at: r.get(0)?,
                slot: r.get::<_, i64>(1)? as u8,
                kind: r.get(2)?,
                title: r.get(3)?,
                label: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn current_service_session(service: State<'_, Arc<ServiceState>>) -> Option<SessionMeta> {
    service.current_session()
}

#[tauri::command]
pub fn list_service_sessions(
    store: State<'_, ContentStore>,
) -> Result<Vec<SessionMeta>, String> {
    list_sessions(&store)
}

#[tauri::command]
pub fn get_service_session_items(
    store: State<'_, ContentStore>,
    id: String,
) -> Result<Vec<SessionItemRow>, String> {
    session_items(&store, &id)
}

#[tauri::command]
pub fn delete_service_session(
    store: State<'_, ContentStore>,
    id: String,
) -> Result<(), String> {
    store.delete_session(&id).map_err(|e| e.to_string())
}

/// Emergency: blank (or restore) every display slot at once.
#[tauri::command]
pub fn set_all_displays_blank(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    blank: bool,
) -> Result<(), String> {
    for slot in 1..=5u8 {
        mgr.set_blank(slot, blank);
        display::emit_slot(&app, slot, &mgr);
    }
    Ok(())
}

// ---- Display profiles ---------------------------------------------------------

#[tauri::command]
pub fn set_slot_style(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    slot: u8,
    style: crate::display::SlotStyle,
) -> Result<(), String> {
    if !(1..=5).contains(&slot) {
        return Err("slot must be 1-5".into());
    }
    mgr.set_style(slot, style);
    mgr.save_style(&store, slot)?;
    display::emit_slot(&app, slot, &mgr);
    Ok(())
}

#[tauri::command]
pub fn set_active_display(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    slot: u8,
) -> Result<(), String> {
    mgr.set_active_display(slot);
    display::emit_all_slots(&app, &mgr);
    Ok(())
}

#[tauri::command]
pub fn list_display_profiles(store: State<'_, ContentStore>) -> Result<Vec<display::DisplayProfile>, String> {
    display::load_profiles(&store)
}

#[tauri::command]
pub fn save_display_profile(
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    name: String,
) -> Result<(), String> {
    mgr.save_profile(&store, &name)
}

#[tauri::command]
pub fn apply_display_profile(
    app: tauri::AppHandle,
    mgr: State<'_, DisplayManager>,
    store: State<'_, ContentStore>,
    name: String,
) -> Result<(), String> {
    mgr.apply_profile(&store, &name)?;
    display::emit_all_slots(&app, &mgr);
    Ok(())
}

#[tauri::command]
pub fn delete_display_profile(
    store: State<'_, ContentStore>,
    name: String,
) -> Result<(), String> {
    let mut profiles = display::load_profiles(&store)?;
    profiles.retain(|p| p.name != name);
    store
        .set_setting("display_profiles", &serde_json::to_string(&profiles).unwrap())
        .map_err(|e| e.to_string())
}

fn new_id(prefix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}-{:x}", prefix, nanos, rand_hint(nanos))
}

fn rand_hint(n: u128) -> u128 {
    // Cheap uniqueness suffix; proper uuid crate not needed yet.
    n ^ (n >> 32) ^ std::process::id() as u128
}

// ---- Voice: audio + STT + intelligence -------------------------------------

use crate::audio::{self, AudioDeviceInfo, AudioTestResult, VoiceConfig};
use crate::intelligence::{self, Suggestion};
use crate::session::{ListenMode, ServiceState};
use crate::stt::SttEngine;
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[derive(Default)]
pub struct SttHolder {
    /// Cached engine tagged with the model choice it was built from
    /// ("base" | "tiny"), so switching models reloads once.
    engine: Mutex<Option<(String, Arc<SttEngine>)>>,
    /// Models shipped inside the installer; copied into %APPDATA% when
    /// missing so a fresh install needs no manual downloads. Set once at
    /// startup (needs the app handle), read by commands afterwards.
    pub bundled_models_dir: Mutex<Option<std::path::PathBuf>>,
}

impl SttHolder {
    /// Load (or reuse) the engine for the configured model choice.
    /// "base" is the accurate default; "tiny" trades accuracy for ~4x speed
    /// on modest church hardware.
    pub fn get_or_load(&self, store: &ContentStore) -> Result<Arc<SttEngine>, String> {
        let choice = store
            .get_setting("stt_model")
            .ok()
            .flatten()
            .unwrap_or_else(|| "base".into());
        let choice = match choice.as_str() {
            "tiny" => "tiny".to_string(),
            _ => "base".to_string(),
        };
        let mut guard = self.engine.lock();
        if let Some((cached, engine)) = guard.as_ref() {
            if cached == &choice {
                return Ok(engine.clone());
            }
        }
        if let Some(dir) = self.bundled_models_dir.lock().as_ref() {
            if let Err(e) = crate::stt::provision_bundled_models(dir) {
                eprintln!("[stt] bundled model provisioning failed: {e}");
            }
        }
        let path = crate::stt::model_path_for(&choice);
        if !path.exists() {
            return Err(format!(
                "whisper model not found at {} — switch models in Audio Setup or place the file there",
                path.display()
            ));
        }
        let engine = Arc::new(SttEngine::load(&path)?);
        *guard = Some((choice, engine.clone()));
        Ok(engine)
    }
}

#[tauri::command]
pub async fn get_stt_model(store: State<'_, ContentStore>) -> Result<String, String> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(s.get_setting("stt_model")
            .ok()
            .flatten()
            .unwrap_or_else(|| "base".into()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Where Automatic-mode suggestions project: "auto" (first AUTO slot,
/// the default) or a pinned "1".."5" — LOCK still protects a pinned slot.
fn normalize_auto_target(v: &str) -> String {
    match v.trim() {
        "1" | "2" | "3" | "4" | "5" => v.trim().to_string(),
        _ => "auto".to_string(),
    }
}

#[tauri::command]
pub async fn get_voice_auto_target(store: State<'_, ContentStore>) -> Result<String, String> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(s.get_setting("voice_auto_target")
            .ok()
            .flatten()
            .map(|v| normalize_auto_target(&v))
            .unwrap_or_else(|| "auto".into()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_voice_auto_target(
    store: State<'_, ContentStore>,
    target: String,
) -> Result<String, String> {
    let t = normalize_auto_target(&target);
    let saved = t.clone();
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        s.set_setting("voice_auto_target", &saved)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(t)
}

#[tauri::command]
pub async fn set_stt_model(
    store: State<'_, ContentStore>,
    model: String,
) -> Result<(), String> {
    if model != "base" && model != "tiny" {
        return Err(format!("unknown model: {model}"));
    }
    // Validate the file exists before saving, so the UI can show an error
    // now instead of failing later at listen time.
    if !crate::stt::model_path_for(&model).exists() {
        return Err(format!(
            "{model} model file not found in %APPDATA%/BibleLive/models"
        ));
    }
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        s.set_setting("stt_model", &model).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_audio_devices() -> Vec<AudioDeviceInfo> {
    audio::list_input_devices()
}

#[tauri::command]
pub async fn get_voice_config(
    store: State<'_, ContentStore>,
) -> Result<VoiceConfig, String> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let raw = s
            .get_setting("voice_config")
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut cfg: VoiceConfig = serde_json::from_str(&raw).unwrap_or_default();
        cfg.validate();
        Ok(cfg)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_voice_config(
    store: State<'_, ContentStore>,
    config: VoiceConfig,
) -> Result<(), String> {
    let s = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut config = config;
        config.validate();
        s.set_setting("voice_config", &serde_json::to_string(&config).unwrap())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_listen_mode(service: State<'_, Arc<ServiceState>>) -> String {
    service.mode().as_str().to_string()
}

#[tauri::command]
pub fn set_listen_mode(service: State<'_, Arc<ServiceState>>, mode: String) -> Result<(), String> {
    let mode = match mode.as_str() {
        "manual" => ListenMode::Manual,
        "assisted" => ListenMode::Assisted,
        "automatic" => ListenMode::Automatic,
        other => return Err(format!("unknown mode: {other}")),
    };
    service.set_mode(mode);
    Ok(())
}

#[tauri::command]
pub fn listening_status(mgr: State<'_, audio::CaptureManager>) -> bool {
    mgr.is_listening()
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TranscriptPayload {
    text: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SuggestionPayload {
    suggestion: Suggestion,
    mode: String,
}

/// What a slot held before an automatic show — primary plus the paired
/// second column, restored together on Undo.
#[derive(Clone)]
pub struct SlotSnapshot {
    pub primary: SectionedContent,
    pub pair: Option<SectionedContent>,
}

/// Snapshots for undoing Automatic-mode auto-shows: suggestion id →
/// (taken-at, slot, snapshot). Entries self-expire on access.
/// Managed as an Arc so the listening pipeline and the undo command share
/// one map.
#[derive(Default)]
pub struct AutoShowUndo(
    Mutex<std::collections::HashMap<String, (std::time::Instant, u8, SlotSnapshot)>>,
);

/// Verified suggestions must clear this confidence to project themselves.
const AUTO_SHOW_CONFIDENCE: f32 = 0.75;
/// How long the operator sees an Undo button after an auto-show.
const AUTO_UNDO_MS: u64 = 10_000;
/// Backend hard limit for honoring an undo (UI window + slack).
const AUTO_UNDO_HARD_LIMIT: std::time::Duration = std::time::Duration::from_secs(15);
/// A sentence flush also needs at least this many words, so short
/// acknowledgements ("okay." "right.") never trigger verified matching.
const FLUSH_MIN_WORDS: usize = 6;

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AutoShownPayload {
    pub id: String,
    pub slot: u8,
    pub undo_ms: u64,
}

#[tauri::command]
pub async fn start_listening(
    app: tauri::AppHandle,
    store: State<'_, ContentStore>,
    service: State<'_, Arc<ServiceState>>,
    mgr: State<'_, audio::CaptureManager>,
    disp: State<'_, DisplayManager>,
    auto_undo: State<'_, std::sync::Arc<AutoShowUndo>>,
    stt: State<'_, SttHolder>,
) -> Result<(), String> {
    let engine = stt.get_or_load(store.inner())?;
    let config = get_voice_config(store.clone()).await?;
    let auto_target = get_voice_auto_target(store.clone()).await?;
    let app_handle = app.clone();
    let store_clone = store.inner().clone();
    let service_clone = service.inner().clone();

    let app_for_level = app.clone();
    let live = Arc::new(Mutex::new(intelligence::LiveMatcher::new()));
    // Last transcript that already triggered a sentence flush this
    // utterance, so a stable tail doesn't re-run verified matching.
    let last_flush = Arc::new(Mutex::new(String::new()));
    // One partial in flight at a time: on a CPU slower than the window, a
    // growing queue of stale partials delayed everything. A new partial is
    // dropped when one is still running — the next carries newer text.
    let partial_busy = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let disp_handle = disp.inner().clone(); // Clone-shared (Arc inside)
    let auto_undo_handle = auto_undo.inner().clone();
    mgr.start(
        &config,
        move |ev| {
        let engine = engine.clone();
        let app = app_handle.clone();
        let store = store_clone.clone();
        let service = service_clone.clone();
        let live = live.clone();
        let last_flush = last_flush.clone();
        let auto_target = auto_target.clone();
        let busy = partial_busy.clone();
        let disp = disp_handle.clone();
        let auto_undo = auto_undo_handle.clone();
        // Process off the audio-forwarding thread. Async so the Automatic
        // mode's projection can await the shared scripture loader.
        tauri::async_runtime::spawn(async move {
            let is_final = matches!(ev, audio::AudioEvent::Final(_));
            let samples = match ev {
                audio::AudioEvent::Partial(buf) | audio::AudioEvent::Final(buf) => buf,
            };
            if !is_final
                && busy
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_err()
            {
                return; // previous partial still transcribing — drop this one
            }
            let samples = if is_final {
                samples
            } else {
                audio::partial_tail(samples)
            };
            let result = engine.transcribe(&samples);
            if !is_final {
                busy.store(false, Ordering::Relaxed);
            }
            match result {
                Ok(text) if !text.is_empty() => {
                    // Continuous reading never hits the closing pause, so a
                    // rolling partial that just completed a sentence (ends
                    // in . ! ? with enough words) runs the full verified
                    // pass right away instead of waiting for silence.
                    let flush = if is_final {
                        last_flush.lock().clear();
                        false
                    } else {
                        let mut lf = last_flush.lock();
                        let hit = text.trim_end().ends_with(|c: char| c == '.' || c == '!' || c == '?')
                            && text.split_whitespace().count() >= FLUSH_MIN_WORDS
                            && *lf != text;
                        if hit {
                            *lf = text.clone();
                        }
                        hit
                    };
                    let mode = service.mode();
                    if is_final {
                        // Final, pause-verified transcript: full matching.
                        let _ = app.emit("voice-transcript", TranscriptPayload { text: text.clone() });
                    } else {
                        // Rolling partial: live transcript + progressive
                        // Scripture matching with confidence/stability gating.
                        let _ = app.emit(
                            "voice-transcript-partial",
                            TranscriptPayload { text: text.clone() },
                        );
                        let mut lm = live.lock();
                        if let Some(s) = lm.observe(&store, &text) {
                            if let Some(s) = service.upsert_suggestion(s) {
                                let _ = app.emit(
                                    "voice-suggestion",
                                    SuggestionPayload {
                                        suggestion: s,
                                        mode: mode.as_str().to_string(),
                                    },
                                );
                            }
                        }
                    }
                    if is_final || flush {
                        let finals = intelligence::analyze_transcript(&store, &service, &text);
                        let confirmed: Vec<(String, String)> = finals
                            .iter()
                            .map(|s| (s.item_id.clone(), s.section_key.clone()))
                            .collect();
                        for s in &finals {
                            let _ = app.emit(
                                "voice-suggestion",
                                SuggestionPayload {
                                    suggestion: s.clone(),
                                    mode: mode.as_str().to_string(),
                                },
                            );
                        }
                        // Automatic mode: a verified, high-confidence match
                        // projects itself to the configured target slot
                        // ("auto" = first AUTO slot; a pinned 1–5 respects
                        // LOCK); the operator keeps an Undo window. No
                        // resolvable target or lower confidence → behaves
                        // like Assisted (cards).
                        if mode == ListenMode::Automatic {
                            if let Some(s) =
                                finals.iter().find(|s| s.confidence >= AUTO_SHOW_CONFIDENCE)
                            {
                                if let Some(slot) = disp.auto_target_slot(&auto_target) {
                                    let snapshot = disp
                                        .content_snapshot(slot)
                                        .map(|primary| SlotSnapshot {
                                            pair: disp.pair_snapshot(slot),
                                            primary,
                                        });
                                    let projected = scripture_to_slot(
                                        &app,
                                        &disp,
                                        &store,
                                        &service,
                                        slot,
                                        s.item_id.clone(),
                                        vec![s.section_key.clone()],
                                    )
                                    .await
                                    .is_ok();
                                    if projected {
                                        if let Some(snap) = snapshot {
                                            let mut map = auto_undo.0.lock();
                                            map.retain(|_, (at, _, _)|
                                                at.elapsed() < AUTO_UNDO_HARD_LIMIT * 4);
                                            map.insert(s.id.clone(), (std::time::Instant::now(), slot, snap));
                                        }
                                        if let Some(shown) = service.resolve_suggestion(&s.id, "shown") {
                                            let _ = app.emit(
                                                "voice-suggestion",
                                                SuggestionPayload {
                                                    suggestion: shown,
                                                    mode: mode.as_str().to_string(),
                                                },
                                            );
                                        }
                                        let _ = app.emit(
                                            "voice-auto-shown",
                                            AutoShownPayload {
                                                id: s.id.clone(),
                                                slot,
                                                undo_ms: AUTO_UNDO_MS,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        if is_final {
                            // Retire a live card the verified pass did not confirm.
                            let mut lm = live.lock();
                            if let Some(key) = lm.promoted_key().cloned() {
                                if !confirmed.contains(&key) {
                                    for p in service.pending_suggestions() {
                                        if p.status == "pending"
                                            && (p.item_id.clone(), p.section_key.clone()) == key
                                        {
                                            if let Some(retired) =
                                                service.set_suggestion_status(&p.id, "ignored")
                                            {
                                                let _ = app.emit(
                                                    "voice-suggestion",
                                                    SuggestionPayload {
                                                        suggestion: retired,
                                                        mode: mode.as_str().to_string(),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                                lm.retire();
                            }
                            // Next utterance starts fresh: verses decided on
                            // during this one may be suggested again later.
                            service.clear_utterance_suppressions();
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("transcription failed: {e}");
                }
            }
        });
        },
        move |level| {
            let _ = app_for_level.emit("voice-level", level);
        },
    )
}

#[tauri::command]
pub fn stop_listening(mgr: State<'_, audio::CaptureManager>) {
    mgr.stop();
}

/// Undo an Automatic-mode auto-show: restore what the slot held before and
/// mark the suggestion ignored. Honored within the undo window only.
#[tauri::command]
pub async fn undo_auto_show(
    app: tauri::AppHandle,
    disp: State<'_, DisplayManager>,
    undo: State<'_, std::sync::Arc<AutoShowUndo>>,
    service: State<'_, Arc<ServiceState>>,
    id: String,
) -> Result<(), String> {
    let entry = undo.0.lock().remove(&id);
    if let Some((at, slot, snap)) = entry {
        if at.elapsed() <= AUTO_UNDO_HARD_LIMIT {
            disp.restore_sections(slot, snap.primary, snap.pair);
            display::emit_slot(&app, slot, &disp);
        }
    }
    if let Some(s) = service.set_suggestion_status(&id, "ignored") {
        let _ = app.emit(
            "voice-suggestion",
            SuggestionPayload {
                suggestion: s,
                mode: service.mode().as_str().to_string(),
            },
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn audio_test(
    store: State<'_, ContentStore>,
    mgr: State<'_, audio::CaptureManager>,
    stt: State<'_, SttHolder>,
) -> Result<AudioTestResult, String> {
    let engine = stt.get_or_load(store.inner())?;
    let config = get_voice_config(store.clone()).await?;
    let s = store.inner().clone();
    let mgr_inner = mgr.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        if mgr_inner.is_listening() {
            return Err("stop listening before running the audio test".into());
        }
        let segments: Arc<Mutex<Vec<Vec<f32>>>> = Arc::new(Mutex::new(Vec::new()));
        let segs = segments.clone();
        let collect = move |ev: audio::AudioEvent| {
            // Only completed utterances count for the test.
            if let audio::AudioEvent::Final(seg) = ev {
                segs.lock().push(seg);
            }
        };

        let device_label = config
            .device
            .clone()
            .unwrap_or_else(|| "Default input".to_string());

        mgr_inner.start(&config, collect, |_| {})?;

        // Listen for 8 seconds.
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        mgr_inner.stop();

        let segs = segments.lock().clone();
        let mut peak: f32 = 0.0;
        for seg in &segs {
            let rms = (seg.iter().map(|x| x * x).sum::<f32>() / seg.len().max(1) as f32).sqrt();
            peak = peak.max(rms);
        }
        // Transcribe the (up to 3) longest segments in parallel — on modest
        // hardware sequential whisper is slower than realtime.
        let mut ranked = segs.clone();
        ranked.sort_by_key(|s| std::cmp::Reverse(s.len()));
        ranked.truncate(3);
        let handles: Vec<_> = ranked
            .into_iter()
            .map(|seg| {
                let engine = engine.clone();
                std::thread::spawn(move || engine.transcribe(&seg).unwrap_or_default())
            })
            .collect();
        let mut parts: Vec<String> = Vec::new();
        for h in handles {
            let t = h.join().unwrap_or_default();
            if !t.is_empty() {
                parts.push(t);
            }
        }
        let transcript = parts.join(" ");
        let _ = s; // reserved: persist test results later

        Ok(AudioTestResult {
            device: device_label,
            peak_level: peak,
            speech_detected: !segs.is_empty(),
            transcript,
            seconds: 8,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- Voice diagnostics ------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDiagnostics {
    pub build: String,
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
    pub device_error: Option<String>,
    pub vad_threshold: f32,
    pub content_type: String,
    pub raw_peak: f32,
    pub gain_applied: f32,
    pub level_max: f32,
    pub level_series: Vec<f32>,
    pub segments: usize,
    pub segment_samples: Vec<usize>,
    pub model_path: String,
    pub transcript: String,
    pub notes: Vec<String>,
}

/// Capture for `seconds`, then report every stage of the audio path with
/// hard numbers. Written to %APPDATA%/BibleLive/diagnostics-report.txt and
/// returned to the UI. This is how we pinpoint failures remotely.
#[tauri::command]
pub async fn run_voice_diagnostics(
    store: State<'_, ContentStore>,
    mgr: State<'_, audio::CaptureManager>,
    stt: State<'_, SttHolder>,
    seconds: Option<u32>,
) -> Result<VoiceDiagnostics, String> {
    let seconds = seconds.unwrap_or(10).clamp(3, 30);
    let config = get_voice_config(store.clone()).await?;
    let desc = audio::describe_capture(&config);

    let engine = stt.get_or_load(store.inner()).ok();
    let model_path = crate::stt::default_model_path();

    let segments: Arc<Mutex<Vec<Vec<f32>>>> = Arc::new(Mutex::new(Vec::new()));
    let levels: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let mgr_inner = mgr.inner().clone();
    let config_for_task = config.clone();
    let segments_for_task = Arc::clone(&segments);
    let levels_for_task = Arc::clone(&levels);

    let result = tauri::async_runtime::spawn_blocking(move || {
        let segs = segments_for_task;
        let lvls = levels_for_task;
        let started = mgr_inner.start_collector(
            &config_for_task,
            move |ev| {
                if let audio::AudioEvent::Final(seg) = ev {
                    segs.lock().push(seg);
                }
            },
            move |lv| {
                let mut q = lvls.lock();
                q.push(lv);
                if q.len() > 400 {
                    q.remove(0);
                }
            },
        );
        match started {
            Ok(feed) => {
                std::thread::sleep(std::time::Duration::from_secs(seconds as u64));
                mgr_inner.stop();
                Ok(feed)
            }
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let feed = result?;
    std::thread::sleep(std::time::Duration::from_millis(300));

    let levels = levels.lock().clone();
    let segs = segments.lock().clone();
    let level_max = levels.iter().fold(0.0f32, |m, x| m.max(*x));
    let seg_count = segs.len();
    let raw_peak = feed.raw_peak();
    let gain_applied = feed.gain();

    let transcript = if let Some(engine) = engine {
        let mut ranked = segs.clone();
        ranked.sort_by_key(|s| std::cmp::Reverse(s.len()));
        ranked.truncate(3);
        let handles: Vec<_> = ranked
            .into_iter()
            .map(|seg| {
                let engine = engine.clone();
                std::thread::spawn(move || engine.transcribe(&seg).unwrap_or_default())
            })
            .collect();
        let mut parts: Vec<String> = Vec::new();
        for h in handles {
            let t = h.join().unwrap_or_default();
            if !t.is_empty() {
                parts.push(t);
            }
        }
        parts.join(" ")
    } else {
        String::new()
    };

    let mut notes: Vec<String> = Vec::new();
    if let Some(err) = &desc.error {
        notes.push(format!("Capture could not start: {err}"));
    }
    if seg_count > 0 && transcript.is_empty() {
        notes.push(
            "Speech segments were captured but transcription produced no text.".to_string(),
        );
    }

    let report = VoiceDiagnostics {
        build: crate::fmt_utc(env!("BL_BUILD_SECS").parse().unwrap_or(0)),
        device: desc.device.clone(),
        sample_rate: desc.sample_rate,
        channels: desc.channels,
        sample_format: desc.sample_format.clone(),
        device_error: desc.error.clone(),
        vad_threshold: config.vad_threshold,
        content_type: config.content_type.clone(),
        raw_peak,
        gain_applied,
        level_max,
        level_series: levels,
        segments: seg_count,
        segment_samples: segs.iter().map(|s| s.len()).collect(),
        model_path: model_path.display().to_string(),
        transcript,
        notes,
    };

    // Persist a copy the user can attach to a report.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let path = format!("{appdata}/BibleLive/diagnostics-report.txt");
        if let Ok(json) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(&path, json);
        }
    }

    Ok(report)
}

// ---- Suggestions ----------------------------------------------------------

#[tauri::command]
pub fn list_suggestions(
    service: State<'_, Arc<ServiceState>>,
) -> Vec<Suggestion> {
    service.recent_suggestions(20)
}

#[tauri::command]
pub fn respond_suggestion(
    service: State<'_, Arc<ServiceState>>,
    id: String,
    show: bool,
) -> Result<Suggestion, String> {
    // resolve (not just set) — an operator decision also suppresses the
    // verse for the rest of this utterance.
    service
        .resolve_suggestion(&id, if show { "shown" } else { "ignored" })
        .ok_or_else(|| format!("suggestion {id} not found"))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub path: String,
    pub exists: bool,
    pub size_mb: Option<u64>,
}

#[tauri::command]
pub async fn model_status(stt: State<'_, SttHolder>) -> Result<ModelStatus, String> {
    // Prefer whichever model is present (base.en accurate, tiny.en fast).
    // A fresh install self-provisions from the installer-bundled copies first.
    let bundled = stt.inner().bundled_models_dir.lock().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(dir) = &bundled {
            if let Err(e) = crate::stt::provision_bundled_models(dir) {
                eprintln!("[stt] bundled model provisioning failed: {e}");
            }
        }
        let path = crate::stt::default_model_path();
        let exists = path.exists();
        let size_mb = if exists {
            std::fs::metadata(&path).ok().map(|m| m.len() / 1024 / 1024)
        } else {
            None
        };
        Ok(ModelStatus {
            path: path.display().to_string(),
            exists,
            size_mb,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::companion_bible_id;

    /// Companion ids: cross-translation only, book slug preserved intact
    /// (slugs may contain dashes, e.g. 1-corinthians).
    #[test]
    fn companion_bible_id_swaps_translation_only() {
        assert_eq!(
            companion_bible_id("bible-kjv-john", "asv").as_deref(),
            Some("bible-asv-john")
        );
        assert_eq!(
            companion_bible_id("bible-asv-1-corinthians", "kjv").as_deref(),
            Some("bible-kjv-1-corinthians")
        );
        assert_eq!(companion_bible_id("bible-kjv-john", "kjv"), None);
        assert_eq!(companion_bible_id("hymn-amazing-grace", "asv"), None);
        assert_eq!(companion_bible_id("bible-niv-john", "asv"), None);
    }
}
