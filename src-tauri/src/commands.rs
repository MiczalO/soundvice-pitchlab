use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::ai::{anthropic, gemini};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::secrets::{self, Secret};
use crate::settings;

#[tauri::command]
pub fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn set_secret(name: Secret, value: String) -> AppResult<()> {
    secrets::set(name, &value)
}

#[tauri::command]
pub fn has_secret(name: Secret) -> bool {
    secrets::has(name)
}

#[tauri::command]
pub fn delete_secret(name: Secret) -> AppResult<()> {
    secrets::delete(name)
}

#[tauri::command]
pub fn get_settings(db: State<Db>) -> AppResult<HashMap<String, String>> {
    let conn =
        db.0.lock()
            .map_err(|_| AppError::Db("blokada bazy".into()))?;
    settings::get_all(&conn)
}

#[tauri::command]
pub fn set_setting(db: State<Db>, key: String, value: String) -> AppResult<()> {
    let conn =
        db.0.lock()
            .map_err(|_| AppError::Db("blokada bazy".into()))?;
    settings::set(&conn, &key, &value)
}

#[tauri::command]
pub async fn list_gemini_models() -> AppResult<Vec<String>> {
    gemini::list_models().await
}

#[derive(Serialize)]
pub struct ConnectionReport {
    pub gemini: Option<String>,
    pub claude: Option<String>,
}

/// Sprawdza Gemini (zawsze) i Claude (jeśli silnik pitchu = claude).
#[tauri::command]
pub async fn test_connection(
    gemini_model: Option<String>,
    claude_model: Option<String>,
    check_claude: bool,
) -> AppResult<ConnectionReport> {
    let gm = gemini_model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| gemini::FALLBACK_MODEL.into());
    let g = gemini::ping(&gm).await?;
    let c = if check_claude {
        let cm = claude_model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| anthropic::DEFAULT_MODEL.into());
        Some(anthropic::ping(&cm).await?)
    } else {
        None
    };
    Ok(ConnectionReport {
        gemini: Some(g),
        claude: c,
    })
}

// ---------- audio i analiza ----------

#[derive(Serialize)]
pub struct PreparedAudio {
    #[serde(flatten)]
    pub audio: crate::audio::ImportedAudio,
    pub waveform: Option<crate::audio::Waveform>,
}

/// Kopiuje plik do katalogu danych i liczy waveform. Brak dekodera nie blokuje
/// analizy — Gemini i tak przyjmie plik, więc waveform jest wtedy `None`.
#[tauri::command]
pub async fn prepare_audio(app: tauri::AppHandle, path: String) -> AppResult<PreparedAudio> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(e.to_string()))?
        .join("audio");
    tauri::async_runtime::spawn_blocking(move || {
        let audio = crate::audio::import(std::path::Path::new(&path), &dir)?;
        let waveform = crate::audio::waveform(std::path::Path::new(&audio.path)).ok();
        Ok(PreparedAudio { audio, waveform })
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub fn start_analysis(
    app: tauri::AppHandle,
    jobs: State<crate::jobs::Jobs>,
    params: crate::pipeline::AnalysisParams,
) -> u64 {
    use crate::jobs::{DoneEvent, emit_done};
    let (job_id, cancel) = jobs.start();
    tauri::async_runtime::spawn(async move {
        let res = crate::pipeline::run(&app, job_id, &cancel, &params).await;
        let ev = match res {
            Ok(track_id) => DoneEvent {
                job_id,
                track_id: Some(track_id),
                cancelled: false,
                error: None,
            },
            Err(AppError::Cancelled) => DoneEvent {
                job_id,
                track_id: None,
                cancelled: true,
                error: None,
            },
            Err(e) => DoneEvent {
                job_id,
                track_id: None,
                cancelled: false,
                error: Some(e),
            },
        };
        use tauri::Manager;
        app.state::<crate::jobs::Jobs>().finish(job_id);
        emit_done(&app, ev);
    });
    job_id
}

#[tauri::command]
pub fn cancel_job(jobs: State<crate::jobs::Jobs>, job_id: u64) {
    jobs.cancel(job_id);
}

#[tauri::command]
pub fn get_track(db: State<Db>, track_id: i64) -> AppResult<Option<crate::db::repo::TrackView>> {
    let conn =
        db.0.lock()
            .map_err(|_| AppError::Db("blokada bazy".into()))?;
    crate::db::repo::get_track(&conn, track_id)
}
