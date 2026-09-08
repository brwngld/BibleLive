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
  "book",
  "document",
];

export default function LibraryPage() {
  const [mode, setMode] = useState<Mode>("browse");
  const [typeFilter, setTypeFilter] = useState<ItemType | "all">("all");
  const [testament, setTestament] = useState<Testament>(null);
  const [sort, setSort] = useState<SortMode>("title-asc");
  const [items, setItems] = useState<ContentSummary[]>([]);
  const [stats, setStats] = useState<LibraryStats | null>(null);
  const [selected, setSelected] = useState<ContentItem | null>(null);

  // search state
  const [searchInput, setSearchInput] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searched, setSearched] = useState(false);

  // import state
  const [showImport, setShowImport] = useState(false);

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

  async function selectItem(id: string) {
    const item = await api.getContent(id);
    setSelected(item);
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
        <button className="primary" onClick={() => setShowImport(true)}>
          ＋ Import / Add
        </button>
      </div>

      {typeFilter === "bible" && (
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
              items.map((it) => (
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
                onClick={() => selectItem(h.itemId)}
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
              onChanged={() => {
                refreshList();
                refreshStats();
              }}
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
    </div>
  );
}

function ItemDetail({
  item,
  onChanged,
  onDeleted,
}: {
  item: ContentItem;
  onChanged: () => void;
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
            <button onClick={() => setEditing(true)}>Edit</button>
            <button className="danger" onClick={onDeleted}>
              Delete
            </button>
          </div>
        </div>
      )}

      <div className="detail-content">
        {item.itemType === "bible" && item.body.chapters
          ? item.body.chapters.map((verses, ci) => (
              <div key={ci} className="chapter">
                <h3>
                  {String(item.metadata.book ?? "")} {ci + 1}
                </h3>
                <p>
                  {verses.map((v, vi) => (
                    <span key={vi}>
                      <sup>{vi + 1}</sup> {v}{" "}
                    </span>
                  ))}
                </p>
              </div>
            ))
          : (item.body.sections ?? []).map((s, i) => (
              <div key={i} className="section">
                <h4>{s.label}</h4>
                {s.lines.map((l, li) => (
                  <div key={li}>{l}</div>
                ))}
              </div>
            ))}
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
