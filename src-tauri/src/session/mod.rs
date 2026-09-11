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
}

impl ServiceState {
    pub fn new() -> Self {
        Self {
            mode: Mutex::new(ListenMode::Assisted),
            suggestions: Mutex::new(VecDeque::new()),
            next_id: Mutex::new(1),
            current: Mutex::new(None),
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

    /// Insert or refresh: a live (partial-transcript) suggestion replaces
    /// the pending card for the same content instead of stacking duplicate
    /// cards as the match strengthens.
    pub fn upsert_suggestion(&self, s: Suggestion) -> Suggestion {
        let mut q = self.suggestions.lock();
        if let Some(existing) = q.iter_mut().rev().find(|e| {
            e.status == "pending" && e.item_id == s.item_id && e.section_key == s.section_key
        }) {
            existing.kind = s.kind.clone();
            existing.label = s.label.clone();
            existing.confidence = s.confidence;
            existing.preview = s.preview.clone();
            return existing.clone();
        }
        drop(q);
        self.add_suggestion(s)
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
