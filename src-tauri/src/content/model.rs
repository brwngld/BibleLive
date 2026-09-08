//! Content item model — the unified representation shared by all content
//! types (Bible, song, hymn, book, document).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    Bible,
    Song,
    Hymn,
    Book,
    Document,
}

impl ItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemType::Bible => "bible",
            ItemType::Song => "song",
            ItemType::Hymn => "hymn",
            ItemType::Book => "book",
            ItemType::Document => "document",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "song" => ItemType::Song,
            "hymn" => ItemType::Hymn,
            "book" => ItemType::Book,
            "document" => ItemType::Document,
            _ => ItemType::Bible,
        }
    }
}

/// A searchable/displayable section of content: a verse, a lyric stanza, a
/// paragraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub label: String,
    pub lines: Vec<String>,
}

/// A labeled, keyed section — the unit the Display Engine puts on screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabeledSection {
    pub key: String,
    pub label: String,
    pub lines: Vec<String>,
}

/// Canonical search/display key for a section at 0-based position `index`.
pub fn section_key(label: &str, index: usize) -> String {
    let slug = slugify(label);
    if slug.is_empty() {
        format!("section-{}", index + 1)
    } else {
        format!("{}-{}", slug, index + 1)
    }
}

impl Section {
    pub fn text(&self) -> String {
        self.lines.join(" ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    pub id: String,
    pub item_type: ItemType,
    pub title: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_license")]
    pub license: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub metadata: Value,
    pub body: Value,
}

fn default_language() -> String {
    "en".to_string()
}
fn default_license() -> String {
    "unknown".to_string()
}
fn default_visibility() -> String {
    "public".to_string()
}

/// Canonical key/label/text triples derived from the body for indexing.
pub struct IndexedSection {
    pub key: String,
    pub label: String,
    pub text: String,
}

impl ContentItem {
    /// Extract the searchable sections from the type-specific body.
    pub fn search_sections(&self) -> Vec<IndexedSection> {
        match self.item_type {
            ItemType::Bible => self.index_bible(),
            _ => self.index_sections(),
        }
    }

    /// Bible body: { translation, book, bookNumber, chapters: [[verses]] }
    fn index_bible(&self) -> Vec<IndexedSection> {
        let book = self
            .metadata
            .get("book")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let slug = slugify(&book);
        let mut out = Vec::new();
        if let Some(chapters) = self.body.get("chapters").and_then(Value::as_array) {
            for (ci, chapter) in chapters.iter().enumerate() {
                if let Some(verses) = chapter.as_array() {
                    for (vi, verse) in verses.iter().enumerate() {
                        let text = verse.as_str().unwrap_or("").trim().to_string();
                        if text.is_empty() {
                            continue;
                        }
                        out.push(IndexedSection {
                            key: format!("{}.{}.{}", slug, ci + 1, vi + 1),
                            label: format!("{} {}:{}", book, ci + 1, vi + 1),
                            text,
                        });
                    }
                }
            }
        }
        out
    }

    /// Song/hymn/book/document body: { sections: [{ label, lines }] }
    fn index_sections(&self) -> Vec<IndexedSection> {
        let mut out = Vec::new();
        if let Some(sections) = self.body.get("sections").and_then(Value::as_array) {
            for (i, raw) in sections.iter().enumerate() {
                let section: Section = match serde_json::from_value(raw.clone()) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let text = section.text();
                if text.trim().is_empty() {
                    continue;
                }
                let label = if section.label.trim().is_empty() {
                    format!("Section {}", i + 1)
                } else {
                    section.label.clone()
                };
                let key = section_key(&label, i);
                out.push(IndexedSection {
                    key,
                    label,
                    text,
                });
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSummary {
    pub id: String,
    pub item_type: ItemType,
    pub title: String,
    pub language: String,
    pub license: String,
    pub visibility: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub item_id: String,
    pub section_key: String,
    pub section_label: String,
    pub snippet: String,
    pub rank: f64,
    pub item_type: ItemType,
    pub item_title: String,
}

pub fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Strip KJV supplied-word braces: "{it was}" -> "it was".
pub fn strip_braces(s: &str) -> String {
    s.replace('{', "").replace('}', "")
}
