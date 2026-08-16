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
    <div className="mb-4 border-t border-base-content/10 pt-3">
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
    <div className="mb-4 border-t border-base-content/10 pt-3">
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

type HistoryEntry = {
  timestamp_ms: number;
  text: string;
  app_name: string | null;
  mode: string | null;
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

function HistorySection() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [retentionDays, setRetentionDays] = useState<number>(30);
  const [loading, setLoading] = useState(true);
  const [copiedAt, setCopiedAt] = useState<number | null>(null);

  function refresh() {
    invoke<HistoryEntry[]>("list_history_entries")
      .then(setEntries)
      .catch((err) => console.error("list_history_entries failed:", err));
  }

  useEffect(() => {
    Promise.all([
      invoke<HistoryEntry[]>("list_history_entries"),
      invoke<number>("get_history_retention_days"),
    ])
      .then(([historyEntries, days]) => {
        setEntries(historyEntries);
        setRetentionDays(days);
      })
      .catch((err) => console.error("failed to load history:", err))
      .finally(() => setLoading(false));

    // New transcripts land while Settings may already be open — without
    // this, the list only ever reflected whatever existed at mount time.
    const unlisten = listen("history-updated", refresh);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  function changeRetention(days: number) {
    setRetentionDays(days);
    invoke("set_history_retention_days", { days })
      .then(refresh) // purging may have removed entries outside the new window
      .catch((err) => console.error("set_history_retention_days failed:", err));
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
    invoke("delete_history_entry", { timestampMs }).catch((err) =>
      console.error("delete_history_entry failed:", err),
    );
  }

  return (
    <div className="mb-4 border-t border-base-content/10 pt-3">
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
      {loading ? (
        <span className="loading loading-spinner loading-xs" />
      ) : entries.length === 0 ? (
        <p className="text-xs opacity-60">No transcripts yet.</p>
      ) : (
        <ul className="flex max-h-48 flex-col gap-1 overflow-y-auto rounded-md bg-base-100 p-1.5">
          {entries.map((entry) => (
            <li key={entry.timestamp_ms} className="group rounded px-1.5 py-1 text-xs hover:bg-base-200">
              <div className="flex items-center justify-between opacity-50">
                <span>{formatTimestamp(entry.timestamp_ms)}</span>
                <div className="flex items-center gap-1.5">
                  {entry.app_name && <span className="truncate">{entry.app_name}</span>}
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
              <p className="truncate" title={entry.text}>
                {entry.text}
              </p>
            </li>
          ))}
        </ul>
      )}
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
    <div className="mb-4 border-t border-base-content/10 pt-3">
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

function GeneralSection() {
  const [autostart, setAutostart] = useState(false);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    invoke<boolean>("get_autostart_enabled")
      .then(setAutostart)
      .catch((err) => console.error("failed to load autostart setting:", err))
      .finally(() => setChecked(true));
  }, []);

  function toggle(enabled: boolean) {
    setAutostart(enabled);
    invoke("set_autostart_enabled", { enabled }).catch((err) => {
      console.error("set_autostart_enabled failed:", err);
      setAutostart(!enabled);
    });
  }

  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="mb-1 block text-xs font-medium opacity-70">General</label>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <label className="flex items-center gap-1.5 text-xs">
          <input
            type="checkbox"
            className="checkbox checkbox-xs"
            checked={autostart}
            onChange={(e) => toggle(e.target.checked)}
          />
          Launch Dev Whisper at login
        </label>
      )}
    </div>
  );
}

function SettingsView() {
  return (
    <main className="h-screen overflow-y-auto bg-base-300 px-5 py-4 text-base-content">
      <h1 className="mb-4 text-base font-semibold">Settings</h1>
      <GeneralSection />
      <DeviceSection />
      <ShortcutSection />
      <ModelsSection />
      <AppModesSection />
      <LlmSection />
      <VocabularySection />
      <HistorySection />
    </main>
  );
}

export default SettingsView;
