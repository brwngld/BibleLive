//! Audio Engine — signal-in, hardware-agnostic.
//!
//! The software cares about the signal it receives, not the equipment that
//! produced it. Audio enters as a configured source (default input, a chosen
//! device such as a mixer aux/bus, or a USB interface) flagged as either
//! speech-only or mixed church audio.
//!
//! Pipeline: capture (cpal) → mono mixdown → 16 kHz resample → energy VAD
//! gate with hangover → speech segments → STT worker (whisper.cpp).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

// ---- Device enumeration -----------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::platform::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());
    let mut out = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            let name = d.name().unwrap_or_else(|_| "Unknown device".into());
            let is_default = default_name.as_deref() == Some(name.as_str());
            let (rate, channels) = match d.default_input_config() {
                Ok(c) => (Some(c.sample_rate().0), Some(c.channels())),
                Err(_) => (None, None),
            };
            out.push(AudioDeviceInfo {
                name,
                is_default,
                sample_rate: rate,
                channels,
            });
        }
    }
    out
}

// ---- Voice configuration -----------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceConfig {
    /// None = system default input. Otherwise the device name
    /// (e.g. the mixer aux channel or USB interface).
    pub device: Option<String>,
    /// "speech" (speech-only aux/bus) or "mixed" (full church mix).
    pub content_type: String,
    /// Energy-gate RMS threshold on RAW (pre-gain) audio. Set it just above
    /// your room's noise floor (the diagnostics report shows both numbers):
    /// ~0.015 works for a typical quiet room with a normal speaking voice.
    pub vad_threshold: f32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            device: None,
            content_type: "speech".into(),
            vad_threshold: 0.015,
        }
    }
}

impl VoiceConfig {
    pub fn validate(&mut self) {
        if self.content_type != "speech" && self.content_type != "mixed" {
            self.content_type = "speech".into();
        }
        self.vad_threshold = self.vad_threshold.clamp(0.0005, 0.1);
    }
}

// ---- Conversion helpers -------------------------------------------------------

/// Mix any interleaved multi-channel f32 buffer down to mono.
pub fn mix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear resample mono audio to 16 kHz (adequate for speech; whisper input).
pub fn resample_to_16k(input: &[f32], in_rate: u32) -> Vec<f32> {
    const TARGET: u32 = 16_000;
    if in_rate == TARGET || input.is_empty() {
        return input.to_vec();
    }
    let ratio = in_rate as f64 / TARGET as f64;
    let out_len = ((input.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let frac = (pos - i0 as f64) as f32;
        out.push(input[i0] * (1.0 - frac) + input[i1] * frac);
    }
    out
}

/// Amplify a completed speech segment toward a healthy whisper input level.
/// Applied AFTER voice detection (which gates on raw levels) so quiet mics
/// still transcribe well without the noise floor ever being amplified past
/// the gate.
pub fn agc_boost_segment(seg: &mut [f32]) {
    let peak = seg.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if peak > 0.0008 {
        let gain = (0.70 / peak).min(60.0);
        if gain > 1.01 {
            for x in seg.iter_mut() {
                *x *= gain;
            }
        }
    }
}

// ---- Speech segmentation (energy VAD) ------------------------------------------

const FRAME: usize = 480; // 30 ms @ 16 kHz
const PREROLL_FRAMES: usize = 16; // ~0.5 s
const END_SILENCE_FRAMES: usize = 24; // 0.72 s hangover before a segment closes
const MIN_SPEECH_FRAMES: usize = 13; // ~0.4 s
const MAX_SEGMENT_FRAMES: usize = 1000; // 30 s hard cap

/// Energy-gate VAD with pre-roll and hangover. Feed 16 kHz mono frames;
/// completed speech segments come back from `feed`.
pub struct Segmenter {
    threshold: f32,
    preroll: std::collections::VecDeque<Vec<f32>>,
    current: Vec<f32>,
    silence_frames: usize,
    speech_frames: usize,
    in_speech: bool,
}

impl Segmenter {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            preroll: std::collections::VecDeque::with_capacity(PREROLL_FRAMES),
            current: Vec::new(),
            silence_frames: 0,
            speech_frames: 0,
            in_speech: false,
        }
    }

    /// Feed one 30 ms frame (480 samples, 16 kHz mono). Returns a completed
    /// segment when the hangover timer expires or the cap is hit.
    pub fn feed(&mut self, frame: &[f32]) -> Option<Vec<f32>> {
        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
        let is_speech = rms >= self.threshold;

        if is_speech {
            self.speech_frames += 1;
            self.silence_frames = 0;
            if !self.in_speech {
                self.in_speech = true;
                // flush pre-roll into the current segment
                for f in self.preroll.drain(..) {
                    self.current.extend_from_slice(&f);
                }
            }
            self.current.extend_from_slice(frame);
        } else if self.in_speech {
            self.current.extend_from_slice(frame);
            self.silence_frames += 1;
            if self.silence_frames >= END_SILENCE_FRAMES {
                return self.finish();
            }
        } else {
            self.speech_frames = 0;
            if self.preroll.len() == PREROLL_FRAMES {
                self.preroll.pop_front();
            }
            self.preroll.push_back(frame.to_vec());
        }

        if self.current.len() >= MAX_SEGMENT_FRAMES * FRAME {
            return self.finish();
        }
        None
    }

    fn finish(&mut self) -> Option<Vec<f32>> {
        self.in_speech = false;
        self.silence_frames = 0;
        let seg = std::mem::take(&mut self.current);
        if self.speech_frames >= MIN_SPEECH_FRAMES && seg.len() >= FRAME {
            self.speech_frames = 0;
            Some(seg)
        } else {
            self.speech_frames = 0;
            None
        }
    }
}

// ---- Capture manager ------------------------------------------------------------

/// cpal's Stream is !Send on some platforms, so it must be owned by a single
/// dedicated thread. CaptureManager only holds the stop signal for that
/// thread, which makes it safely shareable as Tauri state.
#[derive(Clone, Default)]
pub struct CaptureManager {
    session: Arc<Mutex<Option<SessionHandle>>>,
}

struct SessionHandle {
    stop: mpsc::Sender<()>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTestResult {
    pub device: String,
    pub peak_level: f32,
    pub speech_detected: bool,
    pub transcript: String,
    pub seconds: u32,
}

impl CaptureManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_listening(&self) -> bool {
        self.session.lock().is_some()
    }

    /// Start live listening. `on_segment` receives speech segments (16 kHz
    /// mono f32); `on_level` receives the input level (~every 150 ms) for
    /// live meters. Both run on capture worker threads.
    pub fn start(
        &self,
        config: &VoiceConfig,
        on_segment: impl FnMut(Vec<f32>) + Send + 'static,
        on_level: impl Fn(f32) + Send + 'static,
    ) -> Result<(), String> {
        self.start_collector(config, on_segment, on_level)
            .map(|_| ())
    }

    /// Like start(), but also hands back the live feed handle so callers
    /// (diagnostics) can inspect raw levels and applied gain.
    pub fn start_collector(
        &self,
        config: &VoiceConfig,
        on_segment: impl FnMut(Vec<f32>) + Send + 'static,
        on_level: impl Fn(f32) + Send + 'static,
    ) -> Result<Arc<SharedFeed>, String> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            return Err("already listening".into());
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();
        let (feed_tx, feed_rx) = mpsc::channel::<Arc<SharedFeed>>();
        let config = config.clone();

        // The capture thread owns the cpal Stream for its whole lifetime.
        std::thread::spawn(move || match open_capture(&config, on_segment) {
            Ok((stream, feed)) => {
                let _ = init_tx.send(Ok(()));
                let _ = feed_tx.send(Arc::clone(&feed));
                // Level forwarder: publishes the meter value until capture ends.
                {
                    let feed = Arc::clone(&feed);
                    std::thread::spawn(move || loop {
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        on_level(latest_level(&feed));
                    });
                }
                // Block until stop is requested, then drop the stream.
                loop {
                    match stop_rx.recv_timeout(std::time::Duration::from_millis(250)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
                // stream dropped here → capture stops
            }
            Err(e) => {
                let _ = init_tx.send(Err(e));
            }
        });

        match init_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => {
                *guard = Some(SessionHandle { stop: stop_tx });
                let feed = feed_rx
                    .recv_timeout(std::time::Duration::from_secs(1))
                    .map_err(|_| "capture feed not reported".to_string())?;
                Ok(feed)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("audio capture init timed out".into()),
        }
    }

    pub fn stop(&self) {
        if let Some(s) = self.session.lock().take() {
            let _ = s.stop.send(());
        }
    }
}

pub struct SharedFeed {
    segmenter: Mutex<Segmenter>,
    leftover: Mutex<Vec<f32>>,
    level: AtomicU32,     // f32 bits of latest frame RMS (post-gain)
    agc_peak: AtomicU32,  // f32 bits of decaying raw peak used for auto-gain
    last_gain: AtomicU32, // f32 bits of last applied gain factor
}

impl SharedFeed {
    pub fn raw_peak(&self) -> f32 {
        f32::from_bits(self.agc_peak.load(Ordering::Relaxed))
    }
    pub fn gain(&self) -> f32 {
        f32::from_bits(self.last_gain.load(Ordering::Relaxed))
    }
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

/// Describe the device that WOULD be captured (name + actual config).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDescription {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
    pub error: Option<String>,
}

/// Resolve a configured device name to a capture device. A saved name that
/// no longer exists (settings carried to another PC, unplugged USB mixer)
/// falls back to the system default so a service keeps running, mirroring
/// the display engine's degraded-mode philosophy.
fn resolve_input_device(name: Option<&str>) -> Result<(cpal::Device, String), String> {
    let host = cpal::platform::default_host();
    if let Some(name) = name {
        let found = host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().ok().as_deref() == Some(name));
        if let Some(d) = found {
            return Ok((d, name.to_string()));
        }
    }
    host.default_input_device()
        .map(|d| (d, "System default input".to_string()))
        .ok_or_else(|| match name {
            Some(n) => format!("input device not found: {n}, and no system default input"),
            None => "no default input device".to_string(),
        })
}

pub fn describe_capture(config: &VoiceConfig) -> CaptureDescription {
    let resolved = resolve_input_device(config.device.as_deref());
    let Some((device, mut label)) = resolved.ok() else {
        return CaptureDescription {
            device: config.device.clone().unwrap_or_else(|| "default".into()),
            sample_rate: 0,
            channels: 0,
            sample_format: String::new(),
            error: Some("input device not found".into()),
        };
    };
    if let Some(saved) = &config.device {
        if saved != &label {
            label = format!("{label} (saved device \"{saved}\" not found)");
        }
    }
    match device.default_input_config() {
        Ok(c) => CaptureDescription {
            device: label,
            sample_rate: c.sample_rate().0,
            channels: c.channels(),
            sample_format: format!("{:?}", c.sample_format()),
            error: None,
        },
        Err(e) => CaptureDescription {
            device: label,
            sample_rate: 0,
            channels: 0,
            sample_format: String::new(),
            error: Some(format!("device config error: {e}")),
        },
    }
}

fn open_capture(
    config: &VoiceConfig,
    mut on_segment: impl FnMut(Vec<f32>) + Send + 'static,
) -> Result<(cpal::Stream, Arc<SharedFeed>), String> {
    let (device, _label) = resolve_input_device(config.device.as_deref())?;

    let supported = device
        .default_input_config()
        .map_err(|e| format!("device config: {e}"))?;
    let in_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();

    let feed = Arc::new(SharedFeed {
        segmenter: Mutex::new(Segmenter::new(config.vad_threshold)),
        leftover: Mutex::new(Vec::new()),
        level: AtomicU32::new(0),
        agc_peak: AtomicU32::new(0),
        last_gain: AtomicU32::new(0),
    });

    let (seg_tx, seg_rx) = mpsc::channel::<Vec<f32>>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    // Worker: forward completed segments to the consumer until stop.
    std::thread::spawn(move || {
        loop {
            match seg_rx.recv_timeout(std::time::Duration::from_millis(300)) {
                Ok(seg) => on_segment(seg),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if stop_rx.try_recv().is_ok() {
                break;
            }
        }
    });

    let err_fn = move |e| eprintln!("audio stream error: {e}");
    let feed_cb = Arc::clone(&feed);

    let stream = match sample_format {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &supported.into(), feed_cb, seg_tx, in_rate, channels, err_fn),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &supported.into(), feed_cb, seg_tx, in_rate, channels, err_fn),
        cpal::SampleFormat::U8 => build_stream::<u8>(&device, &supported.into(), feed_cb, seg_tx, in_rate, channels, err_fn),
        other => return Err(format!("unsupported sample format: {other:?}")),
    }
    .map_err(|e| format!("open capture stream: {e}"))?;

    stream.play().map_err(|e| format!("start capture: {e}"))?;

    Ok((stream, Arc::clone(&feed)))
}

#[allow(clippy::too_many_arguments)]
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    feed: Arc<SharedFeed>,
    seg_tx: mpsc::Sender<Vec<f32>>,
    in_rate: u32,
    channels: usize,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mono: Vec<f32> = data
                .iter()
                .map(|s| <f32 as cpal::FromSample<T>>::from_sample_(*s))
                .collect();
            let mono = mix_to_mono(&mono, channels);
            let mono16 = resample_to_16k(&mono, in_rate);

            // Voice detection runs on RAW audio so the energy gate compares
            // against the room's true noise floor. Boosting before the gate
            // would amplify the noise floor past every threshold on the
            // slider (the pre-fix behavior: continuous false "speech", CPU
            // burned transcribing noise, real speech buried in mega-segments).
            // Gain is tracked here but applied only to completed segments.
            let chunk_peak = mono16.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let raw_rms =
                (mono16.iter().map(|s| s * s).sum::<f32>() / mono16.len().max(1) as f32).sqrt();
            {
                let mut peak = f32::from_bits(feed.agc_peak.load(Ordering::Relaxed));
                peak = (peak.max(chunk_peak)) * 0.999;
                feed.agc_peak.store(peak.to_bits(), Ordering::Relaxed);
                let gain = if peak > 0.0008 {
                    (0.70 / peak).min(60.0)
                } else {
                    1.0
                };
                feed.last_gain.store(gain.to_bits(), Ordering::Relaxed);
                // level meter shows the boosted view (bar moves on speech)
                let shown = (raw_rms * gain).min(1.0);
                feed.level.store(shown.to_bits(), Ordering::Relaxed);
            }

            // buffer into 30 ms frames and feed the segmenter
            let mut leftover = feed.leftover.lock();
            leftover.extend_from_slice(&mono16);
            let mut segmenter = feed.segmenter.lock();
            while leftover.len() >= FRAME {
                let frame: Vec<f32> = leftover.drain(..FRAME).collect();
                if let Some(mut seg) = segmenter.feed(&frame) {
                    agc_boost_segment(&mut seg);
                    let _ = seg_tx.send(seg);
                }
            }
        },
        err_fn,
        None,
    )
}

/// Collect the current input level (for level meters).
pub fn latest_level(feed: &SharedFeed) -> f32 {
    f32::from_bits(feed.level.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quiet speech segment is amplified toward whisper's healthy input
    /// level; a near-silent one is left alone (no noise multiplication).
    #[test]
    fn agc_boosts_quiet_segments_only() {
        let mut quiet = vec![0.01f32; 4800];
        agc_boost_segment(&mut quiet);
        let peak = quiet.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.5 && peak <= 0.71, "peak should be ~0.7, got {peak}");

        let mut silent = vec![0.0001f32; 4800];
        agc_boost_segment(&mut silent);
        assert!(silent.iter().all(|x| x.abs() <= 0.0001));
    }

    /// A saved device name from another PC must fall back to the system
    /// default instead of hard-failing capture (PC-to-PC transfer case).
    #[test]
    fn missing_saved_device_falls_back_to_default() {
        let host = cpal::platform::default_host();
        if host.default_input_device().is_none() {
            return; // machine has no input device at all — nothing to fall back to
        }
        let cfg = VoiceConfig {
            device: Some("Definitely Not A Real Device (bogus)".into()),
            ..Default::default()
        };
        let d = describe_capture(&cfg);
        assert!(d.device.starts_with("System default input"), "label: {}", d.device);
        assert!(d.device.contains("not found"), "label should name the missing device: {}", d.device);
        assert_ne!(d.error.as_deref(), Some("input device not found"));
    }
}
