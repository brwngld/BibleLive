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
  kind: "scripture" | "lyrics" | "image" | "video" | "blank";
  title: string;
  label: string;
  lines: string[];
  page: string | null;
  imagePath: string | null;
  videoPath: string | null;
  blank: boolean;
  style: SlotStyle;
};

export interface SlotStyle {
  fontFamily: string;
  fontSize: number; // vw units on the output screen
  textColor: string;
  bgColor: string;
}

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
  content: SlotContent;
}

export interface DisplayProfile {
  name: string;
  slots: { monitor: string | null; mode: "auto" | "manual" | "lock" }[];
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
