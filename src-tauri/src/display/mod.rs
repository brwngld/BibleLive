//! Display Engine — five independent Display Slots.
//!
//! The application manages Display Slots 1–5, each mappable to any OS
//! monitor, each independently controllable (AUTO / MANUAL / LOCK), each
//! with its own content (scripture, lyrics, image, video, blank). Output
//! windows are fullscreen Tauri webviews ("display-1" … "display-5") that
//! render whatever the slot holds.
//!
//! Resilience rules: if the target monitor is missing, the output falls
//! back to the primary monitor and the slot is marked degraded; closing or
//! losing an output never touches the slot's content, so the service keeps
//! running and the slot re-attaches when the output is reopened.

use crate::content::model::LabeledSection;
use crate::content::ContentStore;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

const SLOT_COUNT: usize = 5;
const PROFILES_KEY: &str = "display_profiles";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    Auto,
    Manual,
    Lock,
}

impl DisplayMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayMode::Auto => "auto",
            DisplayMode::Manual => "manual",
            DisplayMode::Lock => "lock",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderKind {
    Scripture,
    Lyrics,
}

/// Per-slot display typography and colors. `font_size` is in vw units
/// (percentage of the output screen width) for resolution independence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotStyle {
    pub font_family: String,
    pub font_size: f32,
    pub text_color: String,
    pub bg_color: String,
}

impl Default for SlotStyle {
    fn default() -> Self {
        Self {
            font_family: "Georgia, 'Times New Roman', serif".into(),
            font_size: 6.5,
            text_color: "#ffffff".into(),
            bg_color: "#000000".into(),
        }
    }
}

impl SlotStyle {
    pub fn validate(&mut self) {
        if self.font_family.trim().is_empty() {
            self.font_family = "Georgia, 'Times New Roman', serif".into();
        }
        self.font_size = self.font_size.clamp(2.0, 15.0);
        if !valid_hex_color(&self.text_color) {
            self.text_color = "#ffffff".into();
        }
        if !valid_hex_color(&self.bg_color) {
            self.bg_color = "#000000".into();
        }
    }
}

fn valid_hex_color(s: &str) -> bool {
    let t = s.trim();
    (t.len() == 7 || t.len() == 4)
        && t.starts_with('#')
        && t[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

/// What a slot is currently holding (internal).
#[derive(Clone)]
pub struct SectionedContent {
    item_id: String,
    kind: RenderKind,
    title: String,
    sections: Vec<LabeledSection>,
    index: usize,
}

impl SectionedContent {
    pub fn new(item_id: String, kind: RenderKind, title: String, sections: Vec<LabeledSection>) -> Self {
        Self { item_id, kind, title, sections, index: 0 }
    }

    pub fn with_index(item_id: String, kind: RenderKind, title: String, sections: Vec<LabeledSection>, index: usize) -> Self {
        Self { item_id, kind, title, sections, index }
    }
}

enum SlotContent {
    Sections(SectionedContent),
    Media {
        title: String,
        image_path: Option<String>,
        video_path: Option<String>,
    },
}

/// Payload emitted to output windows and the operator UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotContentEvent {
    pub slot: u8,
    pub kind: String, // "scripture" | "lyrics" | "image" | "video" | "blank"
    pub title: String,
    pub label: String,
    pub lines: Vec<String>,
    pub page: Option<String>, // "2 of 31"
    pub image_path: Option<String>,
    pub video_path: Option<String>,
    pub blank: bool,
    pub style: SlotStyle,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub slot: u8,
    pub monitor: Option<String>,
    pub mode: DisplayMode,
    pub blank: bool,
    pub window_open: bool,
    pub degraded: bool,
    pub active: bool,
    pub content: SlotContentEvent,
}

struct SlotState {
    monitor: Option<String>,
    mode: DisplayMode,
    blank: bool,
    degraded: bool,
    style: SlotStyle,
    content: Option<SlotContent>,
}

impl Default for SlotState {
    fn default() -> Self {
        Self {
            monitor: None,
            mode: DisplayMode::Manual,
            blank: false,
            degraded: false,
            style: SlotStyle::default(),
            content: None,
        }
    }
}

#[derive(Clone)]
pub struct DisplayManager {
    slots: Arc<Mutex<Vec<SlotState>>>,
    active: Arc<std::sync::atomic::AtomicU8>,
}

impl Default for DisplayManager {
    fn default() -> Self {
        Self {
            slots: Arc::new(Mutex::new(
                (0..SLOT_COUNT).map(|_| SlotState::default()).collect(),
            )),
            active: Arc::new(std::sync::atomic::AtomicU8::new(1)),
        }
    }
}

fn empty_event(slot: u8) -> SlotContentEvent {
    SlotContentEvent {
        slot,
        kind: "blank".into(),
        title: String::new(),
        label: String::new(),
        lines: Vec::new(),
        page: None,
        image_path: None,
        video_path: None,
        blank: true,
        style: SlotStyle::default(),
    }
}

fn content_event(slot: u8, state: &SlotState) -> SlotContentEvent {
    let mut ev = match &state.content {
        None => empty_event(slot),
        Some(SlotContent::Media {
            title,
            image_path,
            video_path,
        }) => SlotContentEvent {
            slot,
            kind: if video_path.is_some() { "video" } else { "image" }.into(),
            title: title.clone(),
            label: String::new(),
            lines: Vec::new(),
            page: None,
            image_path: image_path.clone(),
            video_path: video_path.clone(),
            blank: false,
            style: state.style.clone(),
        },
        Some(SlotContent::Sections(c)) => {
            let sec = &c.sections[c.index.min(c.sections.len() - 1)];
            SlotContentEvent {
                slot,
                kind: format!("{:?}", c.kind).to_lowercase(),
                title: c.title.clone(),
                label: sec.label.clone(),
                lines: sec.lines.clone(),
                page: Some(format!("{} of {}", c.index + 1, c.sections.len())),
                image_path: None,
                video_path: None,
                blank: false,
                style: state.style.clone(),
            }
        }
    };
    if state.blank {
        ev.kind = "blank".into();
        ev.blank = true;
    }
    ev
}

impl DisplayManager {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- Active display (hotkey target) -----------------------------------

    pub fn active_display(&self) -> u8 {
        let a = self.active.load(std::sync::atomic::Ordering::Relaxed);
        if (1..=SLOT_COUNT as u8).contains(&a) { a } else { 1 }
    }

    pub fn set_active_display(&self, slot: u8) {
        if (1..=SLOT_COUNT as u8).contains(&slot) {
            self.active
                .store(slot, std::sync::atomic::Ordering::Relaxed);
        }
    }

    // ---- Style --------------------------------------------------------------

    pub fn style_of(&self, slot: u8) -> SlotStyle {
        self.slots.lock()[(slot as usize).clamp(1, SLOT_COUNT) - 1].style.clone()
    }

    pub fn set_style(&self, slot: u8, mut style: SlotStyle) {
        style.validate();
        self.slots.lock()[(slot as usize).clamp(1, SLOT_COUNT) - 1].style = style.clone();
    }

    /// Load persisted per-slot styles at startup (missing keys keep defaults).
    pub fn load_styles(&self, store: &ContentStore) {
        for slot in 1..=SLOT_COUNT as u8 {
            if let Ok(Some(raw)) = store.get_setting(&format!("display_style_{slot}")) {
                if let Ok(mut s) = serde_json::from_str::<SlotStyle>(&raw) {
                    s.validate();
                    self.slots.lock()[(slot - 1) as usize].style = s;
                }
            }
        }
    }

    pub fn save_style(&self, store: &ContentStore, slot: u8) -> Result<(), String> {
        let style = self.style_of(slot);
        store
            .set_setting(
                &format!("display_style_{slot}"),
                &serde_json::to_string(&style).unwrap(),
            )
            .map_err(|e| e.to_string())
    }

    pub fn slots(&self) -> Vec<SlotView> {
        let active = self.active_display();
        let slots = self.slots.lock();
        slots
            .iter()
            .enumerate()
            .map(|(i, s)| SlotView {
                slot: (i + 1) as u8,
                monitor: s.monitor.clone(),
                mode: s.mode,
                blank: s.blank,
                window_open: false, // filled in by the command layer (needs AppHandle)
                degraded: s.degraded,
                active: (i + 1) as u8 == active,
                content: content_event((i + 1) as u8, s),
            })
            .collect()
    }

    pub fn set_monitor(&self, slot: u8, monitor: Option<String>) {
        let mut slots = self.slots.lock();
        slots[(slot as usize).clamp(1, SLOT_COUNT) - 1].monitor = monitor;
    }

    pub fn set_mode(&self, slot: u8, mode: DisplayMode) {
        let mut slots = self.slots.lock();
        slots[(slot as usize).clamp(1, SLOT_COUNT) - 1].mode = mode;
    }

    pub fn set_blank(&self, slot: u8, blank: bool) {
        let mut slots = self.slots.lock();
        slots[(slot as usize).clamp(1, SLOT_COUNT) - 1].blank = blank;
    }

    pub fn set_sections(&self, slot: u8, content: SectionedContent) {
        let mut slots = self.slots.lock();
        let st = &mut slots[(slot as usize).clamp(1, SLOT_COUNT) - 1];
        st.content = Some(SlotContent::Sections(content));
        st.blank = false;
    }

    /// First slot set to AUTO (1-based), if any — the target Automatic-mode
    /// voice suggestions project themselves onto.
    pub fn find_auto_slot(&self) -> Option<u8> {
        self.slots
            .lock()
            .iter()
            .enumerate()
            .find(|(_, s)| s.mode == DisplayMode::Auto)
            .map(|(i, _)| (i + 1) as u8)
    }

    /// Snapshot a slot's current text content so an automatic show can be
    /// undone by restoring what was there before.
    pub fn content_snapshot(&self, slot: u8) -> Option<SectionedContent> {
        match &self.slots.lock()[(slot as usize).clamp(1, SLOT_COUNT) - 1].content {
            Some(SlotContent::Sections(c)) => Some(c.clone()),
            _ => None,
        }
    }

    pub fn set_media(&self, slot: u8, title: String, image_path: Option<String>, video_path: Option<String>) {
        let mut slots = self.slots.lock();
        let st = &mut slots[(slot as usize).clamp(1, SLOT_COUNT) - 1];
        st.content = Some(SlotContent::Media {
            title,
            image_path,
            video_path,
        });
        st.blank = false;
    }

    pub fn mark_degraded(&self, slot: u8, degraded: bool) {
        let mut slots = self.slots.lock();
        slots[(slot as usize).clamp(1, SLOT_COUNT) - 1].degraded = degraded;
    }

    /// Advance within the sectioned content. Returns true if moved.
    pub fn step(&self, slot: u8, delta: i32) -> bool {
        let mut slots = self.slots.lock();
        let st = &mut slots[(slot as usize).clamp(1, SLOT_COUNT) - 1];
        if st.blank {
            st.blank = false; // next from blank returns to content
            return true;
        }
        let Some(SlotContent::Sections(c)) = &mut st.content else {
            return false;
        };
        if c.sections.is_empty() {
            return false;
        }
        let next = c.index as i32 + delta;
        if next < 0 || next >= c.sections.len() as i32 {
            return false;
        }
        c.index = next as usize;
        true
    }

    pub fn clear_content(&self, slot: u8) {
        let mut slots = self.slots.lock();
        slots[(slot as usize).clamp(1, SLOT_COUNT) - 1].content = None;
    }

    // ---- Profiles ----------------------------------------------------------

    pub fn save_profile(&self, store: &ContentStore, name: &str) -> Result<(), String> {
        let mut profiles = load_profiles(store)?;
        profiles.retain(|p| p.name != name);
        let slots = self.slots.lock();
        profiles.push(DisplayProfile {
            name: name.to_string(),
            slots: slots
                .iter()
                .map(|s| ProfileSlot {
                    monitor: s.monitor.clone(),
                    mode: s.mode,
                    style: s.style.clone(),
                })
                .collect(),
        });
        store
            .set_setting(PROFILES_KEY, &serde_json::to_string(&profiles).unwrap())
            .map_err(|e| e.to_string())
    }

    pub fn apply_profile(&self, store: &ContentStore, name: &str) -> Result<(), String> {
        let profiles = load_profiles(store)?;
        let p = profiles
            .into_iter()
            .find(|p| p.name == name)
            .ok_or_else(|| format!("profile not found: {name}"))?;
        let mut slots = self.slots.lock();
        for (i, ps) in p.slots.into_iter().enumerate().take(SLOT_COUNT) {
            slots[i].monitor = ps.monitor;
            slots[i].mode = ps.mode;
            slots[i].style = ps.style;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSlot {
    pub monitor: Option<String>,
    pub mode: DisplayMode,
    #[serde(default)]
    pub style: SlotStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayProfile {
    pub name: String,
    pub slots: Vec<ProfileSlot>,
}

pub fn load_profiles(store: &ContentStore) -> Result<Vec<DisplayProfile>, String> {
    let raw = store
        .get_setting(PROFILES_KEY)
        .ok()
        .flatten()
        .unwrap_or_default();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::model::{ItemType, Section};

    /// Automatic-mode targeting: no slot is AUTO by default (explicit
    /// operator opt-in), and the first AUTO slot wins.
    #[test]
    fn auto_slot_targeting() {
        let mgr = DisplayManager::new();
        assert_eq!(mgr.find_auto_slot(), None, "all slots default to MANUAL");

        mgr.set_mode(3, DisplayMode::Auto);
        mgr.set_mode(5, DisplayMode::Auto);
        assert_eq!(mgr.find_auto_slot(), Some(3), "first AUTO slot is the target");

        mgr.set_mode(3, DisplayMode::Manual);
        assert_eq!(mgr.find_auto_slot(), Some(5));
    }

    fn test_store(tag: &str) -> ContentStore {
        let dir = std::env::temp_dir().join(format!(
            "biblelive-disp-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        ContentStore::open_at(dir).expect("open test store")
    }

    #[test]
    fn slot_content_and_stepping() {
        let mgr = DisplayManager::new();
        let sections = vec![
            LabeledSection { key: "v1".into(), label: "Verse 1".into(), lines: vec!["one".into()] },
            LabeledSection { key: "v2".into(), label: "Verse 2".into(), lines: vec!["two".into()] },
        ];
        mgr.set_sections(1, SectionedContent::new("i".into(), RenderKind::Lyrics, "T".into(), sections));

        // content event reflects index 0
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "Verse 1");
        assert_eq!(ev.page.as_deref(), Some("1 of 2"));

        assert!(mgr.step(1, 1));
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "Verse 2");
        assert!(!mgr.step(1, 1)); // past the end
        assert!(mgr.step(1, -1));
        assert!(!mgr.step(1, -1)); // before the start

        // blanking overrides content; next from blank restores it unchanged
        mgr.set_blank(1, true);
        assert_eq!(content_event(1, &mgr.slots.lock()[0]).kind, "blank");
        mgr.step(1, 1);
        assert_eq!(content_event(1, &mgr.slots.lock()[0]).label, "Verse 1");
    }

    #[test]
    fn profiles_roundtrip() {
        let store = test_store("profiles");
        let mgr = DisplayManager::new();
        mgr.set_monitor(1, Some("SECOND MONITOR".into()));
        mgr.set_mode(2, DisplayMode::Auto);
        mgr.save_profile(&store, "Sunday Service").unwrap();

        // change things, then restore
        mgr.set_monitor(1, None);
        mgr.set_mode(2, DisplayMode::Lock);
        mgr.apply_profile(&store, "Sunday Service").unwrap();

        let views = mgr.slots();
        assert_eq!(views[0].monitor.as_deref(), Some("SECOND MONITOR"));
        assert_eq!(views[1].mode, DisplayMode::Auto);

        let list = load_profiles(&store).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Sunday Service");
    }

    #[test]
    fn song_sections_from_body_keep_lines() {
        let store = test_store("sections");
        let item = crate::content::import::import_text(
            "song-x".into(),
            "Test Song".into(),
            ItemType::Song,
            "[Verse 1]\nline one\nline two\n\n[Chorus]\npraise the Lord",
            "en",
            "public-domain",
        )
        .unwrap();
        store.insert_item(&item).unwrap();

        // chorus key = slugified label + 1-based index of its position (2)
        let (idx, sections) = store.get_song_sections("song-x", "chorus-2").unwrap().unwrap();
        assert_eq!(idx, 1);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[1].lines, vec!["praise the Lord"]);
        assert_eq!(sections[0].lines, vec!["line one", "line two"]); // line breaks preserved
    }
}

// ---- Events -----------------------------------------------------------------

/// Emit a slot's current content to output windows and the operator UI.
pub fn emit_slot(app: &AppHandle, slot: u8, mgr: &DisplayManager) {
    let view = {
        let slots = mgr.slots.lock();
        let s = &slots[(slot as usize).clamp(1, SLOT_COUNT) - 1];
        SlotView {
            slot,
            monitor: s.monitor.clone(),
            mode: s.mode,
            blank: s.blank,
            window_open: app
                .get_webview_window(&format!("display-{slot}"))
                .is_some(),
            degraded: s.degraded,
            active: slot == mgr.active_display(),
            content: content_event(slot, s),
        }
    };
    let _ = app.emit("display-update", &view);
}

pub fn emit_all_slots(app: &AppHandle, mgr: &DisplayManager) {
    for slot in 1..=SLOT_COUNT as u8 {
        emit_slot(app, slot, mgr);
    }
}
