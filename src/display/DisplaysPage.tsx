import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  displayApi,
  onDisplayUpdate,
  FONT_OPTIONS,
  THEME_PRESETS,
  type DisplayProfile,
  type MonitorInfo,
  type SlotStyle,
  type SlotView,
} from "./api";
import * as lib from "../library/api";
import { HOTKEYS } from "../hotkeys";
import type { ContentItem, ContentSummary, SearchHit } from "../library/types";

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

      <label
        className="slot-row"
        title="Show the same scripture in a second Bible version, side by side"
      >
        ⧉ Second version
        <select
          value={view.pairVersion ?? ""}
          onChange={(e) => act(() => displayApi.setPair(view.slot, e.currentTarget.value || null))}
        >
          <option value="">Off</option>
          <option value="kjv">KJV</option>
          <option value="asv">ASV</option>
        </select>
      </label>

      <div className="mode-row">
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
        <button
          className={view.windowOpen ? "" : "primary"}
          onClick={() =>
            act(() =>
              view.windowOpen
                ? displayApi.closeOutput(view.slot)
                : displayApi.openOutput(view.slot),
            )
          }
        >
          {view.windowOpen ? "◼ Close output" : "▶ Open output"}
        </button>
        {view.windowOpen && (
          <button
            title="Move this output to the next connected screen"
            onClick={() => act(() => displayApi.moveToNextMonitor(view.slot))}
          >
            ⇄
          </button>
        )}
      </div>

      {/* Live preview — exactly what the output shows, scaled down */}
      <div
        className="slot-preview"
        style={{
          backgroundColor: c.kind === "blank" || view.blank ? "#000" : style.bgColor,
          backgroundImage:
            c.kind !== "blank" && !view.blank && c.kind !== "image" && style.bgImage
              ? `url("${convertFileSrc(style.bgImage)}")`
              : undefined,
          backgroundSize: "cover",
          backgroundPosition: "center",
        }}
      >
        {c.kind === "blank" || view.blank ? (
          <span className="muted preview-empty">blank</span>
        ) : c.kind === "image" && c.imagePath ? (
          <span className="muted">🖼 {c.title}</span>
        ) : c.kind === "video" && c.videoPath ? (
          <span className="muted">🎬 {c.title}</span>
        ) : c.pair ? (
          <div
            className="preview-inner preview-pair"
            style={{
              fontFamily: style.fontFamily,
              color: style.textColor,
              fontSize: px(style.fontSize * 0.8),
              textAlign: "left",
              textShadow: (style.textShadow ?? true) ? "0 1px 4px rgba(0,0,0,0.8)" : "none",
            }}
          >
            <div className="preview-col">
              <div>{c.lines[0] ?? ""}</div>
              <div className="preview-pair-tag">{c.version ?? ""}</div>
            </div>
            <div className="preview-col">
              <div>{c.pair.lines[0] ?? ""}</div>
              <div className="preview-pair-tag">{c.pair.version}</div>
            </div>
            <div className="preview-label" style={{ fontSize: px(style.fontSize * 0.5) }}>
              {c.label}
            </div>
          </div>
        ) : (
          <div
            className="preview-inner"
            style={{
              fontFamily: style.fontFamily,
              color: style.textColor,
              fontSize: px(c.kind === "lyrics" ? style.fontSize * 0.72 : style.fontSize),
              textAlign: style.align === "left" ? "left" : "center",
              textShadow: (style.textShadow ?? true) ? "0 1px 4px rgba(0,0,0,0.8)" : "none",
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

      <div className="slot-controls compact">
        <button onClick={() => act(() => displayApi.step(view.slot, -1))}>◀</button>
        <button onClick={() => act(() => displayApi.step(view.slot, 1))}>▶</button>
        <button onClick={() => act(() => displayApi.setBlank(view.slot, !view.blank))}>
          {view.blank ? "Unblank" : "Blank"}
        </button>
        <button onClick={() => act(() => displayApi.setBlank(view.slot, true))}>⬛</button>
        <button onClick={() => setShowStyle(!showStyle)}>
          🔠 {showStyle ? "▲" : "▼"}
        </button>
      </div>

      <div className="slot-controls compact">
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
      </div>

      {showStyle && (
        <div className="style-panel">
          <div className="style-presets">
            {THEME_PRESETS.map((p) => (
              <button
                key={p.name}
                title={p.hint}
                onClick={() => updateStyle(p.style)}
              >
                {p.name}
              </button>
            ))}
          </div>
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
          <div className="color-row">
            <label>
              Alignment
              <select
                value={style.align ?? "center"}
                onChange={(e) =>
                  updateStyle({ align: e.currentTarget.value as "center" | "left" })
                }
              >
                <option value="center">Centered</option>
                <option value="left">Left</option>
              </select>
            </label>
            <label className="check-row">
              <input
                type="checkbox"
                checked={style.textShadow ?? true}
                onChange={(e) => updateStyle({ textShadow: e.currentTarget.checked })}
              />
              Text shadow
            </label>
          </div>
          <div className="bg-image-row">
            <button
              onClick={() =>
                act(async () => {
                  const path = await open({
                    multiple: false,
                    filters: [
                      {
                        name: "Background image",
                        extensions: ["png", "jpg", "jpeg", "webp", "bmp"],
                      },
                    ],
                  });
                  if (typeof path === "string") {
                    updateStyle({ bgImage: path });
                  }
                })
              }
            >
              🖼 Background image…
            </button>
            {style.bgImage ? (
              <>
                <span className="muted bg-image-name" title={style.bgImage}>
                  {style.bgImage.split(/[\\/]/).pop()}
                </span>
                <button onClick={() => updateStyle({ bgImage: null })}>✕ Clear</button>
              </>
            ) : (
              <span className="muted">none — solid color</span>
            )}
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
  const [tab, setTab] = useState<"search" | "browse">("browse");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ContentSummary[]>([]);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [selected, setSelected] = useState<ContentSummary | null>(null);
  const [detail, setDetail] = useState<{ key: string; label: string }[]>([]);

  // Browse state (scripture): version → book → chapter → verse.
  const [allBooks, setAllBooks] = useState<ContentSummary[]>([]);
  const [browseVersion, setBrowseVersion] = useState("KJV");
  const [browseBookId, setBrowseBookId] = useState<string | null>(null);
  const [browseBook, setBrowseBook] = useState<ContentItem | null>(null);
  const [browseChapter, setBrowseChapter] = useState(1);
  const [browseVerse, setBrowseVerse] = useState(1);

  useEffect(() => {
    if (kind === "lyrics") {
      lib.listContent({ itemType: "hymn" }).then((r) => {
        setItems(r);
        lib.listContent({ itemType: "song" }).then((s) => setItems((prev) => [...prev, ...s]));
      });
    } else {
      lib
        .listContent({ itemType: "bible", sort: "canonical" })
        .then((r) => {
          setAllBooks(r);
          const first = r.find((b) => b.title.endsWith("(KJV)")) ?? r[0];
          if (first) setBrowseBookId(first.id);
        })
        .catch(console.error);
    }
  }, [kind]);

  // Load the chosen book to populate chapters/verses.
  useEffect(() => {
    if (kind !== "scripture" || !browseBookId) return;
    lib.getContent(browseBookId).then((b) => {
      setBrowseBook(b);
      setBrowseChapter(1);
      setBrowseVerse(1);
    }).catch(console.error);
  }, [browseBookId, kind]);

  const bookChapters: string[][] = browseBook?.body.chapters ?? [];
  const bookVersions = Array.from(
    new Set(allBooks.map((b) => (b.title.match(/\(([^)]+)\)$/)?.[1] ?? "KJV"))),
  );
  const booksOfVersion = allBooks.filter((b) =>
    b.title.endsWith(`(${browseVersion})`),
  );
  const browseItem = (() => {
    if (!browseBook) return null;
    const base = browseBook.id.replace(/^bible-[a-z0-9]+-/, "");
    return { slug: base, key: `bible-${browseVersion.toLowerCase()}-${base}` };
  })();

  async function showBrowseVerse() {
    if (!browseItem) return;
    try {
      await displayApi.setScripture(slot, browseItem.key, [
        `${browseItem.slug}.${browseChapter}.${browseVerse}`,
      ]);
      onPicked();
    } catch (e) {
      alert(String(e));
    }
  }

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
        {kind === "scripture" && (
          <div className="type-tabs">
            <button className={tab === "browse" ? "active" : ""} onClick={() => setTab("browse")}>
              Browse by reference
            </button>
            <button className={tab === "search" ? "active" : ""} onClick={() => setTab("search")}>
              Search
            </button>
          </div>
        )}

        {kind === "scripture" && tab === "browse" ? (
          <div className="browse-picker">
            <div className="field-row">
              <label>
                Version
                <select
                  value={browseVersion}
                  onChange={(e) => {
                    const v = e.currentTarget.value;
                    setBrowseVersion(v);
                    const first = allBooks.find((b) => b.title.endsWith(`(${v})`));
                    if (first) setBrowseBookId(first.id);
                  }}
                >
                  {bookVersions.map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Book
                <select
                  value={browseBookId ?? ""}
                  onChange={(e) => setBrowseBookId(e.currentTarget.value)}
                >
                  {booksOfVersion.map((b) => (
                    <option key={b.id} value={b.id}>
                      {b.title.replace(/\s*\([^)]*\)$/, "")}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="field-row">
              <label>
                Chapter
                <select
                  value={browseChapter}
                  onChange={(e) => {
                    setBrowseChapter(Number(e.currentTarget.value));
                    setBrowseVerse(1);
                  }}
                >
                  {bookChapters.map((_, i) => (
                    <option key={i + 1} value={i + 1}>
                      {i + 1}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Verse
                <select
                  value={Math.min(browseVerse, (bookChapters[browseChapter - 1] ?? []).length)}
                  onChange={(e) => setBrowseVerse(Number(e.currentTarget.value))}
                >
                  {(bookChapters[browseChapter - 1] ?? []).map((_, i) => (
                    <option key={i + 1} value={i + 1}>
                      {i + 1}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="browse-preview muted">
              {browseBook
                ? `${browseBook.metadata.book ?? ""} ${browseChapter}:${browseVerse} — ${
                    (bookChapters[browseChapter - 1] ?? [])[browseVerse - 1] ?? ""
                  }`
                : "Loading…"}
            </div>
            <div className="form-actions">
              <button className="primary" disabled={!browseBook} onClick={showBrowseVerse}>
                Show on display
              </button>
            </div>
          </div>
        ) : (
          <>
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
          </>
        )}

        <div className="form-actions">
          <button onClick={onClose}>Cancel</button>
        </div>
      </div>
    </div>
  );
}
