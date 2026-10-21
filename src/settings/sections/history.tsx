import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { notifyError } from "../../ui/toast";
import { BEHAVIORS, BEHAVIOR_LABEL, type Behavior } from "../shared";

export type HistoryEntry = {
  timestamp_ms: number;
  text: string;
  app_name: string | null;
  mode: string | null;
  summary: string | null;
  /// Text before the LLM rewrote it; present only on refined entries.
  original_text?: string | null;
};

export const RETENTION_OPTIONS = [7, 30, 90, 365];

export function formatTimestamp(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export type ReprocessState = {
  mode: Behavior;
  useLlm: boolean;
  loading: boolean;
  result: string | null;
  error: string | null;
};

export function HistorySection() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [retentionDays, setRetentionDays] = useState<number>(30);
  const [journalEnabled, setJournalEnabledState] = useState(false);
  const [loading, setLoading] = useState(true);
  const [copiedAt, setCopiedAt] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const [reprocessing, setReprocessing] = useState<Record<number, ReprocessState>>({});
  const [showOriginal, setShowOriginal] = useState<Set<number>>(new Set());
  const [editing, setEditing] = useState<Record<number, string>>({});

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
      .catch((err) => notifyError("Couldn't load history", err));
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
      .catch((err) => notifyError("Couldn't load history", err))
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
      .catch((err) => notifyError("set_history_retention_days", err));
  }

  function toggleJournal(enabled: boolean) {
    setJournalEnabledState(enabled);
    invoke("set_journal_enabled", { enabled }).catch((err) => {
      notifyError("set_journal_enabled", err);
      setJournalEnabledState(!enabled);
    });
  }

  function clear() {
    setEntries([]);
    invoke("clear_history").catch((err) => notifyError("clear_history", err));
  }

  function copyEntry(entry: HistoryEntry) {
    navigator.clipboard
      .writeText(entry.text)
      .then(() => {
        setCopiedAt(entry.timestamp_ms);
        setTimeout(() => setCopiedAt((current) => (current === entry.timestamp_ms ? null : current)), 1500);
      })
      .catch((err) => notifyError("Couldn't copy to the clipboard", err));
  }

  function deleteEntry(timestampMs: number) {
    setEntries((current) => current.filter((e) => e.timestamp_ms !== timestampMs));
    setReprocessing((current) => {
      const { [timestampMs]: _removed, ...rest } = current;
      return rest;
    });
    invoke("delete_history_entry", { timestampMs }).catch((err) =>
      notifyError("delete_history_entry", err),
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
    invoke("update_history_entry_text", { timestampMs: entry.timestamp_ms, text: state.result, learn: false })
      .then(() => {
        setReprocessing((current) => {
          const { [entry.timestamp_ms]: _removed, ...rest } = current;
          return rest;
        });
        refresh();
      })
      .catch((err) => notifyError("update_history_entry_text", err));
  }

  function toggleOriginal(timestampMs: number) {
    setShowOriginal((current) => {
      const next = new Set(current);
      if (!next.delete(timestampMs)) next.add(timestampMs);
      return next;
    });
  }

  function copyOriginal(entry: HistoryEntry) {
    navigator.clipboard
      .writeText(entry.original_text ?? "")
      .catch((err) => notifyError("Couldn't copy to the clipboard", err));
  }

  function useOriginal(entry: HistoryEntry) {
    invoke("use_original_history_text", { timestampMs: entry.timestamp_ms })
      .then(refresh)
      .catch((err) => notifyError("use_original_history_text", err));
  }

  function startEdit(entry: HistoryEntry) {
    setEditing((current) => ({ ...current, [entry.timestamp_ms]: entry.text }));
  }

  function stopEdit(timestampMs: number) {
    setEditing((current) => {
      const { [timestampMs]: _removed, ...rest } = current;
      return rest;
    });
  }

  function saveEdit(entry: HistoryEntry) {
    const text = editing[entry.timestamp_ms]?.trim();
    if (!text || text === entry.text) {
      stopEdit(entry.timestamp_ms);
      return;
    }
    invoke("update_history_entry_text", { timestampMs: entry.timestamp_ms, text, learn: true })
      .then(() => {
        stopEdit(entry.timestamp_ms);
        refresh();
      })
      .catch((err) => notifyError("update_history_entry_text", err));
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
            const draft = editing[entry.timestamp_ms];
            const original = entry.original_text;
            const originalOpen = !!original && showOriginal.has(entry.timestamp_ms);
            return (
              <li key={entry.timestamp_ms} className="group rounded px-1.5 py-1 text-xs hover:bg-base-200">
                <div className="flex items-center justify-between opacity-50">
                  <span className="flex items-center gap-1.5">
                    {formatTimestamp(entry.timestamp_ms)}
                    {original && (
                      <button
                        className="badge badge-ghost badge-xs"
                        onClick={() => toggleOriginal(entry.timestamp_ms)}
                        aria-expanded={originalOpen}
                        title="Show the text before refinement"
                      >
                        Refined {originalOpen ? "▾" : "▸"}
                      </button>
                    )}
                  </span>
                  <div className="flex items-center gap-1.5">
                    {entry.app_name && <span className="truncate">{entry.app_name}</span>}
                    <button
                      className={draft !== undefined ? "opacity-100" : "opacity-0 group-hover:opacity-100"}
                      onClick={() => (draft !== undefined ? stopEdit(entry.timestamp_ms) : startEdit(entry))}
                      aria-label="Edit transcript"
                      title="Edit"
                    >
                      ✎
                    </button>
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
                {draft !== undefined ? (
                  <div className="mt-1 flex flex-col gap-1">
                    <textarea
                      className="textarea textarea-xs w-full"
                      rows={3}
                      value={draft}
                      aria-label="Edit transcript text"
                      onChange={(e) => setEditing((current) => ({ ...current, [entry.timestamp_ms]: e.target.value }))}
                    />
                    <div className="flex items-center gap-1.5">
                      <button className="btn btn-xs" onClick={() => saveEdit(entry)}>
                        Save
                      </button>
                      <button className="btn btn-ghost btn-xs" onClick={() => stopEdit(entry.timestamp_ms)}>
                        Cancel
                      </button>
                    </div>
                  </div>
                ) : entry.summary ? (
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
                {originalOpen && (
                  <div className="mt-1 flex flex-col gap-1 rounded bg-base-200 p-1.5" data-testid="original-text">
                    <span className="opacity-50">Original, before refinement</span>
                    <p className="whitespace-pre-wrap">{original}</p>
                    <div className="flex items-center gap-1.5">
                      <button className="btn btn-ghost btn-xs" onClick={() => copyOriginal(entry)}>
                        Copy original
                      </button>
                      <button className="btn btn-ghost btn-xs" onClick={() => useOriginal(entry)}>
                        Use original
                      </button>
                    </div>
                  </div>
                )}
                {reprocess && (
                  <div className="mt-1 flex flex-col gap-1 rounded bg-base-200 p-1.5">
                    <div className="flex items-center gap-1.5">
                      <select
                        className="select select-xs"
                        value={reprocess.mode}
                        onChange={(e) => patchReprocess(entry.timestamp_ms, { mode: e.target.value as Behavior })}
                      >
                        {BEHAVIORS.map((m) => (
                          <option key={m} value={m}>
                            {BEHAVIOR_LABEL[m]}
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
