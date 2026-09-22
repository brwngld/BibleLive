//! Content importers — plain text and Word .docx (v1 formats).
//!
//! Every import carries a license field and defaults visibility to
//! `private` when the license is copyrighted, per the licensing decision
//! in docs/DECISIONS.md.

use super::model::{ContentItem, ItemType, Section};
use super::ContentError;
use std::io::Read;
use std::path::Path;

/// Detect visibility from license: copyrighted content stays private.
pub fn visibility_for_license(license: &str) -> &'static str {
    match license {
        "public-domain" | "cc0" | "cc-by" | "cc-by-sa" => "public",
        "copyrighted" | "unknown" => "private",
        _ => "private",
    }
}

/// Import plain text. Sections are split on blank lines (a blank line
/// separates stanzas / paragraphs); optional "[Verse 1]" / "Chorus:" style
/// headers become section labels.
pub fn import_text(
    id: String,
    title: String,
    item_type: ItemType,
    text: &str,
    language: &str,
    license: &str,
) -> Result<ContentItem, ContentError> {
    let sections = sections_from_text(text);
    if sections.is_empty() {
        return Err(ContentError::Other("no content found in text".into()));
    }
    Ok(ContentItem {
        id,
        item_type,
        title,
        language: language.to_string(),
        license: license.to_string(),
        visibility: visibility_for_license(license).to_string(),
        metadata: serde_json::json!({ "source": "text-import" }),
        body: serde_json::json!({ "sections": sections }),
    })
}

/// Import a file: dispatch on extension (.txt, .docx).
pub fn import_file(
    id: String,
    title: String,
    item_type: ItemType,
    path: &Path,
    language: &str,
    license: &str,
) -> Result<ContentItem, ContentError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "text" | "md" => {
            let text = std::fs::read_to_string(path)?;
            import_text(id, title, item_type, &text, language, license)
        }
        "docx" => {
            let text = extract_docx_text(path)?;
            import_text(id, title, item_type, &text, language, license)
        }
        "xml" => {
            let text = std::fs::read_to_string(path)?;
            import_song_xml(id, title, item_type, &text, language, license)
        }
        other => Err(ContentError::Other(format!(
            "unsupported file type: .{} (supported: .txt, .docx, .xml)",
            other
        ))),
    }
}

/// Import a song from XML: OpenLyrics (`<lyrics>`) or OpenSong (`<song>`),
/// sniffed from the root element. The file's own title/author win over the
/// dialog's when present.
pub fn import_song_xml(
    id: String,
    title: String,
    item_type: ItemType,
    xml: &str,
    language: &str,
    license: &str,
) -> Result<ContentItem, ContentError> {
    // Sniff the ROOT element: skip the XML declaration, then look at the
    // first tag. (Containment would misroute OpenSong files — they embed a
    // <lyrics> block inside <song>.)
    let rest = xml.trim_start();
    let rest = match rest.strip_prefix("<?xml") {
        Some(after) => match after.find('>') {
            Some(i) => &after[i + 1..],
            None => rest,
        },
        None => rest,
    };
    let head = rest.trim_start();
    if head.starts_with("<lyrics") {
        import_openlyrics(id, title, item_type, xml, language, license)
    } else if head.starts_with("<song") {
        import_opsong(id, title, item_type, xml, language, license)
    } else {
        Err(ContentError::Other(
            "not a recognized song XML (expected OpenLyrics <lyrics> or OpenSong <song>)"
                .into(),
        ))
    }
}

/// Prettify a section tag: "v1" → "Verse 1", "c" → "Chorus"; unknown tags
/// pass through unchanged.
fn prettify_tag(tag: &str) -> String {
    let t = tag.trim();
    let split = t.find(|c: char| c.is_ascii_digit()).unwrap_or(t.len());
    let (letters, num) = t.split_at(split);
    let word = match letters.to_ascii_lowercase().as_str() {
        "v" => "Verse",
        "c" => "Chorus",
        "b" => "Bridge",
        "p" => "Pre-chorus",
        "i" => "Intro",
        "e" => "Ending",
        "o" => "Outro",
        "t" => "Tag",
        _ => return t.to_string(),
    };
    if num.is_empty() {
        word.to_string()
    } else {
        format!("{} {}", word, num)
    }
}

/// OpenLyrics (openlyrics.info) — structured XML used by OpenLP and many
/// other worship tools.
pub fn import_openlyrics(
    id: String,
    title: String,
    item_type: ItemType,
    xml: &str,
    language: &str,
    license: &str,
) -> Result<ContentItem, ContentError> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut file_title = String::new();
    let mut author = String::new();
    let mut in_title = false;
    let mut in_author = false;
    let mut sections: Vec<Section> = Vec::new();
    let mut verse_name: Option<String> = None;
    let mut lines_buf: Vec<String> = Vec::new();
    let mut current_line = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"title" => in_title = true,
                b"author" => in_author = true,
                b"verse" => {
                    if let Some(n) = verse_name.take() {
                        if !lines_buf.is_empty() {
                            sections.push(Section { label: prettify_tag(&n), lines: std::mem::take(&mut lines_buf) });
                        }
                    }
                    current_line.clear();
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"name" {
                            verse_name = Some(String::from_utf8_lossy(&attr.value).trim().to_string());
                        }
                    }
                }
                b"br" | b"lines" => {}
                _ => {}
            },
            Ok(Event::Empty(e)) if e.name().as_ref() == b"br" => {
                let t = current_line.trim().to_string();
                if !t.is_empty() {
                    lines_buf.push(t);
                }
                current_line.clear();
            }
            Ok(Event::Text(t)) => {
                let text = t
                    .unescape()
                    .map_err(|e| ContentError::Other(format!("openlyrics text error: {}", e)))?;
                if in_title {
                    file_title = text.trim().to_string();
                } else if in_author {
                    author = text.trim().to_string();
                } else if verse_name.is_some() {
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(text.trim());
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"title" => in_title = false,
                b"author" => in_author = false,
                b"lines" => {
                    let t = current_line.trim().to_string();
                    if !t.is_empty() {
                        lines_buf.push(t);
                    }
                    current_line.clear();
                }
                b"verse" => {
                    if let Some(n) = verse_name.take() {
                        let t = current_line.trim().to_string();
                        if !t.is_empty() {
                            lines_buf.push(t);
                        }
                        current_line.clear();
                        if !lines_buf.is_empty() {
                            sections.push(Section { label: prettify_tag(&n), lines: std::mem::take(&mut lines_buf) });
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ContentError::Other(format!("openlyrics parse error: {}", e)))
            }
            _ => {}
        }
    }

    if sections.is_empty() {
        return Err(ContentError::Other("no verses found in the OpenLyrics file".into()));
    }
    let mut meta = serde_json::json!({ "source": "openlyrics" });
    if !author.is_empty() {
        meta["authors"] = serde_json::json!([author]);
    }
    Ok(ContentItem {
        id,
        item_type,
        title: if file_title.is_empty() { title } else { file_title },
        language: language.to_string(),
        license: license.to_string(),
        visibility: visibility_for_license(license).to_string(),
        metadata: meta,
        body: serde_json::json!({ "sections": sections }),
    })
}

/// OpenSong — lyrics are plain text with [Tag] section markers inside a
/// simple XML wrapper.
pub fn import_opsong(
    id: String,
    title: String,
    item_type: ItemType,
    xml: &str,
    language: &str,
    license: &str,
) -> Result<ContentItem, ContentError> {
    let file_title = xml_text_between(xml, "<title>", "</title>");
    let author = xml_text_between(xml, "<author>", "</author>");
    let lyrics = xml_text_between(xml, "<lyrics>", "</lyrics>");
    if lyrics.is_empty() {
        return Err(ContentError::Other("no <lyrics> block in the OpenSong file".into()));
    }

    let mut sections: Vec<Section> = Vec::new();
    let mut current_label = String::new();
    let mut current_lines: Vec<String> = Vec::new();

    for raw in lyrics.lines() {
        let t = raw.trim();
        if t.is_empty() || t == "." {
            continue; // OpenSong spacer
        }
        if t.starts_with('[') && t.ends_with(']') && t.len() > 2 {
            if !current_lines.is_empty() {
                sections.push(Section {
                    label: if current_label.trim().is_empty() {
                        String::new()
                    } else {
                        prettify_tag(current_label.trim())
                    },
                    lines: std::mem::take(&mut current_lines),
                });
            }
            current_label = t[1..t.len() - 1].to_string();
            continue;
        }
        current_lines.push(t.to_string());
    }
    if !current_lines.is_empty() {
        sections.push(Section {
            label: if current_label.trim().is_empty() {
                String::new()
            } else {
                prettify_tag(current_label.trim())
            },
            lines: current_lines,
        });
    }

    // Label any tag-less stanzas sequentially.
    let mut counter = 0;
    for sec in &mut sections {
        if sec.label.is_empty() {
            counter += 1;
            sec.label = format!("Verse {}", counter);
        }
    }
    if sections.is_empty() {
        return Err(ContentError::Other("no lyric lines in the OpenSong file".into()));
    }

    let mut meta = serde_json::json!({ "source": "opsong" });
    if !author.is_empty() {
        meta["authors"] = serde_json::json!([author]);
    }
    Ok(ContentItem {
        id,
        item_type,
        title: if file_title.is_empty() { title } else { file_title },
        language: language.to_string(),
        license: license.to_string(),
        visibility: visibility_for_license(license).to_string(),
        metadata: meta,
        body: serde_json::json!({ "sections": sections }),
    })
}

/// First occurrence of a raw element's inner text (OpenSong files are
/// simple enough that tag matching beats a full XML parse).
fn xml_text_between(xml: &str, open: &str, close: &str) -> String {
    let start = match xml.find(open) {
        Some(i) => i + open.len(),
        None => return String::new(),
    };
    match xml[start..].find(close) {
        Some(end) => xml[start..start + end].trim().to_string(),
        None => String::new(),
    }
}

/// Split raw text into labeled sections on blank lines.
pub fn sections_from_text(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut current_label: Option<String> = None;

    let flush = |lines: &mut Vec<String>, label: &mut Option<String>, out: &mut Vec<Section>| {
        let trimmed: Vec<String> = lines
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if !trimmed.is_empty() {
            out.push(Section {
                label: label.take().unwrap_or_default(),
                lines: trimmed,
            });
        }
        lines.clear();
        *label = None;
    };

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            flush(&mut current_lines, &mut current_label, &mut sections);
            continue;
        }
        // Optional explicit headers: "[Verse 1]", "Verse 1", "Chorus:"
        if let Some(label) = parse_section_header(line) {
            flush(&mut current_lines, &mut current_label, &mut sections);
            current_label = Some(label);
            continue;
        }
        current_lines.push(line.to_string());
    }
    flush(&mut current_lines, &mut current_label, &mut sections);

    // Give unlabeled sections sequential labels.
    let mut counter = 0;
    for s in &mut sections {
        if s.label.is_empty() {
            counter += 1;
            s.label = format!("Section {}", counter);
        }
    }
    sections
}

fn parse_section_header(line: &str) -> Option<String> {
    let known = [
        "verse", "chorus", "bridge", "refrain", "intro", "outro", "coda", "pre-chorus",
        "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8",
    ];
    let cleaned = line
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches(':')
        .trim()
        .to_lowercase();
    if cleaned.is_empty() {
        return None;
    }
    // A header is a short line that starts with a known marker (optionally
    // numbered, e.g. "Verse 2", "chorus 1").
    if cleaned.len() <= 16 {
        for k in known {
            if cleaned == k || cleaned.starts_with(&format!("{} ", k)) {
                return Some(
                    line.trim()
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .trim_end_matches(':')
                        .trim()
                        .to_string(),
                );
            }
        }
    }
    None
}

/// Extract paragraph text from a .docx (zip of XML) — word/document.xml,
/// one output paragraph per <w:p>, joining all <w:t> runs.
pub fn extract_docx_text(path: &Path) -> Result<String, ContentError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")?
        .read_to_string(&mut xml)?;

    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(false);

    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_t = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => match local_name(e.name().as_ref()).as_str() {
                "p" => current.clear(),
                "t" => in_t = true,
                _ => {}
            },
            Ok(quick_xml::events::Event::Text(e)) => {
                if in_t {
                    if let Ok(decoded) = e.unescape() {
                        current.push_str(&decoded);
                    }
                }
            }
            Ok(quick_xml::events::Event::End(e)) => match local_name(e.name().as_ref()).as_str() {
                "p" => paragraphs.push(current.clone()),
                "t" => in_t = false,
                _ => {}
            },
            Ok(quick_xml::events::Event::Eof) => break,
            Err(err) => return Err(ContentError::Other(format!("docx parse error: {}", err))),
            _ => {}
        }
        buf.clear();
    }

    Ok(paragraphs.join("\n\n"))
}

fn local_name(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    match s.split_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

