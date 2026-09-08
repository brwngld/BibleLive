import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  displayApi,
  onDisplayUpdate,
  FONT_OPTIONS,
  type DisplayProfile,
  type MonitorInfo,
  type SlotStyle,
  type SlotView,
} from "./api";
import * as lib from "../library/api";
import type { ContentSummary, SearchHit } from "../library/types";

const HOTKEYS: [string, string][] = [
  ["Ctrl+Alt+1…5", "Select the active display"],
  ["Ctrl+Alt+→ / ←", "Next / previous verse or stanza on the active display"],
  ["Ctrl+Alt+B", "Blank / unblank the active display"],
  ["← / → (in output)", "Step that display's verses"],
  ["Tab / Shift+Tab (in output)", "Cycle between open fullscreen outputs"],
  ["M (in output)", "Move this output to the next screen"],
  ["Esc (in output)", "Close a fullscreen output"],
];

export default function DisplaysPage() {
  const [slots, setSlots] = useState<SlotView[]>([]);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [profiles, setProfiles] = useState<DisplayProfile[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [newProfileName, setNewProfileName] = useState("");
  const [picker, setPicker] = useState<{ slot: number; kind: "scripture" | "lyrics" } | null>(null);

  const refresh = useCallback(() => {
    displayApi.slots().then(setSlots).catch(console.error);
    displayApi.monitors().then(setMonitors).catch(console.error);
    displayApi.profiles().then(setProfiles).catch(console.error);
  }, []);

  useEffect(() => {
    refresh();
    let un: (() => void) | undefined;
    onDisplayUpdate(() => refresh()).then((u) => (un = u));
    return () => un?.();
  }, [refresh]);

  async function act(fn: () => Promise<unknown>) {
    setError(null);
    try {
      await fn();
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="displays-page">
      <div className="displays-toolbar">
        <div className="profiles-row">
          <b>Profiles:</b>
          {profiles.length === 0 && <span className="muted"> none saved</span>}
          {profiles.map((p) => (
            <span key={p.name} className="profile-chip">
              <button onClick={() => act(() => displayApi.applyProfile(p.name))}>
                {p.name}
              </button>
              <button
                className="chip-x"
                title="Delete profile"
                onClick={() => act(() => displayApi.deleteProfile(p.name))}
              >
                ×
              </button>
            </span>
          ))}
          <input
            className="profile-name"
            placeholder="New profile name…"
            value={newProfileName}
            onChange={(e) => setNewProfileName(e.currentTarget.value)}
          />
          <button
            disabled={!newProfileName.trim()}
            onClick={() =>
              act(async () => {
                await displayApi.saveProfile(newProfileName.trim());
                setNewProfileName("");
              })
            }
          >
            Save current layout
          </button>
        </div>
        <div className="hotkey-legend">
          <b>⌨ Hotkeys (work system-wide):</b>
          {HOTKEYS.map(([k, d]) => (
            <span key={k} className="hotkey-item">
              <code>{k}</code> {d}
            </span>
          ))}
        </div>
      </div>

      <div className="slot-grid">
        {slots.map((s) => (
          <SlotCard
            key={s.slot}
            view={s}
            monitors={monitors}
            onChanged={refresh}
            onError={setError}
            openPicker={(kind) => setPicker({ slot: s.slot, kind })}
          />
        ))}
      </div>

      {picker && (
        <ContentPickerDialog
          kind={picker.kind}
          slot={picker.slot}
          onClose={() => setPicker(null)}
          onPicked={() => {
            setPicker(null);
            refresh();
          }}
        />
      )}

      {error && <div className="error" style={{ margin: "12px 18px" }}>{error}</div>}
    </div>
  );
}

function SlotCard({
  view,
  monitors,
  onChanged,
  onError,
  openPicker,
}: {
  view: SlotView;
  monitors: MonitorInfo[];
  onChanged: () => void;
  onError: (e: string) => void;
  openPicker: (kind: "scripture" | "lyrics") => void;
}) {
  const c = view.content;
  const [style, setStyle] = useState<SlotStyle>(c.style);
  const [showStyle, setShowStyle] = useState(false);

  useEffect(() => setStyle(c.style), [c.style]);

  async function act(fn: () => Promise<unknown>) {
    try {
      await fn();
      onChanged();
    } catch (e) {
      onError(String(e));
    }
  }

  function updateStyle(patch: Partial<SlotStyle>) {
    const next = { ...style, ...patch };
    setStyle(next);
    act(() => displayApi.setStyle(view.slot, next));
  }

  // Mini preview scale: font_size vw on a ~1920px-wide output → px in a
  // preview box of PREVIEW_W px.
  const PREVIEW_W = 300;
  const px = (vw: number) => `${Math.max(6, (vw / 100) * PREVIEW_W)}px`;

  return (
    <div
      className={
        "slot-card" +
        (view.windowOpen ? " open" : "") +
        (view.degraded ? " degraded" : "") +
        (view.active ? " active-slot" : "")
      }
    >
      <div className="slot-head">
        <b>
          Display {view.slot} {view.active && <span className="active-star">★ active</span>}
        </b>
        <span className="slot-status">
          {view.windowOpen ? "● live" : "○ closed"}
          {view.degraded ? " · ⚠ monitor missing" : ""}
        </span>
      </div>

      <button
        className="set-active-btn"
        onClick={() => act(() => displayApi.setActive(view.slot))}
        title="Hotkeys (Ctrl+Alt+←/→/B) act on the active display"
      >
        {view.active ? "★ Hotkey target" : "Set as hotkey target"}
      </button>

      <label className="slot-row">
        Target monitor
        <select
          value={view.monitor ?? ""}
          onChange={(e) => act(() => displayApi.setMonitor(view.slot, e.currentTarget.value || null))}
        >
          <option value="">Primary</option>
          {monitors.map((m) => (
            <option key={m.name} value={m.name}>
              {m.name} {m.isPrimary ? "(primary)" : ""} · {m.width}×{m.height}
            </option>
          ))}
        </select>
      </label>

      <div className="slot-row mode-row">
        {(["auto", "manual", "lock"] as const).map((m) => (
          <button
            key={m}
            className={view.mode === m ? "active" : ""}
            onClick={() => act(() => displayApi.setMode(view.slot, m))}
            title={
              m === "auto"
                ? "AI/content suggestions may target this slot"
                : m === "manual"
                  ? "Only the operator changes this slot"
                  : "Content stays put regardless of AI detection"
            }
          >
            {m.toUpperCase()}
          </button>
        ))}
      </div>

      {/* Live preview — exactly what the output shows, scaled down */}
      <div
        className="slot-preview"
        style={{ backgroundColor: c.kind === "blank" || view.blank ? "#000" : style.bgColor }}
      >
        {c.kind === "blank" || view.blank ? (
          <span className="muted preview-empty">blank</span>
        ) : c.kind === "image" && c.imagePath ? (
          <span className="muted">🖼 {c.title}</span>
        ) : c.kind === "video" && c.videoPath ? (
          <span className="muted">🎬 {c.title}</span>
        ) : (
          <div
            className="preview-inner"
            style={{
              fontFamily: style.fontFamily,
              color: style.textColor,
              fontSize: px(c.kind === "lyrics" ? style.fontSize * 0.72 : style.fontSize),
            }}
          >
            <div>{c.lines[0] ?? ""}</div>
            {c.lines.length > 1 && (
              <div className="preview-more">…</div>
            )}
            <div className="preview-label" style={{ fontSize: px(style.fontSize * 0.5) }}>
              {c.label}
            </div>
          </div>
        )}
        {c.page && c.kind !== "blank" && !view.blank && (
          <span className="preview-page">{c.page}</span>
        )}
      </div>

      <div className="slot-controls">
        <button onClick={() => act(() => displayApi.step(view.slot, -1))}>◀ Prev</button>
        <button onClick={() => act(() => displayApi.step(view.slot, 1))}>Next ▶</button>
        <button onClick={() => act(() => displayApi.setBlank(view.slot, !view.blank))}>
          {view.blank ? "Unblank" : "Blank"}
        </button>
      </div>

      <div className="slot-controls">
        <button onClick={() => openPicker("scripture")}>📖 Scripture…</button>
        <button onClick={() => openPicker("lyrics")}>🎵 Lyrics…</button>
        <button
          onClick={() =>
            act(async () => {
              const path = await open({
                multiple: false,
                filters: [{ name: "Media", extensions: ["png", "jpg", "jpeg", "gif", "webp", "mp4", "webm", "mov"] }],
              });
              if (typeof path === "string") {
                const isVideo = /\.(mp4|webm|mov)$/i.test(path);
                await displayApi.setMedia(view.slot, {
                  title: path.split(/[\\/]/).pop(),
                  imagePath: isVideo ? undefined : path,
                  videoPath: isVideo ? path : undefined,
                });
              }
            })
          }
        >
          🖼 Media…
        </button>
        <button onClick={() => act(() => displayApi.setBlank(view.slot, true))}>⬛ Black</button>
      </div>

      <div className="slot-controls">
        <button onClick={() => setShowStyle(!showStyle)}>
          🔠 Style {showStyle ? "▲" : "▼"}
        </button>
        {view.windowOpen ? (
          <>
            <button
              title="Move this output to the next connected screen"
              onClick={() => act(() => displayApi.moveToNextMonitor(view.slot))}
            >
              ⇄ Move screen
            </button>
            <button onClick={() => act(() => displayApi.closeOutput(view.slot))}>Close output</button>
          </>
        ) : (
          <button className="primary" onClick={() => act(() => displayApi.openOutput(view.slot))}>
            ▶ Open output
          </button>
        )}
      </div>

      {showStyle && (
        <div className="style-panel">
          <label>
            Font
            <select
              value={style.fontFamily}
              onChange={(e) => updateStyle({ fontFamily: e.currentTarget.value })}
            >
              {FONT_OPTIONS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Text size ({style.fontSize.toFixed(1)}vw)
            <input
              type="range"
              min="3"
              max="12"
              step="0.5"
              value={style.fontSize}
              onChange={(e) => updateStyle({ fontSize: Number(e.currentTarget.value) })}
            />
          </label>
          <div className="color-row">
            <label>
              Text
              <input
                type="color"
                value={style.textColor}
                onChange={(e) => updateStyle({ textColor: e.currentTarget.value })}
              />
            </label>
            <label>
              Background
              <input
                type="color"
                value={style.bgColor}
                onChange={(e) => updateStyle({ bgColor: e.currentTarget.value })}
              />
            </label>
          </div>
        </div>
      )}
    </div>
  );
}

/** Search the library and put a verse/stanza on a slot. */
function ContentPickerDialog({
  kind,
  slot,
  onClose,
  onPicked,
}: {
  kind: "scripture" | "lyrics";
  slot: number;
  onClose: () => void;
  onPicked: () => void;
}) {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ContentSummary[]>([]);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [selected, setSelected] = useState<ContentSummary | null>(null);
  const [detail, setDetail] = useState<{ key: string; label: string }[]>([]);

  useEffect(() => {
    if (kind === "lyrics") {
      lib.listContent({ itemType: "hymn" }).then((r) => {
        setItems(r);
        lib.listContent({ itemType: "song" }).then((s) => setItems((prev) => [...prev, ...s]));
      });
    }
  }, [kind]);

  async function runSearch() {
    if (kind === "scripture") {
      // Direct reference or quote — search handles both.
      const results = await lib.searchContent(query);
      setHits(results.filter((h) => h.itemType === "bible"));
    } else {
      const results = await lib.searchContent(query);
      setHits(results.filter((h) => h.itemType === "hymn" || h.itemType === "song"));
      setItems(items.filter((i) => i.title.toLowerCase().includes(query.toLowerCase())));
    }
  }

  async function selectItem(item: ContentSummary) {
    setSelected(item);
    const full = await lib.getContent(item.id);
    const sections =
      full.itemType === "bible"
        ? null // scripture picker selects verses from search hits
        : (full.body.sections ?? []).map((s, i) => {
            const label = s.label || `Section ${i + 1}`;
            return { key: `${slugify(label)}-${i + 1}`, label };
          });
    setDetail(sections ?? []);
  }

  async function choose(hit?: SearchHit, sectionKey?: string) {
    try {
      if (kind === "scripture" && hit) {
        await displayApi.setScripture(slot, hit.itemId, [hit.sectionKey]);
      } else if (selected && sectionKey) {
        await displayApi.setSection(slot, selected.id, sectionKey);
      }
      onPicked();
    } catch (e) {
      alert(String(e));
    }
  }

  function slugify(s: string) {
    return s
      .toLowerCase()
      .split(/[^a-z0-9]+/)
      .filter(Boolean)
      .join("-");
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>
          {kind === "scripture" ? "📖 Scripture" : "🎵 Lyrics"} → Display {slot}
        </h2>
        <div className="search-box">
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            onKeyDown={(e) => e.key === "Enter" && runSearch()}
            placeholder={
              kind === "scripture"
                ? "Reference or words… e.g. John 3:16 / for God so loved"
                : "Search hymns & songs…"
            }
          />
          <button onClick={runSearch}>Search</button>
        </div>

        <div className="picker-list">
          {kind === "scripture" &&
            hits.map((h) => (
              <button key={h.itemId + h.sectionKey} className="hit-row" onClick={() => choose(h)}>
                <b>{h.sectionLabel}</b> <span className="muted">{h.itemTitle}</span>
                <div className="hit-snippet" dangerouslySetInnerHTML={{ __html: h.snippet }} />
              </button>
            ))}
          {kind === "lyrics" &&
            !selected &&
            items.map((i) => (
              <button key={i.id} className="hit-row" onClick={() => selectItem(i)}>
                <b>{i.title}</b> <span className="muted">{i.itemType}</span>
              </button>
            ))}
          {kind === "lyrics" &&
            selected &&
            detail.map((d) => (
              <button key={d.key} className="hit-row" onClick={() => choose(undefined, d.key)}>
                <b>{d.label}</b>
              </button>
            ))}
          {kind === "lyrics" && selected && (
            <button className="muted" onClick={() => setSelected(null)}>
              ← back to song list
            </button>
          )}
        </div>

        <div className="form-actions">
          <button onClick={onClose}>Cancel</button>
        </div>
      </div>
    </div>
  );
}
