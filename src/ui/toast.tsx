import { useEffect, useState } from "react";

// A minimal toast store: any module can call `notifyError`, and the one
// <Toaster /> mounted in the Settings window renders whatever is queued.
// Exists so a failed save is visible instead of a silent console.error
// that makes it look like the change stuck.

export type Toast = { id: number; title: string; detail?: string };

let nextId = 1;
let toasts: Toast[] = [];
const listeners = new Set<(t: Toast[]) => void>();

function publish(next: Toast[]) {
  toasts = next;
  listeners.forEach((l) => l(toasts));
}

export function dismissToast(id: number) {
  publish(toasts.filter((t) => t.id !== id));
}

export function showToast(title: string, detail?: string) {
  const toast = { id: nextId++, title, detail };
  // Repeats of the same failure (a section retrying) collapse into one.
  publish([...toasts.filter((t) => t.title !== title), toast].slice(-3));
  setTimeout(() => dismissToast(toast.id), 7000);
}

const VERBS: Record<string, string> = {
  get: "load",
  list: "load",
  set: "save",
  update: "save",
  delete: "delete",
  clear: "clear",
  download: "download",
  pull: "download",
  send: "send",
  start: "start",
  stop: "stop",
  open: "open",
  search: "search",
  reprocess: "reprocess",
};

/// "set_input_device" → "Couldn't save input device".
export function describeFailure(command: string): string {
  const [verb, ...rest] = command.split("_");
  const action = VERBS[verb] ?? verb;
  const subject = rest.join(" ").replace(/\benabled\b/, "setting").trim();
  return `Couldn't ${action}${subject ? ` ${subject}` : ""}`;
}

/// Logs and surfaces a failed backend call. `command` is the Tauri command
/// name, or a sentence for anything that isn't one.
export function notifyError(command: string, err: unknown) {
  console.error(`${command} failed:`, err);
  const title = command.includes(" ") ? command : describeFailure(command);
  showToast(title, err instanceof Error ? err.message : String(err));
}

export function Toaster() {
  const [items, setItems] = useState<Toast[]>(toasts);
  useEffect(() => {
    listeners.add(setItems);
    return () => {
      listeners.delete(setItems);
    };
  }, []);

  if (items.length === 0) return null;
  return (
    <div className="dw-toasts" role="status" aria-live="polite">
      {items.map((t) => (
        <div key={t.id} className="dw-toast" data-testid="toast">
          <div className="dw-toast-text">
            <strong>{t.title}</strong>
            {t.detail && <span>{t.detail}</span>}
          </div>
          <button type="button" className="dw-toast-close" aria-label="Dismiss" onClick={() => dismissToast(t.id)}>
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
