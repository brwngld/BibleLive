//! Bundled library seeding: KJV + ASV Bibles + public-domain hymn collection.
//! Runs once on first launch (when the content database is empty), and the
//! Bible half re-runs when the bundled text is newer than the seeded one.

use super::model::{slugify, strip_braces, ContentItem, ItemType};
use super::ContentError;
use rusqlite::Transaction;
use serde_json::json;

const KJV_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../resources/kjv.json"
));
const ASV_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../resources/asv.json"
));
const HYMNS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../resources/hymns.json"
));

/// Bump when the bundled Bible text changes materially; existing databases
/// re-seed their Bibles (hymns and user content untouched).
pub const BIBLE_TEXT_VERSION: i64 = 2;

#[derive(serde::Deserialize)]
struct KjvBook {
    name: String,
    #[serde(default)]
    chapters: Vec<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct HymnSeed {
    title: String,
    #[serde(default = "default_language")]
    language: String,
    license: String,
    #[serde(default)]
    metadata: serde_json::Value,
    sections: Vec<serde_json::Value>,
}

fn default_language() -> String {
    "en".to_string()
}

/// Seed one Bible translation — one content item per book (66 items), every
/// verse indexed.
fn seed_bible(tx: &Transaction, file: &str, translation: &str) -> Result<(), ContentError> {
    let text = file.trim_start_matches('\u{feff}');
    let books: Vec<KjvBook> = serde_json::from_str(text)?;

    for (book_num, book) in books.iter().enumerate() {
        let slug = slugify(&book.name);
        let id = format!("bible-{}-{}", translation.to_lowercase(), slug);
        let chapters: Vec<Vec<String>> = book
            .chapters
            .iter()
            .map(|ch| ch.iter().map(|v| strip_braces(v)).collect())
            .collect();
        let verse_count: usize = chapters.iter().map(Vec::len).sum();

        let item = ContentItem {
            id: id.clone(),
            item_type: ItemType::Bible,
            title: format!("{} ({})", book.name, translation),
            language: "en".to_string(),
            license: "public-domain".to_string(),
            visibility: "public".to_string(),
            metadata: json!({
                "translation": translation,
                "book": book.name,
                "bookNumber": book_num + 1,
            }),
            body: json!({
                "translation": translation,
                "book": book.name,
                "chapters": chapters,
            }),
        };

        tx.execute(
            "INSERT INTO content_items
                (id, item_type, title, language, license, visibility, metadata, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                item.id,
                item.item_type.as_str(),
                item.title,
                item.language,
                item.license,
                item.visibility,
                serde_json::to_string(&item.metadata)?,
                serde_json::to_string(&item.body)?,
            ],
        )?;

        for s in item.search_sections() {
            tx.execute(
                "INSERT INTO search_index (item_id, section_key, section_label, text)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![&id, s.key, s.label, s.text],
            )?;
            tx.execute(
                "INSERT INTO search_fts (text, section_label, item_id, section_key)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![s.text, s.label, &id, s.key],
            )?;
        }

        let _ = verse_count; // per-book counts are derivable from the body
    }

    Ok(())
}

/// Seed both bundled translations.
pub fn seed_bibles(tx: &Transaction) -> Result<(), ContentError> {
    seed_bible(tx, KJV_JSON, "KJV")?;
    seed_bible(tx, ASV_JSON, "ASV")?;
    Ok(())
}

/// Legacy entry point (empty database): bibles + hymns.
pub fn seed_kjv(tx: &Transaction) -> Result<(), ContentError> {
    seed_bibles(tx)
}

/// Seed the public-domain hymn collection.
pub fn seed_hymns(tx: &Transaction) -> Result<(), ContentError> {
    #[derive(serde::Deserialize)]
    struct HymnFile {
        items: Vec<HymnSeed>,
    }
    let file: HymnFile = serde_json::from_str(HYMNS_JSON)?;

    for hymn in file.items {
        let id = format!("hymn-{}", slugify(&hymn.title));
        let item = ContentItem {
            id,
            item_type: ItemType::Hymn,
            title: hymn.title,
            language: hymn.language,
            license: hymn.license,
            visibility: "public".to_string(),
            metadata: hymn.metadata,
            body: json!({ "sections": hymn.sections }),
        };

        tx.execute(
            "INSERT INTO content_items
                (id, item_type, title, language, license, visibility, metadata, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                item.id,
                item.item_type.as_str(),
                item.title,
                item.language,
                item.license,
                item.visibility,
                serde_json::to_string(&item.metadata)?,
                serde_json::to_string(&item.body)?,
            ],
        )?;

        for s in item.search_sections() {
            tx.execute(
                "INSERT INTO search_index (item_id, section_key, section_label, text)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![&item.id, s.key, s.label, s.text],
            )?;
            tx.execute(
                "INSERT INTO search_fts (text, section_label, item_id, section_key)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![s.text, s.label, &item.id, s.key],
            )?;
        }
    }

    Ok(())
}
