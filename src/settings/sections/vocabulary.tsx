import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { VOCAB_BOOKS } from "../../vocabularyBooks";
import { notifyError } from "../../ui/toast";

export function VocabularyBooksSection({ terms, onAdopt }: { terms: string[]; onAdopt: (words: string[]) => void }) {
  const termSet = new Set(terms);

  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Vocabulary books</label>
      <p className="mb-2 text-xs opacity-50">
        Curated term bundles you can adopt in one click — each one merges into the list below,
        so you can prune anything you don't need afterward.
      </p>
      <div className="dw-book-grid">
        {VOCAB_BOOKS.map((book) => {
          const newWords = book.words.filter((w) => !termSet.has(w));
          const adopted = newWords.length === 0;
          return (
            <button
              key={book.id}
              type="button"
              className={`dw-book-card ${adopted ? "adopted" : ""}`}
              onClick={() => onAdopt(newWords)}
              disabled={adopted}
            >
              <span className="dw-book-title">{book.label}</span>
              <span className="dw-book-meta">
                {adopted ? `Adopted · ${book.words.length} words` : `${book.words.length} words — ${book.description}`}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

type ProjectStatus = { name: string; names: number } | null;

// Names from the repo open in the frontmost terminal or editor. Found and
// read on this Mac when you start dictating; kept in memory only.
function ProjectNamesToggle() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [status, setStatus] = useState<ProjectStatus>(null);

  useEffect(() => {
    invoke<boolean>("get_project_names_enabled")
      .then(setEnabled)
      .catch((err) => notifyError("get_project_names_enabled", err));
    invoke<ProjectStatus>("get_project_status")
      .then(setStatus)
      .catch((err) => notifyError("get_project_status", err));
  }, []);

  function toggle(next: boolean) {
    setEnabled(next);
    invoke("set_project_names_enabled", { enabled: next }).catch((err) => {
      setEnabled(!next);
      notifyError("set_project_names_enabled", err);
    });
  }

  if (enabled === null) return null;
  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="flex items-center gap-1.5 text-xs">
        <input
          type="checkbox"
          className="checkbox checkbox-xs"
          checked={enabled}
          onChange={(e) => toggle(e.target.checked)}
        />
        Use names from your project
      </label>
      <p className="mt-1 text-xs opacity-60">
        In a terminal or editor, Dev Whisper reads the file, function and package names of the project you're in, so
        "the paste text function in the paste file" comes out as <code>paste_text</code> and{" "}
        <code>@src-tauri/src/paste.rs</code>. It all stays on this Mac, in memory only.
      </p>
      {enabled && (
        <p className="mt-1 text-xs opacity-60" data-testid="project-status">
          {status
            ? `Last project: ${status.name} · ${status.names.toLocaleString("en-US")} names`
            : "No project yet. It's picked up the next time you dictate in a terminal or editor inside a project."}
        </p>
      )}
    </div>
  );
}

export function VocabularySection() {
  const [terms, setTerms] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<string[]>("get_vocabulary")
      .then(setTerms)
      .catch((err) => notifyError("get_vocabulary", err))
      .finally(() => setLoading(false));
  }, []);

  function save(next: string[]) {
    setTerms(next);
    invoke("set_vocabulary", { terms: next }).catch((err) => notifyError("set_vocabulary", err));
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

  function adoptBook(newWords: string[]) {
    if (newWords.length === 0) return;
    save([...terms, ...newWords]);
  }

  return (
    <>
      <ProjectNamesToggle />
      {loading ? null : <VocabularyBooksSection terms={terms} onAdopt={adoptBook} />}
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
    </>
  );
}

export type Snippet = { trigger: string; body: string };

export function SnippetsSection() {
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [loading, setLoading] = useState(true);
  const [triggerDraft, setTriggerDraft] = useState("");
  const [bodyDraft, setBodyDraft] = useState("");
  const [editingIndex, setEditingIndex] = useState<number | null>(null);

  useEffect(() => {
    invoke<Snippet[]>("get_snippets")
      .then(setSnippets)
      .catch((err) => notifyError("get_snippets", err))
      .finally(() => setLoading(false));
  }, []);

  function save(next: Snippet[]) {
    setSnippets(next);
    invoke("set_snippets", { snippets: next }).catch((err) => notifyError("set_snippets", err));
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
