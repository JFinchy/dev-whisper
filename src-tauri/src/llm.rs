use serde::Deserialize;

const OLLAMA_GENERATE_URL: &str = "http://localhost:11434/api/generate";
const OLLAMA_TAGS_URL: &str = "http://localhost:11434/api/tags";

pub fn default_model() -> String {
    "qwen3.5:4b".to_string()
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

    let resp = ureq::post(OLLAMA_GENERATE_URL)
        .timeout(std::time::Duration::from_secs(30))
        .send_json(body)
        .map_err(|e| format!("Ollama request failed (is `ollama serve` running?): {e}"))?;

    let parsed: GenerateResponse = resp
        .into_json()
        .map_err(|e| format!("failed to parse Ollama response: {e}"))?;

    let cleaned = parsed.response.trim().to_string();
    if cleaned.is_empty() {
        Err("Ollama returned an empty response".to_string())
    } else {
        Ok(cleaned)
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
    }
}
