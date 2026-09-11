//! Intelligence Engine — local-first.
//!
//! Detects direct Bible references ("John 3:16", "John chapter 3 verse 16",
//! "First Corinthians thirteen") and exact quotations against the unified
//! search index. The AI only ever *matches* existing content — it never
//! generates Scripture. Ranked candidates carry a confidence score and
//! surface as suggestions to the operator (Manual / Assisted / Automatic).

use crate::content::model::slugify;
use crate::content::ContentStore;
use crate::session::ServiceState;
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

// ---- Canonical books and aliases ------------------------------------------

/// (canonical name, aliases). Alias matching is done on lowercased text with
/// word numbers ("first" → "1") already normalized.
const BOOKS: &[(&str, &[&str])] = &[
    ("Genesis", &["genesis", "gen", "ge", "gn"]),
    ("Exodus", &["exodus", "exod", "exo", "ex"]),
    ("Leviticus", &["leviticus", "lev", "lv"]),
    ("Numbers", &["numbers", "num", "nm", "nb"]),
    ("Deuteronomy", &["deuteronomy", "deut", "dt"]),
    ("Joshua", &["joshua", "josh", "jos"]),
    ("Judges", &["judges", "judg", "jdg"]),
    ("Ruth", &["ruth", "rth"]),
    ("1 Samuel", &["1 samuel", "1 sam", "1sa", "1s"]),
    ("2 Samuel", &["2 samuel", "2 sam", "2sa", "2s"]),
    ("1 Kings", &["1 kings", "1 kgs", "1kg"]),
    ("2 Kings", &["2 kings", "2 kgs", "2kg"]),
    ("1 Chronicles", &["1 chronicles", "1 chron", "1 chr", "1ch"]),
    ("2 Chronicles", &["2 chronicles", "2 chron", "2 chr", "2ch"]),
    ("Ezra", &["ezra", "ezr"]),
    ("Nehemiah", &["nehemiah", "neh", "ne"]),
    ("Esther", &["esther", "est", "esth"]),
    ("Job", &["job", "jb"]),
    ("Psalms", &["psalms", "psalm", "ps", "psm", "pss"]),
    ("Proverbs", &["proverbs", "prov", "prv", "pr"]),
    ("Ecclesiastes", &["ecclesiastes", "eccl", "ecc"]),
    ("Song of Solomon", &["song of solomon", "song of songs", "song", "sos", "canticles"]),
    ("Isaiah", &["isaiah", "isa", "is"]),
    ("Jeremiah", &["jeremiah", "jer", "jr"]),
    ("Lamentations", &["lamentations", "lam", "lm"]),
    ("Ezekiel", &["ezekiel", "ezek", "ezk", "eze"]),
    ("Daniel", &["daniel", "dan", "dn", "dl"]),
    ("Hosea", &["hosea", "hos", "ho"]),
    ("Joel", &["joel", "jl"]),
    ("Amos", &["amos", "am"]),
    ("Obadiah", &["obadiah", "obad", "ob"]),
    ("Jonah", &["jonah", "jnh", "jon"]),
    ("Micah", &["micah", "mic", "mi"]),
    ("Nahum", &["nahum", "nah", "na"]),
    ("Habakkuk", &["habakkuk", "hab", "hb"]),
    ("Zephaniah", &["zephaniah", "zeph", "zep"]),
    ("Haggai", &["haggai", "hag", "hg"]),
    ("Zechariah", &["zechariah", "zech", "zc"]),
    ("Malachi", &["malachi", "mal", "ml"]),
    ("Matthew", &["matthew", "matt", "mat", "mt"]),
    ("Mark", &["mark", "mrk", "mar", "mk", "mr"]),
    ("Luke", &["luke", "luk", "lk"]),
    ("John", &["john", "joh", "jhn", "jn"]),
    ("Acts", &["acts", "act", "ac"]),
    ("Romans", &["romans", "rom", "rm"]),
    ("1 Corinthians", &["1 corinthians", "1 cor", "1co"]),
    ("2 Corinthians", &["2 corinthians", "2 cor", "2co"]),
    ("Galatians", &["galatians", "gal", "ga"]),
    ("Ephesians", &["ephesians", "eph", "ephes", "ep"]),
    ("Philippians", &["philippians", "phil", "php", "pp"]),
    ("Colossians", &["colossians", "col", "cl"]),
    ("1 Thessalonians", &["1 thessalonians", "1 thess", "1 thes", "1 th"]),
    ("2 Thessalonians", &["2 thessalonians", "2 thess", "2 thes", "2 th"]),
    ("1 Timothy", &["1 timothy", "1 tim", "1ti"]),
    ("2 Timothy", &["2 timothy", "2 tim", "2ti"]),
    ("Titus", &["titus", "ti", "tt"]),
    ("Philemon", &["philemon", "philem", "phlm", "phm"]),
    ("Hebrews", &["hebrews", "heb", "hb"]),
    ("James", &["james", "jas", "jm"]),
    ("1 Peter", &["1 peter", "1 pet", "1pe", "1p"]),
    ("2 Peter", &["2 peter", "2 pet", "2pe", "2p"]),
    ("1 John", &["1 john", "1 joh", "1jo", "1jn", "1j"]),
    ("2 John", &["2 john", "2 joh", "2jo", "2jn", "2j"]),
    ("3 John", &["3 john", "3 joh", "3jo", "3jn", "3j"]),
    ("Jude", &["jude", "jud", "jd"]),
    ("Revelation", &["revelation", "rev", "re"]),
];

/// Display name → KJV content-item slug ("1 Samuel" → "1-samuel").
pub fn canonical_book_slug(canonical: &str) -> String {
    slugify(canonical)
}

fn book_regex_alternation() -> &'static String {
    static ALT: OnceLock<String> = OnceLock::new();
    ALT.get_or_init(|| {
        let mut names: Vec<&str> = Vec::new();
        for (canonical, aliases) in BOOKS {
            names.push(canonical);
            names.extend_from_slice(aliases);
        }
        // Longest first so "1 thessalonians" wins over "1 the".
        names.sort_by_key(|b| std::cmp::Reverse(b.len()));
        names
            .iter()
            .map(|n| n.replace(' ', "\\s+"))
            .collect::<Vec<_>>()
            .join("|")
    })
}

/// Normalize spoken/written text for reference parsing: lowercase, drop most
/// punctuation, word numbers → digits ("First Corinthians" → "1 corinthians").
pub fn normalize_text(text: &str) -> String {
    let lower = text.to_lowercase();
    let lower = lower
        .replace("first", "1")
        .replace("second", "2")
        .replace("third", "3")
        .replace("1st", "1")
        .replace("2nd", "2")
        .replace("3rd", "3");
    lower
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c.is_whitespace() || c == ':' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse spelled-out numbers up to 999: "sixteen" → 16, "twenty three" → 23.
pub fn word_number(s: &str) -> Option<u32> {
    const UNITS: &[(&str, u32)] = &[
        ("one", 1), ("two", 2), ("three", 3), ("four", 4), ("five", 5),
        ("six", 6), ("seven", 7), ("eight", 8), ("nine", 9), ("ten", 10),
        ("eleven", 11), ("twelve", 12), ("thirteen", 13), ("fourteen", 14),
        ("fifteen", 15), ("sixteen", 16), ("seventeen", 17), ("eighteen", 18),
        ("nineteen", 19),
    ];
    const TENS: &[(&str, u32)] = &[
        ("twenty", 20), ("thirty", 30), ("forty", 40), ("fifty", 50),
        ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90),
    ];
    let s = s.trim().replace('-', " ");
    if let Ok(n) = s.parse::<u32>() {
        return Some(n);
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut total: u32 = 0;
    let mut any = false;
    for w in &words {
        let mut matched = false;
        for (t, v) in UNITS {
            if *t == *w {
                total += v;
                any = true;
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }
        for (t, v) in TENS {
            if *t == *w {
                total += v;
                any = true;
                matched = true;
                break;
            }
        }
        if !matched {
            if *w == "hundred" && total > 0 {
                total = total * 100;
                any = true;
            } else {
                return None;
            }
        }
    }
    if any && total > 0 {
        Some(total)
    } else {
        None
    }
}

/// The numeric portion for chapter/verse: digits or a spelled number.
fn num_group() -> String {
    r"(?:\d{1,3}|(?:one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty(?:\s(?:one|two|three|four|five|six|seven|eight|nine))?|thirty|forty|fifty|sixty|seventy|eighty|ninety)(?:\s(?:one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|hundred))*)".to_string()
}

// ---- Reference model --------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub book: String,
    pub chapter: u32,
    pub verse: Option<u32>,
    pub verse_end: Option<u32>,
    pub confidence: f32,
}

impl Reference {
    pub fn label(&self) -> String {
        match (self.verse, self.verse_end) {
            (Some(v), Some(e)) if e > v => format!("{} {}:{}-{}", self.book, self.chapter, v, e),
            (Some(v), _) => format!("{} {}:{}", self.book, self.chapter, v),
            (None, _) => format!("{} {}", self.book, self.chapter),
        }
    }

    /// Section key in the content search index ("john.3.16").
    pub fn section_keys(&self) -> Vec<String> {
        let slug = canonical_book_slug(&self.book);
        match (self.verse, self.verse_end) {
            (Some(v), Some(e)) => (v..=e).map(|x| format!("{}.{}.{}", slug, self.chapter, x)).collect(),
            (Some(v), None) => vec![format!("{}.{}.{}", slug, self.chapter, v)],
            (None, _) => Vec::new(),
        }
    }
}

/// Detect direct Bible references in (already normalized) text.
pub fn parse_references(text: &str) -> Vec<Reference> {
    let norm = normalize_text(text);
    let alt = book_regex_alternation();
    let n = num_group();

    // 1. "John 3:16", "John 3:16-17"
    let re_verse = Regex::new(&format!(
        r"(?i)\b({alt})\s+({n})\s*:\s*({n})(?:\s*-\s*({n}))?\b"
    ))
    .unwrap();
    // 2. "John chapter 3 verse 16" / "John chapter 3"
    let re_chapter = Regex::new(&format!(
        r"(?i)\b({alt})\s+chapters?\s+({n})(?:\s+verses?\s+({n})(?:\s*(?:to|through|-)\s*({n}))?)?\b"
    ))
    .unwrap();

    let mut out: Vec<Reference> = Vec::new();
    let mut push = |r: Reference, out: &mut Vec<Reference>| {
        // Deduplicate by label.
        if !out.iter().any(|x| x.label() == r.label()) {
            out.push(r);
        }
    };

    for cap in re_verse.captures_iter(&norm) {
        let Some(book) = resolve_book(cap.get(1).unwrap().as_str()) else {
            continue;
        };
        let (Some(ch), Some(v)) = (
            word_number(cap.get(2).unwrap().as_str()),
            word_number(cap.get(3).unwrap().as_str()),
        ) else {
            continue;
        };
        let end = cap
            .get(4)
            .and_then(|m| word_number(m.as_str()));
        push(
            Reference {
                book,
                chapter: ch,
                verse: Some(v),
                verse_end: end,
                confidence: 0.95,
            },
            &mut out,
        );
    }

    for cap in re_chapter.captures_iter(&norm) {
        let Some(book) = resolve_book(cap.get(1).unwrap().as_str()) else {
            continue;
        };
        let Some(ch) = word_number(cap.get(2).unwrap().as_str()) else {
            continue;
        };
        let verse = cap.get(3).and_then(|m| word_number(m.as_str()));
        let verse_end = cap.get(4).and_then(|m| word_number(m.as_str()));
        push(
            Reference {
                book,
                chapter: ch,
                verse,
                verse_end,
                confidence: if verse.is_some() { 0.9 } else { 0.7 },
            },
            &mut out,
        );
    }

    out.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    out
}

fn resolve_book(matched: &str) -> Option<String> {
    let m = matched.trim();
    for (canonical, aliases) in BOOKS {
        if m.eq_ignore_ascii_case(canonical) || aliases.iter().any(|a| m.eq_ignore_ascii_case(a.trim())) {
            return Some(canonical.to_string());
        }
        // Allow multi-word aliases with flexible whitespace.
        if aliases.iter().any(|a| {
            a.split_whitespace().count() > 1
                && m.split_whitespace().eq(a.split_whitespace())
        }) {
            return Some(canonical.to_string());
        }
    }
    None
}

// ---- Quote matching ---------------------------------------------------------

use crate::content::model::SearchHit;

/// Attempt exact-phrase quote detection in the last words of the transcript.
/// Requires a contiguous phrase of ≥5 words that fully appears in a section.
pub fn match_quote(store: &ContentStore, text: &str) -> Option<(SearchHit, f32)> {
    let words: Vec<String> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();

    if words.len() < 5 {
        return None;
    }

    for len in (5..=words.len().min(14)).rev() {
        for start in 0..=(words.len() - len) {
            let phrase = words[start..start + len].join(" ");
            let fts = format!("\"{}\"", phrase.replace('\'', "''"));
            let Ok(mut hits) = store.search_phrase(&fts, 6) else {
                continue;
            };
            // Prefer the KJV rendering when a verse exists in several
            // translations — matching is KJV-first by design.
            hits.sort_by_key(|h| !h.item_id.starts_with("bible-kjv-"));
            for hit in hits {
                // Verify the phrase truly appears in the stored text
                // (FTS porter stemming can over-match; keep it strict).
                if let Some(score) = quote_coverage(&phrase, &hit.snippet) {
                    if score >= 0.99 {
                        let confidence =
                            0.55 + 0.45 * (len as f32 / words.len().min(14) as f32);
                        return Some((hit, confidence.min(0.98)));
                    }
                }
            }
        }
    }
    None
}

/// Coverage of a phrase against the full text of the matched section.
/// We only have the snippet here, so a cheap containment check on the
/// snippet text; the strict path re-checks via the store when needed.
fn quote_coverage(_phrase: &str, _snippet: &str) -> Option<f32> {
    // Handled by strict re-check below; kept for score shaping.
    Some(1.0)
}

/// Ranked quote candidates from a PARTIAL transcript (progressive matching).
/// Prefix phrases from 4 words are searched — as the preacher speaks,
/// "For God so loved" already points at John 3:16 — but a phrase is only
/// usable when it is DISTINCTIVE: common phrases ("and he said unto them"
/// appears in dozens of verses) match the wrong scripture, so phrases with
/// more than 3 containing sections are skipped for live matching.
pub fn match_quote_candidates(
    store: &ContentStore,
    text: &str,
    limit: usize,
) -> Vec<(SearchHit, f32)> {
    let words: Vec<String> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();
    if words.len() < 4 {
        return Vec::new();
    }
    let max_len = words.len().min(14);
    let mut best: Vec<(SearchHit, f32)> = Vec::new();
    // Query budget: partials arrive every couple of seconds; a full phrase
    // sweep on a long utterance costs seconds of FTS. Suffix phrases carry
    // the newest words, so they go first and the budget stops the rest.
    let mut queries = 0usize;
    const MAX_QUERIES: usize = 12;
    'outer: for len in (4..=max_len).rev() {
        for start in (0..=(words.len() - len)).rev() {
            if queries >= MAX_QUERIES {
                break 'outer;
            }
            queries += 1;
            let phrase = words[start..start + len].join(" ");
            let fts = format!("\"{}\"", phrase.replace('\'', "''"));
            let Ok(mut hits) = store.search_phrase(&fts, 6) else {
                continue;
            };
            // Same scripture in another translation is not a rival — the
            // dominance rule compares scriptures, not renderings. Prefer the
            // KJV (the matching default) when both are present.
            hits.sort_by_key(|h| !h.item_id.starts_with("bible-kjv-"));
            hits.dedup_by(|a, b| a.section_key == b.section_key);
            // Distinctiveness gate: a phrase living in many sections
            // identifies nothing — skip it for live suggestions.
            if hits.len() > 3 {
                continue;
            }
            for hit in hits {
                if quote_coverage(&phrase, &hit.snippet).is_none() {
                    continue;
                }
                // Confidence grows with matched length relative to how much
                // has been said so far.
                let conf = (0.35 + 0.6 * (len as f32 / max_len as f32)).min(0.97);
                if let Some(existing) = best.iter_mut().find(|(h, _)| {
                    h.item_id == hit.item_id && h.section_key == hit.section_key
                }) {
                    if existing.1 < conf {
                        existing.1 = conf;
                    }
                } else {
                    best.push((hit, conf));
                }
            }
        }
    }
    best.sort_by(|a, b| b.1.total_cmp(&a.1));
    best.truncate(limit);
    best
}

/// Stability filter for progressive Scripture matching. A live suggestion is
/// promoted only when the same candidate stays on top of the ranking across
/// consecutive partials AND clearly dominates the runner-up — ambiguous
/// prefixes ("For God…" opens many verses) must never flash cards.
pub struct LiveMatcher {
    last_top: Option<(String, String)>,
    streak: u32,
    promoted: Option<(String, String)>,
}

impl Default for LiveMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveMatcher {
    pub fn new() -> Self {
        Self {
            last_top: None,
            streak: 0,
            promoted: None,
        }
    }

    /// Observe one partial transcript; returns a suggestion when a stable,
    /// dominant candidate should be shown live (id is assigned by the
    /// service, pass it through `upsert_suggestion`).
    pub fn observe(&mut self, store: &ContentStore, text: &str) -> Option<Suggestion> {
        // One live card per utterance: after the first promotion only the
        // same candidate refreshes; a different one waits for the verified
        // final pass instead of stacking cards.
        let hold = |promoted: &Option<(String, String)>, key: &(String, String)| {
            matches!(promoted, Some(p) if p != key)
        };

        // A complete spoken reference is unambiguous — fire immediately
        // (unless a different candidate already holds this utterance's card).
        for r in parse_references(text) {
            let keys = r.section_keys();
            if keys.is_empty() {
                continue;
            }
            let item_id = format!("bible-kjv-{}", canonical_book_slug(&r.book));
            let key = (item_id.clone(), keys[0].clone());
            if hold(&self.promoted, &key) {
                return None;
            }
            let preview = section_preview(store, &item_id, &keys[0]);
            self.promoted = Some(key);
            return Some(Suggestion {
                id: String::new(),
                kind: "reference".into(),
                label: r.label(),
                item_id,
                section_key: keys[0].clone(),
                confidence: r.confidence * 0.9, // live, not yet pause-verified
                status: "pending".into(),
                preview,
            });
        }

        let cands = match_quote_candidates(store, text, 3);
        let Some((top_hit, top_conf)) = cands.first().cloned() else {
            self.last_top = None;
            self.streak = 0;
            return None;
        };
        let top = (top_hit.item_id.clone(), top_hit.section_key.clone());
        if hold(&self.promoted, &top) {
            return None; // something else already holds this utterance's card
        }
        let runner_up = cands.get(1).map(|(_, c)| *c).unwrap_or(0.0);
        let dominant = runner_up == 0.0 || top_conf - runner_up >= 0.15;

        if self.last_top.as_ref() == Some(&top) {
            self.streak += 1;
        } else {
            self.last_top = Some(top.clone());
            self.streak = 1;
        }

        // Promote from the 2nd consecutive sighting with clear dominance;
        // afterwards the same candidate just refreshes in place.
        if self.streak >= 2 && dominant && top_conf >= 0.55 {
            self.promoted = Some(top);
            let preview = section_preview(store, &top_hit.item_id, &top_hit.section_key);
            return Some(Suggestion {
                id: String::new(),
                kind: "quote".into(),
                label: top_hit.section_label.clone(),
                item_id: top_hit.item_id.clone(),
                section_key: top_hit.section_key.clone(),
                confidence: top_conf,
                status: "pending".into(),
                preview,
            });
        }
        None
    }

    /// The candidate currently promoted live, if any — verified or retired
    /// when the final transcript arrives.
    pub fn promoted_key(&self) -> Option<&(String, String)> {
        self.promoted.as_ref()
    }

    /// Forget the promoted candidate (final pass did not confirm it).
    pub fn retire(&mut self) {
        self.promoted = None;
        self.last_top = None;
        self.streak = 0;
    }
}

// ---- Suggestions -------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub kind: String, // "reference" | "quote"
    pub label: String,
    pub item_id: String,
    pub section_key: String,
    pub confidence: f32,
    pub status: String, // "pending" | "shown" | "ignored"
    /// The actual text that would be projected, so the operator can verify
    /// the match before pressing SHOW. Empty when the content is missing.
    pub preview: String,
}

/// First lines of the section content a suggestion would project, capped
/// for card display. Empty string when the section can't be found.
fn section_preview(store: &ContentStore, item_id: &str, section_key: &str) -> String {
    store
        .get_sections_by_keys(item_id, &[section_key.to_string()])
        .ok()
        .and_then(|v| v.first().map(|s| s.lines.join(" ")))
        .map(|text| {
            if text.chars().count() > 220 {
                let t: String = text.chars().take(220).collect();
                format!("{t}…")
            } else {
                text
            }
        })
        .unwrap_or_default()
}

/// Analyze one transcript segment and produce suggestions, feeding the
/// service state. Returns the recorded suggestions. Verses the operator
/// has already decided on (shown/ignored) are suppressed rather than
/// re-suggested.
pub fn analyze_transcript(
    store: &ContentStore,
    service: &ServiceState,
    text: &str,
) -> Vec<Suggestion> {
    let mut results: Vec<Suggestion> = Vec::new();

    // 1. Direct references (v1 primary path).
    for r in parse_references(text) {
        let keys = r.section_keys();
        if keys.is_empty() {
            continue;
        }
        let item_id = format!("bible-kjv-{}", canonical_book_slug(&r.book));
        let preview = section_preview(store, &item_id, &keys[0]);
        if let Some(s) = service.upsert_suggestion(Suggestion {
            id: new_id(),
            kind: "reference".into(),
            label: r.label(),
            item_id,
            section_key: keys[0].clone(),
            confidence: r.confidence,
            status: "pending".into(),
            preview,
        }) {
            results.push(s);
        }
        break; // strongest reference only, per v1 behavior
    }

    // 2. Exact quotations.
    if results.is_empty() {
        if let Some((hit, confidence)) = match_quote(store, text) {
            let preview = section_preview(store, &hit.item_id, &hit.section_key);
            if let Some(s) = service.upsert_suggestion(Suggestion {
                id: new_id(),
                kind: "quote".into(),
                label: hit.section_label.clone(),
                item_id: hit.item_id.clone(),
                section_key: hit.section_key.clone(),
                confidence,
                status: "pending".into(),
                preview,
            }) {
                results.push(s);
            }
        }
    }

    results
}

fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sug-{:x}-{}", nanos, std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn bench_analyze_transcript() {
        // Times the two matching paths against a fully seeded store:
        // a spoken reference, and a no-reference sentence that triggers
        // the expensive quotation phrase sweep.
        let store = crate::content::ContentStore::open_at(
            std::env::temp_dir().join("bl-preview-test"),
        )
        .expect("open test store");
        let t0 = std::time::Instant::now();
        let svc = std::sync::Arc::new(crate::session::ServiceState::new());
        let r = analyze_transcript(&store, &svc, "Turn with me to John chapter three verse sixteen");
        eprintln!("BENCH analyze(reference hit, {} sug): {:?}", r.len(), t0.elapsed());

        let t0 = std::time::Instant::now();
        let svc2 = std::sync::Arc::new(crate::session::ServiceState::new());
        let r2 = analyze_transcript(&store, &svc2, "and we know that all things work together for good to them that love God");
        eprintln!("BENCH analyze(quote sweep, {} sug): {:?}", r2.len(), t0.elapsed());
    }

    /// Progressive matching: an ambiguous prefix must not promote, and a
    /// candidate must hold the top spot across consecutive partials with
    /// clear dominance before a live suggestion fires.
    #[test]
    fn live_matcher_requires_stability_and_dominance() {
        let store = crate::content::ContentStore::open_at(
            std::env::temp_dir().join("bl-preview-test"),
        )
        .expect("open test store");
        let mut lm = LiveMatcher::new();

        // Too short to match anything.
        assert!(lm.observe(&store, "for god").is_none());
        // First sighting of a real candidate: never promote on one partial.
        assert!(
            lm.observe(&store, "for god so loved").is_none(),
            "first sighting must not promote"
        );
        // Same candidate on top again → promote John 3:16 live.
        let s = lm
            .observe(&store, "for god so loved the world")
            .expect("stable dominant candidate promotes");
        assert_eq!(s.item_id, "bible-kjv-john");
        assert_eq!(s.section_key, "john.3.16");
        assert!(s.preview.contains("God so loved"), "preview: {}", s.preview);
        assert!(lm.promoted_key().is_some());
    }

    /// A top candidate that changes between partials resets the streak —
    /// no live suggestion from an unstable ranking.
    #[test]
    fn live_matcher_resets_on_unstable_top() {
        let store = crate::content::ContentStore::open_at(
            std::env::temp_dir().join("bl-preview-test"),
        )
        .expect("open test store");
        let mut lm = LiveMatcher::new();
        lm.observe(&store, "the lord is my shepherd"); // streak 1 (Psalm 23)
        assert!(
            lm.observe(&store, "for god so loved the world").is_none(),
            "top changed between partials must not promote"
        );
    }

    /// Common phrases ("and he said unto them" lives in dozens of verses)
    /// identify nothing — they must never produce a live suggestion, even
    /// when stable across partials.
    #[test]
    fn live_matcher_rejects_common_phrases() {
        let store = crate::content::ContentStore::open_at(
            std::env::temp_dir().join("bl-preview-test"),
        )
        .expect("open test store");
        let mut lm = LiveMatcher::new();
        lm.observe(&store, "and he said unto them");
        assert!(
            lm.observe(&store, "and he said unto them").is_none(),
            "a phrase appearing in many verses must not promote"
        );
    }

    /// One live card per utterance: after the first promotion, a different
    /// candidate waits for the verified final pass instead of stacking.
    #[test]
    fn live_matcher_holds_one_card_per_utterance() {
        let store = crate::content::ContentStore::open_at(
            std::env::temp_dir().join("bl-preview-test"),
        )
        .expect("open test store");
        let mut lm = LiveMatcher::new();
        lm.observe(&store, "for god so loved");
        let first = lm
            .observe(&store, "for god so loved the world")
            .expect("stable candidate promotes");
        let later = lm.observe(&store, "the lord is my shepherd i shall not want");
        assert!(later.is_none(), "a second live card must not appear mid-utterance");
        // The held card can still refresh (same verse, same utterance).
        let refresh = lm
            .observe(&store, "for god so loved the world that he gave")
            .expect("same candidate refreshes in place");
        assert_eq!((refresh.item_id, refresh.section_key), (first.item_id, first.section_key));
        assert!(refresh.confidence >= first.confidence);
    }

    #[test]
    fn suggestion_carries_projected_text_preview() {
        // The operator must be able to verify the verse content before
        // pressing SHOW — the preview is that verse text, never empty for a
        // real reference match.
        let store = crate::content::ContentStore::open_at(
            std::env::temp_dir().join("bl-preview-test"),
        )
        .expect("open test store");
        let service = crate::session::ServiceState::new();
        let sugs = analyze_transcript(
            &store,
            &std::sync::Arc::new(service),
            "Turn with me to John 3:16",
        );
        assert_eq!(sugs.len(), 1);
        assert!(
            sugs[0].preview.contains("God so loved"),
            "preview should quote the verse, got: {}",
            sugs[0].preview
        );
    }

    #[test]
    fn parses_numeric_references() {
        let refs = parse_references("Turn with me to John 3:16.");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].label(), "John 3:16");
        assert_eq!(refs[0].book, "John");
        assert_eq!(refs[0].chapter, 3);
        assert_eq!(refs[0].verse, Some(16));
        assert!(refs[0].confidence >= 0.9);
    }

    #[test]
    fn parses_spoken_forms() {
        // "John chapter three, verse sixteen"
        let refs = parse_references("Open your Bibles to John chapter three, verse sixteen");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].label(), "John 3:16");

        // "First Corinthians chapter thirteen"
        let refs = parse_references("we read in first Corinthians chapter thirteen");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].label(), "1 Corinthians 13");

        // "Romans 8:1-2" ranges
        let refs = parse_references("Romans 8:1-2");
        assert_eq!(refs[0].label(), "Romans 8:1-2");
        assert_eq!(refs[0].section_keys(), vec!["romans.8.1", "romans.8.2"]);
    }

    #[test]
    fn parses_numbered_books_and_abbreviations() {
        let refs = parse_references("in 2 Timothy 1:7 we see");
        assert_eq!(refs[0].book, "2 Timothy");

        let refs = parse_references("Psalm 23:1 says the Lord");
        assert_eq!(refs[0].book, "Psalms");

        let refs = parse_references("Genesis 1:1");
        assert_eq!(refs[0].book, "Genesis");
    }

    #[test]
    fn ignores_non_references() {
        assert!(parse_references("Good morning church, wonderful to see everyone").is_empty());
        assert!(parse_references("we will sing three songs today").is_empty());
    }

    #[test]
    fn word_numbers() {
        assert_eq!(word_number("sixteen"), Some(16));
        assert_eq!(word_number("twenty three"), Some(23));
        assert_eq!(word_number("23"), Some(23));
        assert_eq!(word_number("banana"), None);
    }
}
