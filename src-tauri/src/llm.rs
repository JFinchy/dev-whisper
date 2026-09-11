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

fn prompt_for_mode(mode: crate::modes::Behavior, text: &str, custom_instructions: Option<&str>) -> String {
    let instruction = match mode {
        crate::modes::Behavior::Cli => {
            "Rewrite the following dictated text as a single shell command. Output ONLY the \
             command itself — no explanation, no markdown code fences, no surrounding quotes. \
             If it doesn't actually describe a command, just clean up filler words and output \
             the cleaned text instead."
        }
        crate::modes::Behavior::Casual => {
            "Clean up this dictated text: fix filler words, false starts, and grammar, but keep \
             it casual and conversational — don't make it sound formal. Output ONLY the cleaned \
             text, nothing else."
        }
        crate::modes::Behavior::Plain => {
            "Clean up this dictated text: fix filler words, false starts, and obvious \
             transcription errors, but don't change its meaning or tone. Output ONLY the \
             cleaned text, nothing else."
        }
    };
    // User-supplied per-mode steering (e.g. "sign off messages with 'thanks, Jake'"),
    // folded in as its own instruction line so it can't be confused with the
    // dictated text itself. Skipped entirely when unset/blank rather than
    // appending an empty "Additional instructions:" line for every refinement.
    match custom_instructions.map(str::trim).filter(|s| !s.is_empty()) {
        Some(extra) => format!("{instruction}\n\nAdditional instructions from the user: {extra}\n\nDictated text: {text}"),
        None => format!("{instruction}\n\nDictated text: {text}"),
    }
}

/// Sends the transcript to a locally-running Ollama instance for
/// mode-aware cleanup. Returns a clear error (rather than hanging or
/// panicking) if Ollama isn't installed/running — callers should fall back
/// to the un-refined text rather than blocking the paste on this.
pub fn refine(
    mode: crate::modes::Behavior,
    text: &str,
    model: &str,
    custom_instructions: Option<&str>,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt_for_mode(mode, text, custom_instructions),
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

    // Same "small models ignore 'no markdown fences' instructions" issue
    // `generate_boilerplate` already guards against below — none of the
    // three `prompt_for_mode` instructions ask for code, but a model can
    // still wrap plain refined prose in a fenced block anyway (observed via
    // the eval suite: "make directory for the new service" refined to
    // "```bash\nmkdir -p\n```" instead of a bare command).
    let cleaned = strip_markdown_fences(parsed.response.trim());
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

    let summary = strip_markdown_fences(parsed.response.trim()).trim_matches('"').to_string();
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
            crate::modes::Behavior::Cli,
            "git commit update the readme file",
            model,
            None,
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
    fn prompt_for_mode_folds_in_custom_instructions_when_present() {
        let prompt = prompt_for_mode(crate::modes::Behavior::Plain, "hello", Some("be brief"));
        assert!(prompt.contains("Additional instructions from the user: be brief"));
    }

    #[test]
    fn prompt_for_mode_omits_custom_instructions_block_when_blank_or_absent() {
        let without = prompt_for_mode(crate::modes::Behavior::Plain, "hello", None);
        assert!(!without.contains("Additional instructions"));
        let blank = prompt_for_mode(crate::modes::Behavior::Plain, "hello", Some("   "));
        assert!(!blank.contains("Additional instructions"));
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

    /// Loose, structural quality gates for `refine()` output — not
    /// exact-match assertions (the model's wording varies run to run), but
    /// checks shaped specifically around failure modes actually observed in
    /// manual testing:
    /// - a chat-tuned model wrapping its answer in a conversational
    ///   preamble ("Sure, here's...") instead of just returning the
    ///   cleaned text the prompt asked for,
    /// - leftover markdown fences,
    /// - collapsing free-form prose into a single-token slug (2026-09-09:
    ///   CLI mode's "rewrite as a single shell command" prompt did this to
    ///   ordinary agent-chat dictation — see `modes::is_literal_cli_directive`,
    ///   which now stops that input from reaching `refine()` at all in the
    ///   real pipeline, but the eval case below is kept to track whether the
    ///   underlying prompt/model combo itself ever gets more reliable),
    /// - gutting the message under a plain "clean up filler words" prompt
    ///   rather than lightly editing it.
    fn check_refinement_quality(
        mode: crate::modes::Behavior,
        input: &str,
        output: &str,
    ) -> Result<(), String> {
        if output.trim().is_empty() {
            return Err("output is empty".to_string());
        }
        if output.contains("```") {
            return Err(format!("output still has markdown fences: {output:?}"));
        }
        let lower = output.trim().to_lowercase();
        let preamble_markers = ["sure,", "sure!", "here's", "here is", "certainly", "of course"];
        if preamble_markers.iter().any(|m| lower.starts_with(m)) {
            return Err(format!(
                "output looks like a conversational preamble, not cleaned dictation: {output:?}"
            ));
        }

        let input_words = input.split_whitespace().count();
        let output_words = output.split_whitespace().count();
        match mode {
            crate::modes::Behavior::Cli => {
                // A real single-command rewrite is expected to be short —
                // but collapsing several words of prose into one no-space
                // token isn't a command, it's the slug failure mode.
                if input_words > 4 && output_words <= 1 && output.trim().len() > 20 {
                    return Err(format!(
                        "output collapsed {input_words} input words into a single token: {output:?}"
                    ));
                }
            }
            crate::modes::Behavior::Plain | crate::modes::Behavior::Casual => {
                // Cleanup should trim filler, not gut the message — a >70%
                // reduction on anything but a very short input reads as a
                // rewrite rather than a cleanup.
                if input_words >= 8 && output_words < input_words / 3 {
                    return Err(format!(
                        "output dropped from {input_words} to {output_words} words, looks \
                         rewritten rather than cleaned: {output:?}"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Representative dictations per mode, picked from real manual-testing
    /// reports rather than invented examples. Not exhaustive — add to this
    /// as new quality issues turn up in actual use, the same way the
    /// deterministic passes' unit tests grew from real Whisper output.
    const EVAL_CASES: &[(crate::modes::Behavior, &str)] = &[
        (crate::modes::Behavior::Cli, "git commit update the readme file"),
        (crate::modes::Behavior::Cli, "make directory for the new service"),
        (
            crate::modes::Behavior::Plain,
            "um so I think we should probably go with the the second approach because it's \
             it's simpler and also easier to test",
        ),
        (
            crate::modes::Behavior::Casual,
            "hey can you like send that file over when you get a sec no rush at all",
        ),
    ];

    /// Runs `EVAL_CASES` against a real, locally-running Ollama instance and
    /// checks each output with `check_refinement_quality`. A smoke-test net
    /// for real-world prompt/model quality regressions — not a strict
    /// correctness check, since LLM output isn't deterministic — matching
    /// the "skip if Ollama unreachable" convention of the other live tests
    /// in this file rather than gating on it with `#[ignore]`.
    #[test]
    fn eval_suite_against_real_ollama() {
        let models = list_ollama_models();
        if models.is_empty() {
            eprintln!("skipping: no Ollama models found (is `ollama serve` running?)");
            return;
        }
        let model = &models[0];

        let mut failures = Vec::new();
        for &(mode, input) in EVAL_CASES {
            // Mirrors `recording.rs`'s real pipeline: `apply_mode`'s
            // mechanical pass runs first, and `refine()` only ever sees
            // its output, never the raw transcript directly — skipping
            // this step here made an earlier version of this suite
            // exercise `refine()` on text production never actually sends
            // it (e.g. literal "make directory ..." instead of the
            // already-`mkdir`-formatted string).
            let pre_formatted = crate::modes::apply_mode(mode, input);
            match refine(mode, &pre_formatted, model, None) {
                Ok(output) => {
                    eprintln!("[{mode:?}] {input:?} -> (pre-formatted: {pre_formatted:?}) -> {output:?}");
                    if let Err(reason) = check_refinement_quality(mode, &pre_formatted, &output) {
                        failures.push(format!("{mode:?} {input:?}: {reason}"));
                    }
                }
                Err(err) => failures.push(format!("{mode:?} {input:?}: refine() failed: {err}")),
            }
        }

        assert!(failures.is_empty(), "eval suite failures:\n{}", failures.join("\n"));
    }

    /// Documents a known, still-open weakness in the raw Cli prompt/model
    /// combo rather than the pipeline as a whole: `refine()` called
    /// directly (bypassing `modes::is_literal_cli_directive`'s production
    /// gate) on ordinary agent-chat prose reliably reproduces the
    /// 2026-09-09 slug bug. `#[ignore]`d so a perpetually-known model
    /// limitation doesn't keep the default suite red — run explicitly
    /// (`cargo test --lib -- --ignored eval_suite_prompt_weakness`) after a
    /// model/prompt change to see whether this has actually improved.
    #[test]
    #[ignore]
    fn eval_suite_prompt_weakness_cli_mode_mishandles_agent_chat_prose() {
        let models = list_ollama_models();
        if models.is_empty() {
            eprintln!("skipping: no Ollama models found (is `ollama serve` running?)");
            return;
        }
        let model = &models[0];
        let input = "Let's test and see what happens when we try and paste in here. If it \
                     truly is just a problem with this terminal, we still need to work it out.";

        let pre_formatted = crate::modes::apply_mode(crate::modes::Behavior::Cli, input);
        let output = refine(crate::modes::Behavior::Cli, &pre_formatted, model, None)
            .expect("refine() call itself should succeed");
        eprintln!("[Cli] {input:?} -> (pre-formatted: {pre_formatted:?}) -> {output:?}");

        // Intentionally asserting the *bad* outcome still reproduces —
        // this test passing means the underlying weakness is still present
        // (expected today); it failing is good news worth investigating.
        check_refinement_quality(crate::modes::Behavior::Cli, &pre_formatted, &output)
            .expect_err("expected the known slug-collapse weakness to still reproduce");
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
