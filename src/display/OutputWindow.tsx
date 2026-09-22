import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
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
  // Content-transition class ("out-anim-fade" / "out-anim-slide"), applied
  // whenever the displayed text changes and the slot's style asks for one.
  const [animClass, setAnimClass] = useState<string | null>(null);
  const contentSig = useRef("");
  // Lower-third announcement overlay (independent of slot content).
  const [notice, setNotice] = useState<{ id: number; text: string; durationMs: number } | null>(null);
  const noticeTimer = useRef<number | undefined>(undefined);
  // Per-display auto-advance interval (ms, 0 = off), from the slot view.
  const [autoAdvanceMs, setAutoAdvanceMs] = useState(0);
  const prevActive = useRef(false);
  const toastTimer = useRef<number | undefined>(undefined);
  const scriptureRef = useRef<HTMLDivElement>(null);
  const lyricsRef = useRef<HTMLDivElement>(null);
  const pairRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let un: (() => void) | undefined;
    onDisplayUpdate((v: SlotView) => {
      if (v.slot !== slot) return;
      setView(v.content);
      setAutoAdvanceMs(v.autoAdvanceMs ?? 0);
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
        setAutoAdvanceMs(mine.autoAdvanceMs ?? 0);
        prevActive.current = mine.active;
      }
    }).catch(console.error);
    return () => un?.();
  }, [slot]);

  // Announcements: show/hide via the display-notify event; auto-hide.
  useEffect(() => {
    let un: (() => void) | undefined;
    listen<{ slot: number; id: number; text: string; durationMs: number }>(
      "display-notify",
      (ev) => {
        const p = ev.payload;
        if (p.slot !== slot) return;
        window.clearTimeout(noticeTimer.current);
        if (!p.text) {
          setNotice(null);
          return;
        }
        setNotice({ id: p.id, text: p.text, durationMs: p.durationMs });
        if (p.durationMs > 0) {
          noticeTimer.current = window.setTimeout(() => setNotice(null), p.durationMs);
        }
      },
    ).then((u) => (un = u));
    return () => {
      un?.();
      window.clearTimeout(noticeTimer.current);
    };
  }, [slot]);

  // Passage auto-advance: while enabled and there is a next section,
  // step forward on the interval. Any content change restarts the clock;
  // blank screens and the end of the passage stop it.
  useEffect(() => {
    if (autoAdvanceMs === 0) return;
    const blank = !view || view.blank || view.kind === "blank";
    if (blank || !view?.hasNext) return;
    const t = window.setTimeout(() => {
      invoke("slot_step", { slot, delta: 1 }).catch(console.error);
    }, autoAdvanceMs);
    return () => window.clearTimeout(t);
  }, [slot, autoAdvanceMs, view]);

  // Fire the configured transition when the displayed text changes.
  // Blanking doesn't animate; restoring re-triggers like fresh content.
  useEffect(() => {
    const blank = !view || view.blank || view.kind === "blank";
    const sig = blank ? "" : `${view.kind}|${view.label}|${view.title}|${view.lines.join("\u0001")}`;
    if (sig === contentSig.current) return;
    contentSig.current = sig;
    const t = view?.style?.transition ?? "none";
    if (!blank && (t === "fade" || t === "slide")) {
      setAnimClass(`out-anim-${t}`);
    } else {
      setAnimClass(null);
    }
  }, [view]);

  // Re-fit on window resize (monitor changes, fullscreen transitions).
  useEffect(() => {
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  // Auto-fit: start at the configured size, shrink until the text fits.
  // With a paired second column, both shrink together to one shared size.
  useEffect(() => {
    const kind = view?.kind;
    if (kind !== "scripture" && kind !== "lyrics" && kind !== "slide") return;
    const style = view?.style;
    const baseVw = kind === "lyrics" ? (style?.fontSize ?? 6.5) * 0.72 : style?.fontSize ?? 6.5;
    const els = [kind === "lyrics" ? lyricsRef.current : scriptureRef.current, pairRef.current].filter(
      (el): el is HTMLDivElement => !!el,
    );
    if (els.length === 0) return;
    let size = (baseVw / 100) * viewport.w;
    for (const el of els) el.style.fontSize = `${size}px`;
    // Two passes: fonts settle after first layout.
    requestAnimationFrame(() => {
      let guard = 0;
      while (
        guard < 60 &&
        els.some(
          (el) =>
            el.scrollHeight > window.innerHeight * 0.86 ||
            el.scrollWidth > window.innerWidth * 0.96,
        )
      ) {
        size *= 0.94;
        for (const el of els) el.style.fontSize = `${size}px`;
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
  const isText = !blank && (view.kind === "scripture" || view.kind === "lyrics" || view.kind === "slide");
  const alignLeft = style?.align === "left";
  const shadowCss = (style?.textShadow ?? true) ? "0 2px 14px rgba(0, 0, 0, 0.85)" : "none";

  return (
    <div
      className="output-root"
      style={{
        backgroundColor: bgColor,
        justifyContent: isText && alignLeft ? "flex-start" : "center",
      }}
    >
      {isText && style?.bgImage && (
        <div
          className="output-bg"
          style={{ backgroundImage: `url("${convertFileSrc(style.bgImage)}")` }}
        />
      )}

      {!blank && view.kind === "scripture" && view.pair && (
        <div
          className={"output-scripture output-pair" + (animClass ? " " + animClass : "")}
          onAnimationEnd={() => setAnimClass(null)}
          style={{ textAlign: alignLeft ? "left" : "center", paddingLeft: alignLeft ? "5vw" : undefined }}
        >
          <div className="output-pair-row">
            <div className="output-pair-col">
              <div
                ref={scriptureRef}
                className="output-text"
                style={{ fontFamily: font, fontSize: `${size}vw`, color: textColor, textShadow: shadowCss }}
              >
                {view.lines.map((l, i) => (
                  <span key={i}>
                    {l}{" "}
                  </span>
                ))}
              </div>
              <div className="output-pair-tag" style={{ color: textColor }}>
                {view.version ?? ""}
              </div>
            </div>
            <div className="output-pair-col">
              <div
                ref={pairRef}
                className="output-text"
                style={{ fontFamily: font, fontSize: `${size}vw`, color: textColor, textShadow: shadowCss }}
              >
                {view.pair.lines.map((l, i) => (
                  <span key={i}>
                    {l}{" "}
                  </span>
                ))}
              </div>
              <div className="output-pair-tag" style={{ color: textColor }}>
                {view.pair.version}
              </div>
            </div>
          </div>
          <div
            className="output-label"
            style={{ fontFamily: font, fontSize: `${size * 0.5}vw`, color: textColor, textShadow: shadowCss }}
          >
            {view.label}
          </div>
        </div>
      )}

      {!blank && (view.kind === "scripture" || view.kind === "slide") && !view.pair && (
        <div
          className={"output-scripture" + (animClass ? " " + animClass : "")}
          onAnimationEnd={() => setAnimClass(null)}
          style={{ textAlign: alignLeft ? "left" : "center", paddingLeft: alignLeft ? "7vw" : undefined }}
        >
          <div
            ref={scriptureRef}
            className="output-text"
            style={{ fontFamily: font, fontSize: `${size}vw`, color: textColor, textShadow: shadowCss }}
          >
            {view.kind === "slide"
              ? // Slides: one editor line = one displayed row (stacked).
                view.lines.map((l, i) => <div key={i}>{l || "\u00A0"}</div>)
              : // Scripture reads as flowing prose.
                view.lines.map((l, i) => (
                  <span key={i}>
                    {l}{" "}
                  </span>
                ))}
          </div>
          <div
            className="output-label"
            style={{ fontFamily: font, fontSize: `${size * 0.5}vw`, color: textColor, textShadow: shadowCss }}
          >
            {view.label}
          </div>
        </div>
      )}

      {!blank && view.kind === "lyrics" && (
        <div
          className={"output-lyrics" + (animClass ? " " + animClass : "")}
          onAnimationEnd={() => setAnimClass(null)}
          style={{ textAlign: alignLeft ? "left" : "center", paddingLeft: alignLeft ? "7vw" : undefined }}
        >
          <div
            ref={lyricsRef}
            className="output-lyric-lines"
            style={{ fontFamily: font, fontSize: `${size * 0.72}vw`, color: textColor, textShadow: shadowCss }}
          >
            {view.lines.map((l, i) => (
              <div key={i}>{l || "\u00A0"}</div>
            ))}
          </div>
          <div
            className="output-sublabel"
            style={{ fontFamily: font, fontSize: `${size * 0.32}vw`, color: textColor, textShadow: shadowCss }}
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

      {notice && (
        <div className="output-notice" key={notice.id} style={{ color: textColor }}>
          {notice.text}
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
