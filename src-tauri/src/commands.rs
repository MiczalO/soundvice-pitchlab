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

/// Pigułka z własnego researchu wklejonego przez managera (bez wyszukiwarki Google).
#[tauri::command]
pub fn start_pill_from_dossier(
    app: tauri::AppHandle,
    jobs: State<crate::jobs::Jobs>,
    artist_id: i64,
    dossier: String,
) -> u64 {
    let (job_id, cancel) = jobs.start();
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;
        let res = crate::research::from_dossier(&app, job_id, &cancel, artist_id, dossier)
            .await
            .map(|_| ());
        app.state::<crate::jobs::Jobs>().finish(job_id);
        crate::jobs::emit_done(&app, done_event(job_id, None, res));
    });
    job_id
}

/// Pitch dla istniejącego utworu: `fresh` (pierwszy), `new` (inne ujęcie), `revise` (wg uwag).
#[tauri::command]
pub fn start_pitch(
    app: tauri::AppHandle,
    jobs: State<crate::jobs::Jobs>,
    track_id: i64,
    mode: String,
    feedback: Option<String>,
    current_text: Option<String>,
) -> AppResult<u64> {
    use crate::pitch::Mode;
    let mode = match mode.as_str() {
        "fresh" => Mode::Fresh,
        "new" => Mode::NewVersion,
        "revise" => {
            let fb = feedback.unwrap_or_default();
            if fb.trim().is_empty() {
                return Err(AppError::Other("Wpisz uwagi do poprawki.".into()));
            }
            Mode::Revise(fb)
        }
        other => return Err(AppError::Other(format!("Nieznany tryb pitchu: {other}"))),
    };
    let (job_id, cancel) = jobs.start();
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;
        let res = crate::pitch::run(&app, job_id, &cancel, track_id, mode, current_text)
            .await
            .map(|_| ());
        app.state::<crate::jobs::Jobs>().finish(job_id);
        crate::jobs::emit_done(&app, done_event(job_id, Some(track_id), res));
    });
    Ok(job_id)
}

#[tauri::command]
pub fn save_pitch_edit(db: State<Db>, track_id: i64, text: String) -> AppResult<i64> {
    crate::db::pitches::save_edit(&mut *lock(&db)?, track_id, &text)
}

#[tauri::command]
pub fn restore_pitch(db: State<Db>, pitch_id: i64) -> AppResult<()> {
    crate::db::pitches::set_current(&mut *lock(&db)?, pitch_id)
}

#[tauri::command]
pub fn add_to_examples(db: State<Db>, text: String) -> AppResult<()> {
    if text.trim().is_empty() {
        return Err(AppError::Other("Pitch jest pusty.".into()));
    }
    crate::db::rules::append_example(&*lock(&db)?, &text)
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

// ---------- historia, zasady, import, kopia ----------

#[tauri::command]
pub fn list_history(
    db: State<Db>,
    artist_id: Option<i64>,
    query: String,
    newest_first: bool,
) -> AppResult<Vec<crate::db::history::HistoryRow>> {
    crate::db::history::list(&*lock(&db)?, artist_id, &query, newest_first)
}

#[derive(Serialize)]
pub struct RulesState {
    pub rules: String,
    pub examples: String,
    pub lang: String,
    pub default_rules: &'static str,
    pub rules_versions: Vec<crate::db::rules::RuleVersion>,
    pub examples_versions: Vec<crate::db::rules::RuleVersion>,
}

#[tauri::command]
pub fn get_rules(db: State<Db>) -> AppResult<RulesState> {
    use crate::db::rules;
    let conn = lock(&db)?;
    Ok(RulesState {
        rules: rules::current(&conn, "rules"),
        examples: rules::current(&conn, "examples"),
        lang: settings::get(&conn, "pitch_lang").unwrap_or_else(|| "en".into()),
        default_rules: crate::db::DEFAULT_RULES,
        rules_versions: rules::versions(&conn, "rules")?,
        examples_versions: rules::versions(&conn, "examples")?,
    })
}

/// Zapisuje zasady lub wzorce jako nową wersję (tylko gdy treść się zmieniła).
#[tauri::command]
pub fn save_rules(db: State<Db>, kind: String, body: String) -> AppResult<bool> {
    if kind != "rules" && kind != "examples" {
        return Err(AppError::Other("Nieznany rodzaj zasad.".into()));
    }
    if kind == "rules" && body.trim().is_empty() {
        return Err(AppError::Other("Zasady nie mogą być puste.".into()));
    }
    crate::db::rules::save_if_changed(&*lock(&db)?, &kind, &body)
}

#[tauri::command]
pub fn restore_rule_version(db: State<Db>, version_id: i64) -> AppResult<()> {
    crate::db::rules::restore(&*lock(&db)?, version_id)
}

/// Przywraca domyślne zasady jako nową wersję (własne zostają w historii).
#[tauri::command]
pub fn reset_default_rules(db: State<Db>) -> AppResult<()> {
    crate::db::rules::save_if_changed(&*lock(&db)?, "rules", crate::db::DEFAULT_RULES).map(|_| ())
}

/// Import kopii JSON z prototypu HTML.
#[tauri::command]
pub async fn import_html_json(
    app: tauri::AppHandle,
    path: String,
) -> AppResult<crate::import::ImportReport> {
    use tauri::Manager;
    let text = tauri::async_runtime::spawn_blocking(move || {
        let meta =
            std::fs::metadata(&path).map_err(|e| AppError::Other(format!("Błąd pliku: {e}")))?;
        if meta.len() > 200 * 1024 * 1024 {
            return Err(AppError::Other(
                "Plik jest zbyt duży na kopię pitchLAB (limit 200 MB).".into(),
            ));
        }
        std::fs::read_to_string(&path)
            .map_err(|e| AppError::Other(format!("Błąd odczytu pliku: {e}")))
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;
    let db = app.state::<Db>();
    let mut conn =
        db.0.lock()
            .map_err(|_| AppError::Db("blokada bazy".into()))?;
    crate::import::import_json(&mut conn, &text)
}

#[tauri::command]
pub async fn export_backup(
    app: tauri::AppHandle,
    path: String,
    include_audio: bool,
) -> AppResult<crate::backup::ExportInfo> {
    use tauri::Manager;
    let audio_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(e.to_string()))?
        .join("audio");
    let version = app.package_info().version.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<Db>();
        let conn =
            db.0.lock()
                .map_err(|_| AppError::Db("blokada bazy".into()))?;
        crate::backup::export(
            &conn,
            &audio_dir,
            std::path::Path::new(&path),
            include_audio,
            &version,
        )
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}

/// Rozpakowuje kopię do poczekalni i restartuje aplikację; podmiana bazy następuje przy starcie.
#[tauri::command]
pub async fn restore_backup(app: tauri::AppHandle, path: String) -> AppResult<()> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(e.to_string()))?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::backup::stage_restore(std::path::Path::new(&path), &dir)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;
    app.restart()
}

#[tauri::command]
pub fn get_data_dir(app: tauri::AppHandle) -> AppResult<String> {
    use tauri::Manager;
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(e.to_string()))?
        .to_string_lossy()
        .into_owned())
}
