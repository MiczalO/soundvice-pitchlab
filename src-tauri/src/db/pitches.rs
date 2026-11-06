//! Wersje pitchu utworu. Dokładnie jedna wersja na utwór ma `is_current = 1`.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

use super::now_ms;
use crate::ai::pitchout::{Form, PitchOut};
use crate::error::{AppError, AppResult};

#[derive(Serialize, Clone, Debug)]
pub struct PitchView {
    pub id: i64,
    pub text: String,
    pub text_pl: String,
    pub form: Value,
    pub angle: String,
    pub facts_used: Vec<String>,
    pub engine: String,
    pub model: String,
    pub is_manual_edit: bool,
    pub is_current: bool,
    pub created_at: i64,
}

pub struct NewPitch<'a> {
    pub text: &'a str,
    pub text_pl: &'a str,
    pub form: &'a Form,
    pub angle: &'a str,
    pub facts_used: &'a [String],
    pub engine: &'a str,
    pub model: &'a str,
    pub manual: bool,
}

impl<'a> NewPitch<'a> {
    pub fn from_out(out: &'a PitchOut, engine: &'a str, model: &'a str) -> Self {
        NewPitch {
            text: &out.pitch,
            text_pl: &out.pitch_pl,
            form: &out.form,
            angle: &out.angle,
            facts_used: &out.facts_used,
            engine,
            model,
            manual: false,
        }
    }
}

/// Zapisuje nową wersję i robi z niej aktualną (poprzednia zostaje w historii).
pub fn insert(conn: &mut Connection, track_id: i64, p: &NewPitch) -> AppResult<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE pitches SET is_current=0 WHERE track_id=?1",
        [track_id],
    )?;
    tx.execute(
        "INSERT INTO pitches (track_id, text, text_pl, form_json, angle, facts_used_json,
                              engine, model, is_manual_edit, is_current, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,1,?10)",
        rusqlite::params![
            track_id,
            p.text,
            p.text_pl,
            serde_json::to_string(p.form).map_err(|e| AppError::Other(e.to_string()))?,
            p.angle,
            serde_json::to_string(p.facts_used).map_err(|e| AppError::Other(e.to_string()))?,
            p.engine,
            p.model,
            p.manual,
            now_ms()
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(id)
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<PitchView> {
    let form: String = r.get(3)?;
    let facts: String = r.get(5)?;
    Ok(PitchView {
        id: r.get(0)?,
        text: r.get(1)?,
        text_pl: r.get(2)?,
        form: serde_json::from_str(&form).unwrap_or(Value::Null),
        angle: r.get(4)?,
        facts_used: serde_json::from_str(&facts).unwrap_or_default(),
        engine: r.get(6)?,
        model: r.get(7)?,
        is_manual_edit: r.get(8)?,
        is_current: r.get(9)?,
        created_at: r.get(10)?,
    })
}

const COLS: &str = "id, text, text_pl, form_json, angle, facts_used_json, engine, model,
                    is_manual_edit, is_current, created_at";

/// Wszystkie wersje utworu, najnowsze pierwsze.
pub fn list(conn: &Connection, track_id: i64) -> AppResult<Vec<PitchView>> {
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM pitches WHERE track_id=?1 ORDER BY id DESC"
    ))?;
    let rows = st
        .query_map([track_id], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn current(conn: &Connection, track_id: i64) -> AppResult<Option<PitchView>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLS} FROM pitches WHERE track_id=?1 AND is_current=1
                 ORDER BY id DESC LIMIT 1"
            ),
            [track_id],
            row,
        )
        .optional()?)
}

/// Zapis ręcznej edycji: nowa wersja (kopia pól obecnej) ze zmienionym tekstem.
pub fn save_edit(conn: &mut Connection, track_id: i64, text: &str) -> AppResult<i64> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::Other("Pitch nie może być pusty.".into()));
    }
    let cur = current(conn, track_id)?
        .ok_or_else(|| AppError::Other("Ten utwór nie ma jeszcze pitchu.".into()))?;
    if cur.text == text {
        return Ok(cur.id);
    }
    let form: Form = serde_json::from_value(cur.form.clone()).unwrap_or_default();
    insert(
        conn,
        track_id,
        &NewPitch {
            text,
            text_pl: &cur.text_pl,
            form: &form,
            angle: &cur.angle,
            facts_used: &cur.facts_used,
            engine: &cur.engine,
            model: &cur.model,
            manual: true,
        },
    )
}

/// Przywraca wcześniejszą wersję jako aktualną (nic nie jest usuwane).
pub fn set_current(conn: &mut Connection, pitch_id: i64) -> AppResult<()> {
    let tx = conn.transaction()?;
    let track_id: i64 = tx
        .query_row(
            "SELECT track_id FROM pitches WHERE id=?1",
            [pitch_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Other("Nie ma takiej wersji pitchu.".into()))?;
    tx.execute(
        "UPDATE pitches SET is_current=0 WHERE track_id=?1",
        [track_id],
    )?;
    tx.execute("UPDATE pitches SET is_current=1 WHERE id=?1", [pitch_id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{NewTrack, ensure_artist, insert_track};

    fn setup() -> (Connection, i64) {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrate(&mut c).unwrap();
        let a = ensure_artist(&c, "Mata").unwrap();
        let t = insert_track(
            &c,
            &NewTrack {
                artist_id: a,
                title: "T",
                featuring: "",
                release_date: None,
                campaign_context: "",
                lyrics: "",
                audio_path: "x",
                audio_hash: "h",
                duration_s: None,
            },
        )
        .unwrap();
        (c, t)
    }

    fn out(text: &str) -> PitchOut {
        PitchOut {
            pitch: text.into(),
            pitch_pl: format!("PL {text}"),
            angle: "kąt".into(),
            facts_used: vec!["pill: x".into()],
            form: Form {
                genres: vec!["Hip-Hop".into()],
                ..Form::default()
            },
        }
    }

    #[test]
    fn only_newest_version_is_current() {
        let (mut c, t) = setup();
        insert(&mut c, t, &NewPitch::from_out(&out("v1"), "gemini", "m1")).unwrap();
        let id2 = insert(&mut c, t, &NewPitch::from_out(&out("v2"), "claude", "m2")).unwrap();
        let all = list(&c, t).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all.iter().filter(|p| p.is_current).count(), 1);
        let cur = current(&c, t).unwrap().unwrap();
        assert_eq!(
            (cur.id, cur.text.as_str(), cur.engine.as_str()),
            (id2, "v2", "claude")
        );
        assert_eq!(cur.form["genres"][0], "Hip-Hop");
        assert_eq!(cur.facts_used, vec!["pill: x"]);
    }

    #[test]
    fn manual_edit_creates_new_version_keeping_metadata() {
        let (mut c, t) = setup();
        insert(&mut c, t, &NewPitch::from_out(&out("v1"), "gemini", "m1")).unwrap();
        save_edit(&mut c, t, "  moja wersja ").unwrap();
        let all = list(&c, t).unwrap();
        assert_eq!(all.len(), 2);
        let cur = &all[0];
        assert!(cur.is_manual_edit && cur.is_current);
        assert_eq!(cur.text, "moja wersja");
        assert_eq!(cur.text_pl, "PL v1");
        assert_eq!(cur.form["genres"][0], "Hip-Hop");
        // bez zmian = bez nowej wersji
        save_edit(&mut c, t, "moja wersja").unwrap();
        assert_eq!(list(&c, t).unwrap().len(), 2);
        assert!(save_edit(&mut c, t, "   ").is_err());
    }

    #[test]
    fn restore_switches_current_without_deleting() {
        let (mut c, t) = setup();
        let id1 = insert(&mut c, t, &NewPitch::from_out(&out("v1"), "gemini", "m")).unwrap();
        insert(&mut c, t, &NewPitch::from_out(&out("v2"), "gemini", "m")).unwrap();
        set_current(&mut c, id1).unwrap();
        assert_eq!(current(&c, t).unwrap().unwrap().text, "v1");
        assert_eq!(list(&c, t).unwrap().len(), 2);
        assert!(set_current(&mut c, 999).is_err());
    }
}
