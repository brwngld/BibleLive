import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import LibraryPage from "./library/LibraryPage";
import VoicePage from "./voice/VoicePage";
import DisplaysPage from "./display/DisplaysPage";
import LivePage from "./live/LivePage";
import MenuBar, { type MenuAction, type Tab } from "./menu/MenuBar";
import SettingsDialog from "./settings/SettingsDialog";
import ShortcutsDialog from "./menu/ShortcutsDialog";
import { serviceApi } from "./live/api";

const TABS: Tab[] = ["library", "displays", "voice", "live"];

const TAB_LABELS: Record<Tab, string> = {
  library: "📚 Library",
  displays: "🖼 Displays",
  voice: "🎙 Voice",
  live: "🎛 Live Service",
};

export default function App() {
  const [tab, setTab] = useState<Tab>("library");
  const [dialog, setDialog] = useState<null | "settings" | "shortcuts">(null);
  // Bumped each time File → Import is used; LibraryPage opens its import
  // dialog on change (works both from another tab and while on Library).
  const [importRequest, setImportRequest] = useState(0);

  // Ctrl+1..4 switches pages in the operator window only — display
  // outputs are separate windows and never see these keys.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.altKey && !e.shiftKey) {
        const n = Number(e.key);
        if (n >= 1 && n <= TABS.length) {
          e.preventDefault();
          setTab(TABS[n - 1]);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function onMenuAction(a: MenuAction) {
    switch (a.kind) {
      case "go":
        setTab(a.tab);
        break;
      case "import":
        setTab("library");
        setImportRequest((r) => r + 1);
        break;
      case "manageProfiles":
        setTab("displays");
        break;
      case "applyProfile":
        invoke("apply_display_profile", { name: a.name }).catch(console.error);
        break;
      case "blankAll":
        serviceApi.blankAll(a.blank).catch(console.error);
        break;
      case "settings":
        setDialog("settings");
        break;
      case "micTest":
      case "diagnostics":
        setTab("voice");
        break;
      case "openDataFolder":
        invoke("open_data_folder").catch(console.error);
        break;
      case "shortcuts":
        setDialog("shortcuts");
        break;
      case "exit":
        // The main window's close handler (Rust) exits the whole app,
        // taking any open display outputs with it.
        getCurrentWindow().close().catch(console.error);
        break;
    }
  }

  return (
    <div className="app">
      <nav className="top-nav">
        <div className="brand">
          <span className="brand-mark">B</span> BibleLive
        </div>
        <MenuBar onAction={onMenuAction} />
        <div className="nav-tabs">
          {TABS.map((t) => (
            <button key={t} className={tab === t ? "active" : ""} onClick={() => setTab(t)}>
              {TAB_LABELS[t]}
            </button>
          ))}
        </div>
      </nav>

      <main className="app-main">
        {tab === "library" ? (
          <LibraryPage autoOpenImport={importRequest} />
        ) : tab === "displays" ? (
          <DisplaysPage />
        ) : tab === "voice" ? (
          <VoicePage />
        ) : (
          <LivePage />
        )}
      </main>

      {dialog === "settings" && <SettingsDialog onClose={() => setDialog(null)} />}
      {dialog === "shortcuts" && <ShortcutsDialog onClose={() => setDialog(null)} />}
    </div>
  );
}
