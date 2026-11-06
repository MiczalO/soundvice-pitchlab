//! Artyści, pigułki, propozycje z researchu i historia pigułki.
//!
//! Zasada nadrzędna: research NIGDY nie nadpisuje istniejącej pigułki — trafia
//! do `pill_proposals` jako propozycja. Każda zmiana pigułki zachowuje jej
//! poprzednią wersję w `pill_history` (można cofnąć).

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{now_ms, ymd};
use crate::ai::gemini::Source;
use crate::error::{AppError, AppResult};

#[derive(Serialize)]
pub struct ArtistRow {
    pub id: i64,
    pub name: String,
    pub has_pill: bool,
    pub has_proposal: bool,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SourceRow {
    pub title: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct Proposal {
    pub id: i64,
    pub pill: String,
    pub dossier: String,
    pub sources: Vec<SourceRow>,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct HistoryRow {
    pub id: i64,
    pub pill: String,
    pub changed_at: i64,
    pub change_type: String,
}

#[derive(Serialize)]
pub struct TrackBrief {
    pub id: i64,
    pub title: String,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct ArtistDetail {
    pub id: i64,
    pub name: String,
    pub pill: String,
    pub manager_notes: String,
    pub research_hints: String,
    pub dossier: String,
    pub updated_at: i64,
    pub researched_at: Option<i64>,
    pub sources: Vec<SourceRow>,
    pub proposal: Option<Proposal>,
    pub history: Vec<HistoryRow>,
    pub tracks: Vec<TrackBrief>,
}

/// Wynik researchu gotowy do zapisania.
pub struct ResearchResult {
    pub pill: String,
    pub dossier: String,
    pub sources: Vec<Source>,
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applied {
    /// Artysta nie miał pigułki — zapisana od razu.
    Created,
    /// Artysta miał pigułkę — wynik czeka jako propozycja.
    Proposed,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AcceptMode {
    Replace,
    Append,
}

pub fn list(conn: &Connection) -> AppResult<Vec<ArtistRow>> {
    let mut st = conn.prepare(
        "SELECT a.id, a.name, TRIM(a.pill) <> '',
                EXISTS(SELECT 1 FROM pill_proposals p WHERE p.artist_id=a.id AND p.status='pending'),
                a.updated_at
         FROM artists a ORDER BY a.name COLLATE NOCASE",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(ArtistRow {
                id: r.get(0)?,
                name: r.get(1)?,
                has_pill: r.get(2)?,
                has_proposal: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn sources_for(conn: &Connection, id: i64) -> AppResult<Vec<SourceRow>> {
    let mut st =
        conn.prepare("SELECT title, url FROM artist_sources WHERE artist_id=?1 ORDER BY id")?;
    let rows = st
        .query_map([id], |r| {
            Ok(SourceRow {
                title: r.get(0)?,
                url: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn detail(conn: &Connection, id: i64) -> AppResult<Option<ArtistDetail>> {
    let base = conn
        .query_row(
            "SELECT id, name, pill, manager_notes, research_hints, dossier, updated_at, researched_at
             FROM artists WHERE id=?1",
            [id],
            |r| {
                Ok(ArtistDetail {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    pill: r.get(2)?,
                    manager_notes: r.get(3)?,
                    research_hints: r.get(4)?,
                    dossier: r.get(5)?,
                    updated_at: r.get(6)?,
                    researched_at: r.get(7)?,
                    sources: vec![],
                    proposal: None,
                    history: vec![],
                    tracks: vec![],
                })
            },
        )
        .optional()?;
    let Some(mut d) = base else { return Ok(None) };
    d.sources = sources_for(conn, id)?;
    d.proposal = conn
        .query_row(
            "SELECT id, pill, dossier, sources_json, created_at FROM pill_proposals
             WHERE artist_id=?1 AND status='pending' ORDER BY id DESC LIMIT 1",
            [id],
            |r| {
                let json: String = r.get(3)?;
                Ok(Proposal {
                    id: r.get(0)?,
                    pill: r.get(1)?,
                    dossier: r.get(2)?,
                    sources: serde_json::from_str(&json).unwrap_or_default(),
                    created_at: r.get(4)?,
                })
            },
        )
        .optional()?;
    let mut st = conn.prepare(
        "SELECT id, pill, changed_at, change_type FROM pill_history WHERE artist_id=?1 ORDER BY id DESC",
    )?;
    d.history = st
        .query_map([id], |r| {
            Ok(HistoryRow {
                id: r.get(0)?,
                pill: r.get(1)?,
                changed_at: r.get(2)?,
                change_type: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut st = conn
        .prepare("SELECT id, title, created_at FROM tracks WHERE artist_id=?1 ORDER BY id DESC")?;
    d.tracks = st
        .query_map([id], |r| {
            Ok(TrackBrief {
                id: r.get(0)?,
                title: r.get(1)?,
                created_at: r.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(d))
}

fn current_pill(conn: &Connection, id: i64) -> AppResult<String> {
    conn.query_row("SELECT pill FROM artists WHERE id=?1", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::Other("Nie ma takiego artysty.".into()))
}

/// Zachowuje poprzednią wersję pigułki (jeśli niepusta) przed jej zmianą.
fn snapshot(conn: &Connection, id: i64, old: &str, change_type: &str) -> AppResult<()> {
    if old.trim().is_empty() {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO pill_history (artist_id, pill, changed_at, change_type) VALUES (?1,?2,?3,?4)",
        (id, old, now_ms(), change_type),
    )?;
    Ok(())
}

/// Ręczna edycja pól artysty. Zmiana pigułki trafia do historii.
pub fn update(conn: &Connection, id: i64, pill: &str, notes: &str, hints: &str) -> AppResult<()> {
    let old = current_pill(conn, id)?;
    if old != pill {
        snapshot(conn, id, &old, "manual")?;
    }
    conn.execute(
        "UPDATE artists SET pill=?2, manager_notes=?3, research_hints=?4, updated_at=?5 WHERE id=?1",
        (id, pill, notes, hints, now_ms()),
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM artists WHERE id=?1", [id])?;
    Ok(())
}

fn replace_sources(conn: &Connection, id: i64, sources: &[SourceRow]) -> AppResult<()> {
    conn.execute("DELETE FROM artist_sources WHERE artist_id=?1", [id])?;
    for s in sources {
        conn.execute(
            "INSERT INTO artist_sources (artist_id, title, url, fetched_at) VALUES (?1,?2,?3,?4)",
            (id, &s.title, &s.url, now_ms()),
        )?;
    }
    Ok(())
}

/// Zapisuje wynik researchu: nowa pigułka tylko gdy artysta jej nie miał,
/// w przeciwnym razie propozycja do porównania (stara pigułka bez zmian).
pub fn apply_research(conn: &mut Connection, id: i64, r: &ResearchResult) -> AppResult<Applied> {
    let tx = conn.transaction()?;
    let now = now_ms();
    let sources: Vec<SourceRow> = r
        .sources
        .iter()
        .map(|s| SourceRow {
            title: s.title.clone(),
            url: s.uri.clone(),
        })
        .collect();
    let applied = if current_pill(&tx, id)?.trim().is_empty() {
        tx.execute(
            "UPDATE artists SET pill=?2, dossier=?3, researched_at=?4, updated_at=?4 WHERE id=?1",
            (id, &r.pill, &r.dossier, now),
        )?;
        replace_sources(&tx, id, &sources)?;
        Applied::Created
    } else {
        // Nowa propozycja zastępuje poprzednią, jeszcze nierozpatrzoną.
        tx.execute(
            "UPDATE pill_proposals SET status='rejected' WHERE artist_id=?1 AND status='pending'",
            [id],
        )?;
        tx.execute(
            "INSERT INTO pill_proposals (artist_id, pill, dossier, sources_json, created_at, status)
             VALUES (?1,?2,?3,?4,?5,'pending')",
            (
                id,
                &r.pill,
                &r.dossier,
                serde_json::to_string(&sources).map_err(|e| AppError::Other(e.to_string()))?,
                now,
            ),
        )?;
        tx.execute("UPDATE artists SET researched_at=?2 WHERE id=?1", (id, now))?;
        Applied::Proposed
    };
    tx.commit()?;
    Ok(applied)
}

/// Przyjmuje propozycję: zastępuje pigułkę albo dopisuje ją na końcu.
pub fn accept_proposal(conn: &mut Connection, proposal_id: i64, mode: AcceptMode) -> AppResult<()> {
    let tx = conn.transaction()?;
    let (artist_id, pill, dossier, sources_json, status): (i64, String, String, String, String) =
        tx.query_row(
            "SELECT artist_id, pill, dossier, sources_json, status FROM pill_proposals WHERE id=?1",
            [proposal_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::Other("Nie ma takiej propozycji.".into()))?;
    if status != "pending" {
        return Err(AppError::Other(
            "Ta propozycja została już rozpatrzona.".into(),
        ));
    }
    let old = current_pill(&tx, artist_id)?;
    let (new_pill, kind, st) = match mode {
        AcceptMode::Replace => (pill, "research_accept", "accepted"),
        AcceptMode::Append => (
            format!(
                "{}\n\n--- Z researchu {} ---\n{}",
                old.trim(),
                ymd(now_ms()),
                pill
            ),
            "append",
            "appended",
        ),
    };
    snapshot(&tx, artist_id, &old, kind)?;
    tx.execute(
        "UPDATE artists SET pill=?2, dossier=?3, updated_at=?4 WHERE id=?1",
        (artist_id, &new_pill, &dossier, now_ms()),
    )?;
    let sources: Vec<SourceRow> = serde_json::from_str(&sources_json).unwrap_or_default();
    replace_sources(&tx, artist_id, &sources)?;
    tx.execute(
        "UPDATE pill_proposals SET status=?2 WHERE id=?1",
        (proposal_id, st),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn reject_proposal(conn: &Connection, proposal_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE pill_proposals SET status='rejected' WHERE id=?1 AND status='pending'",
        [proposal_id],
    )?;
    Ok(())
}

/// Przywraca pigułkę z historii (obecna trafia do historii jako „restore”).
pub fn restore_history(conn: &mut Connection, history_id: i64) -> AppResult<()> {
    let tx = conn.transaction()?;
    let (artist_id, pill): (i64, String) = tx
        .query_row(
            "SELECT artist_id, pill FROM pill_history WHERE id=?1",
            [history_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::Other("Nie ma takiej wersji w historii.".into()))?;
    let old = current_pill(&tx, artist_id)?;
    snapshot(&tx, artist_id, &old, "restore")?;
    tx.execute(
        "UPDATE artists SET pill=?2, updated_at=?3 WHERE id=?1",
        (artist_id, &pill, now_ms()),
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::ensure_artist;

    fn mem() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrate(&mut c).unwrap();
        c
    }

    fn res(pill: &str) -> ResearchResult {
        ResearchResult {
            pill: pill.into(),
            dossier: format!("dossier {pill}"),
            sources: vec![Source {
                title: "T".into(),
                uri: "https://x.pl".into(),
            }],
        }
    }

    #[test]
    fn research_on_new_artist_sets_pill_directly() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        assert_eq!(
            apply_research(&mut c, id, &res("P1")).unwrap(),
            Applied::Created
        );
        let d = detail(&c, id).unwrap().unwrap();
        assert_eq!(d.pill, "P1");
        assert_eq!(d.sources.len(), 1);
        assert!(d.proposal.is_none());
        assert!(d.researched_at.is_some());
    }

    #[test]
    fn research_never_overwrites_existing_pill() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        update(&c, id, "MOJA PIGUŁKA", "notatki", "").unwrap();
        assert_eq!(
            apply_research(&mut c, id, &res("NOWA")).unwrap(),
            Applied::Proposed
        );
        let d = detail(&c, id).unwrap().unwrap();
        assert_eq!(d.pill, "MOJA PIGUŁKA");
        assert_eq!(d.manager_notes, "notatki");
        assert_eq!(d.dossier, "");
        assert!(d.sources.is_empty());
        assert_eq!(d.proposal.unwrap().pill, "NOWA");
    }

    #[test]
    fn newer_proposal_supersedes_pending_one() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        update(&c, id, "X", "", "").unwrap();
        apply_research(&mut c, id, &res("A")).unwrap();
        apply_research(&mut c, id, &res("B")).unwrap();
        let pending: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM pill_proposals WHERE status='pending'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pending, 1);
        assert_eq!(detail(&c, id).unwrap().unwrap().proposal.unwrap().pill, "B");
    }

    #[test]
    fn accept_replace_snapshots_old_pill_and_updates_dossier() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        update(&c, id, "STARA", "", "").unwrap();
        apply_research(&mut c, id, &res("NOWA")).unwrap();
        let pid = detail(&c, id).unwrap().unwrap().proposal.unwrap().id;
        accept_proposal(&mut c, pid, AcceptMode::Replace).unwrap();
        let d = detail(&c, id).unwrap().unwrap();
        assert_eq!(d.pill, "NOWA");
        assert_eq!(d.dossier, "dossier NOWA");
        assert_eq!(d.sources.len(), 1);
        assert!(d.proposal.is_none());
        assert_eq!(d.history[0].pill, "STARA");
        assert_eq!(d.history[0].change_type, "research_accept");
        assert!(accept_proposal(&mut c, pid, AcceptMode::Replace).is_err());
    }

    #[test]
    fn accept_append_keeps_old_text_and_adds_dated_section() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        update(&c, id, "STARA", "", "").unwrap();
        apply_research(&mut c, id, &res("NOWA")).unwrap();
        let pid = detail(&c, id).unwrap().unwrap().proposal.unwrap().id;
        accept_proposal(&mut c, pid, AcceptMode::Append).unwrap();
        let d = detail(&c, id).unwrap().unwrap();
        assert!(d.pill.starts_with("STARA\n\n--- Z researchu "));
        assert!(d.pill.ends_with(" ---\nNOWA"));
        assert_eq!(d.history[0].change_type, "append");
    }

    #[test]
    fn reject_leaves_pill_and_clears_proposal() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        update(&c, id, "STARA", "", "").unwrap();
        apply_research(&mut c, id, &res("NOWA")).unwrap();
        let pid = detail(&c, id).unwrap().unwrap().proposal.unwrap().id;
        reject_proposal(&c, pid).unwrap();
        let d = detail(&c, id).unwrap().unwrap();
        assert_eq!(d.pill, "STARA");
        assert!(d.proposal.is_none());
    }

    #[test]
    fn manual_edits_are_versioned_and_restorable() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        update(&c, id, "v1", "", "").unwrap(); // pusta → v1: brak snapshotu
        update(&c, id, "v2", "", "").unwrap();
        update(&c, id, "v2", "inne notatki", "").unwrap(); // pigułka bez zmian: brak snapshotu
        update(&c, id, "v3", "inne notatki", "").unwrap();
        let d = detail(&c, id).unwrap().unwrap();
        let hist: Vec<_> = d.history.iter().map(|h| h.pill.as_str()).collect();
        assert_eq!(hist, vec!["v2", "v1"]);
        let v1 = d.history.iter().find(|h| h.pill == "v1").unwrap().id;
        restore_history(&mut c, v1).unwrap();
        let d = detail(&c, id).unwrap().unwrap();
        assert_eq!(d.pill, "v1");
        assert_eq!(d.history[0].pill, "v3"); // obecna przed przywróceniem nie ginie
        assert_eq!(d.history[0].change_type, "restore");
    }

    #[test]
    fn list_flags_pill_and_pending_proposal() {
        let mut c = mem();
        let a = ensure_artist(&c, "Anna").unwrap();
        ensure_artist(&c, "Zenek").unwrap();
        update(&c, a, "P", "", "").unwrap();
        apply_research(&mut c, a, &res("N")).unwrap();
        let rows = list(&c).unwrap();
        assert_eq!(rows[0].name, "Anna");
        assert!(rows[0].has_pill && rows[0].has_proposal);
        assert!(!rows[1].has_pill && !rows[1].has_proposal);
    }
}
