import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { queueApi, serviceApi, type QueueEntry, type SessionItemRow, type SessionMeta } from "./api";
import * as lib from "../library/api";
import type { ContentSummary } from "../library/types";
import { voiceApi, onSuggestion, onAutoShown, type Suggestion } from "../voice/api";
import { displayApi, onDisplayUpdate, type SlotView } from "../display/api";

/**
 * LIVE SERVICE — the operator's control room.
 * Session timer, audio + AI status, display strip, emergency controls, and
 * the auto-recorded service log.
 */
export default function LivePage() {
  // session
  const [session, setSession] = useState<SessionMeta | null>(null);
  const [nameInput, setNameInput] = useState("");
  const [elapsed, setElapsed] = useState("00:00");
  const [history, setHistory] = useState<SessionMeta[]>([]);
  const [log, setLog] = useState<SessionItemRow[]>([]);
  const [showHistory, setShowHistory] = useState(false);

  // live status
  const [listening, setListening] = useState(false);
  const [mode, setMode] = useState("assisted");
  const [autoTarget, setAutoTarget] = useState("auto");
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [autoShown, setAutoShown] = useState<
    Record<string, { slot: number; until: number }>
  >({});
  const [slots, setSlots] = useState<SlotView[]>([]);
  const [error, setError] = useState<string | null>(null);

  // lower-third announcement
  const [notifyText, setNotifyText] = useState("");
  const [notifySlot, setNotifySlot] = useState(1);
  const [notifyDuration, setNotifyDuration] = useState(30_000);

  // service queue
  const [queue, setQueue] = useState<QueueEntry[]>([]);
  const [queueInput, setQueueInput] = useState("");
  const [queueSlot, setQueueSlot] = useState(1);
  const queueSlotInit = useRef(false);
  const [libraryItems, setLibraryItems] = useState<ContentSummary[]>([]);

  const logRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<SessionMeta | null>(null);
  sessionRef.current = session;

  const refreshSlots = useCallback(() => {
    displayApi.slots().then((vs) => {
      setSlots(vs);
      // First load: point the queue at whichever display is the hotkey target.
      const active = vs.find((v) => v.active);
      if (active && !queueSlotInit.current) {
        queueSlotInit.current = true;
        setQueueSlot(active.slot);
      }
    }).catch(() => {});
  }, []);

  const refreshHistory = useCallback(() => {
    serviceApi.list().then(setHistory).catch(() => {});
  }, []);

  const refreshLog = useCallback(() => {
    const s = sessionRef.current;
    if (s) serviceApi.items(s.id).then(setLog).catch(() => {});
  }, []);

  const refreshQueue = useCallback(() => {
    queueApi.list().then(setQueue).catch(() => {});
  }, []);

  async function act(fn: () => Promise<unknown>) {
    try {
      await fn();
      refreshQueue();
    } catch (e) {
      setError(String(e));
    }
  }

  function addReference() {
    const text = queueInput.trim();
    if (!text) return;
    setQueueInput("");
    act(async () => {
      await queueApi.addReference(text);
    });
  }

  async function showAnnouncement(text: string) {
    const t = text.trim();
    if (!t) return;
    try {
      await invoke("notify_display", {
        slot: notifySlot,
        text: t,
        durationMs: notifyDuration,
      });
    } catch (e) {
      setError(String(e));
    }
  }

  async function hideAnnouncement() {
    try {
      await invoke("hide_notification", { slot: notifySlot });
    } catch (e) {
      setError(String(e));
    }
  }

  async function addContentItem(id: string) {
    if (!id) return;
    act(async () => {
      const full = await lib.getContent(id);
      const sections = full.body.sections ?? [];
      const first = sections[0];
      const label = (first?.label || "Section 1").trim();
      const slug = label
        .toLowerCase()
        .split(/[^a-z0-9]+/)
        .filter(Boolean)
        .join("-");
      const key = slug ? `${slug}-1` : "section-1";
      await queueApi.addItem({
        itemId: id,
        key,
        label: label,
        title: full.title,
        kind: full.itemType === "slide" ? "slide" : "lyrics",
      });
    });
  }

  // initial load + event subscriptions
  useEffect(() => {
    serviceApi.current().then(setSession).catch(() => {});
    refreshHistory();
    refreshSlots();
    voiceApi.listeningStatus().then(setListening).catch(() => {});
    voiceApi.getMode().then(setMode).catch(() => {});
    voiceApi.autoTarget().then(setAutoTarget).catch(() => {});
    voiceApi.suggestions().then(setSuggestions).catch(() => {});
    refreshQueue();
    Promise.all([
      lib.listContent({ itemType: "slide" }),
      lib.listContent({ itemType: "hymn" }),
      lib.listContent({ itemType: "song" }),
    ]).then(([slides, hymns, songs]) => setLibraryItems([...slides, ...hymns, ...songs])).catch(() => {});

    let uns: Promise<UnlistenFnLike>[] = [];
    uns.push(
      onSuggestion((e) =>
        setSuggestions((prev) => [e.suggestion, ...prev.filter((p) => p.id !== e.suggestion.id)].slice(0, 10)),
      ),
    );
    uns.push(
      onAutoShown((e) => {
        setAutoShown((prev) => ({ ...prev, [e.id]: { slot: e.slot, until: Date.now() + e.undoMs } }));
        setTimeout(() => {
          setAutoShown((prev) => {
            const next = { ...prev };
            delete next[e.id];
            return next;
          });
        }, e.undoMs + 250);
      }),
    );
    uns.push(onDisplayUpdate(() => refreshSlots()));
    Promise.all(uns).then((fns) => {
      unref.current = fns;
    });
    const unref = { current: [] as (() => void)[] };
    return () => {
      unref.current.forEach((f) => f());
    };
  }, [refreshSlots, refreshHistory]);

  // log refresh when session changes
  useEffect(() => {
    refreshLog();
  }, [session, refreshLog]);

  // timer
  useEffect(() => {
    if (!session) {
      setElapsed("00:00");
      return;
    }
    const tick = () => {
      const start = new Date(session.startedAt || Date.now()).getTime();
      const secs = Math.max(0, Math.floor((Date.now() - start) / 1000));
      const h = Math.floor(secs / 3600);
      const m = Math.floor((secs % 3600) / 60);
      const s = secs % 60;
      setElapsed(
        h > 0
          ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
          : `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`,
      );
    };
    tick();
    const t = window.setInterval(tick, 1000);
    return () => window.clearInterval(t);
  }, [session]);

  async function startSession() {
    setError(null);
    try {
      const meta = await serviceApi.start(nameInput);
      setSession(meta);
      setLog([]);
      setNameInput("");
      refreshHistory();
    } catch (e) {
      setError(String(e));
    }
  }

  async function endSession() {
    setError(null);
    try {
      await serviceApi.end();
      setSession(null);
      setLog([]);
      refreshHistory();
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggleListening() {
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

  async function changeAutoTarget(t: string) {
    setAutoTarget(t);
    try {
      await voiceApi.setAutoTarget(t);
    } catch (e) {
      setError(String(e));
    }
  }

  async function blankAll(blank: boolean) {
    try {
      await serviceApi.blankAll(blank);
      refreshSlots();
    } catch (e) {
      setError(String(e));
    }
  }

  async function respond(s: Suggestion, show: boolean) {
    try {
      if (show) {
        const isScripture = s.itemId.startsWith("bible-");
        if (isScripture) {
          await displayApi.setScripture(1, s.itemId, [s.sectionKey]);
        } else {
          await displayApi.setSection(1, s.itemId, s.sectionKey);
        }
      }
      await voiceApi.respond(s.id, show);
      setSuggestions((prev) =>
        prev.map((x) => (x.id === s.id ? { ...x, status: show ? "shown" : "ignored" } : x)),
      );
      refreshSlots();
      refreshLog();
    } catch (e) {
      setError(String(e));
    }
  }

  const pending = suggestions.filter((s) => s.status === "pending");

  return (
    <div className="live-page">
      {/* ---- Session bar ---- */}
      <div className="session-bar">
        {session ? (
          <>
            <span className="session-live-dot" />
            <b>{session.name}</b>
            <span className="session-timer">{elapsed}</span>
            <button className="danger" onClick={endSession}>
              ■ End service
            </button>
          </>
        ) : (
          <>
            <input
              className="session-name"
              placeholder="Service name (e.g. Sunday Morning)"
              value={nameInput}
              onChange={(e) => setNameInput(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Enter" && startSession()}
            />
            <button className="primary" onClick={startSession}>
              ▶ Start service
            </button>
          </>
        )}
        <button className="history-toggle" onClick={() => setShowHistory(!showHistory)}>
          📜 History {history.length > 0 && `(${history.length})`}
        </button>
      </div>

      {showHistory && (
        <div className="history-panel">
          {history.length === 0 && <span className="muted">No past services yet.</span>}
          {history.map((h) => (
            <div key={h.id} className="history-row">
              <button
                className="history-name"
                onClick={async () => {
                  setLog(await serviceApi.items(h.id));
                  setShowHistory(false);
                  setSession(null);
                }}
                title="Open this service's log"
              >
                {h.name}
              </button>
              <span className="muted">
                {h.startedAt.replace("T", " ").replace("Z", "")}
                {h.endedAt ? ` → ${h.endedAt.replace("T", " ").replace("Z", "")}` : " (running)"}
              </span>
              <button
                className="danger"
                title="Delete this service record"
                onClick={async () => {
                  await serviceApi.remove(h.id);
                  refreshHistory();
                }}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}

      {/* ---- Main grid ---- */}
      <div className="live-grid">
        {/* Audio + AI */}
        <section className="panel">
          <h3>🎙 Audio & AI</h3>
          <div className="live-row">
            <span>{listening ? "● Listening" : "○ Not listening"}</span>
            <button className={listening ? "danger" : "primary"} onClick={toggleListening}>
              {listening ? "Stop" : "Listen"}
            </button>
          </div>
          <div className="mode-row">
            {[
              ["manual", "Manual"],
              ["assisted", "Assisted"],
              ["automatic", "Automatic"],
            ].map(([m, label]) => (
              <button key={m} className={mode === m ? "active" : ""} onClick={() => changeMode(m)}>
                {label}
              </button>
            ))}
            <select
              className="auto-target-select"
              value={autoTarget}
              onChange={(e) => changeAutoTarget(e.currentTarget.value)}
              title="Which display Automatic mode projects on"
            >
              <option value="auto">→ first AUTO</option>
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={String(n)}>
                  → Display {n}
                </option>
              ))}
            </select>
          </div>
          <h4>AI Suggestions {pending.length > 0 && `(${pending.length})`}</h4>
          {suggestions.length === 0 ? (
            <div className="empty">No suggestions yet.</div>
          ) : (
            suggestions.slice(0, 5).map((s) => (
              <div key={s.id} className={"suggestion " + s.status}>
                <div className="suggestion-head">
                  <b>{s.label}</b>{" "}
                  <span className="muted">{Math.round(s.confidence * 100)}%</span>
                </div>
                {s.preview && <div className="suggestion-preview">“{s.preview}”</div>}
                {autoShown[s.id] && autoShown[s.id].until > Date.now() && (
                  <div className="auto-shown">
                    ⚡ auto → Display {autoShown[s.id].slot}
                    <button
                      className="danger undo-btn"
                      onClick={() =>
                        voiceApi.undoAutoShow(s.id).catch((e) => setError(String(e)))
                      }
                    >
                      UNDO
                    </button>
                  </div>
                )}
                {s.status === "pending" && (
                  <div className="form-actions">
                    <button className="primary" onClick={() => respond(s, true)}>
                      SHOW
                    </button>
                    <button onClick={() => respond(s, false)}>IGNORE</button>
                  </div>
                )}
              </div>
            ))
          )}
        </section>

        {/* Displays strip */}
        <section className="panel">
          <h3>🖼 Displays</h3>
          <div className="display-strip">
            {slots.map((s) => (
              <button
                key={s.slot}
                className={"strip-slot" + (s.windowOpen ? " on" : "") + (s.active ? " hot" : "")}
                onClick={() => displayApi.setActive(s.slot).then(refreshSlots)}
                title={`Display ${s.slot}${s.windowOpen ? " · live" : ""}`}
              >
                <b>{s.slot}</b>
                <span className="strip-kind">
                  {s.blank || s.content.kind === "blank"
                    ? "blank"
                    : s.content.kind === "scripture"
                      ? s.content.label
                      : s.content.kind === "lyrics"
                        ? s.content.label
                        : s.content.kind}
                </span>
                {s.content.page && <span className="strip-page">{s.content.page}</span>}
              </button>
            ))}
          </div>
          <div className="form-actions">
            <button className="danger" onClick={() => blankAll(true)}>
              ⬛ EMERGENCY BLANK ALL
            </button>
            <button onClick={() => blankAll(false)}>Restore all</button>
          </div>
          <div className="hotkey-mini muted">
            Ctrl+Alt+1–5 select · Ctrl+Alt+←/→ step · Ctrl+Alt+B blank
          </div>
        </section>

        {/* Announcements */}
        <section className="panel">
          <h3>📣 Announcement</h3>
          <p className="muted">
            A lower-third banner over the chosen display — the content behind
            it is untouched and it disappears by itself.
          </p>
          <div className="notify-row">
            <input
              value={notifyText}
              onChange={(e) => setNotifyText(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Enter" && showAnnouncement(notifyText)}
              placeholder="e.g., Lunch is served in the hall after service"
            />
            <select
              value={notifySlot}
              onChange={(e) => setNotifySlot(Number(e.currentTarget.value))}
              title="Which display shows the banner"
            >
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  D{n}
                </option>
              ))}
            </select>
            <select
              value={notifyDuration}
              onChange={(e) => setNotifyDuration(Number(e.currentTarget.value))}
              title="How long the banner stays"
            >
              <option value={10_000}>10 s</option>
              <option value={30_000}>30 s</option>
              <option value={60_000}>1 min</option>
              <option value={300_000}>5 min</option>
              <option value={0}>Until hidden</option>
            </select>
          </div>
          <div className="form-actions">
            <button className="primary" onClick={() => showAnnouncement(notifyText)}>
              Show banner
            </button>
            <button onClick={hideAnnouncement}>Hide</button>
          </div>
        </section>

        {/* Service queue */}
        <section className="panel">
          <h3>📋 Queue {queue.length > 0 && `(${queue.length})`}</h3>
          <div className="queue-add">
            <input
              value={queueInput}
              onChange={(e) => setQueueInput(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Enter" && addReference()}
              placeholder="Add reference — John 3:16-18"
            />
            <button onClick={addReference}>＋</button>
            <select
              value=""
              onChange={(e) => addContentItem(e.currentTarget.value)}
              title="Add a slide, hymn or song set (starts at its first section)"
            >
              <option value="">＋ Slide / song…</option>
              {libraryItems.map((i) => (
                <option key={i.id} value={i.id}>
                  {i.title}
                </option>
              ))}
            </select>
            <select
              value={queueSlot}
              onChange={(e) => setQueueSlot(Number(e.currentTarget.value))}
              title="Display the queue projects onto"
            >
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  → D{n}
                </option>
              ))}
            </select>
          </div>
          {queue.length === 0 ? (
            <div className="empty">Plan the service here, then step through it.</div>
          ) : (
            queue.map((q, i) => (
              <div key={q.id} className="queue-row">
                <span className="queue-pos muted">{i + 1}</span>
                <span className="queue-main">
                  <b>{q.label}</b> <span className="muted">{q.label !== q.title ? q.title : ""}</span>
                </span>
                <button title="Move up" onClick={() => act(() => queueApi.move(q.id, -1))}>▲</button>
                <button title="Move down" onClick={() => act(() => queueApi.move(q.id, 1))}>▼</button>
                <button
                  className="primary"
                  title={`Show on Display ${queueSlot}`}
                  onClick={() => act(() => queueApi.show(q.id, queueSlot))}
                >
                  Show
                </button>
                <button title="Remove" onClick={() => act(() => queueApi.remove(q.id))}>✕</button>
              </div>
            ))
          )}
          {queue.length > 0 && (
            <div className="form-actions">
              <button onClick={() => act(() => queueApi.clear())}>Clear queue</button>
            </div>
          )}
        </section>

        {/* Service log */}
        <section className="panel">
          <h3>📜 Service log</h3>
          <div className="log-feed" ref={logRef}>
            {log.length === 0 ? (
              <div className="empty">
                {session
                  ? "Everything shown during this service is recorded here."
                  : "Start a service, or open one from History."}
              </div>
            ) : (
              log.map((it, i) => (
                <div key={i} className="log-line">
                  <span className="muted">
                    {it.at.replace("T", " ").replace("Z", "")} · D{it.slot}
                  </span>{" "}
                  <b>{it.label || it.title}</b>{" "}
                  <span className="muted">{it.label ? it.title : ""}</span>
                </div>
              ))
            )}
          </div>
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

type UnlistenFnLike = () => void;
