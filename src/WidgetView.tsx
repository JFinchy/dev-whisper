import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

type Phase = "idle" | "recording" | "transcribing";

const STATUS_LABEL: Record<Phase, string> = {
  idle: "Ready",
  recording: "Listening…",
  transcribing: "Transcribing…",
};

function WidgetView() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [flashMessage, setFlashMessage] = useState<string | null>(null);

  useEffect(() => {
    function flash(message: string) {
      setFlashMessage(message);
      setTimeout(() => setFlashMessage(null), 2000);
    }

    const unlistenStart = listen("recording-started", () => setPhase("recording"));
    const unlistenStop = listen("recording-stopped", () => setPhase("transcribing"));
    const unlistenRecordingError = listen<string>("recording-error", (event) => {
      setPhase("idle");
      flash(event.payload);
    });
    const unlistenTranscriptReady = listen<string>("transcript-ready", () => {
      setPhase("idle");
      flash("Pasted");
    });
    const unlistenTranscriptError = listen<string>("transcript-error", (event) => {
      setPhase("idle");
      flash(event.payload);
    });

    return () => {
      unlistenStart.then((f) => f());
      unlistenStop.then((f) => f());
      unlistenRecordingError.then((f) => f());
      unlistenTranscriptReady.then((f) => f());
      unlistenTranscriptError.then((f) => f());
    };
  }, []);

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

  return (
    <main className="flex h-screen items-center gap-2 rounded-2xl border border-white/10 bg-neutral/90 px-2.5 text-neutral-content backdrop-blur-md">
      <button
        className={`btn btn-circle btn-sm border-none ${
          phase === "recording" ? "bg-white/15" : "bg-white/8 hover:bg-white/15"
        }`}
        onClick={toggleRecording}
        disabled={phase === "transcribing"}
        aria-label={phase === "idle" ? "Start recording" : "Stop recording"}
      >
        {phase === "transcribing" ? (
          <span className="loading loading-spinner loading-xs" />
        ) : (
          <span
            className={`h-2.5 w-2.5 rounded-full transition-colors ${
              phase === "recording" ? "bg-error shadow-[0_0_8px] shadow-error" : "bg-neutral-content/40"
            }`}
          />
        )}
      </button>
      {/* Drag handle: the buttons need full-area clicks, so only this
          middle strip initiates a window drag. */}
      <span
        className="flex-1 cursor-grab truncate text-sm opacity-75 active:cursor-grabbing"
        onMouseDown={startDrag}
      >
        {flashMessage ?? STATUS_LABEL[phase]}
      </span>
      <button
        className="btn btn-ghost btn-circle h-7 w-7 min-h-0"
        onClick={openSettings}
        aria-label="Open settings"
      >
        ⚙
      </button>
    </main>
  );
}

export default WidgetView;
