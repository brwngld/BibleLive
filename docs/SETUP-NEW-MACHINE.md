# Setting up the BibleLive dev environment on a new PC

Follow top to bottom. Everything is free. Total download ~3 GB, time ~30-45 min.

## 1. Install the toolchain

Install in this order:

1. **Node.js LTS** (18 or later) — https://nodejs.org (default options)
2. **Rust** — https://rustup.rs (default options; close and reopen the terminal after)
3. **Visual Studio 2022 Build Tools** (NOT full Visual Studio) —
   https://visualstudio.microsoft.com/downloads/ → "Build Tools for Visual Studio 2022"
   In its installer, tick ONE workload:
   - **Desktop development with C++**
   That includes MSVC, Windows SDK, and CMake (we need all three).

## 2. Copy the project

Copy the `BibleHub` folder anywhere, e.g. `C:\Users\<you>\Documents\Programming\BibleHub`.

## 3. One-time portability fix (only if the build complains about cmake)

The project remembers where cmake lives on the ORIGINAL machine
(`src-tauri/.cargo/config.toml`, the `CMAKE =` line). If your Build Tools
installed somewhere else, whisper's build step says "run cmake configure" /
"program not found". Fix either way:

- Edit `src-tauri/.cargo/config.toml` and point `CMAKE =` at your cmake, e.g.
  `C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe`
  (search for `cmake.exe` under `C:/Program Files (x86)/Microsoft Visual Studio/`), or
- Add that folder to your PATH and set `CMAKE = "cmake"`.

(build.rs already falls back to PATH cmake automatically when the saved
path does not exist.)

## 4. Build and run

From the project root:

```bash
npm install          # once; installs frontend tooling
npx tauri dev        # compiles Rust (~15 min the FIRST time, ~1 min after)
```

The app opens. To make a release build + installers:

```bash
npx tauri build --no-bundle   # just the exe  → src-tauri/target/release/biblelive.exe
npx tauri build               # exe + installers
```

## 5. Voice model (already on this PC if the app was installed)

The speech model lives OUTSIDE the project at
`%APPDATA%\BibleLive\models\` — if the app was installed there with the
all-in-one package, it is already present. Otherwise copy the `ggml-*.bin`
files there (tiny = fast, base = accurate; the app prefers base when both
exist).

## 6. Tests

```bash
cd src-tauri
cargo test
```
All 14 must pass.

## Troubleshooting

- **"link.exe not found"** → you skipped the C++ workload in step 1.3.
- **"cmake" errors** → see step 3.
- **Whisper builds forever the first time** → normal, it compiles ~200 C++
  files once; after that it is cached in `src-tauri/whisper-build/`.
- **WebView2 missing** (app flashes/closes) → the PC already has it if the
  installed app runs; otherwise run the all-in-one installer once.
