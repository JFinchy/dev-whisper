import { useEffect, useState, type ReactElement } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme, THEME_ORDER, THEME_LABEL, THEMES, type ThemeId } from "./theme";

function IconMic() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <rect x="9" y="3" width="6" height="11" rx="3" />
      <path d="M5 11a7 7 0 0 0 14 0" />
      <path d="M12 18v3" />
      <path d="M9 21h6" />
    </svg>
  );
}
function IconWave() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M3 12h2M7 8v8M11 4v16M15 8v8M19 10v4M21 12h0" />
    </svg>
  );
}
function IconWindow() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M3 9h18" />
    </svg>
  );
}
function IconCpu() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <rect x="6" y="6" width="12" height="12" rx="2" />
      <path d="M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3" />
    </svg>
  );
}
function IconPlug() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M9 3v5M15 3v5M6 8h12v3a6 6 0 0 1-12 0V8Z" />
      <path d="M12 17v4" />
    </svg>
  );
}
function IconGear() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3v2M12 19v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M3 12h2M19 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4" />
    </svg>
  );
}
function IconBook() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v16H6.5A2.5 2.5 0 0 0 4 21.5V5.5Z" />
      <path d="M4 19a2.5 2.5 0 0 1 2.5-2.5H20" />
    </svg>
  );
}
function IconClock() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 3" />
    </svg>
  );
}
function IconPalette() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M12 3a9 9 0 1 0 0 18c1.1 0 2-.9 2-2 0-.5-.2-1-.5-1.4-.3-.3-.5-.8-.5-1.3 0-1.1.9-2 2-2h2.3A4.2 4.2 0 0 0 21 12c0-5-4-9-9-9Z" />
      <circle cx="7.5" cy="10.5" r="1.1" fill="currentColor" stroke="none" />
      <circle cx="12" cy="7.5" r="1.1" fill="currentColor" stroke="none" />
      <circle cx="16.5" cy="10.5" r="1.1" fill="currentColor" stroke="none" />
    </svg>
  );
}

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

type EnrollmentStatus = {
  enrolled: boolean;
  enrolled_at_ms: number | null;
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
  const [doubleTapEnabled, setDoubleTapEnabled] = useState(false);
  const [doubleTapChecked, setDoubleTapChecked] = useState(false);

  useEffect(() => {
    invoke<ShortcutConfig>("get_shortcut")
      .then(setShortcut)
      .catch((err) => console.error("get_shortcut failed:", err));

    invoke<boolean>("get_double_tap_enabled")
      .then(setDoubleTapEnabled)
      .catch((err) => console.error("get_double_tap_enabled failed:", err))
      .finally(() => setDoubleTapChecked(true));
  }, []);

  function toggleDoubleTap(enabled: boolean) {
    setDoubleTapEnabled(enabled);
    invoke("set_double_tap_enabled", { enabled }).catch((err) => {
      console.error("set_double_tap_enabled failed:", err);
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
      {doubleTapChecked && (
        <label className="mt-2 flex items-center gap-1.5 text-xs opacity-80">
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
        <p className="mt-1 text-xs opacity-60">
          Needs macOS's Input Monitoring permission — System Settings → Privacy &amp; Security → Input Monitoring.
        </p>
      )}
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
    <div className="mb-4 border-t border-base-content/10 pt-3">
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
const GLOBAL_MODEL = "__global__";

type AppModeRule = {
  bundle_id: string;
  app_name: string;
  mode: Mode;
  stt_model: string | null;
  use_llm_refinement: boolean;
};
type FrontmostApp = { bundle_id: string; name: string; icon_data_uri: string | null };
type RunningApp = { bundle_id: string; name: string; icon_data_uri: string | null; is_running: boolean };

function AppIcon({ src, name }: { src: string | null; name: string }) {
  if (src) {
    return <img src={src} alt="" className="h-4 w-4 shrink-0 rounded-sm" />;
  }
  // Fallback for apps whose icon couldn't be resolved: first letter avatar.
  return (
    <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-sm bg-base-300 text-[9px] font-semibold opacity-60">
      {name.charAt(0).toUpperCase()}
    </span>
  );
}

function AppModesSection() {
  const [rules, setRules] = useState<AppModeRule[]>([]);
  const [lastApp, setLastApp] = useState<FrontmostApp | null>(null);
  const [runningApps, setRunningApps] = useState<RunningApp[]>([]);
  const [availableModels, setAvailableModels] = useState<ModelStatus[]>([]);
  const [pickerOpen, setPickerOpen] = useState(false);

  function refresh() {
    invoke<AppModeRule[]>("get_mode_rules")
      .then(setRules)
      .catch((err) => console.error("get_mode_rules failed:", err));
  }

  function loadRunningApps() {
    invoke<RunningApp[]>("list_running_apps")
      .then(setRunningApps)
      .catch((err) => console.error("list_running_apps failed:", err));
  }

  useEffect(() => {
    refresh();
    loadRunningApps();
    invoke<FrontmostApp | null>("get_last_frontmost_app")
      .then(setLastApp)
      .catch((err) => console.error("get_last_frontmost_app failed:", err));
    invoke<ModelStatus[]>("list_models")
      .then(setAvailableModels)
      .catch((err) => console.error("list_models failed:", err));
  }, []);

  function addRule(bundleId: string, appName: string, mode: Mode) {
    invoke("set_mode_rule", { bundleId, appName, mode, sttModel: null, useLlmRefinement: false })
      .then(refresh)
      .catch((err) => console.error("set_mode_rule failed:", err));
  }

  function updateRule(rule: AppModeRule, patch: Partial<AppModeRule>) {
    const next = { ...rule, ...patch };
    invoke("set_mode_rule", {
      bundleId: next.bundle_id,
      appName: next.app_name,
      mode: next.mode,
      sttModel: next.stt_model,
      useLlmRefinement: next.use_llm_refinement,
    })
      .then(refresh)
      .catch((err) => console.error("set_mode_rule failed:", err));
  }

  function removeRule(bundleId: string) {
    invoke("remove_mode_rule", { bundleId })
      .then(refresh)
      .catch((err) => console.error("remove_mode_rule failed:", err));
  }

  const lastAppAlreadyRuled = lastApp && rules.some((r) => r.bundle_id === lastApp.bundle_id);
  const pickableApps = runningApps.filter((a) => !rules.some((r) => r.bundle_id === a.bundle_id));
  const ruleIcon = (bundleId: string) => runningApps.find((a) => a.bundle_id === bundleId)?.icon_data_uri ?? null;

  function pickApp(app: RunningApp) {
    addRule(app.bundle_id, app.name, "cli");
    setPickerOpen(false);
  }

  return (
    <div className="mb-4">
      <label className="mb-1 block text-xs font-medium opacity-70">App modes</label>

      {rules.length > 0 && (
        <ul className="mb-2 flex flex-col gap-1.5">
          {rules.map((r) => (
            <li key={r.bundle_id} className="rounded-md bg-base-100 px-2.5 py-1.5 text-xs">
              <div className="mb-1 flex items-center justify-between">
                <span className="flex items-center gap-1.5 truncate font-medium">
                  <AppIcon src={ruleIcon(r.bundle_id)} name={r.app_name} />
                  {r.app_name}
                </span>
                <div className="flex items-center gap-1.5">
                  <select
                    className="select select-xs"
                    value={r.mode}
                    onChange={(e) => updateRule(r, { mode: e.target.value as Mode })}
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
              </div>
              <div className="flex items-center justify-between opacity-70">
                <label className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    className="checkbox checkbox-xs"
                    checked={r.use_llm_refinement}
                    onChange={(e) => updateRule(r, { use_llm_refinement: e.target.checked })}
                  />
                  Refine with LLM
                </label>
                <select
                  className="select select-xs"
                  value={r.stt_model ?? GLOBAL_MODEL}
                  onChange={(e) =>
                    updateRule(r, { stt_model: e.target.value === GLOBAL_MODEL ? null : e.target.value })
                  }
                >
                  <option value={GLOBAL_MODEL}>Global model</option>
                  {availableModels.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label}
                    </option>
                  ))}
                </select>
              </div>
            </li>
          ))}
        </ul>
      )}

      {lastApp && !lastAppAlreadyRuled && (
        <div className="mb-1.5 flex items-center gap-1.5 text-xs">
          <button
            className="btn btn-xs flex-1 justify-start gap-1.5"
            onClick={() => addRule(lastApp.bundle_id, lastApp.name, "cli")}
          >
            <AppIcon src={lastApp.icon_data_uri} name={lastApp.name} />+ Add rule for {lastApp.name}
          </button>
        </div>
      )}

      <div className="relative">
        <button
          className="btn btn-xs w-full justify-start"
          onClick={() => {
            setPickerOpen((open) => !open);
            loadRunningApps();
          }}
        >
          Browse apps…
        </button>
        {pickerOpen && (
          <ul className="absolute z-10 mt-1 max-h-52 w-full overflow-y-auto rounded-md bg-base-100 p-1 shadow-lg">
            {pickableApps.length === 0 && (
              <li className="px-2 py-1 text-xs opacity-50">Loading…</li>
            )}
            {pickableApps.map((a, i) => {
              const prevRunning = i > 0 ? pickableApps[i - 1].is_running : true;
              return (
                <li key={a.bundle_id}>
                  {prevRunning && !a.is_running && (
                    <div className="mt-1 border-t border-base-content/10 pt-1 text-[10px] uppercase opacity-40">
                      Other apps
                    </div>
                  )}
                  <button
                    className="flex w-full items-center gap-1.5 rounded px-2 py-1 text-left text-xs hover:bg-base-200"
                    onClick={() => pickApp(a)}
                  >
                    <AppIcon src={a.icon_data_uri} name={a.name} />
                    <span className="truncate">{a.name}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}

function VocabularySection() {
  const [terms, setTerms] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<string[]>("get_vocabulary")
      .then(setTerms)
      .catch((err) => console.error("get_vocabulary failed:", err))
      .finally(() => setLoading(false));
  }, []);

  function save(next: string[]) {
    setTerms(next);
    invoke("set_vocabulary", { terms: next }).catch((err) => console.error("set_vocabulary failed:", err));
  }

  function addTerm() {
    const term = draft.trim();
    if (!term || terms.includes(term)) {
      setDraft("");
      return;
    }
    save([...terms, term]);
    setDraft("");
  }

  function removeTerm(term: string) {
    save(terms.filter((t) => t !== term));
  }

  return (
    <div className="mb-4 border-t border-base-content/10 pt-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Vocabulary</label>
      {loading ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <>
          <div className="mb-1.5 flex flex-wrap gap-1">
            {terms.map((term) => (
              <span key={term} className="badge badge-sm gap-1">
                {term}
                <button
                  className="opacity-60 hover:opacity-100"
                  onClick={() => removeTerm(term)}
                  aria-label={`Remove ${term}`}
                >
                  ✕
                </button>
              </span>
            ))}
            {terms.length === 0 && <p className="text-xs opacity-60">No terms yet.</p>}
          </div>
          <div className="flex gap-1.5">
            <input
              className="input input-sm flex-1"
              placeholder="Add a term (e.g. kubectl)"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") addTerm();
              }}
            />
            <button className="btn btn-sm" onClick={addTerm}>
              Add
            </button>
          </div>
        </>
      )}
    </div>
  );
}

type Snippet = { trigger: string; body: string };

function SnippetsSection() {
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [loading, setLoading] = useState(true);
  const [triggerDraft, setTriggerDraft] = useState("");
  const [bodyDraft, setBodyDraft] = useState("");
  const [editingIndex, setEditingIndex] = useState<number | null>(null);

  useEffect(() => {
    invoke<Snippet[]>("get_snippets")
      .then(setSnippets)
      .catch((err) => console.error("get_snippets failed:", err))
      .finally(() => setLoading(false));
  }, []);

  function save(next: Snippet[]) {
    setSnippets(next);
    invoke("set_snippets", { snippets: next }).catch((err) => console.error("set_snippets failed:", err));
  }

  function startEdit(index: number) {
    setEditingIndex(index);
    setTriggerDraft(snippets[index].trigger);
    setBodyDraft(snippets[index].body);
  }

  function cancelEdit() {
    setEditingIndex(null);
    setTriggerDraft("");
    setBodyDraft("");
  }

  function submit() {
    const trigger = triggerDraft.trim();
    const body = bodyDraft.trim();
    if (!trigger || !body) return;

    if (editingIndex !== null) {
      const next = snippets.slice();
      next[editingIndex] = { trigger, body };
      save(next);
    } else {
      if (snippets.some((s) => s.trigger.toLowerCase() === trigger.toLowerCase())) return;
      save([...snippets, { trigger, body }]);
    }
    cancelEdit();
  }

  function removeSnippet(index: number) {
    save(snippets.filter((_, i) => i !== index));
    if (editingIndex === index) cancelEdit();
  }

  return (
    <div className="mb-4 border-t border-base-content/10 pt-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Snippets</label>
      <p className="mb-1.5 text-xs opacity-60">
        Say a trigger phrase by itself (e.g. "PR checklist") to paste its saved text instead.
      </p>
      {loading ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <>
          <div className="mb-2 flex flex-col gap-1.5">
            {snippets.map((s, i) => (
              <div key={i} className="flex items-start justify-between gap-2 rounded bg-base-100 px-2 py-1.5">
                <button className="flex-1 text-left" onClick={() => startEdit(i)}>
                  <div className="text-xs font-medium">{s.trigger}</div>
                  <div className="truncate text-xs opacity-60">{s.body.split("\n")[0]}</div>
                </button>
                <button
                  className="opacity-60 hover:opacity-100"
                  onClick={() => removeSnippet(i)}
                  aria-label={`Remove ${s.trigger}`}
                >
                  ✕
                </button>
              </div>
            ))}
            {snippets.length === 0 && <p className="text-xs opacity-60">No snippets yet.</p>}
          </div>
          <div className="flex flex-col gap-1.5">
            <input
              className="input input-sm"
              placeholder="Trigger phrase (e.g. standup update)"
              value={triggerDraft}
              onChange={(e) => setTriggerDraft(e.target.value)}
            />
            <textarea
              className="textarea textarea-sm"
              placeholder="Text to paste"
              rows={3}
              value={bodyDraft}
              onChange={(e) => setBodyDraft(e.target.value)}
            />
            <div className="flex gap-1.5">
              <button className="btn btn-sm flex-1" onClick={submit}>
                {editingIndex !== null ? "Save" : "Add"}
              </button>
              {editingIndex !== null && (
                <button className="btn btn-sm" onClick={cancelEdit}>
                  Cancel
                </button>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

type HistoryEntry = {
  timestamp_ms: number;
  text: string;
  app_name: string | null;
  mode: string | null;
  summary: string | null;
};

const RETENTION_OPTIONS = [7, 30, 90, 365];

function formatTimestamp(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

type ReprocessState = {
  mode: Mode;
  useLlm: boolean;
  loading: boolean;
  result: string | null;
  error: string | null;
};

function HistorySection() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [retentionDays, setRetentionDays] = useState<number>(30);
  const [journalEnabled, setJournalEnabledState] = useState(false);
  const [loading, setLoading] = useState(true);
  const [copiedAt, setCopiedAt] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const [reprocessing, setReprocessing] = useState<Record<number, ReprocessState>>({});

  // Empty query -> the normal recent-first list; non-empty -> a full-text
  // scan of the entire retained history, not just what's currently loaded.
  function fetchEntries(q: string): Promise<HistoryEntry[]> {
    const trimmed = q.trim();
    return trimmed
      ? invoke<HistoryEntry[]>("search_history_entries", { query: trimmed })
      : invoke<HistoryEntry[]>("list_history_entries");
  }

  function refresh() {
    fetchEntries(query)
      .then(setEntries)
      .catch((err) => console.error("history fetch failed:", err));
  }

  useEffect(() => {
    Promise.all([
      fetchEntries(""),
      invoke<number>("get_history_retention_days"),
      invoke<boolean>("get_journal_enabled"),
    ])
      .then(([historyEntries, days, journal]) => {
        setEntries(historyEntries);
        setRetentionDays(days);
        setJournalEnabledState(journal);
      })
      .catch((err) => console.error("failed to load history:", err))
      .finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Debounced re-fetch on every search keystroke. Re-subscribing
  // "history-updated" here (rather than once at mount) is deliberate — it
  // keeps the handler closed over the *current* query, so a new transcript
  // landing mid-search re-filters against that search instead of silently
  // reverting to the unfiltered list.
  useEffect(() => {
    const handle = setTimeout(refresh, 150);
    const unlisten = listen("history-updated", refresh);
    return () => {
      clearTimeout(handle);
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  function changeRetention(days: number) {
    setRetentionDays(days);
    invoke("set_history_retention_days", { days })
      .then(refresh) // purging may have removed entries outside the new window
      .catch((err) => console.error("set_history_retention_days failed:", err));
  }

  function toggleJournal(enabled: boolean) {
    setJournalEnabledState(enabled);
    invoke("set_journal_enabled", { enabled }).catch((err) => {
      console.error("set_journal_enabled failed:", err);
      setJournalEnabledState(!enabled);
    });
  }

  function clear() {
    setEntries([]);
    invoke("clear_history").catch((err) => console.error("clear_history failed:", err));
  }

  function copyEntry(entry: HistoryEntry) {
    navigator.clipboard
      .writeText(entry.text)
      .then(() => {
        setCopiedAt(entry.timestamp_ms);
        setTimeout(() => setCopiedAt((current) => (current === entry.timestamp_ms ? null : current)), 1500);
      })
      .catch((err) => console.error("clipboard write failed:", err));
  }

  function deleteEntry(timestampMs: number) {
    setEntries((current) => current.filter((e) => e.timestamp_ms !== timestampMs));
    setReprocessing((current) => {
      const { [timestampMs]: _removed, ...rest } = current;
      return rest;
    });
    invoke("delete_history_entry", { timestampMs }).catch((err) =>
      console.error("delete_history_entry failed:", err),
    );
  }

  function toggleReprocess(timestampMs: number) {
    setReprocessing((current) => {
      if (current[timestampMs]) {
        const { [timestampMs]: _removed, ...rest } = current;
        return rest;
      }
      return { ...current, [timestampMs]: { mode: "plain", useLlm: false, loading: false, result: null, error: null } };
    });
  }

  function patchReprocess(timestampMs: number, patch: Partial<ReprocessState>) {
    setReprocessing((current) => {
      const existing = current[timestampMs];
      if (!existing) return current;
      return { ...current, [timestampMs]: { ...existing, ...patch } };
    });
  }

  function runReprocess(entry: HistoryEntry) {
    const state = reprocessing[entry.timestamp_ms];
    if (!state) return;
    patchReprocess(entry.timestamp_ms, { loading: true, error: null, result: null });
    invoke<string>("reprocess_history_text", { text: entry.text, mode: state.mode, useLlmRefinement: state.useLlm })
      .then((result) => patchReprocess(entry.timestamp_ms, { loading: false, result }))
      .catch((err) => patchReprocess(entry.timestamp_ms, { loading: false, error: String(err) }));
  }

  function replaceWithReprocessed(entry: HistoryEntry) {
    const state = reprocessing[entry.timestamp_ms];
    if (!state?.result) return;
    invoke("update_history_entry_text", { timestampMs: entry.timestamp_ms, text: state.result })
      .then(() => {
        setReprocessing((current) => {
          const { [entry.timestamp_ms]: _removed, ...rest } = current;
          return rest;
        });
        refresh();
      })
      .catch((err) => console.error("update_history_entry_text failed:", err));
  }

  return (
    <div className="mb-4">
      <div className="mb-1 flex items-center justify-between">
        <label className="text-xs font-medium opacity-70">History</label>
        <div className="flex items-center gap-1.5">
          <select
            className="select select-xs"
            value={retentionDays}
            onChange={(e) => changeRetention(Number(e.target.value))}
          >
            {RETENTION_OPTIONS.map((days) => (
              <option key={days} value={days}>
                Keep {days}d
              </option>
            ))}
          </select>
          <button className="btn btn-ghost btn-xs" onClick={clear} disabled={entries.length === 0}>
            Clear
          </button>
        </div>
      </div>
      <label className="mb-1.5 flex items-center gap-1.5 text-xs opacity-80">
        <input
          type="checkbox"
          className="checkbox checkbox-xs"
          checked={journalEnabled}
          onChange={(e) => toggleJournal(e.target.checked)}
        />
        Summarize into a work journal (adds an LLM call per dictation)
      </label>
      <input
        className="input input-xs mb-1.5 w-full"
        placeholder="Search transcripts…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      {loading ? (
        <span className="loading loading-spinner loading-xs" />
      ) : entries.length === 0 ? (
        <p className="text-xs opacity-60">
          {query.trim() ? `No transcripts match "${query.trim()}".` : "No transcripts yet."}
        </p>
      ) : (
        <ul className="flex max-h-64 flex-col gap-1 overflow-y-auto rounded-md bg-base-100 p-1.5">
          {entries.map((entry) => {
            const reprocess = reprocessing[entry.timestamp_ms];
            return (
              <li key={entry.timestamp_ms} className="group rounded px-1.5 py-1 text-xs hover:bg-base-200">
                <div className="flex items-center justify-between opacity-50">
                  <span>{formatTimestamp(entry.timestamp_ms)}</span>
                  <div className="flex items-center gap-1.5">
                    {entry.app_name && <span className="truncate">{entry.app_name}</span>}
                    <button
                      className={reprocess ? "opacity-100" : "opacity-0 group-hover:opacity-100"}
                      onClick={() => toggleReprocess(entry.timestamp_ms)}
                      aria-label="Reprocess through a different mode"
                      title="Reprocess"
                    >
                      ↻
                    </button>
                    <button
                      className="opacity-0 group-hover:opacity-100"
                      onClick={() => copyEntry(entry)}
                      aria-label="Copy transcript"
                      title="Copy"
                    >
                      {copiedAt === entry.timestamp_ms ? "✓" : "⧉"}
                    </button>
                    <button
                      className="opacity-0 hover:text-error group-hover:opacity-100"
                      onClick={() => deleteEntry(entry.timestamp_ms)}
                      aria-label="Delete transcript"
                      title="Delete"
                    >
                      🗑
                    </button>
                  </div>
                </div>
                {entry.summary ? (
                  <>
                    <p className="truncate font-medium" title={entry.summary}>
                      {entry.summary}
                    </p>
                    <p className="truncate opacity-50" title={entry.text}>
                      {entry.text}
                    </p>
                  </>
                ) : (
                  <p className="truncate" title={entry.text}>
                    {entry.text}
                  </p>
                )}
                {reprocess && (
                  <div className="mt-1 flex flex-col gap-1 rounded bg-base-200 p-1.5">
                    <div className="flex items-center gap-1.5">
                      <select
                        className="select select-xs"
                        value={reprocess.mode}
                        onChange={(e) => patchReprocess(entry.timestamp_ms, { mode: e.target.value as Mode })}
                      >
                        {MODES.map((m) => (
                          <option key={m} value={m}>
                            {MODE_LABEL[m]}
                          </option>
                        ))}
                      </select>
                      <label className="flex items-center gap-1 opacity-80">
                        <input
                          type="checkbox"
                          className="checkbox checkbox-xs"
                          checked={reprocess.useLlm}
                          onChange={(e) => patchReprocess(entry.timestamp_ms, { useLlm: e.target.checked })}
                        />
                        Refine with LLM
                      </label>
                      <button
                        className="btn btn-xs ml-auto"
                        onClick={() => runReprocess(entry)}
                        disabled={reprocess.loading}
                      >
                        {reprocess.loading ? <span className="loading loading-spinner loading-xs" /> : "Run"}
                      </button>
                    </div>
                    {reprocess.error && <p className="text-error">{reprocess.error}</p>}
                    {reprocess.result && (
                      <>
                        <p className="whitespace-pre-wrap rounded bg-base-100 p-1.5" title={reprocess.result}>
                          {reprocess.result}
                        </p>
                        <div className="flex items-center gap-1.5">
                          <button
                            className="btn btn-ghost btn-xs"
                            onClick={() => navigator.clipboard.writeText(reprocess.result ?? "")}
                          >
                            Copy result
                          </button>
                          <button className="btn btn-ghost btn-xs" onClick={() => replaceWithReprocessed(entry)}>
                            Replace
                          </button>
                          <button
                            className="btn btn-ghost btn-xs ml-auto"
                            onClick={() => toggleReprocess(entry.timestamp_ms)}
                          >
                            Discard
                          </button>
                        </div>
                      </>
                    )}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

function WebhookSection() {
  const [url, setUrl] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [testState, setTestState] = useState<"idle" | "sending" | "ok" | "error">("idle");
  const [testError, setTestError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<string | null>("get_webhook_url")
      .then((value) => {
        setUrl(value ?? "");
        setSaved(value ?? null);
      })
      .catch((err) => console.error("get_webhook_url failed:", err))
      .finally(() => setLoading(false));

    const unlisten = listen<{ ok: boolean; error?: string }>("webhook-test-result", (e) => {
      setTestState(e.payload.ok ? "ok" : "error");
      setTestError(e.payload.ok ? null : e.payload.error ?? "unknown error");
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  function save() {
    const trimmed = url.trim();
    invoke("set_webhook_url", { url: trimmed || null })
      .then(() => setSaved(trimmed || null))
      .catch((err) => console.error("set_webhook_url failed:", err));
  }

  function sendTest() {
    setTestState("sending");
    setTestError(null);
    invoke("send_test_webhook").catch((err) => {
      setTestState("error");
      setTestError(String(err));
    });
  }

  return (
    <div className="mb-4 border-t border-base-content/10 pt-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Output webhook</label>
      <p className="mb-1.5 text-xs opacity-50">
        Sends each delivered dictation as a JSON POST to this URL — wire it into Zapier, n8n,
        Make.com, or a Notion/Slack incoming webhook. Off by default: dictated text only leaves
        the device once you set a URL here.
      </p>
      {loading ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <>
          <div className="flex gap-1.5">
            <input
              className="input input-sm flex-1"
              placeholder="https://hooks.example.com/…"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onBlur={save}
              onKeyDown={(e) => {
                if (e.key === "Enter") save();
              }}
            />
            <button className="btn btn-sm" onClick={sendTest} disabled={!saved || testState === "sending"}>
              {testState === "sending" ? "Sending…" : "Send test event"}
            </button>
          </div>
          {testState === "ok" && <p className="mt-1 text-xs text-success">Test event delivered.</p>}
          {testState === "error" && (
            <p className="mt-1 text-xs text-error">Test event failed: {testError}</p>
          )}
        </>
      )}
    </div>
  );
}

type LogEntry = { timestamp_ms: number; message: string };

/// Collapsed by default — this is a diagnostic tool ("why didn't it
/// paste?", "which mode did it pick?"), not something most users need
/// open, and polling only while expanded avoids invoke() calls for a
/// panel nobody's looking at.
function LogsSection() {
  const [open, setOpen] = useState(false);
  const [logs, setLogs] = useState<LogEntry[]>([]);

  function refresh() {
    invoke<LogEntry[]>("get_logs")
      .then(setLogs)
      .catch((err) => console.error("get_logs failed:", err));
  }

  useEffect(() => {
    if (!open) return;
    refresh();
    const interval = setInterval(refresh, 2000);
    return () => clearInterval(interval);
  }, [open]);

  function clear() {
    setLogs([]);
    invoke("clear_logs").catch((err) => console.error("clear_logs failed:", err));
  }

  return (
    <div className="mb-4 border-t border-base-content/10 pt-3">
      <div className="mb-1 flex items-center justify-between">
        <button
          className="text-xs font-medium opacity-70"
          onClick={() => setOpen((o) => !o)}
          aria-expanded={open}
        >
          {open ? "▾" : "▸"} Logs
        </button>
        {open && (
          <div className="flex items-center gap-1.5">
            <button className="btn btn-ghost btn-xs" onClick={refresh}>
              Refresh
            </button>
            <button className="btn btn-ghost btn-xs" onClick={clear} disabled={logs.length === 0}>
              Clear
            </button>
          </div>
        )}
      </div>
      {open &&
        (logs.length === 0 ? (
          <p className="text-xs opacity-60">No log entries yet.</p>
        ) : (
          <pre className="max-h-48 overflow-y-auto whitespace-pre-wrap rounded-md bg-base-100 p-1.5 font-mono text-[10px] leading-tight">
            {logs.map((l) => `[${formatTimestamp(l.timestamp_ms)}] ${l.message}`).join("\n")}
          </pre>
        ))}
    </div>
  );
}

type LlmModelStatus = {
  id: string;
  label: string;
  size_gb: number;
  downloaded: boolean;
  last_latency_ms: number | null;
};

function LlmSection() {
  const [catalog, setCatalog] = useState<LlmModelStatus[]>([]);
  const [activeModel, setActiveModel] = useState("");
  const [checked, setChecked] = useState(false);
  const [progress, setProgress] = useState<Record<string, { status: string; percent: number | null }>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [ollamaMissing, setOllamaMissing] = useState(false);

  function refresh() {
    Promise.all([invoke<LlmModelStatus[]>("list_llm_catalog"), invoke<string>("get_llm_model")])
      .then(([entries, current]) => {
        setCatalog(entries);
        setActiveModel(current);
        setOllamaMissing(!entries.some((m) => m.downloaded));
      })
      .catch((err) => console.error("failed to load LLM settings:", err))
      .finally(() => setChecked(true));
  }

  useEffect(() => {
    refresh();

    const unlistenProgress = listen<{ id: string; status: string; percent: number | null }>(
      "llm-pull-progress",
      (e) => {
        setProgress((p) => ({ ...p, [e.payload.id]: { status: e.payload.status, percent: e.payload.percent } }));
      },
    );
    const unlistenDone = listen<string>("llm-pull-complete", (e) => {
      setProgress((p) => {
        const next = { ...p };
        delete next[e.payload];
        return next;
      });
      refresh();
    });
    const unlistenError = listen<{ id: string; error: string }>("llm-pull-error", (e) => {
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

  function selectModel(model: string) {
    setActiveModel(model);
    invoke("set_llm_model", { model }).catch((err) => console.error("set_llm_model failed:", err));
  }

  function pull(id: string) {
    setErrors((er) => {
      const next = { ...er };
      delete next[id];
      return next;
    });
    setProgress((p) => ({ ...p, [id]: { status: "starting…", percent: null } }));
    invoke("pull_llm_model", { id }).catch((err) => setErrors((er) => ({ ...er, [id]: String(err) })));
  }

  return (
    <div className="mb-4">
      <label className="mb-1 block text-xs font-medium opacity-70">LLM refinement</label>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <>
          {ollamaMissing && (
            <p className="mb-1.5 text-xs opacity-60">
              No local Ollama models found. Install{" "}
              <a href="https://ollama.com" target="_blank" rel="noreferrer" className="link">
                Ollama
              </a>{" "}
              first, then download a model below.
            </p>
          )}
          <ul className="flex flex-col gap-1.5">
            {catalog.map((m) => (
              <li key={m.id} className="flex items-center justify-between rounded-md bg-base-100 px-2.5 py-1.5 text-xs">
                <div className="truncate">
                  <div className="font-medium">{m.label}</div>
                  <div className="flex gap-1.5 opacity-50">
                    {m.size_gb > 0 && <span>{m.size_gb.toFixed(1)}GB</span>}
                    {m.last_latency_ms !== null && (
                      <span title="Last observed refinement round-trip time">
                        ~{(m.last_latency_ms / 1000).toFixed(1)}s
                      </span>
                    )}
                  </div>
                </div>
                {m.id === activeModel ? (
                  <span className="badge badge-success badge-sm">Active</span>
                ) : m.id in progress ? (
                  <span className="w-16 truncate text-right opacity-70" title={progress[m.id].status}>
                    {progress[m.id].percent !== null ? `${progress[m.id].percent}%` : progress[m.id].status}
                  </span>
                ) : m.downloaded ? (
                  <button className="btn btn-xs" onClick={() => selectModel(m.id)}>
                    Use
                  </button>
                ) : (
                  <button className="btn btn-xs" onClick={() => pull(m.id)}>
                    Download
                  </button>
                )}
              </li>
            ))}
          </ul>
          {Object.entries(errors).map(([id, err]) => (
            <p key={id} className="mt-1 text-xs text-error">
              {id}: {err}
            </p>
          ))}
          <p className="mt-1.5 text-xs opacity-50">Used by any mode with "Refine with LLM" enabled above.</p>
        </>
      )}
    </div>
  );
}

type WidgetMode = "minimal" | "compact" | "detailed";

const WIDGET_MODE_LABEL: Record<WidgetMode, string> = {
  minimal: "Minimal — icon only",
  compact: "Compact — status pill",
  detailed: "Detailed — full status panel",
};

function DeliverySection() {
  const [copyOnly, setCopyOnly] = useState(false);
  const [pressEnterEnabled, setPressEnterEnabled] = useState(false);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    Promise.all([invoke<boolean>("get_copy_only"), invoke<boolean>("get_press_enter_enabled")])
      .then(([copyOnlyValue, pressEnterValue]) => {
        setCopyOnly(copyOnlyValue);
        setPressEnterEnabled(pressEnterValue);
      })
      .catch((err) => console.error("failed to load delivery settings:", err))
      .finally(() => setChecked(true));
  }, []);

  function toggleCopyOnly(enabled: boolean) {
    setCopyOnly(enabled);
    invoke("set_copy_only", { enabled }).catch((err) => {
      console.error("set_copy_only failed:", err);
      setCopyOnly(!enabled);
    });
  }

  function togglePressEnter(enabled: boolean) {
    setPressEnterEnabled(enabled);
    invoke("set_press_enter_enabled", { enabled }).catch((err) => {
      console.error("set_press_enter_enabled failed:", err);
      setPressEnterEnabled(!enabled);
    });
  }

  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Delivery</label>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <div className="flex flex-col gap-2">
          <label className="flex items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              className="checkbox checkbox-xs"
              checked={copyOnly}
              onChange={(e) => toggleCopyOnly(e.target.checked)}
            />
            Copy only — don't auto-paste into the active app
          </label>
          <label className="flex items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              className="checkbox checkbox-xs"
              checked={pressEnterEnabled}
              disabled={copyOnly}
              onChange={(e) => togglePressEnter(e.target.checked)}
            />
            Say "press enter" to submit — presses Enter after pasting
            {copyOnly && " (disabled while copy only is on)"}
          </label>
        </div>
      )}
    </div>
  );
}

function ThemeSection({ theme, onChange }: { theme: ThemeId; onChange: (id: ThemeId) => void }) {
  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Appearance</label>
      <div className="flex flex-col gap-1.5">
        {THEME_ORDER.map((id) => (
          <button
            key={id}
            type="button"
            className={`dw-theme-row ${theme === id ? "active" : ""}`}
            onClick={() => onChange(id)}
          >
            <span className="flex items-center gap-2">
              <span className="dw-theme-dot" style={{ background: THEMES[id].accent }} />
              {THEME_LABEL[id]}
            </span>
            {theme === id && <span>✓</span>}
          </button>
        ))}
      </div>
    </div>
  );
}

function GeneralSection() {
  const [autostart, setAutostart] = useState(false);
  const [widgetMode, setWidgetModeState] = useState<WidgetMode>("compact");
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    Promise.all([invoke<boolean>("get_autostart_enabled"), invoke<WidgetMode>("get_widget_mode")])
      .then(([autostartValue, widgetModeValue]) => {
        setAutostart(autostartValue);
        setWidgetModeState(widgetModeValue);
      })
      .catch((err) => console.error("failed to load general settings:", err))
      .finally(() => setChecked(true));
  }, []);

  function toggleAutostart(enabled: boolean) {
    setAutostart(enabled);
    invoke("set_autostart_enabled", { enabled }).catch((err) => {
      console.error("set_autostart_enabled failed:", err);
      setAutostart(!enabled);
    });
  }

  function changeWidgetMode(newMode: WidgetMode) {
    const previous = widgetMode;
    setWidgetModeState(newMode);
    invoke("set_widget_mode", { mode: newMode }).catch((err) => {
      console.error("set_widget_mode failed:", err);
      setWidgetModeState(previous);
    });
  }

  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="mb-1 block text-xs font-medium opacity-70">General</label>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <div className="flex flex-col gap-2">
          <label className="flex items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              className="checkbox checkbox-xs"
              checked={autostart}
              onChange={(e) => toggleAutostart(e.target.checked)}
            />
            Launch Dev Whisper at login
          </label>
          <label className="flex items-center gap-1.5 text-xs">
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

function VoiceIsolationSection() {
  const [enabled, setEnabled] = useState(false);
  const [checked, setChecked] = useState(false);
  const [status, setStatus] = useState<EnrollmentStatus | null>(null);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function refreshStatus() {
    invoke<EnrollmentStatus>("get_voice_enrollment_status")
      .then(setStatus)
      .catch((err) => console.error("get_voice_enrollment_status failed:", err));
  }

  useEffect(() => {
    invoke<boolean>("get_isolated_voice_enabled")
      .then(setEnabled)
      .catch((err) => console.error("get_isolated_voice_enabled failed:", err))
      .finally(() => setChecked(true));
    refreshStatus();

    const unlistenStarted = listen("enrollment-started", () => setRecording(true));
    const unlistenDone = listen("enrollment-complete", () => {
      setRecording(false);
      setError(null);
      refreshStatus();
    });
    const unlistenError = listen<string>("enrollment-error", (e) => {
      setRecording(false);
      setError(e.payload);
    });

    return () => {
      unlistenStarted.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, []);

  function toggleEnabled(next: boolean) {
    setEnabled(next);
    invoke("set_isolated_voice_enabled", { enabled: next }).catch((err) => {
      console.error("set_isolated_voice_enabled failed:", err);
      setEnabled(!next);
    });
  }

  function startEnrollment() {
    setError(null);
    invoke("start_voice_enrollment")
      .then(() => setRecording(true))
      .catch((err) => setError(String(err)));
  }

  function stopEnrollment() {
    invoke("stop_voice_enrollment").catch((err) => {
      console.error("stop_voice_enrollment failed:", err);
      setRecording(false);
    });
  }

  return (
    <div className="mb-4 border-t border-base-content/10 pt-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Voice Isolation</label>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <div className="flex flex-col gap-2">
          <label className="flex items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              className="checkbox checkbox-xs"
              checked={enabled}
              onChange={(e) => toggleEnabled(e.target.checked)}
            />
            Isolated Voice — filter out other speakers/background noise before transcribing
          </label>

          <div className="flex items-center justify-between rounded-md bg-base-100 px-2.5 py-1.5 text-xs">
            <div>
              <div className="font-medium">{status?.enrolled ? "Voice enrolled" : "Not enrolled"}</div>
              {status?.enrolled && status.enrolled_at_ms && (
                <div className="opacity-50">
                  {new Date(status.enrolled_at_ms).toLocaleDateString(undefined, {
                    month: "short",
                    day: "numeric",
                    year: "numeric",
                  })}
                </div>
              )}
            </div>
            {recording ? (
              <button className="btn btn-xs btn-error" onClick={stopEnrollment}>
                Stop
              </button>
            ) : (
              <button className="btn btn-xs" onClick={startEnrollment}>
                {status?.enrolled ? "Re-enroll" : "Enroll voice"}
              </button>
            )}
          </div>

          {recording && (
            <p className="text-xs opacity-60">
              Recording — speak naturally for a few seconds, then press Stop.
            </p>
          )}

          <p className="text-xs opacity-60">
            {status?.enrolled
              ? "Enrolled: recordings are matched against your enrolled voice, rejecting other speakers."
              : "Not enrolled: this only suppresses quiet background noise, not a second person talking at similar volume. Enroll your voice above for stronger speaker-based isolation."}
          </p>

          {error && <p className="text-xs text-error">{error}</p>}
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
const PUNCTUATION_COMMANDS: [string, string][] = [
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

type VoiceCommandGroup = {
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
const VOICE_COMMAND_GROUPS: VoiceCommandGroup[] = [
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
    description: 'Say this at the end to send Enter after pasting. Off by default — enable it in General above.',
    examples: [{ say: '"looks good press enter"', get: 'pastes "looks good", then Enter' }],
  },
  {
    title: "Append clipboard",
    description: 'Say this at the end to append whatever you last copied — a stack trace, a URL, a snippet — without reading it aloud.',
    examples: [{ say: '"here\'s the error append clipboard"', get: "here's the error <clipboard contents>" }],
  },
];

function VoiceCommandsSection() {
  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Voice Commands</label>
      <p className="mb-2 text-xs opacity-60">
        Trigger phrases recognized in any app, in any Mode — if dictation ever came out looking unexpectedly
        transformed, check here first.
      </p>
      <div className="flex flex-col gap-2.5">
        {VOICE_COMMAND_GROUPS.map((group) => (
          <div key={group.title}>
            <p className="text-xs font-medium">{group.title}</p>
            <p className="mb-1 text-xs opacity-60">{group.description}</p>
            <div className="flex flex-col gap-0.5">
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
          <p className="mb-1 text-xs opacity-60">Say the word, get the character — works mid-sentence, not just at the end.</p>
          <div className="grid grid-cols-2 gap-x-3 gap-y-0.5">
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

type PageId =
  | "dictation"
  | "voice"
  | "vocabulary"
  | "modes"
  | "llm"
  | "history"
  | "integrations"
  | "appearance"
  | "advanced";

const NAV: { id: PageId; label: string; icon: () => ReactElement }[] = [
  { id: "dictation", label: "Dictation", icon: IconMic },
  { id: "voice", label: "Voice", icon: IconWave },
  { id: "vocabulary", label: "Vocabulary", icon: IconBook },
  { id: "modes", label: "Modes", icon: IconWindow },
  { id: "llm", label: "LLM", icon: IconCpu },
  { id: "history", label: "History", icon: IconClock },
  { id: "integrations", label: "Integrations", icon: IconPlug },
];

const FOOTER_NAV: { id: PageId; label: string; icon: () => ReactElement }[] = [
  { id: "appearance", label: "Appearance", icon: IconPalette },
  { id: "advanced", label: "Advanced", icon: IconGear },
];

const PAGE_TITLE: Record<PageId, string> = {
  dictation: "Dictation",
  voice: "Voice — Whisper model & isolation",
  vocabulary: "Vocabulary",
  modes: "Modes — app-aware formatting rules",
  llm: "LLM — local refinement (Ollama)",
  history: "History",
  integrations: "Integrations — delivery & webhook",
  appearance: "Appearance",
  advanced: "Advanced",
};

function NavItem({
  label,
  icon,
  active,
  onClick,
}: {
  label: string;
  icon: ReactElement;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" className={`dw-nav-item ${active ? "active" : ""}`} onClick={onClick}>
      <span className="dw-icon">{icon}</span>
      {label}
    </button>
  );
}

function SettingsView() {
  const [theme, setThemeState] = useState<ThemeId>("terminal");
  const [page, setPage] = useState<PageId>("dictation");

  useEffect(() => {
    invoke<ThemeId>("get_theme")
      .then((t) => {
        setThemeState(t);
        applyTheme(t);
      })
      .catch((err) => console.error("get_theme failed:", err));

    const unlisten = listen<ThemeId>("theme-changed", (e) => {
      setThemeState(e.payload);
      applyTheme(e.payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  function changeTheme(next: ThemeId) {
    setThemeState(next);
    applyTheme(next);
    invoke("set_theme", { theme: next }).catch((err) => console.error("set_theme failed:", err));
  }

  return (
    <main className="dw-shell h-screen">
      <div className="dw-app-shell">
        <nav className="dw-sidebar">
          <div className="dw-brand">
            <IconMic />
            Dev Whisper
          </div>
          {NAV.map((item) => (
            <NavItem
              key={item.id}
              label={item.label}
              icon={item.icon()}
              active={page === item.id}
              onClick={() => setPage(item.id)}
            />
          ))}
          <div className="dw-sidebar-foot">
            {FOOTER_NAV.map((item) => (
              <NavItem
                key={item.id}
                label={item.label}
                icon={item.icon()}
                active={page === item.id}
                onClick={() => setPage(item.id)}
              />
            ))}
          </div>
        </nav>

        <div className="dw-page">
          <h2 className="dw-page-title">{PAGE_TITLE[page]}</h2>
          {page === "dictation" && (
            <>
              <GeneralSection />
              <VoiceCommandsSection />
              <DeviceSection />
              <ShortcutSection />
            </>
          )}
          {page === "voice" && (
            <>
              <ModelsSection />
              <VoiceIsolationSection />
            </>
          )}
          {page === "vocabulary" && (
            <>
              <VocabularySection />
              <SnippetsSection />
            </>
          )}
          {page === "modes" && <AppModesSection />}
          {page === "llm" && <LlmSection />}
          {page === "history" && <HistorySection />}
          {page === "integrations" && (
            <>
              <DeliverySection />
              <WebhookSection />
            </>
          )}
          {page === "appearance" && <ThemeSection theme={theme} onChange={changeTheme} />}
          {page === "advanced" && <LogsSection />}
        </div>
      </div>
    </main>
  );
}

export default SettingsView;
