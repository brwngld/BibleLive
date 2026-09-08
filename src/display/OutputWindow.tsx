import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { displayApi, onDisplayUpdate, type SlotContent, type SlotView } from "./api";

/**
 * Fullscreen output renderer for one Display Slot. The window label is
 * "display-N" — the slot number comes from that label.
 * Keys: Esc closes · ← / → prev/next · M moves to the next monitor ·
 * F11 exits fullscreen.
 *
 * Text is auto-fit: rendered at the slot's configured size, then shrunk
 * until it fits the viewport, so nothing overflows on any screen shape.
 */
export default function OutputWindow({ slot }: { slot: number }) {
  const [view, setView] = useState<SlotContent | null>(null);
  const [showHint, setShowHint] = useState(true);
  const [toast, setToast] = useState<string | null>(null);
  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });
  const prevActive = useRef(false);
  const toastTimer = useRef<number | undefined>(undefined);
  const scriptureRef = useRef<HTMLDivElement>(null);
  const lyricsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let un: (() => void) | undefined;
    onDisplayUpdate((v: SlotView) => {
      if (v.slot !== slot) return;
      setView(v.content);
      if (v.active && !prevActive.current) {
        setToast(`Display ${slot} is active — Ctrl+Alt+←/→ step it`);
        window.clearTimeout(toastTimer.current);
        toastTimer.current = window.setTimeout(() => setToast(null), 2200);
      }
      prevActive.current = v.active;
    }).then((u) => (un = u));
    displayApi.slots().then((views) => {
      const mine = views.find((v) => v.slot === slot);
      if (mine) {
        setView(mine.content);
        prevActive.current = mine.active;
      }
    }).catch(console.error);
    return () => un?.();
  }, [slot]);

  // Re-fit on window resize (monitor changes, fullscreen transitions).
  useEffect(() => {
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  // Auto-fit: start at the configured size, shrink until the text fits.
  useEffect(() => {
    const kind = view?.kind;
    if (kind !== "scripture" && kind !== "lyrics") return;
    const style = view?.style;
    const baseVw = kind === "lyrics" ? (style?.fontSize ?? 6.5) * 0.72 : style?.fontSize ?? 6.5;
    const el = kind === "lyrics" ? lyricsRef.current : scriptureRef.current;
    if (!el) return;
    let size = (baseVw / 100) * viewport.w;
    el.style.fontSize = `${size}px`;
    // Two passes: fonts settle after first layout.
    requestAnimationFrame(() => {
      let guard = 0;
      while (guard < 60 && (el.scrollHeight > window.innerHeight * 0.86 || el.scrollWidth > window.innerWidth * 0.96)) {
        size *= 0.94;
        el.style.fontSize = `${size}px`;
        guard++;
      }
    });
  }, [view, viewport]);

  useEffect(() => {
    const win = getCurrentWindow();
    const onKey = (e: KeyboardEvent) => {
      switch (e.key) {
        case "Escape":
          win.close().catch(console.error);
          break;
        case "ArrowRight":
          invoke("slot_step", { slot, delta: 1 }).catch(console.error);
          break;
        case "ArrowLeft":
          invoke("slot_step", { slot, delta: -1 }).catch(console.error);
          break;
        case "m":
        case "M":
          invoke("move_slot_output_to_next_monitor", { slot })
            .then(() => setToast("Moved to the next screen"))
            .catch((err) => setToast(String(err)))
            .finally(() => {
              window.clearTimeout(toastTimer.current);
              toastTimer.current = window.setTimeout(() => setToast(null), 2200);
            });
          break;
        case "Tab":
          // Cycle keyboard focus between open fullscreen outputs.
          e.preventDefault();
          invoke("cycle_output_focus", { fromSlot: slot, dir: e.shiftKey ? -1 : 1 })
            .catch(console.error);
          break;
        case "F11":
          win.setFullscreen(false).catch(() => {});
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    const hintTimer = window.setTimeout(() => setShowHint(false), 6000);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.clearTimeout(hintTimer);
    };
  }, [slot]);

  const blank = !view || view.blank || view.kind === "blank";
  const style = view?.style;
  const font = style?.fontFamily ?? "Georgia, 'Times New Roman', serif";
  const size = style?.fontSize ?? 6.5;
  const textColor = style?.textColor ?? "#ffffff";
  const bgColor = style?.bgColor ?? "#000000";

  return (
    <div className="output-root" style={{ backgroundColor: bgColor }}>
      {!blank && view.kind === "scripture" && (
        <div className="output-scripture">
          <div
            ref={scriptureRef}
            className="output-text"
            style={{ fontFamily: font, fontSize: `${size}vw`, color: textColor }}
          >
            {view.lines.map((l, i) => (
              <span key={i}>
                {l}{" "}
              </span>
            ))}
          </div>
          <div
            className="output-label"
            style={{ fontFamily: font, fontSize: `${size * 0.5}vw`, color: textColor }}
          >
            {view.label}
          </div>
        </div>
      )}

      {!blank && view.kind === "lyrics" && (
        <div className="output-lyrics">
          <div
            ref={lyricsRef}
            className="output-lyric-lines"
            style={{ fontFamily: font, fontSize: `${size * 0.72}vw`, color: textColor }}
          >
            {view.lines.map((l, i) => (
              <div key={i}>{l || "\u00A0"}</div>
            ))}
          </div>
          <div
            className="output-sublabel"
            style={{ fontFamily: font, fontSize: `${size * 0.32}vw`, color: textColor }}
          >
            {view.title}
            {view.label ? ` · ${view.label}` : ""}
          </div>
        </div>
      )}

      {!blank && view.kind === "image" && view.imagePath && (
        <img className="output-media" src={convertFileSrc(view.imagePath)} alt={view.title} />
      )}

      {!blank && view.kind === "video" && view.videoPath && (
        <video
          className="output-media"
          src={convertFileSrc(view.videoPath)}
          autoPlay
          controls
        />
      )}

      {blank && <div className="output-blank" />}

      {!blank && (
        <div className="output-page-indicator" style={{ color: textColor }}>
          {view.page}
        </div>
      )}

      {showHint && (
        <div className="output-hint">
          Display {slot} · ← / → change verse · Tab switches outputs · M moves screen · Esc closes
        </div>
      )}

      {toast && <div className="output-toast">{toast}</div>}
    </div>
  );
}
