//! Pipeline „Analiza i pitch”: przygotowanie pliku → odsłuch Gemini → research/pigułka
//! (krok pitchu dojdzie w kolejnym etapie).

use std::path::Path;

use base64::Engine;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::ai::gemini::{self, GenOpts, Part};
use crate::ai::{analysis, json, prompts};
use crate::db::{Db, repo};
use crate::error::{AppError, AppResult};
use crate::jobs::{StepState, emit_step};
use crate::research;
use crate::settings;

/// Poniżej tego rozmiaru plik idzie inline, powyżej przez Files API.
pub const INLINE_LIMIT: u64 = 14 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisParams {
    pub file_path: String,
    pub hash: String,
    pub mime: String,
    pub file_name: String,
    pub size: u64,
    pub duration_s: Option<f64>,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub featuring: String,
    pub release_date: Option<String>,
    #[serde(default)]
    pub campaign_context: String,
    #[serde(default)]
    pub lyrics: String,
    /// Odśwież research, nawet jeśli artysta ma pigułkę (wynik trafi jako propozycja).
    #[serde(default)]
    pub force_research: bool,
}

/// Zapisany utwór po odsłuchu.
pub struct Saved {
    pub track_id: i64,
    pub artist_id: i64,
    pub genre: String,
}

fn db_err() -> AppError {
    AppError::Db("blokada bazy".into())
}

async fn audio_part(
    app: &AppHandle,
    job_id: u64,
    p: &AnalysisParams,
    cancel: &CancellationToken,
) -> AppResult<Part> {
    let progress =
        |m: &str| emit_step(app, job_id, "audio", StepState::Active, Some(m.to_string()));
    if p.size < INLINE_LIMIT {
        progress(&format!(
            "wysyłam inline ({:.1} MB)",
            p.size as f64 / 1_048_576.0
        ));
        let bytes = gemini::cancellable(cancel, async {
            tokio::fs::read(&p.file_path)
                .await
                .map_err(|e| AppError::Other(format!("Błąd pliku: {e}")))
        })
        .await?;
        let data_b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        return Ok(Part::Inline {
            mime: p.mime.clone(),
            data_b64,
        });
    }
    let cached = {
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|_| db_err())?;
        repo::cached_file(&conn, &p.hash)
    };
    if let Some((uri, mime)) = cached {
        progress("plik już jest w Gemini (cache)");
        return Ok(Part::File { mime, uri });
    }
    progress(&format!(
        "wgrywam przez Files API ({:.1} MB)…",
        p.size as f64 / 1_048_576.0
    ));
    let up = gemini::upload_file(
        Path::new(&p.file_path),
        &p.mime,
        &p.file_name,
        cancel,
        &progress,
    )
    .await?;
    {
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|_| db_err())?;
        repo::cache_file(&conn, &p.hash, &up.uri, &up.mime)?;
    }
    Ok(Part::File {
        mime: up.mime,
        uri: up.uri,
    })
}

/// Odsłuch Gemini + zapis utworu z analizą.
async fn listen_and_save(
    app: &AppHandle,
    job_id: u64,
    cancel: &CancellationToken,
    p: &AnalysisParams,
) -> AppResult<Saved> {
    if p.artist.trim().is_empty() {
        return Err(AppError::Other("Wpisz artystę.".into()));
    }
    let model = {
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|_| db_err())?;
        settings::get(&conn, "gemini_model")
    }
    .filter(|m| !m.is_empty())
    .unwrap_or_else(|| gemini::FALLBACK_MODEL.to_string());

    emit_step(app, job_id, "audio", StepState::Active, None);
    let part = match audio_part(app, job_id, p, cancel).await {
        Ok(part) => part,
        Err(e) => {
            emit_step(app, job_id, "audio", StepState::Err, None);
            return Err(e);
        }
    };
    emit_step(
        app,
        job_id,
        "audio",
        StepState::Done,
        Some(p.file_name.clone()),
    );

    emit_step(
        app,
        job_id,
        "listen",
        StepState::Active,
        Some("to może potrwać 20–90 s".into()),
    );
    let prompt = prompts::analysis(&prompts::TrackInfo {
        title: &p.title,
        artist: &p.artist,
        featuring: &p.featuring,
        lyrics: &p.lyrics,
    });
    let result = async {
        let g = gemini::generate(
            &model,
            &[part, Part::Text(prompt)],
            GenOpts {
                json: true,
                search: false,
                temperature: 0.2,
            },
            cancel,
        )
        .await?;
        let value = json::parse_loose(&g.text)?;
        let parsed = analysis::validate(value)?;
        Ok::<_, AppError>(parsed)
    }
    .await;
    let parsed = match result {
        Ok(a) => a,
        Err(e) => {
            emit_step(app, job_id, "listen", StepState::Err, None);
            return Err(e);
        }
    };
    let summary = [
        Some(parsed.primary_genre.clone()),
        (!parsed.subgenres.is_empty()).then(|| parsed.subgenres.join(", ")),
        parsed.bpm.map(|b| format!("{b:.0} BPM")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    emit_step(app, job_id, "listen", StepState::Done, Some(summary));

    let (track_id, artist_id) = {
        let db = app.state::<Db>();
        let mut conn = db.0.lock().map_err(|_| db_err())?;
        let tx = conn.transaction()?;
        let artist_id = repo::ensure_artist(&tx, &p.artist)?;
        let id = repo::insert_track(
            &tx,
            &repo::NewTrack {
                artist_id,
                title: p.title.trim(),
                featuring: p.featuring.trim(),
                release_date: p.release_date.as_deref(),
                campaign_context: &p.campaign_context,
                lyrics: &p.lyrics,
                audio_path: &p.file_path,
                audio_hash: &p.hash,
                duration_s: p.duration_s,
            },
        )?;
        let json = serde_json::to_string(&parsed).map_err(|e| AppError::Other(e.to_string()))?;
        repo::insert_analysis(&tx, id, &model, &json)?;
        tx.commit()?;
        (id, artist_id)
    };

    Ok(Saved {
        track_id,
        artist_id,
        genre: parsed.primary_genre,
    })
}

/// Pełny przebieg: odsłuch, potem research/pigułka (gdy artysta nie ma pigułki
/// albo wymuszono odświeżenie). Utwór z analizą zostaje zapisany nawet wtedy,
/// gdy późniejszy krok zawiedzie — dlatego zwracamy też `track_id` przy błędzie.
pub async fn run(
    app: &AppHandle,
    job_id: u64,
    cancel: &CancellationToken,
    p: &AnalysisParams,
) -> (Option<i64>, AppResult<()>) {
    let saved = match listen_and_save(app, job_id, cancel, p).await {
        Ok(s) => s,
        Err(e) => return (None, Err(e)),
    };
    let track_id = Some(saved.track_id);

    let has_pill = {
        let db = app.state::<Db>();
        let Ok(conn) = db.0.lock() else {
            return (track_id, Err(db_err()));
        };
        crate::db::artists::detail(&conn, saved.artist_id)
            .ok()
            .flatten()
            .is_some_and(|d| !d.pill.trim().is_empty())
    };
    if has_pill && !p.force_research {
        emit_step(
            app,
            job_id,
            "research",
            StepState::Skip,
            Some("pigułka istnieje — używam jej".into()),
        );
        emit_step(
            app,
            job_id,
            "pill",
            StepState::Done,
            Some("z pamięci".into()),
        );
    } else {
        let ctx = (p.title.clone(), saved.genre);
        if let Err(e) = research::run(app, job_id, cancel, saved.artist_id, Some(ctx)).await {
            return (track_id, Err(e));
        }
    }
    emit_step(
        app,
        job_id,
        "pitch",
        StepState::Skip,
        Some("dostępne w kolejnym etapie".into()),
    );
    (track_id, Ok(()))
}
