//! Bundled library seeding: KJV + ASV + WEB Bibles + public-domain hymns.
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
const WEB_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../resources/web.json"
));
const HYMNS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../resources/hymns.json"
));

/// Bump when the bundled Bible text changes materially; existing databases
/// re-seed their Bibles (hymns and user content untouched).
pub const BIBLE_TEXT_VERSION: i64 = 4;

/// Bump when bundled non-Bible seeds change; older databases re-seed the
/// additions (additive, existing items refreshed by id).
pub const SLIDES_VERSION: i64 = 3;

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
    seed_bible(tx, WEB_JSON, "WEB")?;
    Ok(())
}

/// Legacy entry point (empty database): bibles + hymns.
pub fn seed_kjv(tx: &Transaction) -> Result<(), ContentError> {
    seed_bibles(tx)
}

/// Seed the built-in starter slide sets — editable examples so the Slide
/// category is never empty. Re-runs refresh these ids in place; user-made
/// slides are never touched.
pub fn seed_slides(tx: &Transaction) -> Result<(), ContentError> {
    let sets: [(&str, &str, &str, &[(&str, &[&str])]); 3] = [
        (
            "slide-welcome",
            "Welcome",
            "Starter set — edit for your church",
            &[("Welcome", &["Welcome!", "We're glad you're here"])],
        ),
        (
            "slide-announcements",
            "Announcements",
            "Starter set — edit for your church",
            &[
                ("Announcement 1", &["Bible study", "Friday · 6 pm"]),
                ("Announcement 2", &["Choir practice", "Saturday · 10 am"]),
            ],
        ),
        (
            "slide-sermon-points",
            "Sermon points",
            "Starter set — edit for your church",
            &[
                ("Point 1", &["1. God's grace reaches everyone"]),
                ("Point 2", &["2. Grace transforms us"]),
                ("Point 3", &["3. Grace sends us out"]),
            ],
        ),
    ];

    for (id, title, note, sections) in sets {
        // Refresh in place: these three ids are ours to manage.
        tx.execute("DELETE FROM search_index WHERE item_id = ?1", rusqlite::params![id])?;
        tx.execute("DELETE FROM search_fts WHERE item_id = ?1", rusqlite::params![id])?;
        tx.execute("DELETE FROM content_items WHERE id = ?1", rusqlite::params![id])?;

        let secs: Vec<serde_json::Value> = sections
            .iter()
            .map(|(label, lines)| {
                json!({
                    "label": label,
                    "lines": lines.iter().map(|l| l.to_string()).collect::<Vec<_>>(),
                })
            })
            .collect();
        tx.execute(
            "INSERT INTO content_items
                (id, item_type, title, language, license, visibility, metadata, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id,
                "slide",
                title,
                "en",
                "public-domain",
                "public",
                json!({ "note": note }).to_string(),
                json!({ "sections": secs }).to_string(),
            ],
        )?;

        for (i, (label, lines)) in sections.iter().enumerate() {
            let text = lines.join(" ");
            let key = super::model::section_key(label, i);
            tx.execute(
                "INSERT INTO search_index (item_id, section_key, section_label, text)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, key, label, text],
            )?;
            tx.execute(
                "INSERT INTO search_fts (text, section_label, item_id, section_key)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![text, label, id, key],
            )?;
        }
    }
    Ok(())
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
