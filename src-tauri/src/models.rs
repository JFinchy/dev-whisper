use std::io::{Read, Write};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config;
use crate::recording::RecordingState;

pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
}

pub const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "tiny.en",
        label: "Tiny (English) — fastest, least accurate",
        filename: "ggml-tiny.en-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en-q5_1.bin",
        size_mb: 32,
    },
    ModelInfo {
        id: "base.en",
        label: "Base (English) — balanced",
        filename: "ggml-base.en-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin",
        size_mb: 57,
    },
    ModelInfo {
        id: "small.en",
        label: "Small (English) — more accurate, slower",
        filename: "ggml-small.en-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en-q5_1.bin",
        size_mb: 190,
    },
];

pub fn default_model_id() -> &'static str {
    "base.en"
}

fn find(id: &str) -> Option<&'static ModelInfo> {
    CATALOG.iter().find(|m| m.id == id)
}

fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn model_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let info = find(id).ok_or_else(|| format!("unknown model: {id}"))?;
    Ok(models_dir(app)?.join(info.filename))
}

/// Dev convenience: fall back to the repo-local model fetched via
/// scripts/download-model.sh if it hasn't been downloaded into
/// app_data_dir yet (e.g. right after this feature shipped).
fn dev_fallback_path(id: &str) -> Option<PathBuf> {
    let info = find(id)?;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join(info.filename);
    path.exists().then_some(path)
}

pub fn resolve_model_path(app: &AppHandle, id: &str) -> Option<PathBuf> {
    if let Ok(primary) = model_path(app, id) {
        if primary.exists() {
            return Some(primary);
        }
    }
    dev_fallback_path(id)
}

#[derive(serde::Serialize)]
pub struct ModelStatus {
    pub id: String,
    pub label: String,
    pub size_mb: u32,
    pub downloaded: bool,
    pub active: bool,
}

#[tauri::command]
pub fn list_models(app: AppHandle, state: State<RecordingState>) -> Vec<ModelStatus> {
    let active = state.whisper.active_model_id();
    CATALOG
        .iter()
        .map(|m| ModelStatus {
            id: m.id.to_string(),
            label: m.label.to_string(),
            size_mb: m.size_mb,
            downloaded: resolve_model_path(&app, m.id).is_some(),
            active: m.id == active,
        })
        .collect()
}

#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    let info = find(&id).ok_or_else(|| format!("unknown model: {id}"))?;
    let dest = model_path(&app, &id)?;
    if dest.exists() {
        return Ok(());
    }

    let url = info.url;
    std::thread::spawn(move || {
        let result = download_to_file(&app, &id, url, &dest);
        match result {
            Ok(()) => {
                let _ = app.emit("model-download-complete", &id);
            }
            Err(err) => {
                let _ = app.emit(
                    "model-download-error",
                    serde_json::json!({ "id": id, "error": err }),
                );
            }
        }
    });

    Ok(())
}

fn download_to_file(app: &AppHandle, id: &str, url: &str, dest: &std::path::Path) -> Result<(), String> {
    let resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let total: Option<u64> = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok());
    let mut reader = resp.into_reader();

    let tmp_path = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    let mut last_reported = u32::MAX;

    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;

        if let Some(total) = total {
            let percent = ((downloaded as f64 / total as f64) * 100.0) as u32;
            if percent != last_reported {
                last_reported = percent;
                let _ = app.emit(
                    "model-download-progress",
                    serde_json::json!({ "id": id, "percent": percent }),
                );
            }
        }
    }
    drop(file);

    std::fs::rename(&tmp_path, dest).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_active_model(app: AppHandle, id: String, state: State<RecordingState>) -> Result<(), String> {
    let path = resolve_model_path(&app, &id).ok_or_else(|| "model not downloaded".to_string())?;
    state.whisper.set_model(id.clone(), path);

    let mut cfg = config::load(&app);
    cfg.active_model = Some(id);
    let _ = config::save(&app, &cfg);
    Ok(())
}
