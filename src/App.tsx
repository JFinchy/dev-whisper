import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type RecordingState = "idle" | "recording";

function App() {
  const [recordingState, setRecordingState] = useState<RecordingState>("idle");
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    const unlistenStart = listen("recording-started", () => setRecordingState("recording"));
    const unlistenStop = listen("recording-stopped", (event) => {
      setRecordingState("idle");
      console.log("recording saved to", event.payload);
    });
    const unlistenError = listen<string>("recording-error", (event) => {
      setRecordingState("idle");
      console.error("recording error:", event.payload);
    });

    return () => {
      unlistenStart.then((f) => f());
      unlistenStop.then((f) => f());
      unlistenError.then((f) => f());
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
          recordingState === "recording" ? "bg-white/15" : "bg-white/8 hover:bg-white/15"
        }`}
        onClick={toggleRecording}
        aria-label={recordingState === "idle" ? "Start recording" : "Stop recording"}
      >
        <span
          className={`h-2.5 w-2.5 rounded-full transition-colors ${
            recordingState === "recording" ? "bg-error shadow-[0_0_8px] shadow-error" : "bg-neutral-content/40"
          }`}
        />
      </button>
      <span className="flex-1 truncate text-sm opacity-75">
        {recordingState === "idle" ? "Ready" : "Listening…"}
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
