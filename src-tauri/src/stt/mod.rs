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
    fn bl_run(ctx: WhisperCtx, samples: *const f32, n_samples: c_int, n_threads: c_int) -> c_int;
    fn bl_segment(ctx: WhisperCtx, i: c_int) -> *const c_char;
    fn bl_free(ctx: WhisperCtx);
}

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
        let ctx = *self.ctx.lock();
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as c_int)
            .unwrap_or(4);
        let n = unsafe { bl_run(ctx, samples.as_ptr(), samples.len() as c_int, threads) };
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
