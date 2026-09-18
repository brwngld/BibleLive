import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface MonitorInfo {
  name: string;
  width: number;
  height: number;
  x: number;
  y: number;
  isPrimary: boolean;
}

export type SlotContent = {
  slot: number;
  kind: "scripture" | "lyrics" | "slide" | "image" | "video" | "blank";
  title: string;
  label: string;
  lines: string[];
  page: string | null;
  imagePath: string | null;
  videoPath: string | null;
  blank: boolean;
  style: SlotStyle;
  /** Translation tag of the primary column ("KJV"), present when paired. */
  version?: string;
  /** Second column: the same verse in the paired translation. */
  pair?: { version: string; label: string; lines: string[] };
};

export interface SlotStyle {
  fontFamily: string;
  fontSize: number; // vw units on the output screen
  textColor: string;
  bgColor: string;
  /** Optional background image (absolute file path) behind the text. */
  bgImage?: string | null;
  /** Text block alignment: "center" (default) | "left". */
  align?: "center" | "left";
  /** Soft shadow behind text for legibility over busy images. */
  textShadow?: boolean;
  /** Transition when the content changes: "none" | "fade" | "slide". */
  transition?: "none" | "fade" | "slide";
}

/** Presentation-type presets: style patches applied together. A preset
 *  never touches the background image — that is chosen separately. */
export const THEME_PRESETS: { name: string; hint: string; style: Partial<SlotStyle> }[] = [
  {
    name: "Classic",
    hint: "Serif · centered · white on black",
    style: {
      fontFamily: "Georgia, 'Times New Roman', serif",
      fontSize: 6.5,
      textColor: "#ffffff",
      bgColor: "#000000",
      align: "center",
      textShadow: true,
    },
  },
  {
    name: "Cathedral",
    hint: "Serif · warm parchment on deep blue · shadowed · fades between verses",
    style: {
      fontFamily: "'Book Antiqua', 'Palatino Linotype', serif",
      fontSize: 6.0,
      textColor: "#f6efdc",
      bgColor: "#0a1230",
      align: "center",
      textShadow: true,
      transition: "fade",
    },
  },
  {
    name: "Modern",
    hint: "Clean sans · left-aligned · light on charcoal · slides between verses",
    style: {
      fontFamily: "'Segoe UI', sans-serif",
      fontSize: 5.5,
      textColor: "#f2f4f8",
      bgColor: "#14161a",
      align: "left",
      textShadow: false,
      transition: "slide",
    },
  },
  {
    name: "Lantern",
    hint: "Large serif · amber on near-black · shadowed · fades between verses",
    style: {
      fontFamily: "Cambria, serif",
      fontSize: 7.5,
      textColor: "#ffd98e",
      bgColor: "#0b0805",
      align: "center",
      textShadow: true,
      transition: "fade",
    },
  },
  {
    name: "Bulletin",
    hint: "Compact sans · left column · white on slate",
    style: {
      fontFamily: "Verdana, sans-serif",
      fontSize: 4.8,
      textColor: "#ffffff",
      bgColor: "#1f2a38",
      align: "left",
      textShadow: false,
    },
  },
];

export const FONT_OPTIONS = [
  { value: "Georgia, 'Times New Roman', serif", label: "Georgia (serif)" },
  { value: "'Times New Roman', serif", label: "Times New Roman" },
  { value: "Cambria, serif", label: "Cambria" },
  { value: "'Book Antiqua', 'Palatino Linotype', serif", label: "Book Antiqua" },
  { value: "Arial, sans-serif", label: "Arial" },
  { value: "'Segoe UI', sans-serif", label: "Segoe UI" },
  { value: "Verdana, sans-serif", label: "Verdana" },
  { value: "Tahoma, sans-serif", label: "Tahoma" },
  { value: "'Trebuchet MS', sans-serif", label: "Trebuchet MS" },
  { value: "Calibri, sans-serif", label: "Calibri" },
  { value: "'Comic Sans MS', cursive", label: "Comic Sans MS" },
  { value: "'Courier New', monospace", label: "Courier New" },
];

export interface SlotView {
  slot: number;
  monitor: string | null;
  mode: "auto" | "manual" | "lock";
  blank: boolean;
  windowOpen: boolean;
  degraded: boolean;
  active: boolean;
  /** Second Bible version shown beside scripture: null | "kjv" | "asv". */
  pairVersion: string | null;
  content: SlotContent;
}

export interface DisplayProfile {
  name: string;
  slots: { monitor: string | null; mode: "auto" | "manual" | "lock" }[];
}

/** A named, reusable output look saved from a display's style panel. */
export interface ThemeTemplate {
  name: string;
  style: SlotStyle;
}

export const displayApi = {
  monitors: () => invoke<MonitorInfo[]>("list_monitors"),
  slots: () => invoke<SlotView[]>("get_display_slots"),
  setMonitor: (slot: number, monitor: string | null) =>
    invoke("set_slot_monitor", { slot, monitor }),
  setMode: (slot: number, mode: string) => invoke("set_slot_mode", { slot, mode }),
  setBlank: (slot: number, blank: boolean) => invoke("set_slot_blank", { slot, blank }),
  openOutput: (slot: number) => invoke("open_slot_output", { slot }),
  closeOutput: (slot: number) => invoke("close_slot_output", { slot }),
  setScripture: (slot: number, itemId: string, keys: string[]) =>
    invoke("set_slot_scripture", { input: { slot, itemId, keys } }),
  setPair: (slot: number, version: string | null) =>
    invoke("set_slot_pair", { input: { slot, version } }),
  templates: () => invoke<ThemeTemplate[]>("list_theme_templates"),
  saveTemplate: (name: string, style: SlotStyle) =>
    invoke("save_theme_template", { name, style }),
  deleteTemplate: (name: string) => invoke("delete_theme_template", { name }),
  setSection: (slot: number, itemId: string, key: string) =>
    invoke("set_slot_section", { input: { slot, itemId, key } }),
  setMedia: (slot: number, input: { title?: string; imagePath?: string; videoPath?: string }) =>
    invoke("set_slot_media", { input: { slot, ...input } }),
  step: (slot: number, delta: number) => invoke("slot_step", { slot, delta }),
  moveToNextMonitor: (slot: number) => invoke("move_slot_output_to_next_monitor", { slot }),
  setStyle: (slot: number, style: SlotStyle) => invoke("set_slot_style", { slot, style }),
  setActive: (slot: number) => invoke("set_active_display", { slot }),
  profiles: () => invoke<DisplayProfile[]>("list_display_profiles"),
  saveProfile: (name: string) => invoke("save_display_profile", { name }),
  applyProfile: (name: string) => invoke("apply_display_profile", { name }),
  deleteProfile: (name: string) => invoke("delete_display_profile", { name }),
};

export function onDisplayUpdate(cb: (v: SlotView) => void): Promise<UnlistenFn> {
  return listen<SlotView>("display-update", (ev) => cb(ev.payload));
}
