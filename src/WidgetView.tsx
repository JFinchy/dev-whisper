import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

type Phase = "idle" | "recording" | "transcribing" | "refining";
type WidgetMode = "minimal" | "compact" | "detailed";

const STATUS_LABEL: Record<Phase, string> = {
  idle: "Ready",
  recording: "Listening…",
  transcribing: "Transcribing…",
  refining: "Refining…",
};

const COMPACT_BASE_SIZE = { width: 220, height: 60 };
// How tall compact grows while an error is showing, so the full message
// is readable instead of being cut off by `truncate` — that silently ate
// the Accessibility-permission error before this fix.
const COMPACT_EXPANDED_SIZE = { width: 220, height: 108 };

function WidgetView() {
  const [mode, setMode] = useState<WidgetMode>("compact");
  const [phase, setPhase] = useState<Phase>("idle");
  // `message` is sticky — it holds whatever the last status/error text
  // was until something new replaces it. `flashVisible` controls whether
  // compact/minimal show it as a temporary overlay vs. falling back to
  // the plain phase label; detailed mode ignores `flashVisible` and
  // always shows `message`, since persistence is the point of that mode.
  const [message, setMessage] = useState<string | null>(null);
  const [isError, setIsError] = useState(false);
  const [flashVisible, setFlashVisible] = useState(false);
  const flashTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    invoke<WidgetMode>("get_widget_mode")
      .then(setMode)
      .catch((err) => console.error("get_widget_mode failed:", err));

    function announce(text: string, error: boolean) {
      setMessage(text);
      setIsError(error);
      setFlashVisible(true);
      if (flashTimeout.current) clearTimeout(flashTimeout.current);
      // Errors stay up longer than a plain "Pasted" confirmation — they're
      // the ones worth actually reading.
      flashTimeout.current = setTimeout(() => setFlashVisible(false), error ? 6000 : 2000);
    }

    const unlistenModeChanged = listen<WidgetMode>("widget-mode-changed", (e) => setMode(e.payload));
    const unlistenStart = listen("recording-started", () => setPhase("recording"));
    const unlistenStop = listen("recording-stopped", () => setPhase("transcribing"));
    const unlistenRefining = listen("refining-started", () => setPhase("refining"));
    const unlistenRecordingError = listen<string>("recording-error", (event) => {
      setPhase("idle");
      announce(event.payload, true);
    });
    const unlistenTranscriptReady = listen<string>("transcript-ready", () => {
      setPhase("idle");
      announce("Pasted", false);
    });
    const unlistenTranscriptError = listen<string>("transcript-error", (event) => {
      setPhase("idle");
      announce(event.payload, true);
    });

    return () => {
      unlistenModeChanged.then((f) => f());
      unlistenStart.then((f) => f());
      unlistenStop.then((f) => f());
      unlistenRefining.then((f) => f());
      unlistenRecordingError.then((f) => f());
      unlistenTranscriptReady.then((f) => f());
      unlistenTranscriptError.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (mode !== "compact") return;
    const size = flashVisible && isError ? COMPACT_EXPANDED_SIZE : COMPACT_BASE_SIZE;
    invoke("set_widget_size", size).catch((err) => console.error("set_widget_size failed:", err));
  }, [mode, flashVisible, isError]);

  function toggleRecording() {
    invoke("toggle_recording_command").catch((err) => console.error("toggle_recording_command failed:", err));
  }

  function openSettings() {
    invoke("open_settings").catch((err) => console.error("open_settings failed:", err));
  }

  function startDrag(e: React.MouseEvent) {
    if (e.buttons === 1) {
      getCurrentWindow()
        .startDragging()
        .catch((err) => console.error("startDragging failed:", err));
    }
  }

  const busy = phase === "transcribing" || phase === "refining";
  const flashing = flashVisible && message !== null;

  const recordButton = (
    <button
      className={`btn btn-circle border-none ${mode === "minimal" ? "btn-md" : "btn-sm"} ${
        phase === "recording" ? "bg-white/15" : "bg-white/8 hover:bg-white/15"
      } ${flashing && isError ? "ring-2 ring-error" : ""}`}
      onClick={toggleRecording}
      disabled={busy}
      aria-label={phase === "idle" ? "Start recording" : "Stop recording"}
      title={message ?? STATUS_LABEL[phase]}
    >
      {busy ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <span
          className={`rounded-full transition-colors ${mode === "minimal" ? "h-3.5 w-3.5" : "h-2.5 w-2.5"} ${
            phase === "recording" ? "bg-error shadow-[0_0_8px] shadow-error" : "bg-neutral-content/40"
          }`}
        />
      )}
    </button>
  );

  if (mode === "minimal") {
    return (
      <main
        className="flex h-screen w-screen cursor-grab items-center justify-center rounded-full border border-white/10 bg-neutral/90 backdrop-blur-md active:cursor-grabbing"
        onMouseDown={startDrag}
      >
        {recordButton}
      </main>
    );
  }

  if (mode === "detailed") {
    return (
      <main className="flex h-screen flex-col gap-1.5 rounded-2xl border border-white/10 bg-neutral/90 p-2.5 text-neutral-content backdrop-blur-md">
        <div className="flex items-center gap-2">
          {recordButton}
          <span
            className="flex-1 cursor-grab truncate text-sm font-medium active:cursor-grabbing"
            onMouseDown={startDrag}
          >
            {STATUS_LABEL[phase]}
          </span>
          <button
            className="btn btn-ghost btn-circle h-7 w-7 min-h-0"
            onClick={openSettings}
            aria-label="Open settings"
          >
            ⚙
          </button>
        </div>
        <p
          className={`flex-1 overflow-y-auto whitespace-pre-wrap text-xs ${
            isError ? "text-error" : "opacity-60"
          }`}
        >
          {message ?? "Ready to dictate. Press the hotkey or click the button to start."}
        </p>
      </main>
    );
  }

  // Compact
  return (
    <main className="flex h-screen flex-col justify-center gap-1 rounded-2xl border border-white/10 bg-neutral/90 px-2.5 text-neutral-content backdrop-blur-md">
      <div className="flex items-center gap-2">
        {recordButton}
        <span
          className={`flex-1 cursor-grab text-sm opacity-75 active:cursor-grabbing ${
            flashing && isError ? "whitespace-normal" : "truncate"
          }`}
          onMouseDown={startDrag}
        >
          {flashing ? message : STATUS_LABEL[phase]}
        </span>
        <button
          className="btn btn-ghost btn-circle h-7 w-7 min-h-0"
          onClick={openSettings}
          aria-label="Open settings"
        >
          ⚙
        </button>
      </div>
    </main>
  );
}

export default WidgetView;
