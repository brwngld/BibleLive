import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";

export interface SessionMeta {
  id: string;
  name: string;
  startedAt: string;
  endedAt: string | null;
}

export interface SessionItemRow {
  at: string;
  slot: number;
  kind: string;
  title: string;
  label: string;
}

export interface QueueEntry {
  id: number;
  itemId: string;
  key: string;
  label: string;
  title: string;
  kind: string; // "scripture" | "slide" | "lyrics"
}

export const queueApi = {
  list: () => invoke<QueueEntry[]>("list_queue"),
  addReference: (text: string) => invoke<string>("add_queue_reference", { text }),
  addItem: (item: { itemId: string; key: string; label: string; title: string; kind: string }) =>
    invoke("add_queue_item", { item }),
  remove: (id: number) => invoke("remove_queue_item", { id }),
  move: (id: number, delta: number) => invoke("move_queue_item", { id, delta }),
  clear: () => invoke("clear_queue"),
  show: (id: number, slot: number) => invoke("show_queue_item", { id, slot }),
};

export const serviceApi = {
  current: () => invoke<SessionMeta | null>("current_service_session"),
  start: (name: string) => invoke<SessionMeta>("start_service_session", { name }),
  end: () => invoke<SessionMeta | null>("end_service_session"),
  list: () => invoke<SessionMeta[]>("list_service_sessions"),
  items: (id: string) => invoke<SessionItemRow[]>("get_service_session_items", { id }),
  remove: (id: string) => invoke("delete_service_session", { id }),
  blankAll: (blank: boolean) => invoke("set_all_displays_blank", { blank }),
};

export type { UnlistenFn };
