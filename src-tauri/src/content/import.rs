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
        other => Err(ContentError::Other(format!(
            "unsupported file type: .{} (supported: .txt, .docx)",
            other
        ))),
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
