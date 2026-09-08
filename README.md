# BibleLive

A complete digital church media assistant — desktop-first, offline-first.
Voice-driven Scripture and song detection feeding five independently
controllable display slots, with the operator always in command.

**Stack:** Tauri 2 · Rust · React + TypeScript · SQLite (FTS5)

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — full architecture spec
- [docs/DECISIONS.md](docs/DECISIONS.md) — the decision log from planning
- [docs/ROADMAP.md](docs/ROADMAP.md) — milestones M0–M6+

## Development

```bash
npm install        # frontend dependencies
npm run tauri dev  # run the desktop app in dev mode
npm run tauri build
```

Rust core lives in `src-tauri/` (engines: content, audio, stt, intelligence,
display, session). Frontend lives in `src/` (operator, library, setup,
display-output).

## Status

M0 (scaffold & spec) complete. Next: M1 — Content Engine.
