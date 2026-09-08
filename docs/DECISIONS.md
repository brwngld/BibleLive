# BibleLive — Decision Log

Decisions settled during architecture planning (2026-09-05). Anything not
listed here is open; anything listed here is settled unless Bernard changes it.

## Batch 1 — Core Experience

| # | Question | Decision |
|---|---|---|
| 1 | Primary user | **Anyone can operate.** Church Edition adds optional roles (Admin → Operator → Presenter → Viewer) so larger churches can restrict; small churches don't have to. |
| 2 | Live Mode default | **Church chooses**: Manual / Assisted / Automatic. Assisted is the default suggestion flow. |
| 3 | Low confidence | **Configurable by mode**: Assisted shows top-3 suggestions; Automatic only acts above a high threshold, otherwise does nothing. |
| 4 | Passage following | **Optional setting**, post-v1. |
| 5 | Scripture triggers | **Staged rollout**: v1 = direct references + exact quotations. Partial quotes, paraphrases, story descriptions later. |
| 6 | Non-scripture speech | Keep listening silently + subtle status indicator (🎙 Listening / 🔎 Searching / 📖 Detected). |
| 7 | Search scope | **All installed content** — Bible, songs, hymns, books — one unified index. |
| 8 | Multiple content-type matches | **Mode-aware**: Assisted shows candidates; Automatic picks by confidence + current service context. |
| 9 | Service context | **Service Sessions**: tracked live (service, speaker, displayed items, current song/passage), savable as a record. |
| 10 | Product vision | **D — complete digital church media assistant** (four engines under Service Control). |

## Batch 2 — Content & Library

| # | Question | Decision |
|---|---|---|
| 1 | Content model | **Unified Content Item model** (id, type, language, metadata, structured body) + one search index across all types. Not separate per-type modules, not flat documents. |
| 2 | Bundled content | **KJV + public-domain hymns** on first install. |
| 3 | Import formats v1 | **Plain text + Word .docx**. OpenLyrics/OpenSong XML and PDF later. |
| 4 | Languages | **Language-aware schema from day one; English-only in v1.** Local languages plug in later. |
| 5 | Privacy & permissions | **Visibility flags (public/private) + roles** in Church Edition. |
| 6 | Licensing | **License metadata field + import warnings.** Copyrighted imports are marked private; no sharing/export of copyrighted content. |

## Batch 3 — Audio & AI

| # | Question | Decision |
|---|---|---|
| 1 | STT engine | **Local Whisper (whisper.cpp)** — fully offline, free. |
| 2 | AI location | **Local-first**: local search + small local models for v1; cloud LLM optional in Church/Cloud editions for paraphrase/story detection. |
| 3 | Listening mode | **Always-listening during an active Service Session** with VAD (silence/music trigger no work), plus a one-click mute/pause in the operator UI. |

## Batch 4 — Platform & scope

| # | Question | Decision |
|---|---|---|
| 1 | Tech stack | **Tauri 2 + Rust core + React/TypeScript UI + SQLite** (rusqlite bundled, FTS5). cpal for audio, whisper-rs for STT. |
| 2 | Network | **v1 = single PC, network-ready**: server-style core with an internal command bus so remote controllers (tablets, second PC) can be added in the Church Edition without re-architecting. |
| 3 | Editions | **One codebase, feature gates**: Community → Community Pro (local AI/voice) → Church (multi-display, users, private content) → Cloud (sync, cloud AI). v1 targets Community Pro level. |

## Physical-interface principles (from the design discussion)

- Software cares about the **signal**, not the equipment. Setup asks:
  connection type (single mic / mixer / USB interface / other), which source,
  and speech-only vs mixed audio.
- **Five Display Slots**, not five monitors. Each slot: own target, own mode
  (AUTO / MANUAL / LOCK), own content, independent control. Display Profiles
  (Sunday Service, Bible Study, Worship Night) load in one click.
- **Manual override is the emergency fallback.** The AI never has absolute
  control of any screen.
- **Audio Test wizard** (10-second pre-service check: signal level, speech
  detected, "say: Testing BibleLive").
- **The service continues** through any component failure.

## Platform

- **Desktop-first** (Windows first, Linux/macOS later). Needs mic access,
  local AI, offline operation, multi-display, video, low latency, hardware
  access — a browser is the wrong fight.
