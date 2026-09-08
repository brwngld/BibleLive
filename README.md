# BibleLive

A complete digital church media assistant — desktop-first, offline-first.
Voice-driven Scripture and song detection feeding five independently
controllable display outputs, with the operator always in command.

**Stack:** Tauri 2 · Rust · React + TypeScript · SQLite (FTS5) · vendored whisper.cpp (fully offline speech-to-text)

## What it does

| Tab | Capabilities |
|---|---|
| 📚 **Library** | Full KJV Bible (66 books) + 12 public-domain hymns built in. Unified full-text search across all content (references, quotations, lyric lines). Canonical Genesis→Revelation ordering, Old/New Testament filters, A–Z / date sorting. Import your own songs and documents (.txt / .docx) with license tracking; copyrighted imports are automatically marked private. |
| 🖼 **Displays** | Five independent Display Slots, each with its own fullscreen output window on any monitor. Scripture / lyrics / image / video renderers with per-slot font family, size and colors (auto-fit — nothing overflows on any screen). Per-slot AUTO / MANUAL / LOCK modes, blank/black, chapter-aware verse stepping (picking John 3:16 loads all of John 3), live styled previews, and saveable display profiles ("Sunday Service", …). |
| 🎙 **Voice** | Fully offline speech-to-text via a vendored whisper.cpp build (no cloud, no data leaving the PC). Microphone/mixer/Bluetooth sources, speech-only vs mixed-church-audio modes, automatic gain control for quiet microphones, live input-level meter, 8-second audio test with transcript, and a one-click diagnostics report. |
| 🎛 **Live Service** | The operator's control room: start/end named service sessions with a running timer, an auto-recorded log of everything projected (reopen any past service from History), AI suggestion cards with SHOW / IGNORE, a live display strip, and EMERGENCY BLANK ALL. |

**Intelligence (local-only):** detects spoken Bible references in natural
forms — "John 3:16", "John chapter three verse sixteen", "First Corinthians
thirteen", verse ranges — plus exact quotations, against the library's
full-text index. The AI only ever *matches* existing content; it never
generates Scripture. Suggestions become display content only through the
Manual → Assisted → Automatic approval chain.

## Hotkeys (work system-wide)

| Keys | Action |
|---|---|
| `Ctrl+Alt+1…5` | Select the active display |
| `Ctrl+Alt+→ / ←` | Next / previous verse or stanza on the active display |
| `Ctrl+Alt+B` | Blank / unblank the active display |
| `← / →` (in an output) | Step that display directly |
| `Tab / Shift+Tab` (in an output) | Cycle between open fullscreen outputs |
| `M` (in an output) | Move the output to the next monitor |
| `Esc` (in an output) | Close that output (content stays on its slot) |

## Development

Requires Windows 10/11 x64 with Node.js LTS, Rust (rustup), and
Visual Studio 2022 Build Tools with the **Desktop development with C++**
workload. Full walkthrough for a new machine: [docs/SETUP-NEW-MACHINE.md](docs/SETUP-NEW-MACHINE.md).

```bash
npm install        # frontend dependencies (once)
npx tauri dev      # run the app (first Rust build ~15 min, then ~1 min)
npx tauri build    # release exe + NSIS/MSI installers
npx tauri build --no-bundle   # just the exe
```

Tests (content engine, ordering, importers, reference parser, whisper FFI,
display manager, profiles):

```bash
cd src-tauri && cargo test    # 14 tests
```

### Project structure

```
src-tauri/            Rust core
  src/content/        unified content store: SQLite + FTS5, seeds, importers
  src/audio/          cpal capture, mono mixdown, resample, AGC, VAD segmenter
  src/stt/            whisper.cpp FFI via a tiny C shim (shim/bl_shim.c)
  src/intelligence/   Bible reference parser + quote matching + suggestions
  src/display/        5-slot display manager, styles, profiles, event bus
  src/session/        service state, listen modes, suggestion queue
  whisper-cpp/        vendored whisper.cpp v1.7.5 (built by build.rs via CMake)
src/                  React + TypeScript UI (library / display / voice / live)
resources/            bundled KJV JSON + public-domain hymns
release/              (gitignored) all-in-one distribution packages
docs/                 architecture, decisions, roadmap, install guides
```

## Installing on another PC

Everything a church PC needs — installer, offline WebView2 runtime, voice
model, one-click setup script, and the user manual — is assembled by:

```bash
npx tauri build
python make_portable_zip.py        # (portable *source* copy, not the installer)
```

The all-in-one installer folder is assembled under `release/BibleLive-FullSetup/`
(installer + `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` + `ggml-*.bin`
model + `Install-BibleLive.cmd`). Run that script on the target PC and it
installs WebView2 (if missing), the app, and the voice model in one click.
Details: [docs/INSTALL.md](docs/INSTALL.md) and the user manual
([BibleLive-User-Manual.pdf](BibleLive-User-Manual.pdf)).

The app is self-contained (static CRT) — no VC++ redistributable, no
internet required. Crashes (if any ever occur) are logged to
`%APPDATA%\BibleLive\crash.log` / `panic.log`, and the Voice tab's
diagnostics button produces a full audio-path report.

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — full architecture spec
- [docs/DECISIONS.md](docs/DECISIONS.md) — decision log from planning
- [docs/ROADMAP.md](docs/ROADMAP.md) — milestones M0–M6+
- [docs/INSTALL.md](docs/INSTALL.md) — installing on church PCs
- [docs/SETUP-NEW-MACHINE.md](docs/SETUP-NEW-MACHINE.md) — dev environment setup
- [BibleLive-User-Manual.pdf](BibleLive-User-Manual.pdf) — end-user manual

## Status

**v0.1.0 — milestones M0–M5 complete** (content, displays, voice,
intelligence, live service control). 14/14 tests passing.

Next (M6+): phone companion app over the church LAN (remote control, then
phone-as-microphone), user roles, private-content access control, more
import formats (OpenLyrics/OpenSong, PDF), backup/restore, cloud options.
