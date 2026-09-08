import { useState } from "react";
import LibraryPage from "./library/LibraryPage";
import VoicePage from "./voice/VoicePage";
import DisplaysPage from "./display/DisplaysPage";
import LivePage from "./live/LivePage";

type Tab = "library" | "displays" | "voice" | "live";

export default function App() {
  const [tab, setTab] = useState<Tab>("library");

  return (
    <div className="app">
      <nav className="top-nav">
        <div className="brand">
          <span className="brand-mark">B</span> BibleLive
        </div>
        <div className="nav-tabs">
          <button
            className={tab === "library" ? "active" : ""}
            onClick={() => setTab("library")}
          >
            📚 Library
          </button>
          <button
            className={tab === "displays" ? "active" : ""}
            onClick={() => setTab("displays")}
          >
            🖼 Displays
          </button>
          <button
            className={tab === "voice" ? "active" : ""}
            onClick={() => setTab("voice")}
          >
            🎙 Voice
          </button>
          <button
            className={tab === "live" ? "active" : ""}
            onClick={() => setTab("live")}
          >
            🎛 Live Service
          </button>
        </div>
      </nav>

      <main className="app-main">
        {tab === "library" ? (
          <LibraryPage />
        ) : tab === "displays" ? (
          <DisplaysPage />
        ) : tab === "voice" ? (
          <VoicePage />
        ) : (
          <LivePage />
        )}
      </main>
    </div>
  );
}
