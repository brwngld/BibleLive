# BibleLive — Architecture

> A complete digital church media assistant. Desktop-first, offline-first,
> one core that scales from a tiny church with one microphone to a large
> church with multiple displays, digital mixers, and local AI.

## 1. Product philosophy

**The software adapts to the church's equipment, rather than forcing the
church to adapt to the software.**

- The software does not care what equipment the church owns — it cares what
  **signal** it receives. No hardware auto-detection ("this is a Shure
  wireless mic"); just "audio is coming through input X".
- The software does not think in "five monitors" — it manages **five Display
  Slots**, each mapped to whatever output the church has (projector, TV,
  LED wall, streaming PC, confidence monitor).
- A tiny church can plug in one microphone and run entirely offline. A large
  church can feed a dedicated mixer channel into the system and run multiple
  displays, local AI, and cloud services. Same application, same core.

## 2. System overview

```
SERVICE CONTROL (session, modes Manual/Assisted/Auto, operator UI)
        ├── AUDIO ENGINE: capture → source config (mic / mixer / interface)
        │     → content-type flag (speech-only vs mixed) → VAD → noise gate
        │     → Whisper STT (streaming segments) → text stream
        ├── INTELLIGENCE ENGINE: reference parser + exact-quote matcher
        │     over unified search index → ranked candidates + confidence
        │     → Suggestion queue → operator [SHOW]/[IGNORE] or auto-show
        ├── CONTENT ENGINE: unified ContentItem store (Bible/Song/Hymn/Book/Doc)
        │     + FTS5 full-text index + language tag + license + visibility
        │     + importers (.txt, .docx) + bundled KJV + public-domain hymns
        └── DISPLAY ENGINE: 5 Display Slots mapped to OS monitors/windows
              → per-slot content renderer (scripture/lyrics/image/video/blank)
              → per-slot mode AUTO/MANUAL/LOCK, profiles
              → survives display disconnect (service keeps running)
```

## 3. Non-negotiable rules (baked into the design)

1. **AI never writes Scripture.** It only matches content that exists in the
   installed library. No generated verses presented as Scripture, ever.
2. **Suggestions are not display commands.** Nothing reaches a screen without
   an operator rule permitting it: Manual → Assisted → Automatic.
3. **Manual override always wins.** Each display slot has AUTO / MANUAL /
   LOCK. LOCK keeps current content regardless of AI detection.
4. **The service continues.** If the mic disconnects, the AI fails, the model
   is slow, a projector disappears, or the network drops — the service keeps
   running and the operator can always take over manually. Manual control is
   the emergency fallback, not just a feature.
5. **Local data ownership.** Offline-first; the church's library lives on its
   machine. Cloud is always optional.
6. **Audio in = a signal.** Setup asks: how is audio connected (single mic /
   mixer / USB interface / other), which source, and does it contain
   speech-only or mixed audio. Advanced settings for channels, noise
   suppression, sensitivity exist but are optional.

## 4. Audio architecture

```
CHURCH AUDIO ──► Audio Interface ──► GENERAL AUDIO INPUT ──► AUDIO ENGINE
                                                        └► SPEECH DETECTION (VAD)
                                                        └► SPEECH-TO-TEXT (Whisper)
```

- **Simple Mode** (small churches): Microphone → PC → BibleLive. Select the
  USB microphone, done.
- **Mixer Mode** (larger churches): a speech-only aux/bus output from the
  mixer feeds the PC, so the engine receives pastor/speech audio rather than
  the full worship mix. If the church only has one mixer output, the mixed
  signal is used and the processing pipeline (noise reduction, VAD, music
  suppression, AGC) handles it — no need to solve every audio problem in v1.
- **Multiple microphones**: the assistant only needs to know *which source to
  listen to* (e.g. pastor mic routed to Aux 1). No microphone-management
  system.
- **Audio Test wizard** before the service: 10-second listen, then reports
  signal level, speech detected (e.g. "Say: Testing BibleLive"), estimated
  quality. Prevents the classic "Windows was listening to the laptop
  microphone while the mixer was on USB" failure.
- **Listening behavior**: always-listening during an active Service Session
  (voice-activity gated, so silence/music trigger no work), with a one-click
  mute/pause in the operator UI.

## 5. Display architecture

- **Five Display Slots** (D1–D5), each independently settable:
  target monitor, mode (AUTO / MANUAL / LOCK), and content.
- **Content types per slot**: scripture, song lyrics, image, video, PDF,
  announcement, presentation, church logo, blank.
- **Manual controls per slot**: previous / next, blank, show image / video /
  scripture.
- **Display Profiles**: saved configurations (Sunday Service, Bible Study,
  Worship Night) loadable with one click.
- **Disconnect resilience**: if a display disappears mid-service the service
  continues; the slot re-attaches when the output returns.

## 6. Content architecture

### Unified Content Item model

A Bible has book/chapter/verse; a song has verse/chorus; a book has
chapters/pages. One underlying model, type-specific bodies:

```
content_items(
  id, item_type,            -- bible | song | hymn | book | document
  title, language,          -- language-aware from day one, English in v1
  license,                  -- public-domain | cc | copyrighted | unknown
  visibility,               -- public | private
  metadata JSON,            -- type-specific metadata (author, album, …)
  body JSON,                -- type-specific structure
  created_at, updated_at
)
search_index(item_id, section_key, section_label, text)   -- normalized sections
search_fts        -- FTS5 full-text index across ALL content types
```

- **One search index across everything**: "Let's sing Amazing Grace" finds
  the hymn; "For God so loved the world" finds John 3:16.
- **Bundled content**: KJV (public domain) + a starter collection of
  public-domain hymns.
- **Importers (v1)**: plain text and Word `.docx`. OpenLyrics/OpenSong XML
  and PDF come later.
- **Licensing**: every imported item carries a license field; import warns on
  copyrighted material and copyrighted items are marked private — no
  sharing/export of copyrighted content. Clear bookkeeping, not legal advice.

## 7. Intelligence architecture

- **Staged trigger rollout**: v1 handles (a) direct references —
  "John 3:16", "John chapter 3 verse 16" — and (b) exact quotations.
  Partial quotes, paraphrases, and story descriptions come in later versions.
- **Local-first**: reference parsing + quote matching run entirely locally
  (fast search over FTS + small local models). Cloud LLM is an optional
  enhancement in Church/Cloud editions for paraphrase/story detection.
- **STT**: local Whisper (whisper.cpp) — fully offline, free, good accuracy.
- **Confidence handling is mode-aware**:
  - *Manual*: operator drives everything; AI still suggests.
  - *Assisted* (default): AI shows a suggestion card — `John 3:16 — 98%`
    with [SHOW] / [IGNORE] buttons; low confidence shows top-3 candidates.
  - *Automatic*: AI shows automatically, only above a high confidence
    threshold; otherwise does nothing and keeps the current display.
- **Status feedback**: operator UI shows 🎙 Listening / 🔎 Searching /
  📖 Detected subtly at all times, even when no match exists.
- **Context**: the Service Session tracks current service, speaker, displayed
  items, current song/passage and can be saved afterward as a service record.
  Passage-following (auto-advancing verse by verse) is an optional setting,
  post-v1.

## 8. Service Control (the operator's control room)

```
┌─────────────────────────────────────────────┐
│              LIVE SERVICE                  │
├─────────────┬───────────────────────────────┤
│ AUDIO       │ CURRENT CONTENT              │
│ ● Listening │ John 3:16                    │
├─────────────┼───────────────────────────────┤
│ AI          │ DISPLAYS                     │
│ 98%         │ D1 ✓  D2 ✓  D3 ✓  D4 ✓ D5 ✓ │
├─────────────┴───────────────────────────────┤
│ [MANUAL] [AUTO] [NEXT] [BLANK] [EMERGENCY]│
└─────────────────────────────────────────────┘
```

Designed so the person operating it does not need to be a software engineer.

## 9. Users, editions, and network

- **Users**: anyone with basic computer skills can operate; the Church
  Edition adds optional roles — Administrator → Operator → Presenter →
  Viewer — governing who can upload/delete content, start services, control
  displays, change AI settings, manage users, access private documents.
- **One codebase, feature-gated editions**:

  | | Community | Community Pro | Church | Cloud |
  |---|---|---|---|---|
  | Offline | ✅ | ✅ | ✅ | Partial |
  | Basic search | ✅ | ✅ | ✅ | ✅ |
  | Local AI | — | ✅ | Optional | — |
  | Cloud AI | — | — | Optional | ✅ |
  | Voice | Basic | Advanced | Advanced | Advanced |
  | Displays | Basic | Basic | Advanced | Advanced |
  | Church users | — | — | ✅ | ✅ |
  | Private content | Limited | Limited | ✅ | ✅ |
  | Cloud sync | — | — | Optional | ✅ |

  v1 targets Community Pro level.
- **Network**: v1 is a single PC, but the core is server-style — all engine
  commands flow through an internal command bus, so a later Church Edition
  can add operator tablets / second control PCs / phone controllers on the
  church LAN (fully internet-free) without re-architecting.

## 10. Technology

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 | small installer, low memory, native multi-window |
| Core | Rust | performance, safety, whisper.cpp & cpal ecosystems |
| UI | React + TypeScript | rich display layouts, web-tech rendering |
| Storage | SQLite (rusqlite, bundled) + FTS5 | zero-config local database |
| STT | whisper.cpp (whisper-rs) | fully offline local Whisper |
| Capture | cpal | cross-platform audio input |
| VAD | Silero | robust voice activity detection |

## 11. Failure behavior (design checklist)

| Failure | Behavior |
|---|---|
| Microphone disconnects | Listening stops, operator notified, service continues; manual control unaffected |
| STT/AI crashes or hangs | Supervised worker; on failure falls back to manual-only; suggestion UI degrades gracefully |
| Model slow (8s) | Suggestions are async; current content never interrupted |
| Wrong verse detected | [IGNORE] / undo in one click; LOCK mode prevents overrides |
| Projector disappears | Slot marked disconnected; other slots and service continue; re-attaches automatically |
| Network loss | Irrelevant in v1 (single PC); Church Edition LAN features degrade to single-PC operation |
| Cloud API down | Optional dependency only; everything local keeps working |
