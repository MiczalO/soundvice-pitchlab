//! Generowanie pitchu (Gemini lub Claude): nowy, nowa wersja albo poprawki wg uwag.
//! Wywołania są odporne (ponawianie, zapasowe modele Gemini), a pitch powyżej
//! 500 znaków jest raz automatycznie skracany.

use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::ai::gemini::{self, GenOpts, Part};
use crate::ai::pitchout::{self, PitchOut};
use crate::ai::{anthropic, prompts, resilient};
use crate::db::pitches::{self, NewPitch};
use crate::db::{Db, artists, repo, rules};
use crate::error::{AppError, AppResult};
use crate::jobs::{StepState, emit_step};
use crate::settings;

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Fresh,
    /// Wyraźnie inne ujęcie niż poprzednia wersja.
    NewVersion,
    /// Poprawki wg uwag managera (zachowaj to, co działa).
    Revise(String),
}

struct Ctx {
    title: String,
    artist: String,
    featuring: String,
    release_date: String,
    campaign_context: String,
    lyrics: String,
    manager_notes: String,
    pill: String,
    analysis_json: String,
    rules: String,
    examples: String,
    polish: bool,
    engine: String,
    gemini_models: Vec<String>,
    claude_model: String,
    previous: Option<String>,
}

fn db_err() -> AppError {
    AppError::Db("blokada bazy".into())
}

fn load(app: &AppHandle, track_id: i64) -> AppResult<Ctx> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| db_err())?;
    let t = repo::get_track(&conn, track_id)?
        .ok_or_else(|| AppError::Other("Nie ma takiego utworu.".into()))?;
    let analysis = t.analysis.as_ref().ok_or_else(|| {
        AppError::Other("Ten utwór nie ma analizy odsłuchu — najpierw uruchom analizę.".into())
    })?;
    let a = artists::detail(&conn, t.artist_id)?
        .ok_or_else(|| AppError::Other("Nie ma takiego artysty.".into()))?;
    Ok(Ctx {
        title: t.title.clone(),
        artist: t.artist_name.clone(),
        featuring: t.featuring.clone(),
        release_date: t.release_date.clone().unwrap_or_default(),
        campaign_context: t.campaign_context.clone(),
        lyrics: t.lyrics.clone(),
        manager_notes: a.manager_notes,
        pill: a.pill,
        analysis_json: serde_json::to_string_pretty(analysis).unwrap_or_default(),
        rules: rules::current(&conn, "rules"),
        examples: rules::current(&conn, "examples"),
        polish: settings::get(&conn, "pitch_lang").as_deref() == Some("pl"),
        engine: settings::get(&conn, "engine").unwrap_or_else(|| "gemini".into()),
        gemini_models: settings::model_candidates(&conn, "gemini_model"),
        claude_model: settings::get(&conn, "claude_model")
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| anthropic::DEFAULT_MODEL.into()),
        previous: t
            .pitches
            .iter()
            .find(|p| p.is_current)
            .map(|p| p.text.clone()),
    })
}

/// Jedno odporne zapytanie do wybranego silnika; `parse` waliduje odpowiedź
/// (nieczytelna odpowiedź = ponowienie).
async fn ask<T>(
    ctx: &Ctx,
    prompt: &str,
    temperature: f64,
    cancel: &CancellationToken,
    progress: &(dyn Fn(&str) + Send + Sync),
    parse: impl Fn(&str) -> AppResult<T> + Copy,
) -> AppResult<(T, String)> {
    if ctx.engine == "claude" {
        resilient::run(
            std::slice::from_ref(&ctx.claude_model),
            cancel,
            resilient::MAX_TOTAL,
            resilient::backoff,
            progress,
            |m| async move { parse(&anthropic::complete(&m, prompt, cancel).await?) },
        )
        .await
    } else {
        resilient::run(
            &ctx.gemini_models,
            cancel,
            resilient::MAX_TOTAL,
            resilient::backoff,
            progress,
            |m| async move {
                let g = gemini::generate(
                    &m,
                    &[Part::Text(prompt.to_string())],
                    GenOpts {
                        json: true,
                        search: false,
                        temperature,
                    },
                    cancel,
                )
                .await?;
                parse(&g.text)
            },
        )
        .await
    }
}

/// Tworzy nową wersję pitchu utworu i zapisuje ją jako aktualną. Zwraca id wersji.
/// `current_text`: tekst z edytora (dla poprawek wg uwag), gdy różni się od zapisanego.
pub async fn run(
    app: &AppHandle,
    job_id: u64,
    cancel: &CancellationToken,
    track_id: i64,
    mode: Mode,
    current_text: Option<String>,
) -> AppResult<i64> {
    let ctx = load(app, track_id)?;
    let who = if ctx.engine == "claude" {
        "Claude"
    } else {
        "Gemini"
    };
    emit_step(
        app,
        job_id,
        "pitch",
        StepState::Active,
        Some(format!("pisze {who}")),
    );
    let progress =
        |m: &str| emit_step(app, job_id, "pitch", StepState::Active, Some(m.to_string()));

    let extra = match &mode {
        Mode::Fresh => String::new(),
        Mode::NewVersion => prompts::new_version_extra(ctx.previous.as_deref().unwrap_or("")),
        Mode::Revise(feedback) => prompts::revision_extra(
            current_text
                .as_deref()
                .or(ctx.previous.as_deref())
                .unwrap_or(""),
            feedback,
        ),
    };
    let prompt = prompts::pitch(&prompts::PitchInput {
        rules: &ctx.rules,
        examples: &ctx.examples,
        polish: ctx.polish,
        title: &ctx.title,
        artist: &ctx.artist,
        featuring: &ctx.featuring,
        release_date: &ctx.release_date,
        manager_notes: &ctx.manager_notes,
        campaign_context: &ctx.campaign_context,
        lyrics: &ctx.lyrics,
        pill: &ctx.pill,
        analysis_json: &ctx.analysis_json,
        extra: &extra,
    });

    let (mut out, model): (PitchOut, String) =
        match ask(&ctx, &prompt, 0.7, cancel, &progress, pitchout::parse).await {
            Ok(ok) => ok,
            Err(e) => {
                emit_step(app, job_id, "pitch", StepState::Err, None);
                return Err(e);
            }
        };

    // Limit Spotify: jedno automatyczne skrócenie (bez dalszych pętli).
    if pitchout::char_count(&out.pitch) > pitchout::LIMIT {
        progress("pitch przekracza 500 znaków — skracam…");
        match ask(
            &ctx,
            &prompts::shorten(&out.pitch),
            0.3,
            cancel,
            &progress,
            pitchout::parse_shortened,
        )
        .await
        {
            Ok(((pitch, pl), _)) => {
                out.pitch = pitch;
                if let Some(pl) = pl {
                    out.pitch_pl = pl;
                }
            }
            Err(AppError::Cancelled) => return Err(AppError::Cancelled),
            Err(_) => {} // zostaje dłuższa wersja; UI pokaże przekroczenie limitu
        }
    }

    let id = {
        let db = app.state::<Db>();
        let mut conn = db.0.lock().map_err(|_| db_err())?;
        pitches::insert(
            &mut conn,
            track_id,
            &NewPitch::from_out(&out, &ctx.engine, &model),
        )?
    };
    emit_step(
        app,
        job_id,
        "pitch",
        StepState::Done,
        Some(format!("{} znaków", pitchout::char_count(&out.pitch))),
    );
    Ok(id)
}
