//! Lista pitchy/utworów do widoku „Historia”.

use rusqlite::Connection;
use serde::Serialize;

use crate::ai::pitchout::char_count;
use crate::error::AppResult;

#[derive(Serialize, Debug)]
pub struct HistoryRow {
    pub track_id: i64,
    pub title: String,
    pub artist_id: i64,
    pub artist_name: String,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    /// Liczba znaków aktualnego pitchu (`None`, gdy utwór nie ma jeszcze pitchu).
    pub chars: Option<usize>,
    pub created_at: i64,
}

/// `artist_id`: tylko ten artysta; `query`: fragment tytułu lub artysty (bez względu na
/// wielkość liter, także polskich znaków); `newest_first`: kolejność po dacie utworzenia.
pub fn list(
    conn: &Connection,
    artist_id: Option<i64>,
    query: &str,
    newest_first: bool,
) -> AppResult<Vec<HistoryRow>> {
    let mut st = conn.prepare(
        "SELECT t.id, t.title, a.id, a.name, t.release_date,
                (SELECT json_extract(json, '$.primary_genre') FROM analyses
                 WHERE track_id=t.id ORDER BY id DESC LIMIT 1),
                (SELECT text FROM pitches WHERE track_id=t.id AND is_current=1
                 ORDER BY id DESC LIMIT 1),
                t.created_at
         FROM tracks t JOIN artists a ON a.id=t.artist_id
         WHERE (?1 IS NULL OR a.id=?1)
         ORDER BY t.created_at DESC, t.id DESC",
    )?;
    let q = query.trim().to_lowercase();
    let mut rows = st
        .query_map([artist_id], |r| {
            let text: Option<String> = r.get(6)?;
            Ok(HistoryRow {
                track_id: r.get(0)?,
                title: r.get(1)?,
                artist_id: r.get(2)?,
                artist_name: r.get(3)?,
                release_date: r.get(4)?,
                genre: r.get(5)?,
                chars: text.as_deref().map(char_count),
                created_at: r.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if !q.is_empty() {
        rows.retain(|r| {
            r.title.to_lowercase().contains(&q) || r.artist_name.to_lowercase().contains(&q)
        });
    }
    if !newest_first {
        rows.reverse();
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pitches::{self, NewPitch};
    use crate::db::repo::{NewTrack, ensure_artist, insert_track};

    fn track(c: &Connection, artist: i64, title: &str) -> i64 {
        insert_track(
            c,
            &NewTrack {
                artist_id: artist,
                title,
                featuring: "",
                release_date: None,
                campaign_context: "",
                lyrics: "",
                audio_path: "x",
                audio_hash: "h",
                duration_s: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn filters_by_artist_and_polish_case_insensitive_search() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrate(&mut c).unwrap();
        let a = ensure_artist(&c, "Żółć").unwrap();
        let b = ensure_artist(&c, "Mata").unwrap();
        let t1 = track(&c, a, "Łąka");
        track(&c, b, "RETRO");
        c.execute("UPDATE tracks SET created_at=100 WHERE id=?1", [t1])
            .unwrap();
        c.execute("UPDATE tracks SET created_at=200 WHERE id!=?1", [t1])
            .unwrap();

        assert_eq!(list(&c, None, "", true).unwrap().len(), 2);
        assert_eq!(list(&c, Some(b), "", true).unwrap().len(), 1);
        assert_eq!(list(&c, None, "łĄk", true).unwrap()[0].title, "Łąka");
        assert_eq!(list(&c, None, "żółć", true).unwrap().len(), 1); // po nazwie artysty
        assert_eq!(list(&c, None, "nic", true).unwrap().len(), 0);
        // sortowanie
        assert_eq!(list(&c, None, "", true).unwrap()[0].title, "RETRO");
        assert_eq!(list(&c, None, "", false).unwrap()[0].title, "Łąka");
    }

    #[test]
    fn shows_genre_and_current_pitch_length() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrate(&mut c).unwrap();
        let a = ensure_artist(&c, "Mata").unwrap();
        let t = track(&c, a, "T");
        assert_eq!(list(&c, None, "", true).unwrap()[0].chars, None);
        crate::db::repo::insert_analysis(&c, t, "m", r#"{"primary_genre":"rap"}"#).unwrap();
        let out = crate::ai::pitchout::PitchOut {
            pitch: "zażółć".into(),
            ..Default::default()
        };
        pitches::insert(&mut c, t, &NewPitch::from_out(&out, "gemini", "m")).unwrap();
        let r = &list(&c, None, "", true).unwrap()[0];
        assert_eq!(r.genre.as_deref(), Some("rap"));
        assert_eq!(r.chars, Some(6));
    }
}
