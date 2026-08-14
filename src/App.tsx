import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type Phase = "idle" | "recording" | "transcribing";

const STATUS_LABEL: Record<Phase, string> = {
  idle: "Ready",
  recording: "Listening…",
  transcribing: "Transcribing…",
};

function App() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [settingsOpen, setSettingsOpen] = useState(false);
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
    invoke("toggle_recording_command");
  }

  if (settingsOpen) {
    return (
      <main className="flex h-screen flex-col gap-1.5 rounded-2xl border border-white/10 bg-neutral/90 px-3.5 py-2.5 text-neutral-content backdrop-blur-md [-webkit-app-region:drag]">
        <div className="mb-1 flex items-center justify-between font-semibold">
          <span>Settings</span>
          <button
            className="btn btn-ghost btn-xs btn-circle [-webkit-app-region:no-drag]"
            onClick={() => setSettingsOpen(false)}
            aria-label="Close settings"
          >
            ✕
          </button>
        </div>
        <div className="flex justify-between text-xs opacity-75">
          <span>Push-to-talk key</span>
          <span className="opacity-50">⌘⇧Space</span>
        </div>
        <div className="flex justify-between text-xs opacity-75">
          <span>Whisper model</span>
          <span className="opacity-50">base</span>
        </div>
      </main>
    );
  }

  return (
    <main className="flex h-screen items-center gap-2.5 rounded-2xl border border-white/10 bg-neutral/90 px-3.5 text-neutral-content backdrop-blur-md [-webkit-app-region:drag]">
      <button
        className={`btn btn-circle btn-sm border-none [-webkit-app-region:no-drag] ${
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
      <span className="flex-1 truncate text-sm opacity-75">
        {flashMessage ?? STATUS_LABEL[phase]}
      </span>
      <button
        className="btn btn-ghost btn-xs btn-circle [-webkit-app-region:no-drag]"
        onClick={() => setSettingsOpen(true)}
        aria-label="Open settings"
      >
        ⚙
      </button>
    </main>
  );
}

export default App;
