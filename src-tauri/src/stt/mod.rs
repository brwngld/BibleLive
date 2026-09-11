//! Speech-to-text — local Whisper (vendored whisper.cpp, built by build.rs
//! with a tiny C shim in shim/bl_shim.c) for fully offline operation.
//! One engine instance is loaded lazily and shared; each transcription runs
//! through its own context (whisper contexts are not thread-safe per call).

use parking_lot::Mutex;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

type WhisperCtx = *mut c_void;

extern "C" {
    fn bl_init(model_path: *const c_char) -> WhisperCtx;
    fn bl_install_crash_handler();
    fn bl_run(
        ctx: WhisperCtx,
        samples: *const f32,
        n_samples: c_int,
        n_threads: c_int,
        initial_prompt: *const c_char,
    ) -> c_int;
    fn bl_segment(ctx: WhisperCtx, i: c_int) -> *const c_char;
    fn bl_free(ctx: WhisperCtx);
}

/// Decoder bias toward the app's domain: scripture phrasing and book names
/// in KJV diction. Without it whisper guesses everyday English ("God said"
/// became "got saved"). The silence test verifies the prompt never makes
/// non-speech hallucinate scripture.
const BIBLE_PROMPT: &str = "King James Bible reading. And God said, Let there \
be light: and there was light. In the beginning God created the heaven and the \
earth. For God so loved the world, that he gave his only begotten Son. Genesis,\
 Exodus, Psalms, Proverbs, Isaiah, Matthew, Mark, Luke, John, Acts, Romans, \
Revelation.";

/// Thread-safe handle to a loaded whisper context. Calls are serialized with
/// a mutex because whisper_full mutates the context.
pub struct SttEngine {
    ctx: Mutex<WhisperCtx>,
}

unsafe impl Send for SttEngine {}
unsafe impl Sync for SttEngine {}

impl SttEngine {
    pub fn load(model_path: &Path) -> Result<Self, String> {
        let cpath = CString::new(
            model_path
                .to_str()
                .ok_or("model path is not valid unicode")?,
        )
        .map_err(|_| "model path contains NUL")?;
        let ctx = unsafe { bl_init(cpath.as_ptr()) };
        if ctx.is_null() {
            return Err(format!(
                "failed to load whisper model from {}",
                model_path.display()
            ));
        }
        Ok(Self {
            ctx: Mutex::new(ctx),
        })
    }

    /// Transcribe 16 kHz mono speech audio. Returns the joined text.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        if samples.len() < 8_000 {
            // < 0.5 s of audio: nothing useful to transcribe.
            return Ok(String::new());
        }
        // The guard must stay alive for the whole call: whisper_full mutates
        // the context, so two callers inside bl_run at once corrupt it and
        // crash the process (0xC0000005 in crash.log). Copying the raw
        // pointer out and dropping the guard early serializes nothing.
        let guard = self.ctx.lock();
        let ctx = *guard;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as c_int)
            .unwrap_or(4);
        let prompt = CString::new(BIBLE_PROMPT).unwrap_or_default();
        let n = unsafe {
            bl_run(ctx, samples.as_ptr(), samples.len() as c_int, threads, prompt.as_ptr())
        };
        if n < 0 {
            return Err("whisper inference failed".into());
        }
        let mut text = String::new();
        for i in 0..n {
            let seg = unsafe {
                let p = bl_segment(ctx, i);
                if p.is_null() {
                    continue;
                }
                CStr::from_ptr(p).to_string_lossy().into_owned()
            };
            text.push_str(&seg);
            text.push(' ');
        }
        Ok(text.trim().to_string())
    }
}

impl Drop for SttEngine {
    fn drop(&mut self) {
        let ctx = *self.ctx.lock();
        if !ctx.is_null() {
            unsafe { bl_free(ctx) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn bench_cpu_inference_5s_audio() {
        // Measures pure CPU inference speed on THIS machine for each
        // installed model, using 5 s of low-level noise as stand-in audio.
        for name in ["ggml-tiny.en.bin", "ggml-base.en.bin"] {
            let path = models_dir().join(name);
            if !path.exists() {
                eprintln!("BENCH {name}: model not present, skipped");
                continue;
            }
            let engine = SttEngine::load(&path).expect("model loads");
            let mut samples = vec![0.0f32; 16_000 * 5];
            for (i, s) in samples.iter_mut().enumerate() {
                *s = ((i % 97) as f32 / 97.0 - 0.5) * 0.01;
            }
            let t0 = std::time::Instant::now();
            let _ = engine.transcribe(&samples).expect("inference runs");
            eprintln!("BENCH {name}: 5.0s of audio transcribed in {:?}", t0.elapsed());
        }
    }

    #[test]
    fn loads_model_and_transcribes_silence() {
        let path = default_model_path();
        if !path.exists() {
            eprintln!("whisper model not present, skipping");
            return;
        }
        let engine = SttEngine::load(&path).expect("model loads via shim FFI");
        // One second of silence → no segments, empty text, no crash.
        let silence = vec![0.0f32; 16_000];
        let text = engine.transcribe(&silence).expect("inference runs");
        assert!(text.is_empty(), "silence should transcribe to nothing");
    }
}

/// Install the native crash logger (call once at startup).
pub fn install_crash_handler() {
    unsafe {
        bl_install_crash_handler();
    }
}

fn models_dir() -> std::path::PathBuf {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_default()
        });
    base.join("BibleLive").join("models")
}

/// Model candidates in preference order: base.en is more accurate,
/// tiny.en is ~4x faster (better for modest church hardware).
pub fn model_candidates() -> Vec<std::path::PathBuf> {
    let dir = models_dir();
    vec![
        dir.join("ggml-base.en.bin"),
        dir.join("ggml-tiny.en.bin"),
    ]
}

/// First existing model, else the preferred default.
pub fn default_model_path() -> std::path::PathBuf {
    model_candidates()
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| models_dir().join("ggml-base.en.bin"))
}

/// Copy the installer-bundled models into %APPDATA%/BibleLive/models when
/// the user doesn't already have them, so a fresh install needs no manual
/// model placement. Never overwrites an existing file (e.g. a manually
/// updated model). Returns the paths that were copied.
pub fn provision_bundled_models(
    bundled_dir: &std::path::Path,
) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut copied = Vec::new();
    for name in ["ggml-base.en.bin", "ggml-tiny.en.bin"] {
        let src = bundled_dir.join(name);
        let dest = models_dir().join(name);
        if src.exists() && !dest.exists() {
            std::fs::create_dir_all(models_dir())?;
            std::fs::copy(&src, &dest)?;
            copied.push(dest);
        }
    }
    Ok(copied)
}

/// Resolve a model choice ("base" | "tiny") to its file path.
pub fn model_path_for(choice: &str) -> std::path::PathBuf {
    let file = match choice {
        "tiny" => "ggml-tiny.en.bin",
        _ => "ggml-base.en.bin",
    };
    models_dir().join(file)
}
