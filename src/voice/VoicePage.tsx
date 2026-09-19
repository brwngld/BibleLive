import { useCallback, useEffect, useRef, useState } from "react";
import {
  voiceApi,
  onTranscript,
  onPartialTranscript,
  onSuggestion,
  onAutoShown,
  onLevel,
  type UnlistenFn,
  type Suggestion,
  type TranscriptEvent,
  type SuggestionEvent,
} from "./api";
import { displayApi } from "../display/api";
import { ITEM_TYPE_LABELS, type ItemType } from "../library/types";

interface TranscriptLine {
  text: string;
  at: string;
}

export default function VoicePage() {
  const [mode, setMode] = useState("assisted");
  const [listening, setListening] = useState(false);
  const [model, setModel] = useState<{ exists: boolean; path: string; sizeMb: number | null } | null>(null);
  const [transcript, setTranscript] = useState<TranscriptLine[]>([]);
  const [partial, setPartial] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [level, setLevel] = useState(0);
  const [error, setError] = useState<string | null>(null);
  // Automatic mode: id → { slot, until } while an Undo window is open.
  const [autoShown, setAutoShown] = useState<
    Record<string, { slot: number; until: number }>
  >({});

  const feedRef = useRef<HTMLDivElement>(null);

  const refreshSuggestions = useCallback(() => {
    voiceApi.suggestions().then(setSuggestions).catch(() => {});
  }, []);

  useEffect(() => {
    voiceApi.getMode().then(setMode).catch(console.error);
    voiceApi.modelStatus().then(setModel).catch(console.error);
    voiceApi.listeningStatus().then(setListening).catch(console.error);
    refreshSuggestions();

    let unlisteners: Promise<UnlistenFn>[] = [];
    unlisteners.push(
      onTranscript((e: TranscriptEvent) => {
        setPartial(null); // the final line replaces the live one
        setTranscript((prev) =>
          [...prev, { text: e.text, at: new Date().toLocaleTimeString() }].slice(-100),
        );
        requestAnimationFrame(() => {
          feedRef.current?.scrollTo({ top: feedRef.current.scrollHeight });
        });
      }),
    );
    unlisteners.push(
      onPartialTranscript((e: TranscriptEvent) => {
        setPartial(e.text);
        requestAnimationFrame(() => {
          feedRef.current?.scrollTo({ top: feedRef.current.scrollHeight });
        });
      }),
    );
    unlisteners.push(
      onSuggestion((e: SuggestionEvent) => {
        // Live (partial) and verified suggestions for the same verse replace
        // the pending card instead of stacking duplicates.
        setSuggestions((prev) => {
          const rest = prev.filter(
            (x) =>
              x.status !== "pending" ||
              x.itemId !== e.suggestion.itemId ||
              x.sectionKey !== e.suggestion.sectionKey,
          );
          return [e.suggestion, ...rest].slice(0, 40);
        });
      }),
    );
    unlisteners.push(onLevel((lv) => setLevel(lv)));
    unlisteners.push(
      onAutoShown((e) => {
        const until = Date.now() + e.undoMs;
        setAutoShown((prev) => ({ ...prev, [e.id]: { slot: e.slot, until } }));
        setTimeout(() => {
          setAutoShown((prev) => {
            const next = { ...prev };
            delete next[e.id];
            return next;
          });
        }, e.undoMs + 250);
      }),
    );
    let fns: UnlistenFn[] = [];
    Promise.all(unlisteners).then((f) => {
      fns = f;
    });
    return () => {
      fns.forEach((f) => f());
    };
  }, [refreshSuggestions]);

  async function toggleListening() {
    setError(null);
    try {
      if (listening) {
        await voiceApi.stop();
        setListening(false);
        setLevel(0);
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

  async function undoAutoShow(id: string) {
    try {
      await voiceApi.undoAutoShow(id);
    } catch (e) {
      setError(String(e));
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

  return (
    <div className="voice-page">
      {model && !model.exists && (
        <div className="warning" style={{ margin: "12px 18px" }}>
          ⚠ Whisper model not found at <code>{model.path}</code>. Place{" "}
          <code>ggml-base.en.bin</code> there to enable voice features.
        </div>
      )}

      <div className="voice-grid voice-console">
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
              "High-confidence verified matches project automatically to the target display chosen in ⚙ Settings (default: first slot set to AUTO — changeable on the Live page too). UNDO is offered for 10 seconds."}
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
            If it barely moves, raise the sensitivity in ⚙ Settings.
          </p>

          <h4>Transcript</h4>
          <div className="transcript-feed" ref={feedRef}>
            {transcript.length === 0 && !partial && (
              <div className="empty">Nothing heard yet.</div>
            )}
            {transcript.map((t, i) => (
              <div key={i} className="transcript-line">
                <span className="muted">{t.at}</span> {t.text}
              </div>
            ))}
            {partial && (
              <div className="transcript-line partial">
                <span className="muted">live</span> {partial}…
              </div>
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
                {autoShown[s.id] && autoShown[s.id].until > Date.now() && (
                  <div className="auto-shown">
                    ⚡ shown automatically → Display {autoShown[s.id].slot}
                    <button
                      className="danger undo-btn"
                      onClick={() => undoAutoShow(s.id)}
                    >
                      UNDO
                    </button>
                  </div>
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
