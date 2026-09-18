export type ItemType = "bible" | "song" | "hymn" | "book" | "document" | "slide";

export interface Section {
  label: string;
  lines: string[];
}

export interface ContentItem {
  id: string;
  itemType: ItemType;
  title: string;
  language: string;
  license: string;
  visibility: string;
  metadata: Record<string, unknown>;
  body: {
    sections?: Section[];
    chapters?: string[][];
    [key: string]: unknown;
  };
}

export interface ContentSummary {
  id: string;
  itemType: ItemType;
  title: string;
  language: string;
  license: string;
  visibility: string;
  createdAt: string;
}

export type SortMode = "title-asc" | "title-desc" | "added-desc" | "added-asc" | "canonical";
export type Testament = "ot" | "nt" | null;

export const SORT_LABELS: { value: SortMode; label: string }[] = [
  { value: "title-asc", label: "Title A → Z" },
  { value: "title-desc", label: "Title Z → A" },
  { value: "canonical", label: "Canonical order (Genesis → Revelation)" },
  { value: "added-desc", label: "Date added (newest first)" },
  { value: "added-asc", label: "Date added (oldest first)" },
];

export interface SearchHit {
  itemId: string;
  sectionKey: string;
  sectionLabel: string;
  snippet: string;
  rank: number;
  itemType: ItemType;
  itemTitle: string;
}

export interface LibraryStats {
  total: number;
  byType: Record<string, number>;
}

export const ITEM_TYPE_LABELS: Record<ItemType, string> = {
  bible: "Bible",
  song: "Song",
  hymn: "Hymn",
  book: "Book",
  document: "Document",
  slide: "Slide",
};

export const LICENSE_OPTIONS = [
  { value: "public-domain", label: "Public domain" },
  { value: "cc0", label: "CC0" },
  { value: "cc-by", label: "CC-BY" },
  { value: "cc-by-sa", label: "CC-BY-SA" },
  { value: "copyrighted", label: "Copyrighted" },
  { value: "unknown", label: "Unknown" },
];
