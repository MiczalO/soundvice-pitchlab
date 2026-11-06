//! Research artysty (Gemini + Google Search) → dossier → pigułka.
//! Wynik trafia do bazy przez `db::artists::apply_research`: nowa pigułka tylko
//! gdy artysta jej nie miał, w przeciwnym razie propozycja do porównania.

use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::ai::gemini::{self, GenOpts, Part};
use crate::ai::prompts;
use crate::db::Db;
use crate::db::artists::{self, Applied, ResearchResult};
use crate::error::{AppError, AppResult};
use crate::jobs::{StepState, emit_step};
use crate::settings;

const NO_SEARCH_QUOTA: &str = "Wyszukiwarka Google w Gemini nie ma przydziału dla tego klucza \
    (zwykle wymaga płatnego planu — włącz rozliczenia w Google AI Studio). \
    Do tego czasu pigułkę możesz napisać ręcznie w karcie Artyści.";

fn db_err() -> AppError {
    AppError::Db("blokada bazy".into())
}

fn model_for(app: &AppHandle, key: &str) -> String {
    let db = app.state::<Db>();
    let Ok(conn) = db.0.lock() else {
        return gemini::FALLBACK_MODEL.into();
    };
    settings::get(&conn, key)
        .filter(|m| !m.is_empty())
        .or_else(|| settings::get(&conn, "gemini_model").filter(|m| !m.is_empty()))
        .unwrap_or_else(|| gemini::FALLBACK_MODEL.into())
}

/// `ctx`: (tytuł bieżącego singla, gatunek) — pomaga modelowi zidentyfikować artystę.
pub async fn run(
    app: &AppHandle,
    job_id: u64,
    cancel: &CancellationToken,
    artist_id: i64,
    ctx: Option<(String, String)>,
) -> AppResult<Applied> {
    let (name, hints, old_pill) = {
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|_| db_err())?;
        let d = artists::detail(&conn, artist_id)?
            .ok_or_else(|| AppError::Other("Nie ma takiego artysty.".into()))?;
        (d.name, d.research_hints, d.pill)
    };
    let research_model = model_for(app, "gemini_model_research");
    let pill_model = model_for(app, "gemini_model");

    emit_step(
        app,
        job_id,
        "research",
        StepState::Active,
        Some("szukam w Google…".into()),
    );
    let prompt = prompts::research(
        &name,
        &hints,
        ctx.as_ref()
            .map(|(t, g)| prompts::ResearchCtx { title: t, genre: g })
            .as_ref(),
    );
    let r = match gemini::generate(
        &research_model,
        &[Part::Text(prompt)],
        GenOpts {
            json: false,
            search: true,
            temperature: 0.3,
        },
        cancel,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            emit_step(app, job_id, "research", StepState::Err, None);
            // Wyszukiwarka Google w Gemini ma osobny przydział (zwykle tylko płatny plan).
            return Err(match e {
                AppError::RateLimited(svc, _) => AppError::RateLimited(svc, NO_SEARCH_QUOTA.into()),
                other => other,
            });
        }
    };
    if r.text.trim().is_empty() {
        emit_step(app, job_id, "research", StepState::Err, None);
        return Err(AppError::ModelOutput(
            "research zwrócił pustą odpowiedź".into(),
        ));
    }
    let queries = if r.queries.is_empty() {
        String::new()
    } else {
        format!(
            " · zapytania: {}",
            r.queries
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    emit_step(
        app,
        job_id,
        "research",
        StepState::Done,
        Some(format!("{} źródeł{queries}", r.sources.len())),
    );

    emit_step(
        app,
        job_id,
        "pill",
        StepState::Active,
        Some("streszczam dossier…".into()),
    );
    let pill = match gemini::generate(
        &pill_model,
        &[Part::Text(prompts::pill(&name, &r.text, &old_pill))],
        GenOpts {
            json: false,
            search: false,
            temperature: 0.3,
        },
        cancel,
    )
    .await
    {
        Ok(g) if !g.text.trim().is_empty() => g.text.trim().to_string(),
        Ok(_) => {
            emit_step(app, job_id, "pill", StepState::Err, None);
            return Err(AppError::ModelOutput("model zwrócił pustą pigułkę".into()));
        }
        Err(e) => {
            emit_step(app, job_id, "pill", StepState::Err, None);
            return Err(e);
        }
    };

    let applied = {
        let db = app.state::<Db>();
        let mut conn = db.0.lock().map_err(|_| db_err())?;
        artists::apply_research(
            &mut conn,
            artist_id,
            &ResearchResult {
                pill,
                dossier: r.text,
                sources: r.sources,
            },
        )?
    };
    let msg = match applied {
        Applied::Created => "nowa pigułka zapisana — możesz ją edytować w karcie Artyści",
        Applied::Proposed => {
            "propozycja aktualizacji czeka w karcie Artyści (Twoja pigułka bez zmian)"
        }
    };
    emit_step(app, job_id, "pill", StepState::Done, Some(msg.into()));
    Ok(applied)
}
