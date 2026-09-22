import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ContentItem,
  ContentSummary,
  LibraryStats,
  SearchHit,
} from "./types";

export async function getStats(): Promise<LibraryStats> {
  return invoke<LibraryStats>("library_stats");
}

export async function createContent(item: ContentItem): Promise<void> {
  return invoke("create_content", { item });
}

export async function listContent(filter: {
  itemType?: string | null;
  titleQuery?: string | null;
  testament?: string | null;
  sort?: string;
}): Promise<ContentSummary[]> {
  return invoke<ContentSummary[]>("list_content", {
    filter: {
      itemType: filter.itemType ?? null,
      titleQuery: filter.titleQuery ?? null,
      testament: filter.testament ?? null,
      sort: filter.sort ?? "title-asc",
      limit: 500,
      offset: 0,
    },
  });
}

export async function getContent(id: string): Promise<ContentItem> {
  return invoke<ContentItem>("get_content", { id });
}

export async function updateContent(item: ContentItem): Promise<void> {
  return invoke("update_content", { item });
}

export async function deleteContent(id: string): Promise<void> {
  return invoke("delete_content", { id });
}

export async function searchContent(
  query: string,
  limit = 50,
): Promise<SearchHit[]> {
  return invoke<SearchHit[]>("search_content", { query, limit });
}

export async function importText(input: {
  title: string;
  itemType: string;
  text: string;
  language: string;
  license: string;
}): Promise<ContentItem> {
  return invoke<ContentItem>("import_text_content", { input });
}

export async function importFile(input: {
  path: string;
  title?: string | null;
  itemType?: string | null;
  language: string;
  license: string;
}): Promise<ContentItem> {
  return invoke<ContentItem>("import_file_content", { input });
}

export async function pickFile(): Promise<string | null> {
  const path = await open({
    multiple: false,
    filters: [
      { name: "Importable content", extensions: ["txt", "md", "docx", "xml"] },
      { name: "Song XML (OpenLyrics / OpenSong)", extensions: ["xml"] },
    ],
  });
  return typeof path === "string" ? path : null;
}
