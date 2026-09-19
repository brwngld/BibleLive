import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  voiceApi,
  type AudioDeviceInfo,
  type AudioTestResult,
  type VoiceConfig,
  type VoiceDiagnostics,
} from "../voice/api";

/**
 * ⚙ SETTINGS — the single home for app configuration, grouped the way you
 * look for it. Operational consoles keep their contextual controls (display
 * style panels stay on the display cards; the Automatic target is also
 * selectable from the Live page); everything "set up once" lives here.
 */
export default function SettingsPage() {
  // audio & voice
  const [config, setConfig] = useState<VoiceConfig | null>(null);
  const [devices, setDevices] = useState<AudioDeviceInfo[]>([]);
  const [sttModel, setSttModel] = useState("base");
  const [testRunning, setTestRunning] = useState(false);
  const [testResult, setTestResult] = useState<AudioTestResult | null>(null);
  const [diagRunning, setDiagRunning] = useState(false);
  const [diagText, setDiagText] = useState<string | null>(null);
  // displays
  const [autoTarget, setAutoTarget] = useState("auto");
  // reader
  const [animation, setAnimation] = useState("none");
  // about
  const [about, setAbout] = useState<{ version: string; build: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshDevices = useCallback(() => {
    voiceApi.listDevices().then(setDevices).catch(console.error);
  }, []);

  useEffect(() => {
    refreshDevices();
    voiceApi.getConfig().then(setConfig).catch(console.error);
    voiceApi.sttModel().then(setSttModel).catch(console.error);
    voiceApi.autoTarget().then(setAutoTarget).catch(console.error);
    setAnimation(localStorage.getItem("bible-animation") ?? "none");
    invoke<{ version: string; build: string }>("app_status")
      .then((st) => setAbout({ version: st.version, build: st.build }))
      .catch(() => {});
  }, [refreshDevices]);

  async function saveConfig(patch: Partial<VoiceConfig>) {
    if (!config) return;
    const prev = config;
    const next = { ...prev, ...patch };
    setConfig(next);
    try {
      await voiceApi.setConfig(next);
    } catch (e) {
      setConfig(prev);
      setError(String(e));
    }
  }

  async function saveSttModel(m: string) {
    const prev = sttModel;
    setSttModel(m);
    try {
      await voiceApi.setSttModel(m);
    } catch (e) {
      setSttModel(prev);
      setError(String(e));
    }
  }

  async function saveAutoTarget(t: string) {
    const prev = autoTarget;
    setAutoTarget(t);
    try {
      await voiceApi.setAutoTarget(t);
    } catch (e) {
      setAutoTarget(prev);
      setError(String(e));
    }
  }

  function saveAnimation(v: string) {
    setAnimation(v);
    localStorage.setItem("bible-animation", v);
  }

  async function runTest() {
    setTestRunning(true);
    setTestResult(null);
    setError(null);
    try {
      setTestResult(await voiceApi.audioTest());
    } catch (e) {
      setError(String(e));
    } finally {
      setTestRunning(false);
    }
  }

  const savedDeviceMissing =
    !!config?.device &&
    devices.length > 0 &&
    !devices.some((d) => d.name === config.device);

  return (
    <div className="voice-grid settings-page">
      {error && <div className="error" style={{ margin: "12px 18px" }}>{error}</div>}

      <div>
        {/* ---- Audio & Voice ---- */}
        <section className="panel">
          <h3>🎙 Audio &amp; Voice</h3>
          <p className="muted">
            How is your church audio connected? Speech-only means a dedicated
            mic or mixer aux channel; Mixed means the full church mix.
          </p>
          <label>
            Audio source
            <select
              value={config?.device ?? ""}
              onFocus={refreshDevices}
              onChange={(e) => saveConfig({ device: e.currentTarget.value || null })}
            >
              <option value="">System default input</option>
              {savedDeviceMissing && config?.device && (
                <option value={config.device}>
                  {config.device} — not found on this PC
                </option>
              )}
              {devices.map((d) => (
                <option key={d.name} value={d.name}>
                  {d.name} {d.isDefault ? "(default)" : ""}{" "}
                  {d.sampleRate ? `· ${d.sampleRate} Hz` : ""}
                </option>
              ))}
            </select>
          </label>
          <button onClick={refreshDevices} style={{ alignSelf: "flex-start" }}>
            ↻ Re-scan devices
          </button>
          {savedDeviceMissing && (
            <div className="warning">
              ⚠ Saved audio device <b>{config?.device}</b> is not available on
              this PC (settings carried over from another machine?). Listening
              will use the <b>System default input</b> until you pick a device
              above.
            </div>
          )}
          <label>
            What does this source contain?
            <select
              value={config?.contentType ?? "speech"}
              onChange={(e) =>
                saveConfig({
                  contentType: e.currentTarget.value as VoiceConfig["contentType"],
                })
              }
            >
              <option value="speech">Speech only (mic / mixer aux)</option>
              <option value="mixed">Mixed church audio</option>
            </select>
          </label>
          <label>
            Speech sensitivity
            <input
              type="range"
              min="0.001"
              max="0.05"
              step="0.001"
              value={config?.vadThreshold ?? 0.015}
              onChange={(e) => saveConfig({ vadThreshold: Number(e.currentTarget.value) })}
            />
            <span className="muted">
              threshold {(config?.vadThreshold ?? 0).toFixed(3)} · auto-adapts
              to room noise (this is the minimum)
            </span>
          </label>
          <label>
            Transcription model
            <select value={sttModel} onChange={(e) => saveSttModel(e.currentTarget.value)}>
              <option value="base">Base — most accurate (default)</option>
              <option value="tiny">Tiny — faster on slower PCs</option>
            </select>
            <span className="muted">
              Tiny transcribes ~4× faster but makes more mistakes.
            </span>
          </label>
          <label>
            Live matching window
            <select
              value={config?.partialWindowMs ?? 2000}
              onChange={(e) => saveConfig({ partialWindowMs: Number(e.currentTarget.value) })}
            >
              <option value="1200">1.2 s — snappiest</option>
              <option value="2000">2 s — recommended</option>
              <option value="3000">3 s</option>
              <option value="4000">4 s — gentlest</option>
            </select>
            <span className="muted">
              How often live text and Scripture matches update while someone
              is still speaking.
            </span>
          </label>

          <h3>🧪 Audio test</h3>
          <p className="muted">
            The system listens for 8 seconds. Say: “Testing BibleLive.”
          </p>
          <button className="primary" onClick={runTest} disabled={testRunning}>
            {testRunning ? "Listening… (8s)" : "Run audio test"}
          </button>
          {testResult && (
            <div className="test-result">
              <div>
                Input: <b>{testResult.device}</b>
              </div>
              <div className="level-bar">
                <div
                  className={"level-fill " + (testResult.peakLevel > 0.005 ? "good" : "low")}
                  style={{ width: `${Math.min(100, testResult.peakLevel * 2000)}%` }}
                />
              </div>
              <div>
                Signal: <b>{testResult.peakLevel > 0.005 ? "Good" : "Low"}</b> ·
                Speech detected: <b>{testResult.speechDetected ? "YES" : "NO"}</b>
              </div>
              {testResult.transcript && (
                <div className="muted">“{testResult.transcript}”</div>
              )}
              {!testResult.speechDetected && (
                <div className="warning">
                  Nobody spoke during the test. Run it again and speak
                  continuously for the whole 8 seconds — e.g. “Testing
                  BibleLive, one, two, three.”
                </div>
              )}
            </div>
          )}

          <h3>🔬 Diagnostics</h3>
          <p className="muted">
            Speaks straight through the whole audio path and reports exact
            numbers. Speak normally for the whole capture.
          </p>
          <button
            disabled={diagRunning}
            onClick={async () => {
              setDiagRunning(true);
              setDiagText(null);
              try {
                const report = (await voiceApi.diagnostics(10)) as unknown as VoiceDiagnostics;
                const text = JSON.stringify(report, null, 2);
                setDiagText(text);
                try {
                  await navigator.clipboard.writeText(text);
                  setDiagText(text + "\n\n(copied to clipboard — paste it to Bernard)");
                } catch {
                  setDiagText(
                    text +
                      "\n\n(saved to %APPDATA%\\BibleLive\\diagnostics-report.txt — send that file)",
                  );
                }
              } catch (e) {
                setDiagText("Diagnostics failed: " + String(e));
              } finally {
                setDiagRunning(false);
              }
            }}
          >
            {diagRunning ? "Capturing… speak now (10s)" : "🔬 Run diagnostics & produce report"}
          </button>
          {diagText && <pre className="diag-report">{diagText}</pre>}
        </section>

        {/* ---- Displays ---- */}
        <section className="panel">
          <h3>🖥 Displays</h3>
          <label>
            Automatic mode target display
            <select value={autoTarget} onChange={(e) => saveAutoTarget(e.currentTarget.value)}>
              <option value="auto">First AUTO display</option>
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={String(n)}>
                  Always Display {n}
                </option>
              ))}
            </select>
            <span className="muted">
              Which screen Automatic mode projects verified Scripture on. A
              display set to LOCK is never taken over. Also changeable from the
              Live page while a service runs.
            </span>
          </label>
          <p className="muted">
            Per-display looks (background image, font, colors, transition,
            second Bible version) live on each display card in the 🖼 Displays
            tab — save any look as a reusable template from there.
          </p>
        </section>

        {/* ---- Bible reader ---- */}
        <section className="panel">
          <h3>📖 Bible reader</h3>
          <label>
            Chapter-change animation
            <select value={animation} onChange={(e) => saveAnimation(e.currentTarget.value)}>
              <option value="none">None</option>
              <option value="fade">Fade</option>
              <option value="slide">Slide</option>
            </select>
            <span className="muted">Applies to the Library Bible reader.</span>
          </label>
        </section>

        {/* ---- Data ---- */}
        <section className="panel">
          <h3>💾 Data</h3>
          <p className="muted">
            <b>File → Backup data…</b> saves your whole library, slides,
            templates and history to a file; <b>File → Restore from backup…</b>{" "}
            brings it back (with confirmation). Do this before reinstalling or
            moving to another PC.
          </p>
          <button
            onClick={() => invoke("open_data_folder").catch(console.error)}
            style={{ alignSelf: "flex-start" }}
          >
            📂 Open data folder
          </button>
          <span className="muted">
            Database, crash log and diagnostics live in %APPDATA%\BibleLive.
          </span>
        </section>

        {/* ---- About ---- */}
        <section className="panel">
          <h3>ℹ About</h3>
          <div className="settings-about">
            <b>BibleLive</b>
            {about ? (
              <span className="muted">
                v{about.version} · built {about.build}
              </span>
            ) : (
              <span className="muted">community edition</span>
            )}
            <span className="muted">
              Free church media assistant — offline speech recognition via
              whisper.cpp. All keyboard shortcuts are listed under Help.
            </span>
          </div>
        </section>
      </div>
    </div>
  );
}
