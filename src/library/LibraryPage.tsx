import { useCallback, useEffect, useState } from "react";
import * as api from "./api";
import {
  ITEM_TYPE_LABELS,
  LICENSE_OPTIONS,
  SORT_LABELS,
  type ContentItem,
  type ContentSummary,
  type ItemType,
  type LibraryStats,
  type SearchHit,
  type SortMode,
  type Testament,
} from "./types";

type Mode = "browse" | "search";

const TYPE_FILTERS: (ItemType | "all")[] = [
  "all",
  "bible",
  "hymn",
  "song",
  "slide",
  "book",
  "document",
];

const BIBLE_VERSIONS = ["KJV", "ASV", "WEB"] as const;

export default function LibraryPage({
  autoOpenImport = 0,
}: {
  /** Bumped by File → Import; opens the import dialog on change. */
  autoOpenImport?: number;
}) {
  const [mode, setMode] = useState<Mode>("browse");
  const [typeFilter, setTypeFilter] = useState<ItemType | "all">("all");
  const [testament, setTestament] = useState<Testament>(null);
  const [version, setVersion] = useState<(typeof BIBLE_VERSIONS)[number] | "all">("all");
  const [sort, setSort] = useState<SortMode>("title-asc");
  const [items, setItems] = useState<ContentSummary[]>([]);
  const [stats, setStats] = useState<LibraryStats | null>(null);
  const [selected, setSelected] = useState<ContentItem | null>(null);
  const [focusVerse, setFocusVerse] = useState<string | null>(null);

  // search state
  const [searchInput, setSearchInput] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searched, setSearched] = useState(false);

  // import state
  const [showImport, setShowImport] = useState(false);

  // slide editor: new, or editing an existing slide item
  const [slideEdit, setSlideEdit] = useState<
    { mode: "new" } | { mode: "edit"; item: ContentItem } | null
  >(null);

  useEffect(() => {
    if (autoOpenImport > 0) setShowImport(true);
  }, [autoOpenImport]);

  const refreshStats = useCallback(() => {
    api.getStats().then(setStats).catch(console.error);
  }, []);

  const refreshList = useCallback(() => {
    api
      .listContent({
        itemType: typeFilter === "all" ? null : typeFilter,
        titleQuery: null,
        testament: typeFilter === "bible" ? testament : null,
        sort,
      })
      .then(setItems)
      .catch(console.error);
  }, [typeFilter, testament, sort]);

  useEffect(() => {
    refreshStats();
    refreshList();
  }, [refreshStats, refreshList]);

  async function selectItem(id: string, sectionKey?: string) {
    const item = await api.getContent(id);
    setSelected(item);
    setFocusVerse(sectionKey ?? null);
  }

  async function runSearch() {
    if (!searchInput.trim()) {
      setSearched(false);
      setHits([]);
      return;
    }
    const results = await api.searchContent(searchInput.trim());
    setHits(results);
    setSearched(true);
    setMode("search");
  }

  return (
    <div className="library">
      <div className="library-toolbar">
        <div className="type-tabs">
          {TYPE_FILTERS.map((t) => (
            <button
              key={t}
              className={typeFilter === t && mode === "browse" ? "active" : ""}
              onClick={() => {
                setMode("browse");
                setTypeFilter(t);
              }}
            >
              {t === "all" ? "All" : ITEM_TYPE_LABELS[t]}
            </button>
          ))}
        </div>
        <div className="search-box">
          <input
            value={searchInput}
            onChange={(e) => setSearchInput(e.currentTarget.value)}
            onKeyDown={(e) => e.key === "Enter" && runSearch()}
            placeholder="Search all content… e.g. for God so loved, or amazing grace"
          />
          <button onClick={runSearch}>Search</button>
        </div>
        <select
          value={sort}
          onChange={(e) => setSort(e.currentTarget.value as SortMode)}
          title="Sort order"
        >
          {SORT_LABELS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
        <button onClick={() => setSlideEdit({ mode: "new" })} title="Create a custom slide — welcome screen, announcement, sermon point">
          📝 New slide
        </button>
        <button className="primary" onClick={() => setShowImport(true)}>
          ＋ Import / Add
        </button>
      </div>

      {typeFilter === "bible" && (
        <>
          <div className="sub-tabs">
            <span className="sub-tabs-label">Version:</span>
            {(["all", ...BIBLE_VERSIONS] as const).map((v) => (
              <button
                key={v}
                className={version === v ? "active" : ""}
                onClick={() => setVersion(v)}
              >
                {v === "all" ? "All versions" : v}
              </button>
            ))}
          </div>
          <div className="sub-tabs">
            <span className="sub-tabs-label">Testament:</span>
            {(
              [
                [null, "All"],
                ["ot", "Old Testament"],
                ["nt", "New Testament"],
              ] as [Testament, string][]
            ).map(([value, label]) => (
              <button
                key={label}
                className={testament === value ? "active" : ""}
                onClick={() => setTestament(value)}
              >
                {label}
              </button>
            ))}
          </div>
        </>
      )}

      {stats && (
        <div className="stats-line">
          {stats.total.toLocaleString()} items ·{" "}
          {Object.entries(stats.byType)
            .map(
              ([t, n]) =>
                `${n.toLocaleString()} ${ITEM_TYPE_LABELS[t as ItemType] ?? t}`,
            )
            .join(" · ")}
        </div>
      )}

      <div className="library-body">
        <div className="item-list">
          {mode === "browse" ? (
            items.length === 0 ? (
              <div className="empty">No items.</div>
            ) : (
              items
                .filter(
                  (it) =>
                    version === "all" ||
                    !it.itemType ||
                    it.itemType !== "bible" ||
                    it.title.endsWith(`(${version})`),
                )
                .map((it) => (
                <button
                  key={it.id}
                  className={`item-row ${
                    selected?.id === it.id ? "selected" : ""
                  }`}
                  onClick={() => selectItem(it.id)}
                >
                  <span className="item-title">{it.title}</span>
                  <span className="item-meta">
                    {ITEM_TYPE_LABELS[it.itemType]}
                    {it.visibility === "private" ? " · 🔒 private" : ""}
                  </span>
                </button>
              ))
            )
          ) : searched && hits.length === 0 ? (
            <div className="empty">No matches.</div>
          ) : (
            hits.map((h) => (
              <button
                key={h.itemId + h.sectionKey}
                className="hit-row"
                onClick={() => selectItem(h.itemId, h.sectionKey)}
              >
                <div className="hit-label">
                  {h.sectionLabel} — {h.itemTitle}
                  <span className="item-meta">
                    {" "}
                    {ITEM_TYPE_LABELS[h.itemType]}
                  </span>
                </div>
                <div
                  className="hit-snippet"
                  dangerouslySetInnerHTML={{ __html: h.snippet }}
                />
              </button>
            ))
          )}
        </div>

        <div className="detail-pane">
          {selected ? (
            <ItemDetail
              item={selected}
              focusVerse={focusVerse}
              onOpenItem={(id) => selectItem(id)}
              onChanged={() => {
                refreshList();
                refreshStats();
                selectItem(selected.id);
              }}
              onEditSlides={(it) => setSlideEdit({ mode: "edit", item: it })}
              onDeleted={() => {
                setSelected(null);
                refreshList();
                refreshStats();
              }}
            />
          ) : (
            <div className="empty">Select an item to view it.</div>
          )}
        </div>
      </div>

      {showImport && (
        <ImportDialog
          onClose={() => setShowImport(false)}
          onImported={(item) => {
            setShowImport(false);
            refreshList();
            refreshStats();
            selectItem(item.id);
          }}
        />
      )}

      {slideEdit && (
        <SlideEditorDialog
          edit={slideEdit.mode === "edit" ? slideEdit.item : null}
          onClose={() => setSlideEdit(null)}
          onSaved={(id) => {
            setSlideEdit(null);
            refreshList();
            refreshStats();
            selectItem(id);
          }}
        />
      )}
    </div>
  );
}

function ItemDetail({
  item,
  focusVerse,
  onOpenItem,
  onChanged,
  onEditSlides,
  onDeleted,
}: {
  item: ContentItem;
  focusVerse: string | null;
  onOpenItem: (id: string) => void;
  onChanged: () => void;
  onEditSlides: (item: ContentItem) => void;
  onDeleted: () => void;
}) {
  const [draft, setDraft] = useState<ContentItem>(item);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setDraft(item);
    setEditing(false);
  }, [item]);

  async function save() {
    setSaving(true);
    try {
      await api.updateContent(draft);
      setEditing(false);
      onChanged();
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="detail">
      {editing ? (
        <div className="detail-form">
          <input
            className="detail-title-input"
            value={draft.title}
            onChange={(e) => setDraft({ ...draft, title: e.currentTarget.value })}
          />
          <div className="field-row">
            <label>
              Type
              <select
                value={draft.itemType}
                disabled={draft.itemType === "bible"}
                onChange={(e) =>
                  setDraft({ ...draft, itemType: e.currentTarget.value as ItemType })
                }
              >
                {(Object.keys(ITEM_TYPE_LABELS) as ItemType[]).map((t) => (
                  <option key={t} value={t}>
                    {ITEM_TYPE_LABELS[t]}
                  </option>
                ))}
              </select>
            </label>
            <label>
              License
              <select
                value={draft.license}
                onChange={(e) =>
                  setDraft({ ...draft, license: e.currentTarget.value })
                }
              >
                {LICENSE_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Visibility
              <select
                value={draft.visibility}
                onChange={(e) =>
                  setDraft({ ...draft, visibility: e.currentTarget.value })
                }
              >
                <option value="public">Public</option>
                <option value="private">Private</option>
              </select>
            </label>
          </div>
          <div className="form-actions">
            <button className="primary" onClick={save} disabled={saving}>
              {saving ? "Saving…" : "Save"}
            </button>
            <button onClick={() => setEditing(false)}>Cancel</button>
          </div>
        </div>
      ) : (
        <div className="detail-header">
          <h2>{item.title}</h2>
          <div className="detail-meta">
            {ITEM_TYPE_LABELS[item.itemType]} · {item.license}
            {item.visibility === "private" ? " · 🔒 private" : ""}
          </div>
          <div className="form-actions">
            {item.itemType === "slide" && (
              <button className="primary" onClick={() => onEditSlides(item)}>
                ✎ Edit slides
              </button>
            )}
            <button onClick={() => setEditing(true)}>Edit</button>
            <button className="danger" onClick={onDeleted}>
              Delete
            </button>
          </div>
        </div>
      )}

      <div className="detail-content">
        {item.itemType === "bible" && item.body.chapters ? (
          <BibleReader item={item} focusVerse={focusVerse} onOpenItem={onOpenItem} />
        ) : (
          (item.body.sections ?? []).map((s, i) => (
            <div key={i} className="section">
              <h4>{s.label}</h4>
              {s.lines.map((l, li) => (
                <div key={li}>{l}</div>
              ))}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

const ANIMATIONS = ["none", "fade", "slide"] as const;
type ReaderAnimation = (typeof ANIMATIONS)[number];

function readerAnimation(): ReaderAnimation {
  const v = localStorage.getItem("bible-animation");
  return (ANIMATIONS as readonly string[]).includes(v ?? "") ? (v as ReaderAnimation) : "fade";
}

/**
 * Chapter-based Bible reader: version + book + chapter selectors, prev/next
 * chapter, one chapter at a time, optional transition animation. The book
 * list comes from the library (canonical order) so switching books or
 * versions loads the right item by id (`bible-{version}-{book-slug}`).
 */
function BibleReader({
  item,
  focusVerse,
  onOpenItem,
}: {
  item: ContentItem;
  focusVerse: string | null;
  onOpenItem: (id: string) => void;
}) {
  const [, setTick] = useState(0);
  const [animation, setAnimation] = useState<ReaderAnimation>(readerAnimation);
  const [chapter, setChapter] = useState(1);
  const [books, setBooks] = useState<{ id: string; title: string }[]>([]);
  const [skipScroll, setSkipScroll] = useState(false);

  const m = item.id.match(/^bible-([a-z0-9]+)-(.+)$/);
  const version = (m?.[1] ?? "kjv").toUpperCase();
  const bookSlug = m?.[2] ?? "";
  const bookName = String(item.metadata.book ?? item.title.replace(/\s*\([^)]*\)$/, ""));
  const chapters = item.body.chapters ?? [];

  // Book list for the switchers (canonical order from the backend).
  useEffect(() => {
    api
      .listContent({ itemType: "bible", titleQuery: null, testament: null, sort: "canonical" })
      .then((all) => setBooks(all.filter((b) => b.title.endsWith(`(${version})`)).map((b) => ({ id: b.id, title: b.title }))))
      .catch(console.error);
  }, [version]);

  // Entering a book: chapter from a search hit, else chapter 1.
  useEffect(() => {
    if (skipScroll) {
      setSkipScroll(false);
      return;
    }
    const parsed = focusVerse?.match(/\.\d+\.(\d+)$/);
    setChapter(parsed ? Number(parsed[1]) : 1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [item.id]);

  // Scroll the focused verse into view after render.
  useEffect(() => {
    if (!focusVerse) return;
    const el = document.getElementById(`rv-${focusVerse}`);
    el?.scrollIntoView({ block: "center" });
  }, [focusVerse, chapter, item.id]);

  function switchBook(id: string) {
    if (id === item.id) return;
    onOpenItem(id);
  }

  function switchVersion(v: string) {
    if (!m) return;
    switchBook(`bible-${v.toLowerCase()}-${bookSlug}`);
  }

  function goChapter(delta: number) {
    setChapter((c) => Math.min(chapters.length, Math.max(1, c + delta)));
  }

  const verses = chapters[chapter - 1] ?? [];

  return (
    <div className="bible-reader">
      <div className="reader-toolbar">
        <select
          value={version.toLowerCase()}
          onChange={(e) => switchVersion(e.currentTarget.value)}
          title="Bible version"
        >
          {BIBLE_VERSIONS.map((v) => (
            <option key={v} value={v.toLowerCase()}>
              {v}
            </option>
          ))}
        </select>
        <select
          value={item.id}
          onChange={(e) => switchBook(e.currentTarget.value)}
          title="Book"
        >
          {books.map((b) => (
            <option key={b.id} value={b.id}>
              {b.title.replace(/\s*\([^)]*\)$/, "")}
            </option>
          ))}
          {/* current book always present even if list not loaded yet */}
          {!books.some((b) => b.id === item.id) && (
            <option value={item.id}>{bookName}</option>
          )}
        </select>
        <select
          value={chapter}
          onChange={(e) => setChapter(Number(e.currentTarget.value))}
          title="Chapter"
        >
          {chapters.map((_, i) => (
            <option key={i + 1} value={i + 1}>
              Chapter {i + 1}
            </option>
          ))}
        </select>
        <button onClick={() => goChapter(-1)} disabled={chapter <= 1}>
          ◀
        </button>
        <button onClick={() => goChapter(1)} disabled={chapter >= chapters.length}>
          ▶
        </button>
        <span className="reader-position">
          {bookName} {chapter}
        </span>
        <select
          className="reader-anim"
          value={animation}
          onChange={(e) => {
            const v = e.currentTarget.value as ReaderAnimation;
            setAnimation(v);
            localStorage.setItem("bible-animation", v);
            setTick((t) => t + 1);
          }}
          title="Page-turn animation"
        >
          {ANIMATIONS.map((a) => (
            <option key={a} value={a}>
              {a === "none" ? "No animation" : a === "fade" ? "Fade" : "Slide"}
            </option>
          ))}
        </select>
      </div>

      <div
        key={`${item.id}-${chapter}`}
        className={`reader-chapter anim-${animation}`}
      >
        <h3>
          {bookName} {chapter}
        </h3>
        <p className="reader-verses">
          {verses.map((v, vi) => {
            const key = `${bookSlug}.${chapter}.${vi + 1}`;
            return (
              <span
                key={vi}
                id={`rv-${key}`}
                className={focusVerse === key ? "verse-focus" : undefined}
              >
                <sup>{vi + 1}</sup> {v}{" "}
              </span>
            );
          })}
        </p>
      </div>
    </div>
  );
}

function ImportDialog({
  onClose,
  onImported,
}: {
  onClose: () => void;
  onImported: (item: ContentItem) => void;
}) {
  const [tab, setTab] = useState<"text" | "file">("text");
  const [title, setTitle] = useState("");
  const [itemType, setItemType] = useState<ItemType>("document");
  const [license, setLicense] = useState("unknown");
  const [text, setText] = useState("");
  const [filePath, setFilePath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const copyrighted = license === "copyrighted" || license === "unknown";

  async function doImport() {
    setBusy(true);
    setError(null);
    try {
      if (tab === "text") {
        const item = await api.importText({
          title: title.trim() || "Untitled content",
          itemType,
          text,
          language: "en",
          license,
        });
        onImported(item);
      } else {
        if (!filePath) {
          setError("Choose a file first.");
          return;
        }
        const item = await api.importFile({
          path: filePath,
          title: title.trim() || null,
          itemType,
          language: "en",
          license,
        });
        onImported(item);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>Import / Add content</h2>
        <div className="type-tabs">
          <button
            className={tab === "text" ? "active" : ""}
            onClick={() => setTab("text")}
          >
            Paste text
          </button>
          <button
            className={tab === "file" ? "active" : ""}
            onClick={() => setTab("file")}
          >
            From file (.txt / .docx)
          </button>
        </div>

        <label>
          Title
          <input
            value={title}
            onChange={(e) => setTitle(e.currentTarget.value)}
            placeholder={
              tab === "file" ? "(defaults to file name)" : "My song lyrics"
            }
          />
        </label>
        <div className="field-row">
          <label>
            Type
            <select
              value={itemType}
              onChange={(e) => setItemType(e.currentTarget.value as ItemType)}
            >
              <option value="document">Document</option>
              <option value="song">Song</option>
              <option value="hymn">Hymn</option>
              <option value="book">Book</option>
            </select>
          </label>
          <label>
            License
            <select
              value={license}
              onChange={(e) => setLicense(e.currentTarget.value)}
            >
              {LICENSE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </label>
        </div>
        {copyrighted && (
          <div className="warning">
            ⚠ Copyrighted / unknown-license content will be marked{" "}
            <b>private</b> and will not be shareable.
          </div>
        )}

        {tab === "text" ? (
          <textarea
            rows={10}
            value={text}
            onChange={(e) => setText(e.currentTarget.value)}
            placeholder={
              "Paste content here.\n\nBlank lines separate stanzas/sections.\nLines like [Verse 1] or Chorus: become section labels."
            }
          />
        ) : (
          <div className="file-row">
            <button
              onClick={async () => {
                const p = await api.pickFile();
                setFilePath(p);
              }}
            >
              Choose file…
            </button>
            <span className="file-path">{filePath ?? "No file chosen"}</span>
          </div>
        )}

        {error && <div className="error">{error}</div>}
        <div className="form-actions">
          <button className="primary" onClick={doImport} disabled={busy}>
            {busy ? "Importing…" : "Import"}
          </button>
          <button onClick={onClose}>Cancel</button>
        </div>
      </div>
    </div>
  );
}

/** Create/edit a custom slide item — welcome screens, announcements,
 *  sermon points. Each section is one slide; the projector walks them
 *  with Next/Prev. */
function SlideEditorDialog({
  edit,
  onClose,
  onSaved,
}: {
  edit: ContentItem | null;
  onClose: () => void;
  onSaved: (id: string) => void;
}) {
  const fromExisting = edit?.itemType === "slide" ? edit : null;
  const [title, setTitle] = useState(fromExisting?.title ?? "");
  const [slides, setSlides] = useState<{ label: string; text: string }[]>(
    fromExisting
      ? (fromExisting.body.sections ?? []).map((s) => ({
          label: s.label,
          text: s.lines.join("\n"),
        }))
      : [{ label: "", text: "" }],
  );
  const [reveal, setReveal] = useState(
    fromExisting ? fromExisting.body.revealLines !== false : true,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function setSlide(i: number, patch: Partial<{ label: string; text: string }>) {
    setSlides((prev) => prev.map((s, j) => (j === i ? { ...s, ...patch } : s)));
  }

  async function save() {
    const name = title.trim();
    if (!name) {
      setError("Give the slide set a title (e.g., Welcome).");
      return;
    }
    const clean = slides.filter((s) => s.label.trim() || s.text.trim());
    if (clean.length === 0) {
      setError("Add at least one slide.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const id =
        fromExisting?.id ??
        `slide-${name
          .toLowerCase()
          .split(/[^a-z0-9]+/)
          .filter(Boolean)
          .join("-")}-${Date.now() % 10000}`;
      const item: ContentItem = {
        id,
        itemType: "slide",
        title: name,
        language: fromExisting?.language ?? "en",
        license: fromExisting?.license ?? "public-domain",
        visibility: fromExisting?.visibility ?? "public",
        metadata: fromExisting?.metadata ?? {},
        body: {
          sections: clean.map((s, i) => ({
            label: s.label.trim() || `Slide ${i + 1}`,
            lines: s.text.split(/\r?\n/).map((l) => l.trim()).filter(Boolean),
          })),
        },
      };
      if (fromExisting) {
        await api.updateContent(item);
      } else {
        await api.createContent(item);
      }
      onSaved(id);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal slide-editor" onClick={(e) => e.stopPropagation()}>
        <h2>{fromExisting ? "✎ Edit slides" : "📝 New slide"}</h2>
        <label className="slide-title-row">
          Title
          <input
            autoFocus
            value={title}
            onChange={(e) => setTitle(e.currentTarget.value)}
            placeholder="e.g., Welcome / Announcements / Sermon points"
            onKeyDown={(e) => e.key === "Enter" && save()}
          />
        </label>

        <div className="slide-list">
          {slides.map((s, i) => (
            <div className="slide-row" key={i}>
              <div className="slide-row-head">
                <span className="muted">Slide {i + 1}</span>
                <button
                  className="danger"
                  title="Remove this slide"
                  disabled={slides.length === 1}
                  onClick={() => setSlides((prev) => prev.filter((_, j) => j !== i))}
                >
                  ✕
                </button>
              </div>
              <input
                value={s.label}
                onChange={(e) => setSlide(i, { label: e.currentTarget.value })}
                placeholder="Heading (optional — shown small under the text)"
              />
              <textarea
                value={s.text}
                onChange={(e) => setSlide(i, { text: e.currentTarget.value })}
                placeholder={"What this slide says…\nOne line per displayed line"}
                rows={3}
              />
            </div>
          ))}
        </div>
        <button
          className="slide-add"
          onClick={() => setSlides((prev) => [...prev, { label: "", text: "" }])}
        >
          ＋ Add slide
        </button>

        <label className="check-row slide-reveal-row">
          <input
            type="checkbox"
            checked={reveal}
            onChange={(e) => setReveal(e.currentTarget.checked)}
          />
          Reveal lines one by one (bullet-build) — untick to always show every
          line and step whole slides
        </label>

        {error && <div className="error">{error}</div>}
        <div className="form-actions">
          <button className="primary" onClick={save} disabled={busy}>
            {busy ? "Saving…" : fromExisting ? "Save changes" : "Create"}
          </button>
          <button onClick={onClose}>Cancel</button>
        </div>
      </div>
    </div>
  );
}
