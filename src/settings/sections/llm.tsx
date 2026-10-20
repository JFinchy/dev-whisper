import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { notifyError } from "../../ui/toast";
import { type LlmModelStatus } from "../shared";

export function LlmSection() {
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
      .catch((err) => notifyError("Couldn't load LLM settings", err))
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
    invoke("set_llm_model", { model }).catch((err) => notifyError("set_llm_model", err));
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
      <OnlyWhenNeededToggle />
      <label className="mb-1 block text-xs font-medium opacity-70">LLM refinement</label>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <>
          {ollamaMissing && (
            <p className="mb-1.5 text-xs opacity-60">
              No local Ollama models found. Install{" "}
              <button
                type="button"
                className="link"
                onClick={() => openUrl("https://ollama.com/download").catch((err) => notifyError("Couldn't open ollama.com", err))}
              >
                Ollama
              </button>{" "}
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

// Refinement takes seconds (the model rewrites the whole message), and on
// a clean transcript it hands back the same text. This skips it unless
// the transcript has filler, a stutter, or no punctuation; Casual mode
// and modes with custom instructions still always refine.
function OnlyWhenNeededToggle() {
  const [enabled, setEnabled] = useState<boolean | null>(null);

  useEffect(() => {
    invoke<boolean>("get_llm_only_when_needed")
      .then(setEnabled)
      .catch((err) => notifyError("get_llm_only_when_needed", err));
  }, []);

  function toggle(next: boolean) {
    setEnabled(next);
    invoke("set_llm_only_when_needed", { enabled: next }).catch((err) => {
      setEnabled(!next);
      notifyError("set_llm_only_when_needed", err);
    });
  }

  if (enabled === null) return null;
  return (
    <div className="mb-3 flex flex-col gap-1">
      <label className="flex items-center gap-1.5 text-xs">
        <input
          type="checkbox"
          className="checkbox checkbox-xs"
          checked={enabled}
          onChange={(e) => toggle(e.target.checked)}
        />
        Only refine when it's needed (faster)
      </label>
      <p className="text-xs opacity-60">
        Clean transcripts paste right away. Refinement still runs when there's filler ("um", "you know"), a
        repeated word, or a long stretch with no punctuation, and always in Casual mode or modes with custom
        instructions.
      </p>
    </div>
  );
}
