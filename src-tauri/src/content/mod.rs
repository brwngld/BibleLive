//! Content Engine — unified content store.
//!
//! One underlying Content Item model (id, type, title, language, license,
//! visibility, structured body) with a full-text search index across all
//! content types (FTS5). Bible, songs, hymns, books and documents all live
//! in `content_items`; searchable sections live in `search_index` and the
//! `search_fts` index.

pub mod import;
pub mod model;
pub mod seed;

use model::{ContentItem, ContentSummary, ItemType, LabeledSection, SearchHit};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for ContentError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS content_items (
    id          TEXT PRIMARY KEY,
    item_type   TEXT NOT NULL,                -- bible | song | hymn | book | document
    title       TEXT NOT NULL,
    language    TEXT NOT NULL DEFAULT 'en',
    license     TEXT NOT NULL DEFAULT 'unknown',
    visibility  TEXT NOT NULL DEFAULT 'public',
    metadata    TEXT NOT NULL DEFAULT '{}',   -- JSON
    body        TEXT NOT NULL,                -- JSON, type-specific structure
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS search_index (
    item_id       TEXT NOT NULL REFERENCES content_items(id) ON DELETE CASCADE,
    section_key   TEXT NOT NULL,              -- e.g. 'john.3.16' or 'verse-1'
    section_label TEXT NOT NULL,              -- e.g. 'John 3:16' or 'Verse 1'
    text          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_item ON search_index(item_id);

CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
    text,
    section_label,
    item_id UNINDEXED,
    section_key UNINDEXED,
    tokenize = 'porter unicode61'
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS service_sessions (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at   TEXT
);

CREATE TABLE IF NOT EXISTS session_items (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES service_sessions(id) ON DELETE CASCADE,
    at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    slot       INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    title      TEXT NOT NULL,
    label      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_session_items ON session_items(session_id);
"#;

/// Shared, thread-safe handle to the SQLite content database.
#[derive(Clone)]
pub struct ContentStore {
    conn: Arc<parking_lot::Mutex<Connection>>,
}

impl ContentStore {
    /// Open (creating if needed) the content database in the app data dir
    /// and seed the bundled library on first run.
    pub fn open() -> Result<Self, ContentError> {
        let dir = default_data_dir()?;
        Self::open_at(dir)
    }

    /// Open the database in a specific directory (used by tests and, later,
    /// by profile handling).
    pub fn open_at(dir: PathBuf) -> Result<Self, ContentError> {
        std::fs::create_dir_all(&dir)?;
        let conn = Connection::open(dir.join("biblelive.db"))?;

        // Pre-M1 databases shipped a different search_fts shape; rebuild it.
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            conn.execute_batch("DROP TABLE IF EXISTS search_fts;")?;
        }

        conn.execute_batch(SCHEMA)?;
        conn.execute_batch("PRAGMA user_version = 1;")?;
        let store = Self {
            conn: Arc::new(parking_lot::Mutex::new(conn)),
        };

        if store.is_empty()? {
            store.seed()?;
        }

        Ok(store)
    }

    pub fn is_ready(&self) -> bool {
        self.conn.try_lock().is_some()
    }

    /// Direct connection handle — used by the Service Control query layer.
    pub fn conn_handle(&self) -> Arc<parking_lot::Mutex<Connection>> {
        self.conn.clone()
    }

    fn is_empty(&self) -> Result<bool, ContentError> {
        let n: i64 = self
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM content_items", [], |r| r.get(0))?;
        Ok(n == 0)
    }

    fn seed(&self) -> Result<(), ContentError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        seed::seed_kjv(&tx)?;
        seed::seed_hymns(&tx)?;
        tx.commit()?;
        Ok(())
    }

    // ---- CRUD ------------------------------------------------------------

    pub fn insert_item(&self, item: &ContentItem) -> Result<(), ContentError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO content_items
                (id, item_type, title, language, license, visibility, metadata, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
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
            insert_search_row(&tx, &item.id, &s.key, &s.label, &s.text)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_item(&self, item: &ContentItem) -> Result<(), ContentError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE content_items
             SET item_type = ?2, title = ?3, language = ?4, license = ?5,
                 visibility = ?6, metadata = ?7, body = ?8,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
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
        if changed == 0 {
            return Err(ContentError::Other(format!("item {} not found", item.id)));
        }
        // Re-index sections from scratch for this item.
        tx.execute("DELETE FROM search_index WHERE item_id = ?1", params![item.id])?;
        tx.execute("DELETE FROM search_fts WHERE item_id = ?1", params![item.id])?;
        for s in item.search_sections() {
            insert_search_row(&tx, &item.id, &s.key, &s.label, &s.text)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_item(&self, id: &str) -> Result<(), ContentError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM search_fts WHERE item_id = ?1", params![id])?;
        conn.execute("DELETE FROM content_items WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_item(&self, id: &str) -> Result<Option<ContentItem>, ContentError> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT id, item_type, title, language, license, visibility, metadata, body
                 FROM content_items WHERE id = ?1",
                params![id],
                |r| {
                    Ok(ContentItem {
                        id: r.get(0)?,
                        item_type: ItemType::from_str_lossy(&r.get::<_, String>(1)?),
                        title: r.get(2)?,
                        language: r.get(3)?,
                        license: r.get(4)?,
                        visibility: r.get(5)?,
                        metadata: parse_json_column(r.get::<_, String>(6)?)?,
                        body: parse_json_column(r.get::<_, String>(7)?)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    pub fn list_items(
        &self,
        item_type: Option<&ItemType>,
        title_query: Option<&str>,
        testament: Option<&str>,
        sort: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ContentSummary>, ContentError> {
        let conn = self.conn.lock();
        let mut sql = String::from(
            "SELECT id, item_type, title, language, license, visibility, created_at
             FROM content_items WHERE 1=1",
        );
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(t) = item_type {
            args.push(Box::new(t.as_str().to_string()));
            sql.push_str(&format!(" AND item_type = ?{}", args.len()));
        }
        if let Some(q) = title_query {
            if !q.trim().is_empty() {
                args.push(Box::new(format!("%{}%", q.trim())));
                sql.push_str(&format!(" AND title LIKE ?{}", args.len()));
            }
        }
        if let Some(t) = testament {
            // Old Testament = books 1–39, New Testament = 40–66 (canonical
            // bookNumber stored in metadata at seed time).
            let clause = match t {
                "ot" => " AND (item_type != 'bible' OR CAST(json_extract(metadata, '$.bookNumber') AS INTEGER) BETWEEN 1 AND 39)",
                "nt" => " AND (item_type != 'bible' OR CAST(json_extract(metadata, '$.bookNumber') AS INTEGER) BETWEEN 40 AND 66)",
                _ => "",
            };
            sql.push_str(clause);
        }
        // Canonical order = Bible books Genesis→Revelation first (by
        // bookNumber), then everything else alphabetically.
        sql.push_str(match sort {
            "title-desc" => " ORDER BY title COLLATE NOCASE DESC",
            "added-desc" => " ORDER BY created_at DESC, title COLLATE NOCASE",
            "added-asc" => " ORDER BY created_at ASC, title COLLATE NOCASE",
            "canonical" => " ORDER BY CASE WHEN item_type = 'bible' THEN 0 ELSE 1 END,
                CASE WHEN item_type = 'bible'
                     THEN CAST(json_extract(metadata, '$.bookNumber') AS INTEGER) END,
                title COLLATE NOCASE",
            _ => " ORDER BY title COLLATE NOCASE ASC",
        });
        args.push(Box::new(limit));
        sql.push_str(&format!(" LIMIT ?{}", args.len()));
        args.push(Box::new(offset));
        sql.push_str(&format!(" OFFSET ?{}", args.len()));

        let mut stmt = conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), |r| {
            Ok(ContentSummary {
                id: r.get(0)?,
                item_type: ItemType::from_str_lossy(&r.get::<_, String>(1)?),
                title: r.get(2)?,
                language: r.get(3)?,
                license: r.get(4)?,
                visibility: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- Search ----------------------------------------------------------

    /// Full-text search across ALL content types. Terms are AND-combined with
    /// prefix matching ("amaz*" "grac*").
    pub fn search(&self, query: &str, limit: i64) -> Result<Vec<SearchHit>, ContentError> {
        let fts_query = build_fts_query(query);
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }
        self.search_phrase(&fts_query, limit)
    }

    /// FTS search with a raw (already-quoted) FTS5 query, e.g. exact phrases
    /// used by quote matching.
    pub fn search_phrase(&self, fts_query: &str, limit: i64) -> Result<Vec<SearchHit>, ContentError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT fts.item_id, fts.section_key, fts.section_label,
                    snippet(search_fts, 0, '[', ']', '…', 14) AS snip,
                    bm25(search_fts) AS rank,
                    ci.item_type, ci.title
             FROM search_fts fts
             JOIN content_items ci ON ci.id = fts.item_id
             WHERE search_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit], |r| {
            Ok(SearchHit {
                item_id: r.get(0)?,
                section_key: r.get(1)?,
                section_label: r.get(2)?,
                snippet: r.get(3)?,
                rank: r.get(4)?,
                item_type: ItemType::from_str_lossy(&r.get::<_, String>(5)?),
                item_title: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- Section retrieval (for the Display Engine) -----------------------

    /// Fetch named sections (e.g. specific Bible verses) from the search
    /// index, returned in the order requested. Scripture verses are single
    /// lines.
    pub fn get_sections_by_keys(
        &self,
        item_id: &str,
        keys: &[String],
    ) -> Result<Vec<LabeledSection>, ContentError> {
        let conn = self.conn.lock();
        let mut out = Vec::new();
        for k in keys {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT section_label, text FROM search_index
                     WHERE item_id = ?1 AND section_key = ?2",
                    params![item_id, k],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((label, text)) = row {
                out.push(LabeledSection {
                    key: k.clone(),
                    label,
                    lines: vec![text],
                });
            }
        }
        Ok(out)
    }

    /// Fetch all sections of a song/hymn/book/document item from its body
    /// (preserving line breaks), plus the index of the requested key.
    pub fn get_song_sections(
        &self,
        item_id: &str,
        key: &str,
    ) -> Result<Option<(usize, Vec<LabeledSection>)>, ContentError> {
        let Some(item) = self.get_item(item_id)? else {
            return Ok(None);
        };
        let Some(sections) = item.body.get("sections").and_then(serde_json::Value::as_array) else {
            return Ok(None);
        };
        let mut out: Vec<LabeledSection> = Vec::new();
        let mut hit_index: Option<usize> = None;
        for (i, raw) in sections.iter().enumerate() {
            let Ok(s) = serde_json::from_value::<model::Section>(raw.clone()) else {
                continue;
            };
            let label = if s.label.trim().is_empty() {
                format!("Section {}", i + 1)
            } else {
                s.label.clone()
            };
            let k = model::section_key(&label, i);
            if k == key {
                hit_index = Some(out.len());
            }
            out.push(LabeledSection {
                key: k,
                label,
                lines: s.lines,
            });
        }
        Ok(hit_index.map(|idx| (idx, out)))
    }

    /// Fetch ALL verses of the chapter that `key` belongs to ("john.3.16" →
    /// all of John 3, in verse order), plus the index of the requested verse.
    /// Lets the operator step through the chapter with Next/Prev.
    pub fn get_chapter_sections(
        &self,
        item_id: &str,
        key: &str,
    ) -> Result<Option<(usize, Vec<LabeledSection>)>, ContentError> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 3 {
            return Ok(None);
        }
        let prefix = format!("{}.{}.", parts[0], parts[1]);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT section_key, section_label, text FROM search_index
             WHERE item_id = ?1 AND section_key LIKE ?2
             ORDER BY rowid",
        )?;
        let pattern = format!("{prefix}%");
        let rows = stmt.query_map(params![item_id, pattern], |r| {
            Ok(LabeledSection {
                key: r.get(0)?,
                label: r.get(1)?,
                lines: vec![r.get::<_, String>(2)?],
            })
        })?;
        let sections: Vec<LabeledSection> = rows.collect::<Result<Vec<_>, _>>()?;
        if sections.is_empty() {
            return Ok(None);
        }
        let index = sections.iter().position(|s| s.key == key).unwrap_or(0);
        Ok(Some((index, sections)))
    }

    // ---- Service sessions ---------------------------------------------------

    pub fn insert_session(&self, id: &str, name: &str) -> Result<(), ContentError> {
        self.conn.lock().execute(
            "INSERT INTO service_sessions (id, name, started_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![id, name],
        )?;
        Ok(())
    }

    pub fn end_session(&self, id: &str) -> Result<(), ContentError> {
        self.conn.lock().execute(
            "UPDATE service_sessions SET ended_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete_session(&self, id: &str) -> Result<(), ContentError> {
        self.conn.lock().execute(
            "DELETE FROM service_sessions WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn add_session_item(
        &self,
        session_id: &str,
        slot: u8,
        kind: &str,
        title: &str,
        label: &str,
    ) -> Result<(), ContentError> {
        self.conn.lock().execute(
            "INSERT INTO session_items (session_id, slot, kind, title, label)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, slot, kind, title, label],
        )?;
        Ok(())
    }

    // ---- Settings ----------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, ContentError> {
        let conn = self.conn.lock();
        let v = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), ContentError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- Stats -----------------------------------------------------------

    pub fn stats(&self) -> Result<serde_json::Value, ContentError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT item_type, COUNT(*) FROM content_items GROUP BY item_type")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut by_type = serde_json::Map::new();
        let mut total = 0i64;
        for row in rows {
            let (t, n) = row?;
            total += n;
            by_type.insert(t, serde_json::json!(n));
        }
        Ok(serde_json::json!({ "total": total, "byType": by_type }))
    }
}

fn parse_json_column<T: serde::de::DeserializeOwned>(
    s: String,
) -> rusqlite::Result<T> {
    serde_json::from_str(&s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}

fn insert_search_row(
    tx: &rusqlite::Transaction,
    item_id: &str,
    key: &str,
    label: &str,
    text: &str,
) -> Result<(), ContentError> {
    tx.execute(
        "INSERT INTO search_index (item_id, section_key, section_label, text)
         VALUES (?1, ?2, ?3, ?4)",
        params![item_id, key, label, text],
    )?;
    tx.execute(
        "INSERT INTO search_fts (text, section_label, item_id, section_key)
         VALUES (?1, ?2, ?3, ?4)",
        params![text, label, item_id, key],
    )?;
    Ok(())
}

/// Turn a free-text query into an FTS5 query of prefix-AND terms.
fn build_fts_query(query: &str) -> String {
    query
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn default_data_dir() -> Result<PathBuf, ContentError> {
    // Windows-first: %APPDATA%\BibleLive. Other platforms fall back sensibly.
    Ok(dirs_fallback())
}

fn dirs_fallback() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("BibleLive");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".biblelive");
    }
    PathBuf::from(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(tag: &str) -> ContentStore {
        let dir = std::env::temp_dir().join(format!(
            "biblelive-test-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        ContentStore::open_at(dir).expect("open test store")
    }

    #[test]
    fn seeds_and_searches_bundled_library() {
        let store = test_store("seed");
        let stats = store.stats().unwrap();
        assert_eq!(stats["total"], serde_json::json!(78)); // 66 KJV books + 12 hymns

        // Exact quotation should find John 3:16.
        let hits = store.search("For God so loved the world", 10).unwrap();
        assert!(
            hits.iter()
                .any(|h| h.section_key == "john.3.16" && h.item_title.starts_with("John")),
            "expected John 3:16 in {hits:?}"
        );

        // Hymn search across content types.
        let hits = store.search("amazing grace how sweet the sound", 5).unwrap();
        assert!(
            hits.iter()
                .any(|h| h.item_title == "Amazing Grace" && h.section_label.contains("Verse 1")),
            "expected Amazing Grace verse 1 in {hits:?}"
        );
    }

    #[test]
    fn canonical_order_and_testaments() {
        let store = test_store("order");
        let bible = model::ItemType::Bible;

        // Canonical: Genesis first, Revelation last.
        let items = store.list_items(Some(&bible), None, None, "canonical", 100, 0).unwrap();
        assert_eq!(items.first().unwrap().title, "Genesis (KJV)");
        assert_eq!(items.last().unwrap().title, "Revelation (KJV)");
        // Matthew should be first book of the New Testament.
        let matthew_pos = items.iter().position(|i| i.title == "Matthew (KJV)").unwrap();
        assert_eq!(matthew_pos, 39);

        // Old Testament: 39 books, Genesis in, Matthew out.
        let ot = store.list_items(Some(&bible), None, Some("ot"), "canonical", 100, 0).unwrap();
        assert_eq!(ot.len(), 39);
        assert!(ot.iter().any(|i| i.title == "Genesis (KJV)"));
        assert!(!ot.iter().any(|i| i.title == "Matthew (KJV)"));

        // New Testament: 27 books, Matthew in, Malachi out.
        let nt = store.list_items(Some(&bible), None, Some("nt"), "canonical", 100, 0).unwrap();
        assert_eq!(nt.len(), 27);
        assert!(nt.iter().any(|i| i.title == "Matthew (KJV)"));
        assert!(!nt.iter().any(|i| i.title == "Malachi (KJV)"));

        // Title descending sorts Z→A.
        let desc = store.list_items(Some(&bible), None, None, "title-desc", 100, 0).unwrap();
        assert!(desc[0].title.as_str() > desc[1].title.as_str());
    }

    #[test]
    fn import_and_reindex() {
        let store = test_store("import");
        let item = import::import_text(
            "test-song-1".into(),
            "Test Song".into(),
            model::ItemType::Song,
            "[Verse 1]\nGrace led me home today\nSafe through storms and wind\n\n[Chorus]\nSing praise to the King",
            "en",
            "public-domain",
        )
        .unwrap();
        store.insert_item(&item).unwrap();

        let hits = store.search("storms wind", 5).unwrap();
        assert!(hits.iter().any(|h| h.item_id == "test-song-1" && h.section_label == "Verse 1"));

        // Copyrighted content must default to private.
        let item = import::import_text(
            "test-doc-1".into(),
            "Private Doc".into(),
            model::ItemType::Document,
            "Some copyrighted text for testing purposes",
            "en",
            "copyrighted",
        )
        .unwrap();
        assert_eq!(item.visibility, "private");

        store.delete_item("test-song-1").unwrap();
        let hits = store.search("storms wind", 5).unwrap();
        assert!(!hits.iter().any(|h| h.item_id == "test-song-1"));
    }

    #[test]
    fn chapter_loading() {
        let store = test_store("chapter");
        let (idx, sections) = store
            .get_chapter_sections("bible-kjv-john", "john.3.16")
            .unwrap()
            .expect("john 3 exists");
        assert_eq!(sections.len(), 36); // John 3 has 36 verses
        assert_eq!(idx, 15); // verse 16 is at index 15
        assert_eq!(sections[0].label, "John 3:1");
        assert_eq!(sections[15].label, "John 3:16");
        assert!(sections[15].lines[0].contains("For God so loved"));
    }

    #[test]
    fn docx_import() {
        // Build a minimal .docx (zip containing word/document.xml) in temp.
        let path = std::env::temp_dir().join(format!("biblelive-test-{}.docx", std::process::id()));
        let xml = r#"<?xml version="1.0"?><w:document xmlns:w="w"><w:body><w:p><w:r><w:t>Praise the Lord in the morning</w:t></w:r></w:p><w:p><w:r><w:t>and bless His holy name forever</w:t></w:r></w:p></w:body></w:document>"#;
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zip, xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let item = import::import_file(
            "test-docx-1".into(),
            "Docx Import".into(),
            model::ItemType::Document,
            &path,
            "en",
            "public-domain",
        )
        .unwrap();
        let _ = std::fs::remove_file(&path);

        let store = test_store("docx");
        store.insert_item(&item).unwrap();
        let hits = store.search("holy name forever", 5).unwrap();
        assert!(
            hits.iter().any(|h| h.item_id == "test-docx-1"),
            "expected docx content searchable"
        );
    }
}
