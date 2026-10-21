import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { notifyError } from "../../ui/toast";
import { type WidgetMode } from "../shared";
import { shortcutLabel, skipVariant, type ShortcutConfig } from "../../shortcut";




export const IGNORED_KEYS = new Set(["Meta", "Control", "Alt", "Shift"]);

export function ShortcutSection() {
  const [shortcut, setShortcut] = useState<ShortcutConfig | null>(null);
  const [listening, setListening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [doubleTapEnabled, setDoubleTapEnabled] = useState(false);
  const [doubleTapChecked, setDoubleTapChecked] = useState(false);

  useEffect(() => {
    invoke<ShortcutConfig>("get_shortcut")
      .then(setShortcut)
      .catch((err) => notifyError("get_shortcut", err));

    invoke<boolean>("get_double_tap_enabled")
      .then(setDoubleTapEnabled)
      .catch((err) => notifyError("get_double_tap_enabled", err))
      .finally(() => setDoubleTapChecked(true));
  }, []);

  function toggleDoubleTap(enabled: boolean) {
    setDoubleTapEnabled(enabled);
    invoke("set_double_tap_enabled", { enabled }).catch((err) => {
      notifyError("set_double_tap_enabled", err);
      setDoubleTapEnabled(!enabled);
    });
  }

  useEffect(() => {
    if (!listening) return;

    function onKeyDown(e: KeyboardEvent) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setListening(false);
        return;
      }
      if (IGNORED_KEYS.has(e.key)) return;

      const next: ShortcutConfig = {
        meta: e.metaKey,
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        code: e.code,
      };
      setListening(false);
      setError(null);
      invoke<ShortcutConfig>("set_shortcut", { newCfg: next })
        .then(setShortcut)
        .catch((err) => setError(String(err)));
    }

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [listening]);

  return (
    <div className="rounded-lg border border-base-content/10 bg-base-100 p-4">
      <h3 className="mb-3 text-xs font-semibold uppercase tracking-wide opacity-60">Push-to-talk key</h3>
      <div className="flex items-center gap-2">
        <button
          className={`btn btn-sm flex-1 ${listening ? "btn-primary" : ""}`}
          onClick={() => setListening(true)}
        >
          {listening ? "Press a key combo… (Esc to cancel)" : shortcut ? shortcutLabel(shortcut) : "…"}
        </button>
      </div>
      {error && <p className="mt-1.5 text-xs text-error">{error}</p>}
      {shortcut && (
        <p className="mt-1.5 text-xs opacity-60" data-testid="skip-shortcut-hint">
          {shortcutLabel(skipVariant(shortcut))} stops and pastes as-is, without LLM cleanup. Also works while it's
          refining, as does clicking the violet bar.
        </p>
      )}
      {doubleTapChecked && (
        <label className="mt-3 flex items-center gap-2 text-xs opacity-80">
          <input
            type="checkbox"
            className="checkbox checkbox-xs"
            checked={doubleTapEnabled}
            onChange={(e) => toggleDoubleTap(e.target.checked)}
          />
          Double-tap Fn to start/stop recording (in addition to the key above)
        </label>
      )}
      {doubleTapEnabled && (
        <p className="mt-1.5 text-xs opacity-60">
          Needs macOS's Input Monitoring permission — System Settings → Privacy &amp; Security → Input Monitoring.
        </p>
      )}
    </div>
  );
}

export function DeviceSection() {
  const [devices, setDevices] = useState<string[]>([]);
  const [activeDevice, setActiveDevice] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([
      invoke<string[]>("list_input_devices"),
      invoke<string | null>("get_active_input_device"),
    ])
      .then(([deviceList, active]) => {
        setDevices(deviceList);
        setActiveDevice(active);
      })
      .catch((err) => notifyError("Couldn't load audio devices", err))
      .finally(() => setLoading(false));
  }, []);

  function selectDevice(name: string) {
    setActiveDevice(name);
    invoke("set_input_device", { name }).catch((err) => notifyError("set_input_device", err));
  }

  return (
    <div className="rounded-lg border border-base-content/10 bg-base-100 p-4">
      <h3 className="mb-3 text-xs font-semibold uppercase tracking-wide opacity-60">Microphone</h3>
      {loading ? (
        <span className="loading loading-spinner loading-xs" />
      ) : devices.length === 0 ? (
        <p className="text-xs opacity-60">No input devices found.</p>
      ) : (
        <select
          className="select select-sm w-full"
          value={activeDevice ?? ""}
          onChange={(e) => selectDevice(e.target.value)}
        >
          {devices.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}

export const WIDGET_MODE_LABEL: Record<WidgetMode, string> = {
  minimal: "Minimal — icon only",
  compact: "Compact — status pill",
  detailed: "Detailed — full status panel",
};

export function GeneralSection() {
  const [autostart, setAutostart] = useState(false);
  const [widgetMode, setWidgetModeState] = useState<WidgetMode>("compact");
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    Promise.all([invoke<boolean>("get_autostart_enabled"), invoke<WidgetMode>("get_widget_mode")])
      .then(([autostartValue, widgetModeValue]) => {
        setAutostart(autostartValue);
        setWidgetModeState(widgetModeValue);
      })
      .catch((err) => notifyError("Couldn't load general settings", err))
      .finally(() => setChecked(true));
  }, []);

  function toggleAutostart(enabled: boolean) {
    setAutostart(enabled);
    invoke("set_autostart_enabled", { enabled }).catch((err) => {
      notifyError("set_autostart_enabled", err);
      setAutostart(!enabled);
    });
  }

  function changeWidgetMode(newMode: WidgetMode) {
    const previous = widgetMode;
    setWidgetModeState(newMode);
    invoke("set_widget_mode", { mode: newMode }).catch((err) => {
      notifyError("set_widget_mode", err);
      setWidgetModeState(previous);
    });
  }

  return (
    <div className="rounded-lg border border-base-content/10 bg-base-100 p-4">
      <h3 className="mb-3 text-xs font-semibold uppercase tracking-wide opacity-60">General</h3>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <div className="flex flex-col gap-3">
          <label className="flex items-center gap-2 text-xs">
            <input
              type="checkbox"
              className="checkbox checkbox-xs"
              checked={autostart}
              onChange={(e) => toggleAutostart(e.target.checked)}
            />
            Launch Dev Whisper at login
          </label>
          <label className="flex items-center gap-2 text-xs">
            Widget:
            <select
              className="select select-xs flex-1"
              value={widgetMode}
              onChange={(e) => changeWidgetMode(e.target.value as WidgetMode)}
            >
              {(Object.keys(WIDGET_MODE_LABEL) as WidgetMode[]).map((m) => (
                <option key={m} value={m}>
                  {WIDGET_MODE_LABEL[m]}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
    </div>
  );
}

/// Punctuation words that insert a literal character — kept to one
/// canonical phrase per symbol (`punctuation.rs`'s `COMMANDS` table has
/// more aliases than are useful to show here, e.g. "full stop" alongside
/// "period"). Order matches roughly how often each comes up dictating
/// code/prose, not alphabetical.
export const PUNCTUATION_COMMANDS: [string, string][] = [
  ["period", "."],
  ["comma", ","],
  ["question mark", "?"],
  ["exclamation point", "!"],
  ["colon", ":"],
  ["semicolon", ";"],
  ["new line", "↵"],
  ["new paragraph", "↵↵"],
  ["open paren", "("],
  ["close paren", ")"],
  ["dash", "–"],
  ["em dash", "—"],
  ["underscore", "_"],
  ["slash", "/"],
  ["backslash", "\\"],
  ["apostrophe", "'"],
  ["quotation mark", "“ ” (paired)"],
  ["asterisk", "*"],
  ["ampersand", "&"],
  ["at sign", "@"],
  ["hashtag", "#"],
  ["plus", "+"],
  ["minus", "-"],
  ["equals", "="],
  ["tilde", "~"],
  ["percent sign", "%"],
  ["degree sign", "°"],
  ["copyright", "©"],
  ["trademark", "™"],
];

export type VoiceCommandGroup = {
  title: string;
  description: string;
  examples: { say: string; get: string }[];
};

/// Cross-cutting trigger phrases: work in any app, regardless of Mode, and
/// resolve instantly without waiting on the LLM (except Boilerplate, which
/// is the one group that needs Ollama). Kept as one reference list so a
/// user hearing "why did my dictation turn into a checklist" or "why did
/// that get pasted in ALL_CAPS" has one place to check rather than having
/// to ask.
export const VOICE_COMMAND_GROUPS: VoiceCommandGroup[] = [
  {
    title: "Casing & syntax",
    description: "Say a case name, then what to convert. Instant, no LLM involved.",
    examples: [
      { say: '"snake case error response handler"', get: "error_response_handler" },
      { say: '"camel case error response handler"', get: "errorResponseHandler" },
      { say: '"pascal case error response handler"', get: "ErrorResponseHandler" },
      { say: '"kebab case error response handler"', get: "error-response-handler" },
      { say: '"title case error response handler"', get: "Error Response Handler" },
      { say: '"screaming snake case max retry count"', get: "MAX_RETRY_COUNT" },
    ],
  },
  {
    title: "Numbered lists",
    description: 'Count out loud ("one... two... three...", or "first... second...") and it becomes a real list.',
    examples: [{ say: '"one set up the repo two install deps three run tests"', get: "1. set up the repo\n2. install deps\n3. run tests" }],
  },
  {
    title: "Self-correction (Backtrack)",
    description: 'Correct yourself mid-dictation instead of stopping and restarting — everything before the trigger is discarded.',
    examples: [
      { say: '"call the client at three scratch that four"', get: "four" },
      { say: '"let\'s do coffee at two, actually three"', get: "three" },
    ],
  },
  {
    title: "Boilerplate generation",
    description: 'Say one of these, then describe what to generate. Sent to your local LLM instead of pasted as text — needs Ollama running.',
    examples: [
      {
        say: '"generate boilerplate for a React component called UserCard with name and avatar props"',
        get: "the generated component, pasted directly",
      },
    ],
  },
  {
    title: "Snippets",
    description: 'Speak a saved trigger phrase on its own and it expands to its full saved body. Manage the list from Vocabulary → Snippets.',
    examples: [{ say: '"pr checklist"', get: "the saved PR checklist template, pasted directly" }],
  },
  {
    title: "Press enter",
    description:
      'Say this at the end to send Enter after pasting. "Send it" (or "submit it") does the same, but only after a pause or period, so "…and send it" in a sentence stays text. Off by default — enable it in General above.',
    examples: [
      { say: '"looks good press enter"', get: 'pastes "looks good", then Enter' },
      { say: '"fix the failing test. send it."', get: 'pastes "Fix the failing test.", then Enter' },
    ],
  },
  {
    title: "Attach the error",
    description:
      'Say this at the end to append the latest error output as a code block: the visible text of the terminal you\'re in (Terminal, iTerm), otherwise the clipboard if it holds an error. "With the error" works after a pause. If nothing is found, a note appears and just your words are pasted.',
    examples: [{ say: '"why does this test fail, attach the error"', get: "your sentence, then the stack trace in a ``` block" }],
  },
  {
    title: "Scratch that (undo)",
    description:
      'Say it on its own to take back the last paste: Cmd+Z in most apps; in a terminal, exactly as many backspaces as were pasted (single line, within 15 seconds, not sent). Anywhere else it does nothing and says so. Mid-sentence it is still a correction.',
    examples: [{ say: '"scratch that"', get: "the last pasted text is removed" }],
  },
  {
    title: "Append clipboard",
    description: 'Say this at the end to append whatever you last copied — a stack trace, a URL, a snippet — without reading it aloud.',
    examples: [{ say: '"here\'s the error append clipboard"', get: "here's the error <clipboard contents>" }],
  },
];

export function VoiceCommandsSection() {
  return (
    <div className="rounded-lg border border-base-content/10 bg-base-100 p-4">
      <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide opacity-60">Voice Commands</h3>
      <p className="mb-3 text-xs opacity-60">
        Trigger phrases recognized in any app, in any Mode — if dictation ever came out looking unexpectedly
        transformed, check here first.
      </p>
      <div className="flex flex-col gap-4">
        {VOICE_COMMAND_GROUPS.map((group) => (
          <div key={group.title}>
            <p className="text-xs font-medium">{group.title}</p>
            <p className="mb-1.5 text-xs opacity-60">{group.description}</p>
            <div className="flex flex-col gap-1">
              {group.examples.map((ex) => (
                <p key={ex.say} className="text-xs">
                  <code className="opacity-80">{ex.say}</code>
                  <span className="opacity-50"> {"→"} </span>
                  <span className="opacity-80">{ex.get}</span>
                </p>
              ))}
            </div>
          </div>
        ))}
        <div>
          <p className="text-xs font-medium">Punctuation & symbols</p>
          <p className="mb-1.5 text-xs opacity-60">Say the word, get the character — works mid-sentence, not just at the end.</p>
          <div className="grid grid-cols-2 gap-x-3 gap-y-1">
            {PUNCTUATION_COMMANDS.map(([word, symbol]) => (
              <div key={word} className="flex items-center justify-between text-xs">
                <code className="opacity-80">{word}</code>
                <span className="opacity-60">{symbol}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
