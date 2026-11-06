//! Zapytania do bazy używane przez pipeline i komendy.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

use super::{now_ms, slug::slugify};
use crate::error::AppResult;

/// Zwraca id artysty o danej nazwie (zakłada, jeśli go nie ma). Slug jest unikalny.
pub fn ensure_artist(conn: &Connection, name: &str) -> AppResult<i64> {
    let name = name.trim();
    let slug = slugify(name);
    let now = now_ms();
    conn.execute(
        "INSERT OR IGNORE INTO artists (name, slug, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        (name, &slug, now),
    )?;
    Ok(
        conn.query_row("SELECT id FROM artists WHERE slug=?1", [&slug], |r| {
            r.get(0)
        })?,
    )
}

pub struct NewTrack<'a> {
    pub artist_id: i64,
    pub title: &'a str,
    pub featuring: &'a str,
    pub release_date: Option<&'a str>,
    pub campaign_context: &'a str,
    pub lyrics: &'a str,
    pub audio_path: &'a str,
    pub audio_hash: &'a str,
    pub duration_s: Option<f64>,
}

pub fn insert_track(conn: &Connection, t: &NewTrack) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO tracks (artist_id, title, featuring, release_date, campaign_context, lyrics,
                             audio_path, audio_hash, duration_s, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params![
            t.artist_id,
            t.title,
            t.featuring,
            t.release_date.filter(|d| !d.is_empty()),
            t.campaign_context,
            t.lyrics,
            t.audio_path,
            t.audio_hash,
            t.duration_s,
            now_ms()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_analysis(conn: &Connection, track_id: i64, model: &str, json: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO analyses (track_id, model, json, created_at) VALUES (?1,?2,?3,?4)",
        (track_id, model, json, now_ms()),
    )?;
    Ok(())
}

/// Wpis cache Files API, jeśli jeszcze nie wygasł.
pub fn cached_file(conn: &Connection, hash: &str) -> Option<(String, String)> {
    conn.query_row(
        "SELECT file_uri, mime FROM gemini_files WHERE audio_hash=?1 AND expires_at>?2",
        (hash, now_ms()),
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .ok()
    .flatten()
}

/// Pliki w Gemini wygasają po 48 h; trzymamy 46 h zapasu.
pub const FILE_TTL_MS: i64 = 46 * 3600 * 1000;

pub fn cache_file(conn: &Connection, hash: &str, uri: &str, mime: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO gemini_files (audio_hash, file_uri, mime, expires_at) VALUES (?1,?2,?3,?4)
         ON CONFLICT(audio_hash) DO UPDATE SET file_uri=excluded.file_uri, mime=excluded.mime,
                                               expires_at=excluded.expires_at",
        (hash, uri, mime, now_ms() + FILE_TTL_MS),
    )?;
    Ok(())
}

#[derive(Serialize)]
pub struct TrackView {
    pub id: i64,
    pub artist_id: i64,
    pub artist_name: String,
    pub title: String,
    pub featuring: String,
    pub release_date: Option<String>,
    pub campaign_context: String,
    pub lyrics: String,
    pub audio_path: Option<String>,
    pub duration_s: Option<f64>,
    pub created_at: i64,
    pub analysis: Option<Value>,
    /// Wszystkie wersje pitchu (najnowsze pierwsze); aktualna ma `is_current`.
    pub pitches: Vec<super::pitches::PitchView>,
}

pub fn get_track(conn: &Connection, id: i64) -> AppResult<Option<TrackView>> {
    let row = conn
        .query_row(
            "SELECT t.id, t.artist_id, a.name, t.title, t.featuring, t.release_date,
                    t.campaign_context, t.lyrics, t.audio_path, t.duration_s, t.created_at,
                    (SELECT json FROM analyses WHERE track_id=t.id ORDER BY id DESC LIMIT 1)
             FROM tracks t JOIN artists a ON a.id=t.artist_id WHERE t.id=?1",
            [id],
            |r| {
                let analysis: Option<String> = r.get(11)?;
                Ok(TrackView {
                    id: r.get(0)?,
                    artist_id: r.get(1)?,
                    artist_name: r.get(2)?,
                    title: r.get(3)?,
                    featuring: r.get(4)?,
                    release_date: r.get(5)?,
                    campaign_context: r.get(6)?,
                    lyrics: r.get(7)?,
                    audio_path: r.get(8)?,
                    duration_s: r.get(9)?,
                    created_at: r.get(10)?,
                    analysis: analysis.and_then(|s| serde_json::from_str(&s).ok()),
                    pitches: vec![],
                })
            },
        )
        .optional()?;
    let mut row = row;
    if let Some(t) = row.as_mut() {
        t.pitches = super::pitches::list(conn, t.id)?;
    }
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrate(&mut c).unwrap();
        c
    }

    #[test]
    fn ensure_artist_is_idempotent_across_spellings() {
        let c = mem();
        let a = ensure_artist(&c, "Żółć").unwrap();
        let b = ensure_artist(&c, " zolc ").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn track_roundtrip_with_analysis() {
        let c = mem();
        let artist = ensure_artist(&c, "Mata").unwrap();
        let id = insert_track(
            &c,
            &NewTrack {
                artist_id: artist,
                title: "RETRO",
                featuring: "",
                release_date: Some(""),
                campaign_context: "TikTok",
                lyrics: "",
                audio_path: "x.wav",
                audio_hash: "h",
                duration_s: Some(180.5),
            },
        )
        .unwrap();
        insert_analysis(&c, id, "gemini-2.5-pro", r#"{"primary_genre":"rap"}"#).unwrap();
        let v = get_track(&c, id).unwrap().unwrap();
        assert_eq!(v.artist_name, "Mata");
        assert_eq!(v.release_date, None);
        assert_eq!(v.analysis.unwrap()["primary_genre"], "rap");
        assert!(get_track(&c, 999).unwrap().is_none());
    }

    #[test]
    fn file_cache_respects_expiry() {
        let c = mem();
        cache_file(&c, "h", "https://f/1", "audio/wav").unwrap();
        assert_eq!(cached_file(&c, "h").unwrap().0, "https://f/1");
        c.execute("UPDATE gemini_files SET expires_at=1", [])
            .unwrap();
        assert!(cached_file(&c, "h").is_none());
    }
}
