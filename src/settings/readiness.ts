import { invoke } from "@tauri-apps/api/core";

// Mirrors `readiness::Check` in the backend.
export type ReadinessLevel = "ok" | "warn" | "error";
export type ReadinessFix =
  | "download_model"
  | "grant_accessibility"
  | "change_shortcut"
  | "choose_microphone"
  | "allow_microphone"
  | "open_microphone_settings"
  | "start_ollama"
  | "pull_llm_model";
export type ReadinessCheck = {
  id: string;
  level: ReadinessLevel;
  title: string;
  detail: string;
  fix: ReadinessFix | null;
};

export const FIX_LABEL: Record<ReadinessFix, string> = {
  download_model: "Choose a model",
  grant_accessibility: "Open System Settings",
  change_shortcut: "Change hotkey",
  choose_microphone: "Choose microphone",
  allow_microphone: "Allow microphone",
  open_microphone_settings: "Open System Settings",
  start_ollama: "How to start Ollama",
  pull_llm_model: "Download model",
};

export function getReadiness(): Promise<ReadinessCheck[]> {
  return invoke<ReadinessCheck[]>("get_readiness");
}

/// The most severe problem, for one-line summaries (the widget).
export function topProblem(checks: ReadinessCheck[]): ReadinessCheck | null {
  return checks.find((c) => c.level === "error") ?? checks.find((c) => c.level === "warn") ?? null;
}
