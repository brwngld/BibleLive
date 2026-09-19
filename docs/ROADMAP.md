# BibleLive — Roadmap

Edition gating: **v1 targets Community Pro level.** Church Edition features
are designed-in (schemas, command bus) but shipped later.

## M0 — Scaffold & spec ✅ (this milestone)
- [x] Tauri 2 + React + TypeScript project scaffolded
- [x] Rust module structure: content / audio / stt / intelligence / display / session
- [x] SQLite store opening with unified `content_items` + `search_index` + FTS5 schema
- [x] docs/ARCHITECTURE.md, docs/DECISIONS.md, docs/ROADMAP.md
- [x] `cargo check` + frontend build verified on Windows

## M1 — Content Engine ✅
- [x] Full ContentItem CRUD via Tauri commands
- [x] FTS5 index maintenance (insert / update / delete, schema-version migration)
- [x] KJV seeding from bundled JSON (resources/kjv.json — 66 books, 31,100+ verses)
- [x] Public-domain hymn seed collection (resources/hymns.json — 12 hymns)
- [x] Importers: plain text, .docx (zip + quick-xml paragraph extraction,
      with license prompt + copyright warning; copyrighted/unknown → private)
- [x] Library UI: browse, type filter, unified search, import dialog,
      detail view, edit metadata/visibility, delete
- [x] Library sorting: title A→Z / Z→A, date added, canonical order
      (Genesis → Revelation via bookNumber metadata)
- [x] Bible testament filter: Old Testament / New Testament sub-tabs
- [x] Unified search across all content types (FTS5, porter tokenizer,
      bm25 ranking, snippets) — verified: "god so loved" → John 3:16

## M2 — Display Engine ✅ (was deferred; built after M3/M4)
- [x] Display Slot manager: 5 slots, per-slot monitor mapping (name + size)
- [x] Fullscreen output windows ("display-1"…"display-5") opened per slot,
      positioned on the chosen monitor, fallback to primary if the target
      monitor is missing (slot marked ⚠ degraded — service keeps running)
- [x] Renderers: scripture (large serif text + reference), lyrics (stanza
      lines + song/section caption), image, video, blank/black
- [x] Per-slot mode: AUTO / MANUAL / LOCK; prev/next steps verses/stanzas;
      blank/unblank; one-next-from-blank restores content
- [x] Content pickers: scripture search (reference or quote → verse on
      screen), lyrics picker (song → stanza list), media file dialog
- [x] Voice SHOW integration: showing a suggestion pushes it onto Display 1
- [x] Display Profiles: save current slot layout, one-click apply, delete
- [x] ContentStore section retrieval: verses by key, song sections with
      line breaks preserved from the body

## M3 — Audio + STT ✅
- [x] Device enumeration + voice source config (device, speech-only/mixed,
      sensitivity) persisted in the settings table
- [x] Capture pipeline (cpal): mixdown → 16 kHz resample → energy VAD with
      pre-roll/hangover → speech segments (Stream owned by dedicated thread;
      cpal Stream is !Send)
- [x] whisper.cpp v1.7.5 vendored (src-tauri/whisper-cpp), built by build.rs
      via CMake + tiny C shim (shim/bl_shim.c), static CRT, no bindgen
      (whisper-rs abandoned due to bindgen/version-skew failures)
- [x] Model: ggml-base.en (~148 MB) at %APPDATA%/BibleLive/models
- [x] Audio Test command (8s listen → level, speech detected, transcript)
- [x] Voice UI: source setup, sensitivity, test wizard, live listening
      toggle, transcript feed

## M4 — Intelligence ✅
- [x] Bible reference parser: "John 3:16", "John chapter three verse
      sixteen", "First Corinthians 13", ranges, 66-book alias table;
      5 unit tests
- [x] Exact-phrase quote matching over FTS5 (≥5-word contiguous phrases)
- [x] Suggestion queue (ServiceState) with Manual / Assisted / Automatic
      modes; suggestions emitted to the UI with confidence scores
- [x] SHOW / IGNORE handling in the suggestions panel

## M5 — Service Control ✅
- [x] Service Session lifecycle: start (named or auto-dated) / end, persisted
      in the database with timestamps
- [x] Auto-recorded service log: every scripture, lyric, image, or video put
      on any display during a session is captured (slot, kind, title, label,
      timestamp)
- [x] Live control room (🎛 Live Service tab): session bar with running
      timer, listen toggle, Manual/Assisted/Automatic modes, AI suggestion
      cards with SHOW/IGNORE
- [x] Display strip: one-click active-display switching, live status per
      slot, content labels and page counters
- [x] EMERGENCY BLANK ALL / Restore all displays at once
- [x] Service history: browse past services, reopen their logs, delete
- [x] Command bus groundwork proven (all control flows through commands —
      ready for the phone companion / remote controllers)

## M5.5 — Operator experience & presentation features ✅ (post-M5 increments)
- [x] Stability: whisper mutex crash fixed; VAD rebuilt (raw-level gating,
      adaptive noise floor, AGC after VAD); bounded partial transcription
- [x] Progressive live matching from partial transcripts with
      confidence/stability gating; sentence-boundary flush for continuous
      reading (no-pause scripture still produces verified matches)
- [x] Application menu bar (File/View/Settings/Tools/Help) + Settings
      dialog; Ctrl+1..4 page switching; open data folder (support)
- [x] Automatic mode: verified high-confidence matches project themselves
      with a 10s Undo window; configurable target display (first AUTO slot
      or pinned 1–5, LOCK always protected)
- [x] Theme system: per-display background image, alignment, text shadow,
      transition; presentation-type presets (Classic/Cathedral/Modern/
      Lantern/Bulletin)
- [x] Two-version scripture display (KJV + ASV side by side, lockstep
      stepping, works with voice auto-show too)
- [x] Custom slides: slide content type, editor, picker, projector render;
      built-in starter sets (Welcome / Announcements / Sermon points);
      line-by-line reveal on Next/Prev
- [x] Output transitions (fade / slide) when projected content changes
- [x] Saved theme templates: capture a display's whole look by name, apply
      to any display in one click
- [x] Settings page (5th tab) + classic two-row header; backup/restore;
      verse ranges; WEB Bible; service queue; lower-third announcements

## M6+ — Church Edition & beyond
- [ ] **Designed theme templates** (requested): built-in, artist-designed
      looks (background art + typography + layout) beyond the user-made
      templates. Blocked on reference material — user will supply design
      samples (screenshots of liked designs, background image packs, or
      theme/.pptx files from other software); we then recreate them as
      built-in presets.
- [x] Notification/lower-third alerts (banner announcement over any display)
- [ ] Service queue view (planned order, drag to re-order, go-live)
- [ ] Roles (Admin/Operator/Presenter/Viewer) + private content access control
- [ ] **Phone companion app** (requested): connects to the desktop over the
      church LAN (no internet needed) for (a) remote control — suggestions
      SHOW/IGNORE, display control, modes, audio settings — and (b) using the
      phone's microphone as a wireless audio source streaming to the PC.
      Desktop side needs: LAN server (WebSocket) exposing the existing
      command bus + an audio-ingest path into the capture pipeline.
- [ ] Local-network controllers (tablets, second PC)
- [ ] Passage following (auto-advance verse by verse) — optional setting
- [ ] Partial-quote matching
- [ ] More import formats (OpenLyrics/OpenSong XML, PDF)
- [ ] More Bible versions (e.g., WEB) + verse-range display in picker/suggestions
- [ ] Backup / restore / export-import of the whole library
- [ ] Cloud LLM option (paraphrase/story detection)
- [ ] Cloud sync (Cloud Edition)
- [ ] Linux/macOS packaging
