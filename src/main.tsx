import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import OutputWindow from "./display/OutputWindow";
import "./styles.css";

// Display output windows are separate webviews labeled "display-1"…"display-5";
// everything else is the main operator window.
const match = getCurrentWindow().label.match(/^display-(\d)$/);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {match ? (
      <OutputWindow slot={Number(match[1])} />
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
