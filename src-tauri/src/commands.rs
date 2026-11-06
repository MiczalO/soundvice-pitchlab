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

/// Twardy limit czasu dla krótkich zapytań (lista modeli, test połączenia).
async fn within<T>(
    secs: u64,
    fut: impl std::future::Future<Output = AppResult<T>>,
) -> AppResult<T> {
    tokio::time::timeout(std::time::Duration::from_secs(secs), fut)
        .await
        .unwrap_or(Err(AppError::Timeout))
}

#[tauri::command]
pub async fn list_gemini_models() -> AppResult<Vec<String>> {
    within(45, gemini::list_models()).await
}

/// Sprawdza, które modele faktycznie odpowiadają dla tego klucza i planu.
#[tauri::command]
pub async fn probe_gemini_models(models: Vec<String>) -> AppResult<Vec<gemini::Probe>> {
    within(60, gemini::probe_models(models)).await
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
    let g = within(60, gemini::ping(&gm)).await?;
    let c = if check_claude {
        let cm = claude_model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| anthropic::DEFAULT_MODEL.into());
        Some(within(60, anthropic::ping(&cm)).await?)
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

/// Zamienia wynik joba na zdarzenie końcowe dla UI.
fn done_event(job_id: u64, track_id: Option<i64>, res: AppResult<()>) -> crate::jobs::DoneEvent {
    use crate::jobs::DoneEvent;
    match res {
        Ok(()) => DoneEvent {
            job_id,
            track_id,
            cancelled: false,
            error: None,
        },
        Err(AppError::Cancelled) => DoneEvent {
            job_id,
            track_id,
            cancelled: true,
            error: None,
        },
        Err(e) => DoneEvent {
            job_id,
            track_id,
            cancelled: false,
            error: Some(e),
        },
    }
}

#[tauri::command]
pub fn start_analysis(
    app: tauri::AppHandle,
    jobs: State<crate::jobs::Jobs>,
    params: crate::pipeline::AnalysisParams,
) -> u64 {
    let (job_id, cancel) = jobs.start();
    tauri::async_runtime::spawn(async move {
        let (track_id, res) = crate::pipeline::run(&app, job_id, &cancel, &params).await;
        use tauri::Manager;
        app.state::<crate::jobs::Jobs>().finish(job_id);
        crate::jobs::emit_done(&app, done_event(job_id, track_id, res));
    });
    job_id
}

/// Research istniejącego artysty (z karty Artyści): tworzy propozycję albo pierwszą pigułkę.
#[tauri::command]
pub fn start_research(
    app: tauri::AppHandle,
    jobs: State<crate::jobs::Jobs>,
    artist_id: i64,
) -> u64 {
    let (job_id, cancel) = jobs.start();
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;
        // Kontekst: ostatni utwór artysty (tytuł + gatunek z analizy).
        let ctx = {
            let db = app.state::<Db>();
            db.0.lock().ok().and_then(|conn| {
                let tid: i64 = conn
                    .query_row(
                        "SELECT id FROM tracks WHERE artist_id=?1 ORDER BY id DESC LIMIT 1",
                        [artist_id],
                        |r| r.get(0),
                    )
                    .ok()?;
                let t = crate::db::repo::get_track(&conn, tid).ok()??;
                Some((
                    t.title,
                    t.analysis
                        .and_then(|a| a["primary_genre"].as_str().map(String::from))
                        .unwrap_or_default(),
                ))
            })
        };
        let res = crate::research::run(&app, job_id, &cancel, artist_id, ctx)
            .await
            .map(|_| ());
        app.state::<crate::jobs::Jobs>().finish(job_id);
        crate::jobs::emit_done(&app, done_event(job_id, None, res));
    });
    job_id
}

// ---------- artyści ----------

fn lock<'a>(db: &'a State<'_, Db>) -> AppResult<std::sync::MutexGuard<'a, rusqlite::Connection>> {
    db.0.lock().map_err(|_| AppError::Db("blokada bazy".into()))
}

#[tauri::command]
pub fn list_artists(db: State<Db>) -> AppResult<Vec<crate::db::artists::ArtistRow>> {
    crate::db::artists::list(&*lock(&db)?)
}

#[tauri::command]
pub fn get_artist(db: State<Db>, id: i64) -> AppResult<Option<crate::db::artists::ArtistDetail>> {
    crate::db::artists::detail(&*lock(&db)?, id)
}

#[tauri::command]
pub fn create_artist(db: State<Db>, name: String) -> AppResult<i64> {
    if name.trim().is_empty() {
        return Err(AppError::Other("Wpisz nazwę artysty.".into()));
    }
    crate::db::repo::ensure_artist(&*lock(&db)?, &name)
}

#[tauri::command]
pub fn update_artist(
    db: State<Db>,
    id: i64,
    pill: String,
    manager_notes: String,
    research_hints: String,
) -> AppResult<()> {
    crate::db::artists::update(&*lock(&db)?, id, &pill, &manager_notes, &research_hints)
}

#[tauri::command]
pub fn delete_artist(db: State<Db>, id: i64) -> AppResult<()> {
    crate::db::artists::delete(&*lock(&db)?, id)
}

#[tauri::command]
pub fn accept_proposal(
    db: State<Db>,
    proposal_id: i64,
    mode: crate::db::artists::AcceptMode,
) -> AppResult<()> {
    crate::db::artists::accept_proposal(&mut *lock(&db)?, proposal_id, mode)
}

#[tauri::command]
pub fn reject_proposal(db: State<Db>, proposal_id: i64) -> AppResult<()> {
    crate::db::artists::reject_proposal(&*lock(&db)?, proposal_id)
}

#[tauri::command]
pub fn restore_pill(db: State<Db>, history_id: i64) -> AppResult<()> {
    crate::db::artists::restore_history(&mut *lock(&db)?, history_id)
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
