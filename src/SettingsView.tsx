import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type ShortcutConfig = {
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  code: string;
};

type ModelStatus = {
  id: string;
  label: string;
  size_mb: number;
  downloaded: boolean;
  active: boolean;
};

function prettyCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

function shortcutLabel(cfg: ShortcutConfig): string {
  let s = "";
  if (cfg.ctrl) s += "⌃";
  if (cfg.alt) s += "⌥";
  if (cfg.shift) s += "⇧";
  if (cfg.meta) s += "⌘";
  return s + prettyCode(cfg.code);
}

const IGNORED_KEYS = new Set(["Meta", "Control", "Alt", "Shift"]);

function ShortcutSection() {
  const [shortcut, setShortcut] = useState<ShortcutConfig | null>(null);
  const [listening, setListening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ShortcutConfig>("get_shortcut")
      .then(setShortcut)
      .catch((err) => console.error("get_shortcut failed:", err));
  }, []);

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
    <div className="mb-4">
      <label className="mb-1 block text-xs font-medium opacity-70">Push-to-talk key</label>
      <div className="flex items-center gap-2">
        <button
          className={`btn btn-sm flex-1 ${listening ? "btn-primary" : ""}`}
          onClick={() => setListening(true)}
        >
          {listening ? "Press a key combo… (Esc to cancel)" : shortcut ? shortcutLabel(shortcut) : "…"}
        </button>
      </div>
      {error && <p className="mt-1 text-xs text-error">{error}</p>}
    </div>
  );
}

function ModelsSection() {
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  function refresh() {
    invoke<ModelStatus[]>("list_models")
      .then(setModels)
      .catch((err) => console.error("list_models failed:", err));
  }

  useEffect(() => {
    refresh();

    const unlistenProgress = listen<{ id: string; percent: number }>("model-download-progress", (e) => {
      setProgress((p) => ({ ...p, [e.payload.id]: e.payload.percent }));
    });
    const unlistenDone = listen<string>("model-download-complete", (e) => {
      setProgress((p) => {
        const next = { ...p };
        delete next[e.payload];
        return next;
      });
      refresh();
    });
    const unlistenError = listen<{ id: string; error: string }>("model-download-error", (e) => {
      setProgress((p) => {
        const next = { ...p };
        delete next[e.payload.id];
        return next;
      });
      setErrors((er) => ({ ...er, [e.payload.id]: e.payload.error }));
    });

    return () => {
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, []);

  function download(id: string) {
    setErrors((er) => {
      const next = { ...er };
      delete next[id];
      return next;
    });
    invoke("download_model", { id }).catch((err) => setErrors((er) => ({ ...er, [id]: String(err) })));
  }

  function activate(id: string) {
    invoke("set_active_model", { id })
      .then(refresh)
      .catch((err) => setErrors((er) => ({ ...er, [id]: String(err) })));
  }

  return (
    <div className="mb-4">
      <label className="mb-1 block text-xs font-medium opacity-70">Whisper model</label>
      <ul className="flex flex-col gap-1.5">
        {models.map((m) => (
          <li key={m.id} className="flex items-center justify-between rounded-md bg-base-100 px-2.5 py-1.5 text-xs">
            <div>
              <div className="font-medium">{m.label}</div>
              <div className="opacity-50">{m.size_mb}MB</div>
            </div>
            {m.active ? (
              <span className="badge badge-success badge-sm">Active</span>
            ) : m.id in progress ? (
              <span className="w-10 text-right opacity-70">{progress[m.id]}%</span>
            ) : m.downloaded ? (
              <button className="btn btn-xs" onClick={() => activate(m.id)}>
                Use
              </button>
            ) : (
              <button className="btn btn-xs" onClick={() => download(m.id)}>
                Download
              </button>
            )}
          </li>
        ))}
      </ul>
      {Object.entries(errors).map(([id, err]) => (
        <p key={id} className="mt-1 text-xs text-error">
          {err}
        </p>
      ))}
    </div>
  );
}

function DeviceSection() {
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
      .catch((err) => console.error("failed to load audio devices:", err))
      .finally(() => setLoading(false));
  }, []);

  function selectDevice(name: string) {
    setActiveDevice(name);
    invoke("set_input_device", { name }).catch((err) => console.error("set_input_device failed:", err));
  }

  return (
    <div className="mb-4">
      <label className="mb-1 block text-xs font-medium opacity-70">Microphone</label>
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

type Mode = "plain" | "casual" | "cli";
const MODES: Mode[] = ["plain", "casual", "cli"];
const MODE_LABEL: Record<Mode, string> = { plain: "Plain", casual: "Casual", cli: "CLI" };

type AppModeRule = { bundle_id: string; app_name: string; mode: Mode };
type FrontmostApp = { bundle_id: string; name: string };

function AppModesSection() {
  const [rules, setRules] = useState<AppModeRule[]>([]);
  const [lastApp, setLastApp] = useState<FrontmostApp | null>(null);
  const [newRuleMode, setNewRuleMode] = useState<Mode>("cli");

  function refresh() {
    invoke<AppModeRule[]>("get_mode_rules")
      .then(setRules)
      .catch((err) => console.error("get_mode_rules failed:", err));
  }

  useEffect(() => {
    refresh();
    invoke<FrontmostApp | null>("get_last_frontmost_app")
      .then(setLastApp)
      .catch((err) => console.error("get_last_frontmost_app failed:", err));
  }, []);

  function addRule(bundleId: string, appName: string, mode: Mode) {
    invoke("set_mode_rule", { bundleId, appName, mode })
      .then(refresh)
      .catch((err) => console.error("set_mode_rule failed:", err));
  }

  function updateRule(rule: AppModeRule, mode: Mode) {
    addRule(rule.bundle_id, rule.app_name, mode);
  }

  function removeRule(bundleId: string) {
    invoke("remove_mode_rule", { bundleId })
      .then(refresh)
      .catch((err) => console.error("remove_mode_rule failed:", err));
  }

  const lastAppAlreadyRuled = lastApp && rules.some((r) => r.bundle_id === lastApp.bundle_id);

  return (
    <div className="mb-4">
      <label className="mb-1 block text-xs font-medium opacity-70">App modes</label>

      {rules.length > 0 && (
        <ul className="mb-2 flex flex-col gap-1.5">
          {rules.map((r) => (
            <li key={r.bundle_id} className="flex items-center justify-between rounded-md bg-base-100 px-2.5 py-1.5 text-xs">
              <span className="truncate">{r.app_name}</span>
              <div className="flex items-center gap-1.5">
                <select
                  className="select select-xs"
                  value={r.mode}
                  onChange={(e) => updateRule(r, e.target.value as Mode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {MODE_LABEL[m]}
                    </option>
                  ))}
                </select>
                <button className="btn btn-ghost btn-xs" onClick={() => removeRule(r.bundle_id)} aria-label={`Remove rule for ${r.app_name}`}>
                  ✕
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {lastApp && !lastAppAlreadyRuled && (
        <div className="flex items-center gap-1.5 text-xs">
          <span className="flex-1 truncate opacity-70">Add rule for {lastApp.name}?</span>
          <select
            className="select select-xs"
            value={newRuleMode}
            onChange={(e) => setNewRuleMode(e.target.value as Mode)}
          >
            {MODES.map((m) => (
              <option key={m} value={m}>
                {MODE_LABEL[m]}
              </option>
            ))}
          </select>
          <button className="btn btn-xs" onClick={() => addRule(lastApp.bundle_id, lastApp.name, newRuleMode)}>
            Add
          </button>
        </div>
      )}

      {rules.length === 0 && !lastApp && (
        <p className="text-xs opacity-60">
          Switch to another app, then reopen Settings to add a mode for it.
        </p>
      )}
    </div>
  );
}

function SettingsView() {
  return (
    <main className="min-h-screen bg-base-300 px-5 py-4 text-base-content">
      <h1 className="mb-4 text-base font-semibold">Settings</h1>
      <DeviceSection />
      <ShortcutSection />
      <ModelsSection />
      <AppModesSection />
    </main>
  );
}

export default SettingsView;
