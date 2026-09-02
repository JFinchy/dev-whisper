import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { applyTheme, type ThemeId } from "./theme";

type Phase = "idle" | "recording" | "transcribing" | "refining";
type WidgetMode = "minimal" | "compact" | "detailed";
// Only the name is needed here — the flyout just offers a pick list, the
// backend resolves the full mode (behavior/models) by name when the
// override is consumed. See `modes::ModeDefinition` for the full shape.
type ModeSummary = { name: string };

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
// How tall compact grows for the hover flyout (mic mode + a one-off mode
// override for the next dictation), sized for exactly one row of
// "Next dictation" buttons. Grows downward rather than upward — this
// window has no titlebar to anchor from, and downward reuses the exact
// resize mechanism COMPACT_EXPANDED_SIZE already relies on instead of
// introducing new position math.
const COMPACT_FLYOUT_BASE_HEIGHT = 168;
// "Next dictation" is one full-width button per mode (see .dw-fly-col in
// App.css) rather than a single cramped row, so the flyout has to grow
// with however many modes exist — matches that CSS's row height + gap.
const FLYOUT_MODE_ROW_HEIGHT = 26;
const FLYOUT_MODE_ROW_GAP = 4;

function compactFlyoutSize(modeCount: number) {
  const extraRows = Math.max(modeCount, 1) - 1;
  return {
    width: 220,
    height: COMPACT_FLYOUT_BASE_HEIGHT + extraRows * (FLYOUT_MODE_ROW_HEIGHT + FLYOUT_MODE_ROW_GAP),
  };
}

// How many recent level samples the meter shows at once — enough to read
// as a little waveform rather than a single flickering bar, short enough
// that the whole strip reacts quickly to a pause or a loud word.
const LEVEL_BARS = 7;
const SILENT_LEVELS = Array<number>(LEVEL_BARS).fill(0);

// Small live "is audio actually coming through" indicator, in the spirit
// of the waveform most other dictation apps show while listening —
// `levels` is the most recent LEVEL_BARS samples from the backend's
// `audio-level` event (oldest first), each independently bar-height'd
// rather than smoothed into one shape, so it visibly reacts to loud vs.
// quiet speech instead of just pulsing.
function LevelMeter({ levels, className }: { levels: number[]; className?: string }) {
  return (
    <div className={`flex h-full items-center gap-[2px] ${className ?? ""}`}>
      {levels.map((level, i) => (
        <span
          key={i}
          className="w-[3px] shrink-0 rounded-full bg-error transition-[height] duration-100 ease-out"
          style={{ height: `${Math.max(level * 100, 15)}%` }}
        />
      ))}
    </div>
  );
}

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

  // Quick-actions hover flyout state (compact mode only, for now) — see
  // the `.dw-fly-*` styles in App.css. `micMode`/`overrideMode` mirror
  // real backend state (isolated_voice_enabled / the one-shot mode
  // override), not local-only UI state.
  const [flyoutHovered, setFlyoutHovered] = useState(false);
  const [micMode, setMicMode] = useState(false);
  const [overrideMode, setOverrideMode] = useState<string | null>(null);
  const [modes, setModes] = useState<ModeSummary[]>([]);
  // Set once per launch if the backend's startup check (see lib.rs) finds
  // a newer release; null means either "no update" or "haven't heard back
  // yet" — both render the same (no badge), so they don't need telling
  // apart here.
  const [updateAvailable, setUpdateAvailable] = useState<string | null>(null);

  // Rolling window of recent input levels for the live level meter — see
  // LevelMeter above. Reset to silence whenever recording isn't active, so
  // a stale "last thing you said was loud" reading doesn't linger once
  // idle/transcribing.
  const [levels, setLevels] = useState<number[]>(SILENT_LEVELS);

  useEffect(() => {
    if (phase !== "recording") setLevels(SILENT_LEVELS);
  }, [phase]);

  useEffect(() => {
    invoke<WidgetMode>("get_widget_mode")
      .then(setMode)
      .catch((err) => console.error("get_widget_mode failed:", err));

    invoke<boolean>("get_isolated_voice_enabled")
      .then(setMicMode)
      .catch((err) => console.error("get_isolated_voice_enabled failed:", err));

    invoke<ModeSummary[]>("get_modes")
      .then(setModes)
      .catch((err) => console.error("get_modes failed:", err));

    invoke<ThemeId>("get_theme")
      .then(applyTheme)
      .catch((err) => console.error("get_theme failed:", err));
    const unlistenTheme = listen<ThemeId>("theme-changed", (e) => applyTheme(e.payload));

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
    const unlistenLevel = listen<number>("audio-level", (e) => {
      setLevels((prev) => [...prev.slice(1), e.payload]);
    });
    const unlistenRecordingError = listen<string>("recording-error", (event) => {
      setPhase("idle");
      announce(event.payload, true);
    });
    const unlistenTranscriptReady = listen<string>("transcript-ready", () => {
      setPhase("idle");
      announce("Pasted", false);
      // The backend consumes the override the moment it's used (see
      // recording.rs's `mode_override.take()`) — clear the pill so it
      // doesn't look like it's still armed for the next dictation.
      setOverrideMode(null);
    });
    const unlistenTranscriptError = listen<string>("transcript-error", (event) => {
      setPhase("idle");
      setOverrideMode(null);
      announce(event.payload, true);
    });
    const unlistenUpdateAvailable = listen<string>("update-available", (event) => {
      setUpdateAvailable(event.payload);
    });

    return () => {
      unlistenTheme.then((f) => f());
      unlistenModeChanged.then((f) => f());
      unlistenStart.then((f) => f());
      unlistenStop.then((f) => f());
      unlistenRefining.then((f) => f());
      unlistenLevel.then((f) => f());
      unlistenRecordingError.then((f) => f());
      unlistenTranscriptReady.then((f) => f());
      unlistenTranscriptError.then((f) => f());
      unlistenUpdateAvailable.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (mode !== "compact") return;
    // The flyout takes priority over the error-expansion size — hovering
    // to check mic mode mid-error is a rare enough overlap that showing
    // the flyout (rather than fighting over which one "wins" the window
    // height) is the simpler, still-correct choice.
    const size = flyoutHovered
      ? compactFlyoutSize(modes.length)
      : flashVisible && isError
        ? COMPACT_EXPANDED_SIZE
        : COMPACT_BASE_SIZE;
    invoke("set_widget_size", size).catch((err) => console.error("set_widget_size failed:", err));
  }, [mode, flashVisible, isError, flyoutHovered, modes.length]);

  function toggleRecording() {
    invoke("toggle_recording_command").catch((err) => console.error("toggle_recording_command failed:", err));
  }

  function openSettings() {
    invoke("open_settings").catch((err) => console.error("open_settings failed:", err));
  }

  function toggleMicMode(next: boolean) {
    setMicMode(next);
    invoke("set_isolated_voice_enabled", { enabled: next }).catch((err) => {
      console.error("set_isolated_voice_enabled failed:", err);
      setMicMode(!next);
    });
  }

  function pickOverride(next: string) {
    // Clicking the already-selected pill clears it back to "auto" rather
    // than being stuck — this is a one-off nudge for the very next
    // dictation, not a persistent setting.
    const nextValue = overrideMode === next ? null : next;
    setOverrideMode(nextValue);
    invoke("set_next_mode_override", { modeName: nextValue }).catch((err) =>
      console.error("set_next_mode_override failed:", err),
    );
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
          className={`rounded-full transition-transform duration-100 ${
            mode === "minimal" ? "h-3.5 w-3.5" : "h-2.5 w-2.5"
          } ${phase === "recording" ? "bg-error shadow-[0_0_8px] shadow-error" : "bg-neutral-content/40"}`}
          // Minimal mode has no room for a multi-bar meter, so instead the
          // dot itself pulses with the latest input level — same "is audio
          // actually coming through" signal, scaled to fit a 46x46 icon.
          style={
            mode === "minimal" && phase === "recording"
              ? { transform: `scale(${1 + (levels[levels.length - 1] ?? 0) * 0.6})` }
              : undefined
          }
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
          {phase === "recording" ? (
            <div className="h-4 flex-1 cursor-grab active:cursor-grabbing" onMouseDown={startDrag}>
              <LevelMeter levels={levels} />
            </div>
          ) : (
            <span
              className="flex-1 cursor-grab truncate text-sm font-medium active:cursor-grabbing"
              onMouseDown={startDrag}
            >
              {STATUS_LABEL[phase]}
            </span>
          )}
          <button
            className="btn btn-ghost btn-circle relative h-7 w-7 min-h-0"
            onClick={openSettings}
            aria-label={updateAvailable ? `Open settings — update to v${updateAvailable} available` : "Open settings"}
            title={updateAvailable ? `Update to v${updateAvailable} available` : undefined}
          >
            ⚙
            {updateAvailable && (
              <span className="absolute right-0.5 top-0.5 h-2 w-2 rounded-full bg-accent" />
            )}
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
    <main
      className={`flex h-screen flex-col gap-1 rounded-2xl border border-white/10 bg-neutral/90 px-2.5 py-2 text-neutral-content backdrop-blur-md ${
        flyoutHovered ? "justify-start" : "justify-center"
      }`}
      onMouseEnter={() => setFlyoutHovered(true)}
      onMouseLeave={() => setFlyoutHovered(false)}
    >
      <div className="flex items-center gap-2">
        {recordButton}
        {phase === "recording" ? (
          <div className="h-4 flex-1 cursor-grab active:cursor-grabbing" onMouseDown={startDrag}>
            <LevelMeter levels={levels} />
          </div>
        ) : (
          <span
            className={`flex-1 cursor-grab text-sm opacity-75 active:cursor-grabbing ${
              flashing && isError ? "whitespace-normal" : "truncate"
            }`}
            onMouseDown={startDrag}
          >
            {flashing ? message : STATUS_LABEL[phase]}
          </span>
        )}
        <button
          className="btn btn-ghost btn-circle relative h-7 w-7 min-h-0"
          onClick={openSettings}
          aria-label={updateAvailable ? `Open settings — update to v${updateAvailable} available` : "Open settings"}
          title={updateAvailable ? `Update to v${updateAvailable} available` : undefined}
        >
          ⚙
          {updateAvailable && <span className="absolute right-0.5 top-0.5 h-2 w-2 rounded-full bg-accent" />}
        </button>
      </div>
      {flyoutHovered && (
        <div className="dw-fly-body">
          <div className="dw-fly-label">Mic mode</div>
          <div className="dw-fly-row">
            <button
              type="button"
              className={`dw-fly-btn ${!micMode ? "sel" : ""}`}
              onClick={() => toggleMicMode(false)}
            >
              Standard
            </button>
            <button
              type="button"
              className={`dw-fly-btn ${micMode ? "sel" : ""}`}
              onClick={() => toggleMicMode(true)}
            >
              Voice Isolation
            </button>
          </div>
          <div className="dw-fly-label">Next dictation</div>
          <div className="dw-fly-col">
            {modes.map((m) => (
              <button
                key={m.name}
                type="button"
                className={`dw-fly-btn ${overrideMode === m.name ? "sel" : ""}`}
                onClick={() => pickOverride(m.name)}
                title={overrideMode === m.name ? "Click again to use the auto-detected mode" : undefined}
              >
                {m.name}
              </button>
            ))}
          </div>
        </div>
      )}
    </main>
  );
}

export default WidgetView;
