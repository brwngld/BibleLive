# Installing BibleLive on another PC

## What you need

Copy these to a USB drive or shared folder:

| File | Size | Required? |
|---|---|---|
| `src-tauri/target/release/bundle/nsis/BibleLive_0.1.0_x64-setup.exe` | ~5 MB | ✅ This is the installer |
| `%APPDATA%\BibleLive\models\ggml-base.en.bin` | ~148 MB | Optional — only for voice features |

## Requirements on the target PC

- Windows 10 or 11, 64-bit
- **Nothing else.** The app is fully self-contained: the user interface,
  KJV Bible, hymn collection, SQLite database, and the Whisper speech engine
  are all built into the exe. No Visual C++ redistributable, no .NET, no
  internet connection needed.
- **WebView2 Runtime** — already built into up-to-date Windows 10/11. If the
  PC is very old/offline and missing it, the installer downloads it
  automatically (needs internet once) or grab the offline "WebView2 Runtime
  Evergreen Standalone" installer from Microsoft's site and run it first.

## Install steps

1. Copy `BibleLive_0.1.0_x64-setup.exe` to the target PC and run it.
2. Follow the installer (choose per-user or all-users). A desktop shortcut
   is created.
3. Launch BibleLive — the KJV (66 books) and 12 public-domain hymns are
   seeded automatically on first start.

## Voice (speech-to-text) on the target PC

The Whisper model is too big to embed, so copy it once:

1. On this PC, copy `C:\Users\Bernard\AppData\Roaming\BibleLive\models\ggml-base.en.bin`
2. On the target PC, create `C:\Users\<user>\AppData\Roaming\BibleLive\models\`
   and paste the file there. (Tip: paste `%APPDATA%\BibleLive\models` into
   File Explorer's address bar to jump straight there.)
3. Start BibleLive → 🎙 Voice tab should show the model present.

Without the model file, everything works except listening/transcription
(the Voice tab shows a reminder banner until the model is in place).

## What is NOT transferred

- Imported documents/songs you added yourself (they live in the church's
  database at `%APPDATA%\BibleLive\biblelive.db`) — copy `biblelive.db` to
  the same location on the target PC to clone the library.
- Display profiles/styles (also in the database, travels with it).

## Rebuilding after code changes

```bash
npx tauri build          # installers in src-tauri/target/release/bundle/
npx tauri build --no-bundle   # just the raw exe (development use)
```

The installer compresses the 19 MB app down to ~5 MB.
