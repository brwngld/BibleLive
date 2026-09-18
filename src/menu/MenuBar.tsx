import { useEffect, useRef, useState } from "react";
import { displayApi, type DisplayProfile } from "../display/api";

export type Tab = "library" | "displays" | "voice" | "live";

export type MenuAction =
  | { kind: "go"; tab: Tab }
  | { kind: "import" }
  | { kind: "manageProfiles" }
  | { kind: "applyProfile"; name: string }
  | { kind: "blankAll"; blank: boolean }
  | { kind: "settings" }
  | { kind: "micTest" }
  | { kind: "diagnostics" }
  | { kind: "openDataFolder" }
  | { kind: "shortcuts" }
  | { kind: "exit" };

type Entry = {
  label: string;
  hint?: string;
  action: MenuAction;
  /** Render a separator line above this entry. */
  sep?: boolean;
};

const GO_ENTRIES: Entry[] = [
  { label: "📚 Library", hint: "Ctrl+1", action: { kind: "go", tab: "library" } },
  { label: "🖼 Displays", hint: "Ctrl+2", action: { kind: "go", tab: "displays" } },
  { label: "🎙 Voice", hint: "Ctrl+3", action: { kind: "go", tab: "voice" } },
  { label: "🎛 Live Service", hint: "Ctrl+4", action: { kind: "go", tab: "live" } },
];

/**
 * Application menu bar (in-app, styled to the dark theme). Classic
 * behavior: click to open, hover to switch while open, Escape or a click
 * outside closes. Rendered inside the top nav; the tab strip stays for
 * one-click page switching.
 */
export default function MenuBar({ onAction }: { onAction: (a: MenuAction) => void }) {
  const [open, setOpen] = useState<string | null>(null);
  const [profiles, setProfiles] = useState<DisplayProfile[]>([]);
  const rootRef = useRef<HTMLDivElement>(null);

  // Close on outside click / Escape.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(null);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(null);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  // Saved display profiles are fetched when the File menu opens — the
  // freshest list without polling.
  useEffect(() => {
    if (open === "File") {
      displayApi.profiles().then(setProfiles).catch(() => setProfiles([]));
    }
  }, [open]);

  function fire(a: MenuAction) {
    setOpen(null);
    onAction(a);
  }

  const fileEntries: Entry[] = [
    { label: "Import content…", action: { kind: "import" } },
    ...profiles.map((p, i) => ({
      label: `Apply profile “${p.name}”`,
      action: { kind: "applyProfile", name: p.name } as MenuAction,
      sep: i === 0,
    })),
    { label: "Manage display profiles…", action: { kind: "manageProfiles" }, sep: true },
    { label: "Exit", action: { kind: "exit" }, sep: true },
  ];

  const menus: { name: string; entries: Entry[] }[] = [
    { name: "File", entries: fileEntries },
    {
      name: "View",
      entries: [
        ...GO_ENTRIES,
        { label: "Blank all displays", action: { kind: "blankAll", blank: true }, sep: true },
        { label: "Show all displays", action: { kind: "blankAll", blank: false } },
      ],
    },
    {
      name: "Settings",
      entries: [{ label: "Open Settings…", action: { kind: "settings" } }],
    },
    {
      name: "Tools",
      entries: [
        { label: "🎤 Microphone test", action: { kind: "micTest" } },
        { label: "🔬 Voice diagnostics", action: { kind: "diagnostics" } },
        { label: "📂 Open data folder", action: { kind: "openDataFolder" }, sep: true },
      ],
    },
    {
      name: "Help",
      entries: [
        { label: "Keyboard shortcuts", action: { kind: "shortcuts" } },
        { label: "About BibleLive", action: { kind: "settings" } },
      ],
    },
  ];

  return (
    <div className="menu-bar" ref={rootRef}>
      {menus.map((m) => (
        <div className="menu-root" key={m.name}>
          <button
            className={"menu-trigger" + (open === m.name ? " open" : "")}
            onMouseDown={(e) => {
              e.preventDefault();
              setOpen(open === m.name ? null : m.name);
            }}
            onMouseEnter={() => {
              if (open && open !== m.name) setOpen(m.name);
            }}
          >
            {m.name}
          </button>
          {open === m.name && (
            <div className="menu-dropdown">
              {m.entries.map((entry, i) => (
                <div key={i}>
                  {entry.sep && <div className="menu-sep" />}
                  <button className="menu-entry" onClick={() => fire(entry.action)}>
                    <span>{entry.label}</span>
                    {entry.hint && <span className="menu-hint">{entry.hint}</span>}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
