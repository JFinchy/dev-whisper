

export type ModelStatus = {
  id: string;
  label: string;
  size_mb: number;
  downloaded: boolean;
  active: boolean;
};

export type Behavior = "plain" | "casual" | "cli";

export const BEHAVIORS: Behavior[] = ["plain", "casual", "cli"];

export const BEHAVIOR_LABEL: Record<Behavior, string> = { plain: "Plain", casual: "Casual", cli: "CLI" };

export type LlmModelStatus = {
  id: string;
  label: string;
  size_gb: number;
  downloaded: boolean;
  last_latency_ms: number | null;
};

export type WidgetMode = "minimal" | "compact" | "detailed" | "pill" | "notch" | "caption" | "stacked";
