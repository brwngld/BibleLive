//! Service Control — the layer above the four engines.
//!
//! Owns the service mode (Manual / Assisted / Automatic) and the suggestion
//! queue. The command bus through which all engine commands flow lives here,
//! making remote controllers on the church network a later drop-in.

use crate::intelligence::Suggestion;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListenMode {
    Manual,
    Assisted,
    Automatic,
}

impl ListenMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ListenMode::Manual => "manual",
            ListenMode::Assisted => "assisted",
            ListenMode::Automatic => "automatic",
        }
    }
}

const MAX_SUGGESTIONS: usize = 20;

/// A service session record (live or historical).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

pub struct ServiceState {
    mode: Mutex<ListenMode>,
    suggestions: Mutex<VecDeque<Suggestion>>,
    next_id: Mutex<u64>,
    current: Mutex<Option<SessionMeta>>,
    /// Verses the operator decided on (shown/ignored) during the CURRENT
    /// utterance. Cleared when the next utterance begins, so a decision
    /// settles that mention without blacklisting the verse for the service.
    suppressed: Mutex<std::collections::HashSet<(String, String)>>,
}

impl ServiceState {
    pub fn new() -> Self {
        Self {
            mode: Mutex::new(ListenMode::Assisted),
            suggestions: Mutex::new(VecDeque::new()),
            next_id: Mutex::new(1),
            current: Mutex::new(None),
            suppressed: Mutex::new(std::collections::HashSet::new()),
        }
    }

    // ---- Session lifecycle -------------------------------------------------

    pub fn current_session(&self) -> Option<SessionMeta> {
        self.current.lock().clone()
    }

    pub fn start_session(&self, meta: SessionMeta) {
        *self.current.lock() = Some(meta);
    }

    /// Take (end) the current session.
    pub fn end_session(&self) -> Option<SessionMeta> {
        self.current.lock().take()
    }

    pub fn mode(&self) -> ListenMode {
        *self.mode.lock()
    }

    pub fn set_mode(&self, mode: ListenMode) {
        *self.mode.lock() = mode;
    }

    /// Record a new suggestion; returns its assigned id.
    pub fn add_suggestion(&self, mut s: Suggestion) -> Suggestion {
        let mut id = self.next_id.lock();
        s.id = format!("sug-{}", *id);
        *id += 1;
        let mut q = self.suggestions.lock();
        q.push_back(s.clone());
        while q.len() > MAX_SUGGESTIONS {
            q.pop_front();
        }
        s
    }

    /// Operator decision: set a card's status AND remember the verse as
    /// decided for the current utterance (no re-suggesting this mention).
    pub fn resolve_suggestion(&self, id: &str, status: &str) -> Option<Suggestion> {
        let mut q = self.suggestions.lock();
        let s = q.iter_mut().rev().find(|s| s.id == id)?;
        s.status = status.to_string();
        let key = (s.item_id.clone(), s.section_key.clone());
        let out = s.clone();
        drop(q);
        if status == "shown" || status == "ignored" {
            self.suppressed.lock().insert(key);
        }
        Some(out)
    }

    /// A new utterance begins: verses decided on during the previous one
    /// become suggestible again (the preacher may return to them later).
    pub fn clear_utterance_suppressions(&self) {
        self.suppressed.lock().clear();
    }

    /// Insert or refresh: a live (partial-transcript) suggestion replaces
    /// the pending card for the same content instead of stacking duplicate
    /// cards as the match strengthens. Verses the operator decided on during
    /// this utterance are suppressed → None.
    pub fn upsert_suggestion(&self, s: Suggestion) -> Option<Suggestion> {
        if self
            .suppressed
            .lock()
            .contains(&(s.item_id.clone(), s.section_key.clone()))
        {
            return None; // decided this utterance — do not nag
        }
        let mut q = self.suggestions.lock();
        if let Some(existing) = q.iter_mut().rev().find(|e| {
            e.status == "pending" && e.item_id == s.item_id && e.section_key == s.section_key
        }) {
            existing.kind = s.kind.clone();
            existing.label = s.label.clone();
            existing.confidence = s.confidence;
            existing.preview = s.preview.clone();
            return Some(existing.clone());
        }
        drop(q);
        Some(self.add_suggestion(s))
    }

    pub fn pending_suggestions(&self) -> Vec<Suggestion> {
        self.suggestions
            .lock()
            .iter()
            .filter(|s| s.status == "pending")
            .cloned()
            .collect()
    }

    pub fn recent_suggestions(&self, limit: usize) -> Vec<Suggestion> {
        let q = self.suggestions.lock();
        q.iter().rev().take(limit).cloned().collect()
    }

    pub fn set_suggestion_status(&self, id: &str, status: &str) -> Option<Suggestion> {
        let mut q = self.suggestions.lock();
        let s = q.iter_mut().rev().find(|s| s.id == id)?;
        s.status = status.to_string();
        Some(s.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intelligence::Suggestion;

    fn sug(item: &str, key: &str) -> Suggestion {
        Suggestion {
            id: String::new(),
            kind: "quote".into(),
            label: format!("{item} {key}"),
            item_id: item.into(),
            section_key: key.into(),
            confidence: 0.8,
            status: "pending".into(),
            preview: String::new(),
        }
    }

    /// Once the operator shows or ignores a verse, later partials for the
    /// same verse must not raise a new card — until the next utterance,
    /// where the preacher may legitimately return to it.
    #[test]
    fn decided_verses_return_next_utterance() {
        let service = ServiceState::new();
        let first = service
            .upsert_suggestion(sug("bible-kjv-john", "john.3.16"))
            .expect("first card is created");
        service
            .resolve_suggestion(&first.id, "shown")
            .unwrap();

        // Decided this utterance → suppressed.
        assert!(
            service.upsert_suggestion(sug("bible-kjv-john", "john.3.16")).is_none(),
            "decided verse must be quiet for the rest of this utterance"
        );
        // Other verses still flow through.
        assert!(service
            .upsert_suggestion(sug("bible-kjv-psalm", "psalm.23.1"))
            .is_some());

        // The preacher returns to the verse later in the service → fresh card.
        service.clear_utterance_suppressions();
        assert!(
            service.upsert_suggestion(sug("bible-kjv-john", "john.3.16")).is_some(),
            "a new utterance must be able to re-suggest the verse"
        );
    }

    /// A pending card for the same verse is refreshed in place, not duplicated.
    #[test]
    fn pending_cards_refresh_in_place() {
        let service = ServiceState::new();
        let a = service.upsert_suggestion(sug("bible-kjv-john", "john.3.16")).unwrap();
        let mut stronger = sug("bible-kjv-john", "john.3.16");
        stronger.confidence = 0.95;
        let b = service.upsert_suggestion(stronger).unwrap();
        assert_eq!(a.id, b.id, "same card refreshed");
        assert_eq!(b.confidence, 0.95);
        assert_eq!(service.recent_suggestions(50).len(), 1);
    }
}
