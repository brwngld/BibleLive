import { invoke } from "@tauri-apps/api/core";
export type { UnlistenFn } from "@tauri-apps/api/event";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface AudioDeviceInfo {
  name: string;
  isDefault: boolean;
  sampleRate: number | null;
  channels: number | null;
}

export interface VoiceConfig {
  device: string | null;
  contentType: "speech" | "mixed";
  vadThreshold: number;
  /** Rolling window for live transcription while speech continues (ms). */
  partialWindowMs: number;
}

export interface AudioTestResult {
  device: string;
  peakLevel: number;
  speechDetected: boolean;
  transcript: string;
  seconds: number;
}

export interface Suggestion {
  id: string;
  kind: "reference" | "quote";
  label: string;
  itemId: string;
  sectionKey: string;
  confidence: number;
  status: "pending" | "shown" | "ignored";
  /** Actual text that would be projected — verify before SHOW. */
  preview: string;
}

export interface ModelStatus {
  path: string;
  exists: boolean;
  sizeMb: number | null;
}

export const voiceApi = {
  listDevices: () => invoke<AudioDeviceInfo[]>("list_audio_devices"),
  getConfig: () => invoke<VoiceConfig>("get_voice_config"),
  setConfig: (config: VoiceConfig) => invoke("set_voice_config", { config }),
  getMode: () => invoke<string>("get_listen_mode"),
  setMode: (mode: string) => invoke("set_listen_mode", { mode }),
  listeningStatus: () => invoke<boolean>("listening_status"),
  start: () => invoke("start_listening"),
  stop: () => invoke("stop_listening"),
  audioTest: () => invoke<AudioTestResult>("audio_test"),
  suggestions: () => invoke<Suggestion[]>("list_suggestions"),
  respond: (id: string, show: boolean) =>
    invoke<Suggestion>("respond_suggestion", { id, show }),
  undoAutoShow: (id: string) => invoke("undo_auto_show", { id }),
  modelStatus: () => invoke<ModelStatus>("model_status"),
  sttModel: () => invoke<string>("get_stt_model"),
  setSttModel: (model: string) => invoke("set_stt_model", { model }),
  /** "auto" (first AUTO slot) or a pinned "1".."5". */
  autoTarget: () => invoke<string>("get_voice_auto_target"),
  setAutoTarget: (target: string) => invoke("set_voice_auto_target", { target }),
  diagnostics: (seconds = 10) =>
    invoke<Record<string, unknown>>("run_voice_diagnostics", { seconds }),
};

export interface VoiceDiagnostics {
  build: string;
  device: string;
  sampleRate: number;
  channels: number;
  sampleFormat: string;
  deviceError: string | null;
  vadThreshold: number;
  contentType: string;
  rawPeak: number;
  gainApplied: number;
  levelMax: number;
  levelSeries: number[];
  segments: number;
  segmentSamples: number[];
  modelPath: string;
  transcript: string;
  notes: string[];
}

export interface TranscriptEvent {
  text: string;
}
export interface SuggestionEvent {
  suggestion: Suggestion;
  mode: string;
}
/** An Automatic-mode match projected itself; Undo is offered briefly. */
export interface AutoShownEvent {
  id: string;
  slot: number;
  undoMs: number;
}

export function onTranscript(cb: (e: TranscriptEvent) => void): Promise<UnlistenFn> {
  return listen<TranscriptEvent>("voice-transcript", (ev) => cb(ev.payload));
}
export function onPartialTranscript(cb: (e: TranscriptEvent) => void): Promise<UnlistenFn> {
  return listen<TranscriptEvent>("voice-transcript-partial", (ev) => cb(ev.payload));
}

export function onSuggestion(cb: (e: SuggestionEvent) => void): Promise<UnlistenFn> {
  return listen<SuggestionEvent>("voice-suggestion", (ev) => cb(ev.payload));
}
export function onAutoShown(cb: (e: AutoShownEvent) => void): Promise<UnlistenFn> {
  return listen<AutoShownEvent>("voice-auto-shown", (ev) => cb(ev.payload));
}

export function onLevel(cb: (level: number) => void): Promise<UnlistenFn> {
  return listen<number>("voice-level", (ev) => cb(ev.payload));
}
