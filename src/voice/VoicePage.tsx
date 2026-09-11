import { useCallback, useEffect, useRef, useState } from "react";
import {
  voiceApi,
  onTranscript,
  onSuggestion,
  onLevel,
  type UnlistenFn,
  type AudioDeviceInfo,
  type AudioTestResult,
  type Suggestion,
  type VoiceConfig,
  type TranscriptEvent,
  type SuggestionEvent,
  type VoiceDiagnostics,
} from "./api";
import { displayApi } from "../display/api";
import { ITEM_TYPE_LABELS, type ItemType } from "../library/types";

interface TranscriptLine {
  text: string;
  at: string;
}

export default function VoicePage() {
  const [devices, setDevices] = useState<AudioDeviceInfo[]>([]);
  const [config, setConfig] = useState<VoiceConfig | null>(null);
  const [mode, setMode] = useState("assisted");
  const [listening, setListening] = useState(false);
  const [model, setModel] = useState<{ exists: boolean; path: string; sizeMb: number | null } | null>(null);
  const [transcript, setTranscript] = useState<TranscriptLine[]>([]);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [testRunning, setTestRunning] = useState(false);
  const [level, setLevel] = useState(0);
  const [buildTag, setBuildTag] = useState("");
  const [sttModel, setSttModel] = useState("base");
  const [diagRunning, setDiagRunning] = useState(false);
  const [diagText, setDiagText] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<AudioTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const feedRef = useRef<HTMLDivElement>(null);

  const refreshSuggestions = useCallback(() => {
    voiceApi.suggestions().then(setSuggestions).catch(() => {});
  }, []);

  // Device lists go stale (mics plugged in after launch, DroidCam started
  // later, Bluetooth headsets pairing) — re-query whenever the user might
  // be about to look for a new device.
  const refreshDevices = useCallback(() => {
    voiceApi.listDevices().then(setDevices).catch(console.error);
  }, []);

  useEffect(() => {
    refreshDevices();
    voiceApi.getConfig().then(setConfig).catch(console.error);
    voiceApi.getMode().then(setMode).catch(console.error);
    voiceApi.modelStatus().then(setModel).catch(console.error);
    voiceApi.sttModel().then(setSttModel).catch(console.error);
    import("@tauri-apps/api/core").then(({ invoke }) =>
      invoke<{ version: string; build: string }>("app_status")
        .then((st) => setBuildTag(`v${st.version} · built ${st.build}`))
        .catch(() => {}),
    );
    voiceApi.listeningStatus().then(setListening).catch(console.error);
    refreshSuggestions();

    let unlisteners: Promise<UnlistenFn>[] = [];
    let u1: UnlistenFn | undefined;
    let u2: UnlistenFn | undefined;
    unlisteners.push(
      onTranscript((e: TranscriptEvent) => {
        setTranscript((prev) =>
          [...prev, { text: e.text, at: new Date().toLocaleTimeString() }].slice(-100),
        );
        requestAnimationFrame(() => {
          feedRef.current?.scrollTo({ top: feedRef.current.scrollHeight });
        });
      }),
    );
    unlisteners.push(
      onSuggestion((e: SuggestionEvent) => {
        setSuggestions((prev) => [e.suggestion, ...prev].slice(0, 40));
      }),
    );
    unlisteners.push(onLevel((lv) => setLevel(lv)));
    Promise.all(unlisteners).then(([a, b]) => {
      u1 = a;
      u2 = b;
    });
    return () => {
      u1?.();
      u2?.();
    };
  }, [refreshSuggestions]);

  useEffect(() => {
    const onFocus = () => refreshDevices();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refreshDevices]);

  async function saveConfig(patch: Partial<VoiceConfig>) {
    if (!config) return;
    const next = { ...config, ...patch };
    setConfig(next);
    try {
      await voiceApi.setConfig(next);
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggleListening() {
    setError(null);
    try {
      if (listening) {
        await voiceApi.stop();
        setListening(false);
      } else {
        await voiceApi.start();
        setListening(true);
      }
    } catch (e) {
      setError(String(e));
    }
  }

  async function changeMode(m: string) {
    setMode(m);
    try {
      await voiceApi.setMode(m);
    } catch (e) {
      setError(String(e));
    }
  }

  async function changeSttModel(m: string) {
    const prev = sttModel;
    setSttModel(m);
    try {
      await voiceApi.setSttModel(m);
    } catch (e) {
      setSttModel(prev);
      setError(String(e));
    }
  }

  async function runTest() {
    setTestRunning(true);
    setTestResult(null);
    setError(null);
    try {
      const result = await voiceApi.audioTest();
      setTestResult(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setTestRunning(false);
    }
  }

  async function respond(s: Suggestion, show: boolean) {
    try {
      if (show) {
        // Push onto a display slot (default 1), then mark shown.
        const isScripture = s.itemId.startsWith("bible-");
        if (isScripture) {
          await displayApi.setScripture(1, s.itemId, [s.sectionKey]);
        } else {
          await displayApi.setSection(1, s.itemId, s.sectionKey);
        }
      }
      await voiceApi.respond(s.id, show);
      setSuggestions((prev) =>
        prev.map((x) =>
          x.id === s.id ? { ...x, status: show ? "shown" : "ignored" } : x,
        ),
      );
    } catch (e) {
      setError(String(e));
    }
  }

  const pending = suggestions.filter((s) => s.status === "pending");
  const savedDeviceMissing =
    !!config?.device &&
    devices.length > 0 &&
    !devices.some((d) => d.name === config.device);

  return (
    <div className="voice-page">
      {model && !model.exists && (
        <div className="warning" style={{ margin: "12px 18px" }}>
          ⚠ Whisper model not found at <code>{model.path}</code>. Place{" "}
          <code>ggml-base.en.bin</code> there to enable voice features.
        </div>
      )}

      <div className="voice-grid">
        {/* ---- Audio setup ---- */}
        <section className="panel">
          <h3>🎙 Audio Setup</h3>
          <p className="muted">
            How is your church audio connected? Speech-only means a dedicated
            mic or mixer aux channel; Mixed means the full church mix.
          </p>
          <label>
            Audio source
            <select
              value={config?.device ?? ""}
              onFocus={refreshDevices}
              onChange={(e) =>
                saveConfig({ device: e.currentTarget.value || null })
              }
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
              onChange={(e) =>
                saveConfig({ vadThreshold: Number(e.currentTarget.value) })
              }
            />
            <span className="muted">
              threshold {(config?.vadThreshold ?? 0).toFixed(3)} (lower = more
              sensitive)
            </span>
          </label>
          <label>
            Transcription model
            <select value={sttModel} onChange={(e) => changeSttModel(e.currentTarget.value)}>
              <option value="base">Base — most accurate (default)</option>
              <option value="tiny">Tiny — faster on slower PCs</option>
            </select>
            <span className="muted">
              Tiny transcribes ~4× faster but makes more mistakes.
            </span>
          </label>

          {buildTag && <div className="muted build-tag">{buildTag}</div>}

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
          {diagText && (
            <>
              <pre className="diag-report">{diagText}</pre>
            </>
          )}

          <h3>🧪 Audio Test</h3>
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
                  className={
                    "level-fill " +
                    (testResult.peakLevel > 0.005 ? "good" : "low")
                  }
                  style={{
                    width: `${Math.min(100, testResult.peakLevel * 2000)}%`,
                  }}
                />
              </div>
              <div>
                Signal: <b>{testResult.peakLevel > 0.005 ? "Good" : "Low"}</b>{" "}
                · Speech detected:{" "}
                <b>{testResult.speechDetected ? "YES" : "NO"}</b>
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
        </section>

        {/* ---- Live listening ---- */}
        <section className="panel">
          <h3>
            ● Live Listening{" "}
            <span className={listening ? "live-dot on" : "live-dot"} />
          </h3>
          <div className="mode-row">
            {[
              ["manual", "Manual"],
              ["assisted", "Assisted"],
              ["automatic", "Automatic"],
            ].map(([m, label]) => (
              <button
                key={m}
                className={mode === m ? "active" : ""}
                onClick={() => changeMode(m)}
              >
                {label}
              </button>
            ))}
          </div>
          <p className="muted">
            {mode === "manual" && "Suggestions are queued; you control everything."}
            {mode === "assisted" &&
              "Scripture detected is shown here for you to approve."}
            {mode === "automatic" &&
              "High-confidence matches would be pushed to displays automatically (display engine pending)."}
          </p>
          <button
            className={listening ? "danger" : "primary"}
            onClick={toggleListening}
            disabled={!!model && !model.exists}
          >
            {listening ? "■ Stop listening" : "▶ Start listening"}
          </button>

          <h4>Input level</h4>
          <div className="level-bar live">
            <div
              className={"level-fill " + (level > 0.004 ? "good" : "")}
              style={{ width: `${Math.min(100, level * 1500)}%` }}
            />
          </div>
          <p className="muted">
            Speak into the microphone — the bar should jump while you talk.
            If it barely moves, raise the sensitivity above.
          </p>

          <h4>Transcript</h4>
          <div className="transcript-feed" ref={feedRef}>
            {transcript.length === 0 ? (
              <div className="empty">Nothing heard yet.</div>
            ) : (
              transcript.map((t, i) => (
                <div key={i} className="transcript-line">
                  <span className="muted">{t.at}</span> {t.text}
                </div>
              ))
            )}
          </div>
        </section>

        {/* ---- Suggestions ---- */}
        <section className="panel">
          <h3>📖 Suggestions {pending.length > 0 && `(${pending.length} pending)`}</h3>
          {suggestions.length === 0 ? (
            <div className="empty">No suggestions yet.</div>
          ) : (
            suggestions.map((s) => (
              <div
                key={s.id}
                className={"suggestion " + s.status}
              >
                <div className="suggestion-head">
                  <b>{s.label}</b>
                  <span className="muted">
                    {" "}
                    {Math.round(s.confidence * 100)}% ·{" "}
                    {s.kind === "reference" ? "reference" : "quote"}
                  </span>
                </div>
                <div className="muted suggestion-src">
                  {ITEM_TYPE_LABELS[s.itemId.split("-")[0] as ItemType] ??
                    s.itemId}{" "}
                  · {s.sectionKey}
                </div>
                {s.preview && (
                  <div className="suggestion-preview">“{s.preview}”</div>
                )}
                {s.status === "pending" ? (
                  <div className="form-actions">
                    <button className="primary" onClick={() => respond(s, true)}>
                      SHOW
                    </button>
                    <button onClick={() => respond(s, false)}>IGNORE</button>
                  </div>
                ) : (
                  <div className="muted">({s.status})</div>
                )}
              </div>
            ))
          )}
        </section>
      </div>

      {error && (
        <div className="error" style={{ margin: "12px 18px" }}>
          {error}
        </div>
      )}
    </div>
  );
}
