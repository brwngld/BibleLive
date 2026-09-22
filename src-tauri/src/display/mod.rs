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
    /// Custom slides — rendered like scripture but stepped line by line.
    Slide,
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
    /// Optional background image (absolute file path) rendered behind the
    /// text — the base of the theme system. Falls back to `bg_color`.
    #[serde(default)]
    pub bg_image: Option<String>,
    /// Text block alignment on the output screen: "center" | "left".
    #[serde(default = "default_align")]
    pub align: String,
    /// Soft shadow behind the text for legibility over busy images.
    #[serde(default = "default_text_shadow")]
    pub text_shadow: bool,
    /// Transition when the content changes: "none" | "fade" | "slide".
    #[serde(default = "default_transition")]
    pub transition: String,
}

fn default_align() -> String {
    "center".into()
}

fn default_text_shadow() -> bool {
    true
}

fn default_transition() -> String {
    "none".into()
}

impl Default for SlotStyle {
    fn default() -> Self {
        Self {
            font_family: "Georgia, 'Times New Roman', serif".into(),
            font_size: 6.5,
            text_color: "#ffffff".into(),
            bg_color: "#000000".into(),
            bg_image: None,
            align: default_align(),
            text_shadow: true,
            transition: default_transition(),
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
        if self.align != "left" {
            self.align = "center".into();
        }
        if self.transition != "fade" && self.transition != "slide" {
            self.transition = "none".into();
        }
        self.bg_image = self
            .bg_image
            .take()
            .filter(|p| !p.trim().is_empty());
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
    /// How many lines of the current section are revealed. `usize::MAX`
    /// means all (scripture, lyrics). Slides start at 1 and grow with
    /// Next — the line-by-line reveal.
    reveal: usize,
}

impl SectionedContent {
    pub fn new(item_id: String, kind: RenderKind, title: String, sections: Vec<LabeledSection>) -> Self {
        Self { item_id, kind, title, sections, index: 0, reveal: usize::MAX }
    }

    pub fn with_index(item_id: String, kind: RenderKind, title: String, sections: Vec<LabeledSection>, index: usize) -> Self {
        Self { item_id, kind, title, sections, index, reveal: usize::MAX }
    }

    /// Slides start showing only the first line of the section.
    pub fn revealing_first_line(mut self) -> Self {
        self.reveal = 1;
        self
    }

    pub fn current_key(&self) -> Option<&str> {
        self.sections
            .get(self.index.min(self.sections.len().saturating_sub(1)))
            .map(|s| s.key.as_str())
    }

    /// Jump to the section with this key (verse keys are identical across
    /// translations, so this keeps a paired version in lockstep). Returns
    /// false when the key is absent — index is left where it was.
    pub fn seek_key(&mut self, key: &str) -> bool {
        match self.sections.iter().position(|s| s.key == key) {
            Some(i) => {
                self.index = i;
                true
            }
            None => false,
        }
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

/// The second Bible column of a paired two-version display.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairEvent {
    pub version: String,
    pub label: String,
    pub lines: Vec<String>,
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
    /// Translation tag of the primary scripture ("KJV"), set when paired.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Second column: same verse in the paired translation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair: Option<PairEvent>,
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
    /// Configured second Bible version for this slot: None | "kjv" | "asv".
    pub pair_version: Option<String>,
    pub content: SlotContentEvent,
}

struct SlotState {
    monitor: Option<String>,
    mode: DisplayMode,
    blank: bool,
    degraded: bool,
    style: SlotStyle,
    content: Option<SlotContent>,
    /// Same scripture in the paired translation, stepped in lockstep.
    pair: Option<SectionedContent>,
    /// The configured pairing for this slot ("kjv"/"asv"), persisted.
    pair_version: Option<String>,
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
            pair: None,
            pair_version: None,
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
        version: None,
        pair: None,
    }
}

/// "John (KJV)" → "KJV" — the translation tag shown over a paired column.
/// Only bible titles carry a parenthetical, and only scripture is paired.
fn version_tag(title: &str) -> Option<String> {
    let t = title.trim();
    t.ends_with(')')
        .then(|| {
            t.rfind('(')
                .map(|i| t[i + 1..t.len() - 1].trim().to_string())
        })
        .flatten()
        .filter(|s| !s.is_empty())
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
            version: None,
            pair: None,
        },
        Some(SlotContent::Sections(c)) => {
            let sec = &c.sections[c.index.min(c.sections.len() - 1)];
            let shown = if c.reveal >= sec.lines.len() {
                &sec.lines[..]
            } else {
                &sec.lines[..c.reveal.max(1)]
            };
            let page = if c.kind == RenderKind::Slide && c.reveal != usize::MAX {
                // Slides reveal line by line: show both positions.
                let line = c.reveal.min(sec.lines.len());
                format!("{} of {} · line {} of {}", c.index + 1, c.sections.len(), line, sec.lines.len())
            } else {
                format!("{} of {}", c.index + 1, c.sections.len())
            };
            let mut ev = SlotContentEvent {
                slot,
                kind: format!("{:?}", c.kind).to_lowercase(),
                title: c.title.clone(),
                label: sec.label.clone(),
                lines: shown.to_vec(),
                page: Some(page),
                image_path: None,
                video_path: None,
                blank: false,
                style: state.style.clone(),
                version: None,
                pair: None,
            };
            if c.kind == RenderKind::Scripture {
                if let Some(pair) = &state.pair {
                    let psec = &pair.sections[pair.index.min(pair.sections.len() - 1)];
                    ev.pair = Some(PairEvent {
                        version: version_tag(&pair.title).unwrap_or_default(),
                        label: psec.label.clone(),
                        lines: psec.lines.clone(),
                    });
                    ev.version = version_tag(&c.title);
                }
            }
            ev
        }
    };
    if state.blank {
        ev.kind = "blank".into();
        ev.blank = true;
        ev.pair = None;
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
                pair_version: s.pair_version.clone(),
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
        st.pair = None; // stale pairing — the projection path re-attaches
        st.blank = false;
    }

    // ---- Two-version pairing ----------------------------------------------

    /// The configured second translation for this slot: None | "kjv" | "asv".
    pub fn pair_version_of(&self, slot: u8) -> Option<String> {
        self.slots.lock()[(slot as usize).clamp(1, SLOT_COUNT) - 1]
            .pair_version
            .clone()
    }

    pub fn set_pair_version(&self, slot: u8, version: Option<String>) {
        let v = version.filter(|v| v == "kjv" || v == "asv" || v == "web");
        self.slots.lock()[(slot as usize).clamp(1, SLOT_COUNT) - 1].pair_version = v;
    }

    /// Load persisted pair settings at startup (missing keys keep Off).
    pub fn load_pair_versions(&self, store: &ContentStore) {
        for slot in 1..=SLOT_COUNT as u8 {
            if let Ok(Some(raw)) = store.get_setting(&format!("display_pair_{slot}")) {
                self.set_pair_version(slot, Some(raw));
            }
        }
    }

    /// Attach the companion scripture and line it up with the primary's
    /// current verse. None removes the second column.
    pub fn set_pair(&self, slot: u8, pair: Option<SectionedContent>) {
        let mut slots = self.slots.lock();
        let st = &mut slots[(slot as usize).clamp(1, SLOT_COUNT) - 1];
        st.pair = pair;
        Self::sync_pair_locked(st);
    }

    fn sync_pair_locked(st: &mut SlotState) {
        let Some(SlotContent::Sections(primary)) = &st.content else {
            return;
        };
        let Some(key) = primary.current_key().map(str::to_string) else {
            return;
        };
        if let Some(pair) = &mut st.pair {
            pair.seek_key(&key);
        }
    }

    /// Item id + current verse key of the scripture on a slot — used to
    /// re-resolve a pairing when the operator toggles it mid-show.
    pub fn current_scripture(&self, slot: u8) -> Option<(String, String)> {
        let slots = self.slots.lock();
        match &slots[(slot as usize).clamp(1, SLOT_COUNT) - 1].content {
            Some(SlotContent::Sections(c)) if c.kind == RenderKind::Scripture => {
                c.current_key().map(|k| (c.item_id.clone(), k.to_string()))
            }
            _ => None,
        }
    }

    /// Snapshot the paired column so an automatic show can restore it.
    pub fn pair_snapshot(&self, slot: u8) -> Option<SectionedContent> {
        self.slots.lock()[(slot as usize).clamp(1, SLOT_COUNT) - 1]
            .pair
            .clone()
    }

    /// Restore a full slot (primary + pair) — the undo path.
    pub fn restore_sections(
        &self,
        slot: u8,
        primary: SectionedContent,
        pair: Option<SectionedContent>,
    ) {
        let mut slots = self.slots.lock();
        let st = &mut slots[(slot as usize).clamp(1, SLOT_COUNT) - 1];
        st.content = Some(SlotContent::Sections(primary));
        st.pair = pair;
        st.blank = false;
        Self::sync_pair_locked(st);
    }

    /// First slot set to AUTO (1-based), if any — the default target
    /// Automatic-mode voice suggestions project themselves onto.
    pub fn find_auto_slot(&self) -> Option<u8> {
        self.slots
            .lock()
            .iter()
            .enumerate()
            .find(|(_, s)| s.mode == DisplayMode::Auto)
            .map(|(i, _)| (i + 1) as u8)
    }

    /// Resolve where Automatic-mode suggestions project. `"auto"` (the
    /// default) picks the first AUTO slot; `"1"`..=`"5"` pins a specific
    /// slot regardless of its mode — except LOCK, which always protects a
    /// slot from automation. Anything unresolvable → None (cards only).
    pub fn auto_target_slot(&self, target: &str) -> Option<u8> {
        let t = target.trim();
        if t.eq_ignore_ascii_case("auto") {
            return self.find_auto_slot();
        }
        let n: u8 = t.parse().ok()?;
        if (1..=SLOT_COUNT as u8).contains(&n) {
            let mode = self.slots.lock()[(n - 1) as usize].mode;
            (mode != DisplayMode::Lock).then_some(n)
        } else {
            None
        }
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
        st.pair = None; // media replaces scripture; pairing re-attaches later
        st.blank = false;
    }

    pub fn mark_degraded(&self, slot: u8, degraded: bool) {
        let mut slots = self.slots.lock();
        slots[(slot as usize).clamp(1, SLOT_COUNT) - 1].degraded = degraded;
    }

    /// Advance within the sectioned content. Slides reveal line by line:
    /// Next adds a line, then moves on; Prev takes a line back, then the
    /// previous slide's last reveal. The paired version (if any) follows
    /// verse-for-verse. Returns true if moved.
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
        // Line-by-line reveal for slides (before crossing to another
        // slide); sets stored with revealLines: false step whole slides.
        if c.kind == RenderKind::Slide && c.reveal != usize::MAX {
            let len = c.sections[c.index.min(c.sections.len() - 1)].lines.len();
            if delta > 0 && c.reveal < len {
                c.reveal += 1;
                return true;
            }
            if delta < 0 && c.reveal > 1 {
                c.reveal -= 1;
                return true;
            }
        }
        let next = c.index as i32 + delta;
        if next < 0 || next >= c.sections.len() as i32 {
            return false;
        }
        c.index = next as usize;
        c.reveal = if c.kind == RenderKind::Slide {
            // Entering forward shows the first line; coming back shows the
            // whole slide (you were heading to the previous one).
            if delta < 0 { c.sections[c.index].lines.len().max(1) } else { 1 }
        } else {
            usize::MAX
        };
        let key = c.current_key().map(str::to_string);
        if let (Some(key), Some(pair)) = (key, &mut st.pair) {
            pair.seek_key(&key); // absent key: pair stays on its verse
        }
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

    /// Pinned auto-show targets: "auto" keeps the first-AUTO behavior,
    /// "1".."5" pins a slot, and LOCK always protects the pinned slot.
    #[test]
    fn auto_target_routing() {
        let mgr = DisplayManager::new();
        assert_eq!(mgr.auto_target_slot("auto"), None, "no AUTO slot → no target");

        mgr.set_mode(4, DisplayMode::Auto);
        assert_eq!(mgr.auto_target_slot("auto"), Some(4));
        assert_eq!(mgr.auto_target_slot("2"), Some(2), "pin works on a MANUAL slot");

        mgr.set_mode(2, DisplayMode::Lock);
        assert_eq!(mgr.auto_target_slot("2"), None, "LOCK protects the pinned slot");
        assert_eq!(mgr.auto_target_slot("auto"), Some(4), "auto routing ignores LOCK slots");

        assert_eq!(mgr.auto_target_slot("9"), None, "out of range");
        assert_eq!(mgr.auto_target_slot("bogus"), None, "garbage → no target");
    }

    /// Theme fields survive validation: legacy stored styles (without the
    /// new fields) deserialize to defaults, and empty images clear.
    #[test]
    fn style_theme_fields_roundtrip() {
        let legacy = serde_json::json!({
            "fontFamily": "Georgia, serif",
            "fontSize": 5.0,
            "textColor": "#ffffff",
            "bgColor": "#000000"
        });
        let mut s: SlotStyle = serde_json::from_value(legacy).expect("legacy style deserializes");
        assert_eq!(s.align, "center");
        assert!(s.bg_image.is_none());
        assert!(s.text_shadow, "shadow defaults on to keep the classic look");
        assert_eq!(s.transition, "none");
        s.transition = "diagonal".into();
        s.validate();
        assert_eq!(s.transition, "none", "unknown transition falls back");
        s.transition = "fade".into();
        s.validate();
        assert_eq!(s.transition, "fade", "valid transition survives");
        s.validate();
        assert_eq!(s.align, "center");

        s.align = "left".into();
        s.bg_image = Some("   ".into());
        s.validate();
        assert_eq!(s.align, "left");
        assert!(s.bg_image.is_none(), "blank path clears");

        s.align = "diagonal".into();
        s.validate();
        assert_eq!(s.align, "center", "unknown alignment falls back");

        let round: SlotStyle =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(round.align, s.align);
        assert_eq!(round.text_shadow, s.text_shadow);
    }

    fn sec(key: &str, label: &str, text: &str) -> LabeledSection {
        LabeledSection { key: key.into(), label: label.into(), lines: vec![text.into()] }
    }

    fn scripture(id: &str, title: &str, keys: &[&str], index: usize) -> SectionedContent {
        SectionedContent::with_index(
            id.into(),
            RenderKind::Scripture,
            title.into(),
            keys.iter()
                .enumerate()
                .map(|(i, k)| sec(k, &format!("John 3:{}", 16 + i), &format!("verse {k}")))
                .collect(),
            index,
        )
    }

    /// Two-version display: the companion follows verse-for-verse while
    /// stepping, and holds its verse when its translation lacks a key.
    #[test]
    fn pair_display_and_lockstep() {
        let mgr = DisplayManager::new();
        let keys = ["john.3.16", "john.3.17", "john.3.18"];
        mgr.set_sections(1, scripture("bible-kjv-john", "John (KJV)", &keys, 0));
        mgr.set_pair(
            1,
            Some(scripture("bible-asv-john", "John (ASV)", &keys, 0)),
        );

        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.version.as_deref(), Some("KJV"));
        let p = ev.pair.as_ref().expect("pair column on the event");
        assert_eq!(p.version, "ASV");
        assert_eq!(p.label, "John 3:16");

        mgr.step(1, 2);
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "John 3:18");
        assert_eq!(
            ev.pair.as_ref().unwrap().label,
            "John 3:18",
            "pair steps in lockstep"
        );

        // Gappy companion: verse 3:17 missing — pair holds its verse.
        let gappy = ["john.3.16", "john.3.18"];
        mgr.step(1, -2); // back to 3:16, pair re-syncs
        mgr.set_pair(1, Some(scripture("bible-asv-john", "John (ASV)", &gappy, 0)));
        mgr.step(1, 1); // primary → 3:17, pair has no 3:17
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "John 3:17");
        assert_eq!(ev.pair.as_ref().unwrap().label, "John 3:16", "missing key holds position");

        // Restore path re-syncs both columns.
        mgr.restore_sections(
            1,
            scripture("bible-kjv-john", "John (KJV)", &keys, 1),
            Some(scripture("bible-asv-john", "John (ASV)", &keys, 2)),
        );
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "John 3:17");
        assert_eq!(ev.pair.as_ref().unwrap().label, "John 3:17", "restore syncs the pair");
    }

    /// Pairing only applies to scripture; blanks and lyrics never show a
    /// second column, and unknown translations are rejected at the gate.
    #[test]
    fn pair_setting_guards() {
        let mgr = DisplayManager::new();
        mgr.set_pair_version(2, Some("niv".into()));
        assert_eq!(mgr.pair_version_of(2), None, "unknown translation rejected");
        mgr.set_pair_version(2, Some("asv".into()));
        assert_eq!(mgr.pair_version_of(2).as_deref(), Some("asv"));

        let keys = ["john.3.16"];
        mgr.set_sections(2, scripture("bible-kjv-john", "John (KJV)", &keys, 0));
        mgr.set_pair(2, Some(scripture("bible-asv-john", "John (ASV)", &keys, 0)));
        mgr.set_blank(2, true);
        let ev = content_event(2, &mgr.slots.lock()[1]);
        assert!(ev.pair.is_none(), "blank hides the second column");
        assert!(ev.blank);

        mgr.set_blank(2, false);
        let ev = content_event(2, &mgr.slots.lock()[1]);
        assert!(ev.pair.is_some(), "unblank brings the column back");

        let song = SectionedContent::new(
            "hymn-x".into(),
            RenderKind::Lyrics,
            "Amazing Grace".into(),
            vec![sec("v1", "1", "line")],
        );
        mgr.set_sections(2, song);
        let ev = content_event(2, &mgr.slots.lock()[1]);
        assert!(ev.pair.is_none(), "lyrics never pair");
    }

    #[test]
    fn version_tag_parsing() {
        assert_eq!(version_tag("John (KJV)").as_deref(), Some("KJV"));
        assert_eq!(version_tag("1 Corinthians (ASV)").as_deref(), Some("ASV"));
        assert_eq!(version_tag("Amazing Grace"), None);
        assert_eq!(version_tag("Weird ("), None);
    }

    /// Slides reveal line by line: Next adds a line, then moves to the
    /// next slide (first line only); Prev takes a line back, then steps
    /// to the previous slide's full reveal. Scripture/lyrics are unchanged
    /// (whole section at once).
    #[test]
    fn slide_reveals_line_by_line() {
        let mgr = DisplayManager::new();
        let slides = SectionedContent::new(
            "slide-x".into(),
            RenderKind::Slide,
            "Points".into(),
            vec![
                sec("p1", "Point 1", "first"),
                sec("p2", "Point 2", "second"),
                sec("p3", "Point 3", "third"),
            ],
        )
        .revealing_first_line();
        // Give slide 1 three lines; the test helper only writes one line
        // per section, so patch it directly through set_sections payload.
        let mut multi = slides.clone();
        if let Some(first) = multi.sections.get_mut(0) {
            first.lines = vec!["line one".into(), "line two".into(), "line three".into()];
        }
        mgr.set_sections(1, multi);

        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.lines, vec!["line one".to_string()], "starts on the first line");
        assert_eq!(ev.page.as_deref(), Some("1 of 3 · line 1 of 3"));

        assert!(mgr.step(1, 1));
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.lines, vec!["line one".to_string(), "line two".to_string()]);
        assert_eq!(ev.page.as_deref(), Some("1 of 3 · line 2 of 3"));

        // Past the last line → next slide, first line only.
        assert!(mgr.step(1, 1));
        assert!(mgr.step(1, 1));
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "Point 2");
        assert_eq!(ev.lines, vec!["second".to_string()]);
        assert_eq!(ev.page.as_deref(), Some("2 of 3 · line 1 of 1"));

        // Back: one-line slide → previous slide fully revealed.
        assert!(mgr.step(1, -1));
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "Point 1");
        assert_eq!(ev.lines.len(), 3, "previous slide restores full reveal");
        assert_eq!(ev.page.as_deref(), Some("1 of 3 · line 3 of 3"));

        // Scripture still shows whole sections and pages plainly.
        mgr.set_sections(2, scripture("bible-kjv-john", "John (KJV)", &["john.3.16", "john.3.17"], 0));
        assert!(mgr.step(2, 1));
        let ev = content_event(2, &mgr.slots.lock()[1]);
        assert_eq!(ev.page.as_deref(), Some("2 of 2"), "no line counting for scripture");
    }

    /// A slide set saved with revealLines: false shows all lines at once
    /// and Next/Prev moves whole slides (no bullet-build).
    #[test]
    fn slide_without_reveal_steps_whole_slides() {
        let mgr = DisplayManager::new();
        let mut multi = SectionedContent::new(
            "slide-x".into(),
            RenderKind::Slide,
            "Welcome".into(),
            vec![
                sec("s1", "Slide 1", "first"),
                sec("s2", "Slide 2", "second"),
            ],
        ); // no revealing_first_line() — all lines visible
        if let Some(first) = multi.sections.get_mut(0) {
            first.lines = vec!["line one".into(), "line two".into()];
        }
        mgr.set_sections(1, multi);

        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.lines.len(), 2, "all lines visible immediately");
        assert_eq!(ev.page.as_deref(), Some("1 of 2"), "no line counting");

        assert!(mgr.step(1, 1));
        let ev = content_event(1, &mgr.slots.lock()[0]);
        assert_eq!(ev.label, "Slide 2", "Next jumps straight to the next slide");
    }

    /// Theme templates: upsert by name, delete, and style validation on
    /// the way in.
    #[test]
    fn theme_templates_roundtrip() {
        let store = test_store("templates");
        let mut style = SlotStyle::default();
        style.bg_color = "#0a1230".into();
        style.transition = "fade".into();

        assert!(load_templates(&store).is_empty(), "fresh store has none");
        save_template(&store, "Christmas", style.clone()).unwrap();
        save_template(&store, "Sunday", SlotStyle::default()).unwrap();

        let tpls = load_templates(&store);
        assert_eq!(tpls.len(), 2);
        assert_eq!(tpls[0].name, "Christmas");
        assert_eq!(tpls[0].style.bg_color, "#0a1230");
        assert_eq!(tpls[0].style.transition, "fade");

        // Upsert by name keeps one entry, with the new look.
        let mut updated = SlotStyle::default();
        updated.bg_image = Some("C:/bg.png".into());
        save_template(&store, "Christmas", updated).unwrap();
        let tpls = load_templates(&store);
        assert_eq!(tpls.len(), 2);
        let xmas = tpls.iter().find(|t| t.name == "Christmas").unwrap();
        assert_eq!(xmas.style.bg_image.as_deref(), Some("C:/bg.png"));

        // Invalid input rejected, nothing stored.
        assert!(save_template(&store, "   ", SlotStyle::default()).is_err());

        delete_template(&store, "Sunday").unwrap();
        let tpls = load_templates(&store);
        assert_eq!(tpls.len(), 1);
        assert_eq!(tpls[0].name, "Christmas");
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

// ---- Theme templates -------------------------------------------------------

const TEMPLATES_KEY: &str = "display_templates";

/// A named, reusable output look: background image + colors + fonts +
/// alignment + shadow + transition. Saved from one display's style panel,
/// applied to any display in one click.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeTemplate {
    pub name: String,
    pub style: SlotStyle,
}

pub fn load_templates(store: &ContentStore) -> Vec<ThemeTemplate> {
    store
        .get_setting(TEMPLATES_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Save (upsert by name) the given style as a named template.
pub fn save_template(store: &ContentStore, name: &str, mut style: SlotStyle) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("template name is empty".into());
    }
    style.validate();
    let mut templates = load_templates(store);
    templates.retain(|t| t.name != name);
    templates.push(ThemeTemplate { name: name.to_string(), style });
    store
        .set_setting(TEMPLATES_KEY, &serde_json::to_string(&templates).unwrap())
        .map_err(|e| e.to_string())
}

pub fn delete_template(store: &ContentStore, name: &str) -> Result<(), String> {
    let mut templates = load_templates(store);
    templates.retain(|t| t.name != name);
    store
        .set_setting(TEMPLATES_KEY, &serde_json::to_string(&templates).unwrap())
        .map_err(|e| e.to_string())
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
            pair_version: s.pair_version.clone(),
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
