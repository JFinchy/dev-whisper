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
    /// SHA-256 of the file at `url`, from Hugging Face's LFS metadata. A
    /// download that doesn't match is deleted rather than loaded, so a
    /// truncated transfer or a tampered mirror can't become the model.
    pub sha256: &'static str,
}

/// URLs are pinned to this whisper.cpp repo revision rather than `main`, so
/// the files (and the hashes above) can't change underneath a release.
macro_rules! model_url {
    ($file:literal) => {
        concat!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/",
            $file
        )
    };
}

pub const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "tiny.en",
        label: "Tiny (English) — fastest, least accurate",
        filename: "ggml-tiny.en-q5_1.bin",
        url: model_url!("ggml-tiny.en-q5_1.bin"),
        size_mb: 32,
        sha256: "c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b",
    },
    ModelInfo {
        id: "base.en",
        label: "Base (English) — balanced",
        filename: "ggml-base.en-q5_1.bin",
        url: model_url!("ggml-base.en-q5_1.bin"),
        size_mb: 57,
        sha256: "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f",
    },
    ModelInfo {
        id: "small.en",
        label: "Small (English) — more accurate, slower",
        filename: "ggml-small.en-q5_1.bin",
        url: model_url!("ggml-small.en-q5_1.bin"),
        size_mb: 190,
        sha256: "bfdff4894dcb76bbf647d56263ea2a96645423f1669176f4844a1bf8e478ad30",
    },
];

/// Short display name ("Base (English)") for readiness messages.
pub fn label_for(id: &str) -> String {
    find(id).map_or_else(
        || id.to_string(),
        |m| m.label.split(" — ").next().unwrap_or(m.label).to_string(),
    )
}

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

    let (url, sha256) = (info.url, info.sha256);
    std::thread::spawn(move || {
        let result = download_to_file(&app, &id, url, sha256, &dest);
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

fn download_to_file(
    app: &AppHandle,
    id: &str,
    url: &str,
    sha256: &str,
    dest: &std::path::Path,
) -> Result<(), String> {
    let tmp_path = dest.with_extension("part");
    let result = stream_to_file(app, id, url, sha256, &tmp_path)
        .and_then(|()| std::fs::rename(&tmp_path, dest).map_err(|e| e.to_string()));
    if result.is_err() {
        // Never leave a partial file behind to be mistaken for progress.
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

fn stream_to_file(
    app: &AppHandle,
    id: &str,
    url: &str,
    sha256: &str,
    tmp_path: &std::path::Path,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    // Connect/read timeouts, not an overall one: a 190MB model on a slow
    // link legitimately takes minutes, but a stalled connection shouldn't
    // hang the download (and its progress bar) forever.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(30))
        .build();
    let resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let total: Option<u64> = resp.header("Content-Length").and_then(|s| s.parse().ok());
    let mut reader = resp.into_reader();

    let mut file = std::fs::File::create(tmp_path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    let mut last_reported = u32::MAX;

    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
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
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);

    let actual = hex(&hasher.finalize());
    if actual != sha256 {
        crate::applog!("models: {id} checksum mismatch (expected {sha256}, got {actual})");
        return Err("The download was corrupted (checksum mismatch). Try again.".to_string());
    }
    crate::applog!("models: {id} downloaded and verified ({downloaded} bytes)");
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tauri::command]
pub fn set_active_model(
    app: AppHandle,
    id: String,
    state: State<RecordingState>,
) -> Result<(), String> {
    let path = resolve_model_path(&app, &id).ok_or_else(|| "model not downloaded".to_string())?;
    state.whisper.set_default_model(id.clone(), path);

    let mut cfg = config::load(&app);
    cfg.active_model = Some(id);
    let _ = config::save(&app, &cfg);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_entries_are_pinned_and_hashed() {
        for model in CATALOG {
            assert!(
                model.url.ends_with(model.filename),
                "{} url/filename mismatch",
                model.id
            );
            assert!(
                !model.url.contains("/resolve/main/"),
                "{} url isn't pinned to a revision",
                model.id
            );
            assert_eq!(
                model.sha256.len(),
                64,
                "{} sha256 isn't a hex SHA-256",
                model.id
            );
            assert!(model
                .sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
        assert!(find(default_model_id()).is_some());
    }

    #[test]
    fn hex_encodes_lowercase() {
        assert_eq!(hex(&[0x00, 0xab, 0x0f]), "00ab0f");
    }
}
