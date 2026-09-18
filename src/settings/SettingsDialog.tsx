import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { voiceApi } from "../voice/api";

/** Quick settings gathered from the scattered pages, plus About. The full
 *  panels on the Voice and Displays pages stay; both read and write the
 *  same backend settings, so they can never disagree. */
export default function SettingsDialog({ onClose }: { onClose: () => void }) {
  const [sttModel, setSttModel] = useState("base");
  const [autoTarget, setAutoTarget] = useState("auto");
  const [animation, setAnimation] = useState("none");
  const [about, setAbout] = useState<{ version: string; build: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    voiceApi.sttModel().then(setSttModel).catch(() => {});
    voiceApi.autoTarget().then(setAutoTarget).catch(() => {});
    setAnimation(localStorage.getItem("bible-animation") ?? "none");
    invoke<{ version: string; build: string }>("app_status")
      .then((st) => setAbout({ version: st.version, build: st.build }))
      .catch(() => {});
  }, []);

  async function saveModel(m: string) {
    const prev = sttModel;
    setSttModel(m);
    try {
      await voiceApi.setSttModel(m);
    } catch (e) {
      setSttModel(prev);
      setError(String(e));
    }
  }

  async function saveTarget(t: string) {
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

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal settings-modal">
        <h3>Settings</h3>

        <label>
          Transcription model
          <select value={sttModel} onChange={(e) => saveModel(e.currentTarget.value)}>
            <option value="base">Base — most accurate (default)</option>
            <option value="tiny">Tiny — faster on slower PCs</option>
          </select>
        </label>

        <label>
          Automatic mode target display
          <select value={autoTarget} onChange={(e) => saveTarget(e.currentTarget.value)}>
            <option value="auto">First AUTO display</option>
            {[1, 2, 3, 4, 5].map((n) => (
              <option key={n} value={String(n)}>
                Always Display {n}
              </option>
            ))}
          </select>
        </label>

        <label>
          Bible reader animation
          <select value={animation} onChange={(e) => saveAnimation(e.currentTarget.value)}>
            <option value="none">None</option>
            <option value="fade">Fade</option>
            <option value="slide">Slide</option>
          </select>
          <span className="muted">Applies to the Library Bible reader.</span>
        </label>

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
            Microphone, sensitivity and live-matching options live on the Voice
            page; display themes and profiles on the Displays page.
          </span>
        </div>

        {error && <div className="error">{error}</div>}

        <div className="form-actions">
          <button className="primary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
