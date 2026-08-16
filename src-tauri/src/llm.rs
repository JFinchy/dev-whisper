use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufRead;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

const OLLAMA_GENERATE_URL: &str = "http://localhost:11434/api/generate";
const OLLAMA_TAGS_URL: &str = "http://localhost:11434/api/tags";
const OLLAMA_PULL_URL: &str = "http://localhost:11434/api/pull";

/// Last observed `refine()` round-trip time per model, in milliseconds —
/// surfaced in Settings so a user picking between e.g. gemma3:1b and
/// mistral:7b can see the real speed difference on their own hardware
/// rather than guessing from parameter count. Process-lifetime only
/// (resets on restart); persisting it wasn't worth the complexity since
/// this is a menu bar app that's typically left running.
static LAST_LATENCY_MS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();

fn latency_store() -> &'static Mutex<HashMap<String, u64>> {
    LAST_LATENCY_MS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn record_latency(model: &str, ms: u64) {
    latency_store()
        .lock()
        .unwrap()
        .insert(model.to_string(), ms);
}

fn last_latency_ms(model: &str) -> Option<u64> {
    latency_store().lock().unwrap().get(model).copied()
}

pub fn default_model() -> String {
    "qwen3.5:4b".to_string()
}

struct CatalogEntry {
    id: &'static str,
    label: &'static str,
    size_gb: f32,
}

/// Recommended models for dictation refinement — deliberately small/fast
/// (1-4B range), since this runs after every recording and needs to feel
/// instant, not deliberate. Ollama's official library names, current as of
/// writing; tags can move, so pulling gracefully surfaces a real error
/// from Ollama if a name is ever stale rather than silently failing.
const CATALOG: &[CatalogEntry] = &[
    CatalogEntry { id: "gemma3:1b", label: "Gemma 3 (1B) — fastest", size_gb: 0.8 },
    CatalogEntry { id: "gemma3:4b", label: "Gemma 3 (4B) — balanced", size_gb: 3.3 },
    CatalogEntry { id: "llama3.2:1b", label: "Llama 3.2 (1B) — fastest", size_gb: 1.3 },
    CatalogEntry { id: "llama3.2:3b", label: "Llama 3.2 (3B) — balanced", size_gb: 2.0 },
    CatalogEntry { id: "phi3.5", label: "Phi-3.5 Mini (3.8B)", size_gb: 2.2 },
    CatalogEntry { id: "qwen2.5:3b", label: "Qwen 2.5 (3B)", size_gb: 1.9 },
    CatalogEntry { id: "mistral:7b", label: "Mistral (7B) — larger, more capable", size_gb: 4.1 },
];

#[derive(Serialize)]
pub struct LlmModelStatus {
    pub id: String,
    pub label: String,
    pub size_gb: f32,
    pub downloaded: bool,
    pub last_latency_ms: Option<u64>,
}

/// Catalog entries plus any already-pulled models not in the catalog
/// (e.g. whatever the user already had via `ollama pull` before this
/// feature existed), so nothing already installed becomes invisible.
#[tauri::command]
pub fn list_llm_catalog() -> Vec<LlmModelStatus> {
    let pulled = list_ollama_models();
    let mut result: Vec<LlmModelStatus> = CATALOG
        .iter()
        .map(|m| LlmModelStatus {
            id: m.id.to_string(),
            label: m.label.to_string(),
            size_gb: m.size_gb,
            downloaded: pulled.iter().any(|p| p == m.id),
            last_latency_ms: last_latency_ms(m.id),
        })
        .collect();

    for p in &pulled {
        if !result.iter().any(|r| &r.id == p) {
            result.push(LlmModelStatus {
                id: p.clone(),
                label: p.clone(),
                size_gb: 0.0,
                downloaded: true,
                last_latency_ms: last_latency_ms(p),
            });
        }
    }
    result
}

fn pull_ollama_model(app: &AppHandle, id: &str) -> Result<(), String> {
    let body = serde_json::json!({ "name": id, "stream": true });
    let resp = ureq::post(OLLAMA_PULL_URL)
        .timeout(std::time::Duration::from_secs(30 * 60))
        .send_json(body)
        .map_err(|e| format!("Ollama pull request failed (is `ollama serve` running?): {e}"))?;

    let reader = std::io::BufReader::new(resp.into_reader());
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(err.to_string());
        }
        let status = parsed.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let completed = parsed.get("completed").and_then(|v| v.as_u64());
        let total = parsed.get("total").and_then(|v| v.as_u64());
        let percent = match (completed, total) {
            (Some(c), Some(t)) if t > 0 => Some(((c as f64 / t as f64) * 100.0) as u32),
            _ => None,
        };
        let _ = app.emit(
            "llm-pull-progress",
            serde_json::json!({ "id": id, "status": status, "percent": percent }),
        );
        if status == "success" {
            return Ok(());
        }
    }
    Err("Ollama closed the connection before confirming the pull finished".to_string())
}

#[tauri::command]
pub fn pull_llm_model(app: AppHandle, id: String) {
    std::thread::spawn(move || match pull_ollama_model(&app, &id) {
        Ok(()) => {
            let _ = app.emit("llm-pull-complete", &id);
        }
        Err(err) => {
            let _ = app.emit(
                "llm-pull-error",
                serde_json::json!({ "id": id, "error": err }),
            );
        }
    });
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagEntry>,
}

#[derive(Deserialize)]
struct TagEntry {
    name: String,
}

fn prompt_for_mode(mode: crate::modes::Mode, text: &str) -> String {
    let instruction = match mode {
        crate::modes::Mode::Cli => {
            "Rewrite the following dictated text as a single shell command. Output ONLY the \
             command itself — no explanation, no markdown code fences, no surrounding quotes. \
             If it doesn't actually describe a command, just clean up filler words and output \
             the cleaned text instead."
        }
        crate::modes::Mode::Casual => {
            "Clean up this dictated text: fix filler words, false starts, and grammar, but keep \
             it casual and conversational — don't make it sound formal. Output ONLY the cleaned \
             text, nothing else."
        }
        crate::modes::Mode::Plain => {
            "Clean up this dictated text: fix filler words, false starts, and obvious \
             transcription errors, but don't change its meaning or tone. Output ONLY the \
             cleaned text, nothing else."
        }
    };
    format!("{instruction}\n\nDictated text: {text}")
}

/// Sends the transcript to a locally-running Ollama instance for
/// mode-aware cleanup. Returns a clear error (rather than hanging or
/// panicking) if Ollama isn't installed/running — callers should fall back
/// to the un-refined text rather than blocking the paste on this.
pub fn refine(mode: crate::modes::Mode, text: &str, model: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt_for_mode(mode, text),
        "stream": false,
        // Dictation cleanup needs to be fast, not deliberate — reasoning
        // ("thinking") models otherwise spend many seconds on an internal
        // chain-of-thought before ever answering. Ignored by models that
        // don't support it.
        "think": false,
    });

    let started = Instant::now();
    let resp = ureq::post(OLLAMA_GENERATE_URL)
        .timeout(std::time::Duration::from_secs(30))
        .send_json(body)
        .map_err(|e| format!("Ollama request failed (is `ollama serve` running?): {e}"))?;

    let parsed: GenerateResponse = resp
        .into_json()
        .map_err(|e| format!("failed to parse Ollama response: {e}"))?;

    // Only recorded on a successful round-trip — a failed/timed-out request
    // isn't a meaningful "how fast is this model" data point.
    record_latency(model, started.elapsed().as_millis() as u64);

    let cleaned = parsed.response.trim().to_string();
    if cleaned.is_empty() {
        Err("Ollama returned an empty response".to_string())
    } else {
        Ok(cleaned)
    }
}

/// Below this word count, a dictation is already about as short as a
/// one-line summary would be — skip the LLM call entirely rather than
/// paying for a round-trip to produce "Said hello" for a two-word message.
const JOURNAL_MIN_WORDS: usize = 6;

/// Returns `None` (not an error) when the text is too short to be worth
/// summarizing — distinct from `Some(Err(_))`, an actual failed attempt.
pub fn summarize_for_journal(text: &str, model: &str) -> Option<Result<String, String>> {
    if text.split_whitespace().count() < JOURNAL_MIN_WORDS {
        return None;
    }

    let instruction = "Summarize the following dictated text as a single short line for a work \
                        journal, in the style of a git commit subject: imperative mood, no \
                        trailing period, under 10 words. Output ONLY the summary line, nothing \
                        else.";
    let body = serde_json::json!({
        "model": model,
        "prompt": format!("{instruction}\n\nDictated text: {text}"),
        "stream": false,
        "think": false,
    });

    let started = Instant::now();
    let resp = match ureq::post(OLLAMA_GENERATE_URL)
        .timeout(std::time::Duration::from_secs(30))
        .send_json(body)
    {
        Ok(resp) => resp,
        Err(e) => return Some(Err(format!("Ollama request failed (is `ollama serve` running?): {e}"))),
    };

    let parsed: GenerateResponse = match resp.into_json() {
        Ok(parsed) => parsed,
        Err(e) => return Some(Err(format!("failed to parse Ollama response: {e}"))),
    };

    record_latency(model, started.elapsed().as_millis() as u64);

    let summary = parsed.response.trim().trim_matches('"').to_string();
    if summary.is_empty() {
        Some(Err("Ollama returned an empty response".to_string()))
    } else {
        Some(Ok(summary))
    }
}

/// Small models often ignore "no markdown fences" instructions and wrap
/// the response in a fenced code block anyway — strip one leading/trailing
/// fence if present so the output pastes as raw code, not fenced markdown.
fn strip_markdown_fences(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let Some(after_open) = trimmed.find('\n').map(|idx| &trimmed[idx + 1..]) else {
        return trimmed.to_string();
    };
    match after_open.rfind("```") {
        Some(idx) => after_open[..idx].trim_end().to_string(),
        None => after_open.trim_end().to_string(),
    }
}

/// Sends a natural-language "generate boilerplate for X" request (see
/// boilerplate.rs) to Ollama for code generation. Separate from `refine()`
/// since code generation is a heavier task than dictation cleanup and
/// warrants a longer timeout.
pub fn generate_boilerplate(request: &str, model: &str) -> Result<String, String> {
    let instruction = "Generate the code requested below. Output ONLY the code itself — no \
                        explanation, no markdown code fences, no surrounding commentary. If the \
                        request doesn't specify a language, infer the most likely one from \
                        context.";
    let body = serde_json::json!({
        "model": model,
        "prompt": format!("{instruction}\n\nRequest: {request}"),
        "stream": false,
        "think": false,
    });

    let started = Instant::now();
    let resp = ureq::post(OLLAMA_GENERATE_URL)
        // Code generation is a heavier task than dictation cleanup and can
        // legitimately take longer than refine()'s 30s budget.
        .timeout(std::time::Duration::from_secs(60))
        .send_json(body)
        .map_err(|e| format!("Ollama request failed (is `ollama serve` running?): {e}"))?;

    let parsed: GenerateResponse = resp
        .into_json()
        .map_err(|e| format!("failed to parse Ollama response: {e}"))?;

    record_latency(model, started.elapsed().as_millis() as u64);

    let code = strip_markdown_fences(&parsed.response);
    if code.is_empty() {
        Err("Ollama returned an empty response".to_string())
    } else {
        Ok(code)
    }
}

/// Names of locally-pulled Ollama models, for the Settings picker. Empty
/// (not an error) if Ollama isn't reachable, so the UI can show "Ollama
/// not running" rather than a raw error.
#[tauri::command]
pub fn list_ollama_models() -> Vec<String> {
    let Ok(resp) = ureq::get(OLLAMA_TAGS_URL).call() else {
        return Vec::new();
    };
    let Ok(parsed) = resp.into_json::<TagsResponse>() else {
        return Vec::new();
    };
    parsed.models.into_iter().map(|m| m.name).collect()
}

#[tauri::command]
pub fn get_llm_model(app: tauri::AppHandle) -> String {
    crate::config::load(&app).llm_model
}

#[tauri::command]
pub fn set_llm_model(app: tauri::AppHandle, model: String) {
    let mut cfg = crate::config::load(&app);
    cfg.llm_model = model;
    let _ = crate::config::save(&app, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test against a real, locally-running Ollama instance — skips
    /// (doesn't fail) if Ollama isn't reachable, e.g. in CI or on a machine
    /// that hasn't installed it. Verifies the actual HTTP/JSON integration,
    /// not just that the code compiles.
    #[test]
    fn refines_text_via_real_ollama() {
        let models = list_ollama_models();
        if models.is_empty() {
            eprintln!("skipping: no Ollama models found (is `ollama serve` running?)");
            return;
        }
        let model = &models[0];

        let result = refine(
            crate::modes::Mode::Cli,
            "git commit update the readme file",
            model,
        );

        match result {
            Ok(text) => {
                assert!(!text.is_empty(), "refined text should not be empty");
                eprintln!("Ollama ({model}) refined CLI text to: {text:?}");
            }
            Err(err) => panic!("Ollama refinement failed: {err}"),
        }

        let latency = last_latency_ms(model);
        assert!(latency.is_some(), "successful refine() should record a latency");
        eprintln!("Ollama ({model}) refine() latency: {}ms", latency.unwrap());
    }

    #[test]
    fn strip_markdown_fences_removes_fenced_block_with_language_tag() {
        let input = "```python\ndef add(a, b):\n    return a + b\n```";
        assert_eq!(strip_markdown_fences(input), "def add(a, b):\n    return a + b");
    }

    #[test]
    fn strip_markdown_fences_removes_fenced_block_without_language_tag() {
        let input = "```\nlet x = 1;\n```";
        assert_eq!(strip_markdown_fences(input), "let x = 1;");
    }

    #[test]
    fn strip_markdown_fences_leaves_unfenced_text_unchanged() {
        assert_eq!(strip_markdown_fences("let x = 1;"), "let x = 1;");
    }

    /// Smoke test against a real, locally-running Ollama instance — skips
    /// if Ollama isn't reachable, matching `refines_text_via_real_ollama`.
    #[test]
    fn generates_boilerplate_via_real_ollama() {
        let models = list_ollama_models();
        if models.is_empty() {
            eprintln!("skipping: no Ollama models found (is `ollama serve` running?)");
            return;
        }
        let model = &models[0];

        let result = generate_boilerplate("a Python function that adds two numbers", model);

        match result {
            Ok(code) => {
                assert!(!code.is_empty(), "generated code should not be empty");
                assert!(
                    !code.trim_start().starts_with("```"),
                    "output should have markdown fences stripped, got: {code:?}"
                );
                eprintln!("Ollama ({model}) generated boilerplate: {code:?}");
            }
            Err(err) => panic!("boilerplate generation failed: {err}"),
        }
    }

    #[test]
    fn summarize_for_journal_skips_short_dictations() {
        assert!(summarize_for_journal("fix the bug", "any-model").is_none());
        assert!(summarize_for_journal("git commit", "any-model").is_none());
    }

    /// Smoke test against a real, locally-running Ollama instance — skips
    /// if Ollama isn't reachable, matching the other live tests in this
    /// file.
    #[test]
    fn summarizes_a_real_dictation_via_real_ollama() {
        let models = list_ollama_models();
        if models.is_empty() {
            eprintln!("skipping: no Ollama models found (is `ollama serve` running?)");
            return;
        }
        let model = &models[0];

        let result = summarize_for_journal(
            "okay so I need to go back and fix the transcribe and paste function because it's \
             not handling the case where the model override is set but the path doesn't resolve",
            model,
        );

        match result {
            Some(Ok(summary)) => {
                assert!(!summary.is_empty(), "summary should not be empty");
                eprintln!("Ollama ({model}) journal summary: {summary:?}");
            }
            Some(Err(err)) => panic!("journal summarization failed: {err}"),
            None => panic!("dictation was long enough that it should not have been skipped"),
        }
    }
}
