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
