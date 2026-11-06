//! Import kopii JSON z prototypu HTML (`soundvice-pitchlab.html`):
//! `{ artists: {id: {...}}, tracks: [...], rules, examples, lang }`.
//!
//! Zasady bezpieczeństwa danych: import jest idempotentny (ponowny import nie dubluje
//! utworów), nigdy nie nadpisuje niepustych pól istniejącego artysty, a inna pigułka
//! istniejącego artysty trafia jako propozycja do porównania.

use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use crate::db::artists::SourceRow;
use crate::db::{now_ms, repo::ensure_artist, rules, slug::slugify};
use crate::error::{AppError, AppResult};

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct ImportReport {
    pub artists_new: usize,
    pub artists_merged: usize,
    pub tracks: usize,
    pub tracks_skipped: usize,
    pub pitches: usize,
    pub proposals: usize,
    pub rules_updated: bool,
    pub examples_updated: bool,
    pub lang_set: bool,
}

fn s(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}

fn ms(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
}

fn sources_of(v: &Value) -> Vec<SourceRow> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let url = x["uri"].as_str().or_else(|| x["url"].as_str())?;
                    Some(SourceRow {
                        title: x["title"].as_str().unwrap_or(url).to_string(),
                        url: url.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Dodaje propozycję pigułki, o ile identycznej (w dowolnym statusie) jeszcze nie ma.
fn add_proposal(
    tx: &Connection,
    artist_id: i64,
    pill: &str,
    dossier: &str,
    sources: &[SourceRow],
    at: i64,
) -> AppResult<bool> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pill_proposals WHERE artist_id=?1 AND pill=?2)",
        (artist_id, pill),
        |r| r.get(0),
    )?;
    if exists {
        return Ok(false);
    }
    tx.execute(
        "INSERT INTO pill_proposals (artist_id, pill, dossier, sources_json, created_at, status)
         VALUES (?1,?2,?3,?4,?5,'pending')",
        (
            artist_id,
            pill,
            dossier,
            serde_json::to_string(sources).map_err(|e| AppError::Other(e.to_string()))?,
            at,
        ),
    )?;
    Ok(true)
}

fn import_artist(tx: &Connection, key: &str, a: &Value, rep: &mut ImportReport) -> AppResult<()> {
    let name = {
        let n = s(a, "name");
        if n.trim().is_empty() {
            key.to_string()
        } else {
            n
        }
    };
    if name.trim().is_empty() {
        return Ok(());
    }
    let existed: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM artists WHERE slug=?1)",
        [slugify(&name)],
        |r| r.get(0),
    )?;
    let id = ensure_artist(tx, &name)?;
    let (pill, notes, hints, dossier) =
        (s(a, "pill"), s(a, "notes"), s(a, "hints"), s(a, "dossier"));
    let sources = sources_of(&a["sources"]);
    let now = now_ms();
    let created = ms(&a["createdAt"]).unwrap_or(now);
    let updated = ms(&a["updatedAt"]).unwrap_or(created);
    let researched = ms(&a["researchedAt"]);

    let (cur_pill, cur_notes, cur_hints, cur_dossier): (String, String, String, String) = tx
        .query_row(
            "SELECT pill, manager_notes, research_hints, dossier FROM artists WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
    let has_sources: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM artist_sources WHERE artist_id=?1)",
        [id],
        |r| r.get(0),
    )?;

    // Puste pola uzupełniamy, niepustych nie ruszamy.
    let fill = |cur: &str, imp: &str| {
        if cur.trim().is_empty() {
            imp.to_string()
        } else {
            cur.to_string()
        }
    };
    let new_pill = fill(&cur_pill, &pill);
    tx.execute(
        "UPDATE artists SET pill=?2, manager_notes=?3, research_hints=?4, dossier=?5,
                created_at=CASE WHEN ?6 THEN ?7 ELSE created_at END,
                updated_at=CASE WHEN ?6 THEN ?8 ELSE updated_at END,
                researched_at=COALESCE(researched_at, ?9)
         WHERE id=?1",
        rusqlite::params![
            id,
            new_pill,
            fill(&cur_notes, &notes),
            fill(&cur_hints, &hints),
            fill(&cur_dossier, &dossier),
            !existed,
            created,
            updated,
            researched
        ],
    )?;
    if !has_sources {
        for src in &sources {
            tx.execute(
                "INSERT INTO artist_sources (artist_id, title, url, fetched_at) VALUES (?1,?2,?3,?4)",
                (id, &src.title, &src.url, now),
            )?;
        }
    }

    // Inna pigułka istniejącego artysty oraz propozycja z prototypu → propozycje do porównania.
    if existed
        && !cur_pill.trim().is_empty()
        && !pill.trim().is_empty()
        && pill != cur_pill
        && add_proposal(tx, id, &pill, &dossier, &sources, now)?
    {
        rep.proposals += 1;
    }
    let prop = &a["proposal"];
    if prop.is_object() {
        let pp = s(prop, "pill");
        if !pp.trim().is_empty() && pp != new_pill && !new_pill.trim().is_empty() {
            let at = ms(&prop["at"]).unwrap_or(now);
            if add_proposal(
                tx,
                id,
                &pp,
                &s(prop, "dossier"),
                &sources_of(&prop["sources"]),
                at,
            )? {
                rep.proposals += 1;
            }
        }
    }
    if existed {
        rep.artists_merged += 1;
    } else {
        rep.artists_new += 1;
    }
    Ok(())
}

fn import_track(
    tx: &Connection,
    t: &Value,
    names: &std::collections::HashMap<String, String>,
    rep: &mut ImportReport,
) -> AppResult<()> {
    let artist_name = {
        let a = s(t, "artist");
        if a.trim().is_empty() {
            names.get(&s(t, "artistId")).cloned().unwrap_or_default()
        } else {
            a
        }
    };
    if artist_name.trim().is_empty() {
        rep.tracks_skipped += 1;
        return Ok(());
    }
    let artist_id = ensure_artist(tx, &artist_name)?;
    let title = {
        let ti = s(t, "title");
        if ti.trim().is_empty() {
            s(t, "fileName")
        } else {
            ti
        }
    };
    let created = ms(&t["createdAt"]).unwrap_or_else(now_ms);
    let dup: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM tracks WHERE artist_id=?1 AND title=?2 AND created_at=?3)",
        (artist_id, &title, created),
        |r| r.get(0),
    )?;
    if dup {
        rep.tracks_skipped += 1;
        return Ok(());
    }
    let release = s(t, "releaseDate");
    tx.execute(
        "INSERT INTO tracks (artist_id, title, featuring, release_date, campaign_context, lyrics,
                             audio_path, audio_hash, duration_s, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,NULL,NULL,NULL,?7)",
        rusqlite::params![
            artist_id,
            title,
            s(t, "feat"),
            if release.is_empty() {
                None
            } else {
                Some(release)
            },
            s(t, "context"),
            s(t, "lyrics"),
            created
        ],
    )?;
    let track_id = tx.last_insert_rowid();
    if t["analysis"].is_object() {
        tx.execute(
            "INSERT INTO analyses (track_id, model, json, created_at) VALUES (?1,'import',?2,?3)",
            (track_id, t["analysis"].to_string(), created),
        )?;
    }

    // Wersje w prototypie: `versions` (najnowsze pierwsze) + `pitch` (aktualny).
    let mut pitches: Vec<&Value> = t["versions"]
        .as_array()
        .map(|a| a.iter().rev().collect())
        .unwrap_or_default();
    if t["pitch"].is_object() {
        pitches.push(&t["pitch"]);
    }
    pitches.retain(|p| !s(p, "pitch").trim().is_empty());
    let last = pitches.len().saturating_sub(1);
    for (i, p) in pitches.iter().enumerate() {
        let form = if p["form"].is_object() {
            p["form"].to_string()
        } else {
            "{}".into()
        };
        let facts = if p["facts_used"].is_array() {
            p["facts_used"].to_string()
        } else {
            "[]".into()
        };
        let engine = {
            let e = s(p, "engine");
            if e.is_empty() {
                "gemini".to_string()
            } else {
                e
            }
        };
        tx.execute(
            "INSERT INTO pitches (track_id, text, text_pl, form_json, angle, facts_used_json,
                                  engine, model, is_manual_edit, is_current, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,'',?8,?9,?10)",
            rusqlite::params![
                track_id,
                s(p, "pitch").trim(),
                s(p, "pitch_pl"),
                form,
                s(p, "angle"),
                facts,
                engine,
                p["manual"].as_bool().unwrap_or(false),
                i == last,
                ms(&p["at"]).unwrap_or(created)
            ],
        )?;
        rep.pitches += 1;
    }
    rep.tracks += 1;
    Ok(())
}

pub fn import_json(conn: &mut Connection, json: &str) -> AppResult<ImportReport> {
    let v: Value = serde_json::from_str(json)
        .map_err(|_| AppError::Other("To nie jest poprawny plik JSON.".into()))?;
    let (Some(artists), Some(tracks)) = (v["artists"].as_object(), v["tracks"].as_array()) else {
        return Err(AppError::Other(
            "To nie jest kopia pitchLAB z wersji HTML (brak artystów lub utworów).".into(),
        ));
    };
    let mut rep = ImportReport::default();
    let tx = conn.transaction()?;

    let mut names = std::collections::HashMap::new();
    for (key, a) in artists {
        names.insert(key.clone(), s(a, "name"));
        import_artist(&tx, key, a, &mut rep)?;
    }
    for t in tracks {
        import_track(&tx, t, &names, &mut rep)?;
    }

    // Zasady i wzorce: tylko jeśli faktycznie edytowane (nie domyślne z prototypu) i inne niż obecne.
    let imp_rules = s(&v, "rules");
    let untouched_default = crate::db::is_default_rules(&imp_rules);
    if !imp_rules.trim().is_empty() && !untouched_default {
        rep.rules_updated = rules::save_if_changed(&tx, "rules", &imp_rules)?;
    }
    let imp_examples = s(&v, "examples");
    if !imp_examples.trim().is_empty() {
        rep.examples_updated = rules::save_if_changed(&tx, "examples", &imp_examples)?;
    }
    if matches!(v["lang"].as_str(), Some("pl" | "en")) {
        tx.execute(
            "INSERT INTO settings (key, value) VALUES ('pitch_lang', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [v["lang"].as_str().unwrap_or("en")],
        )?;
        rep.lang_set = true;
    }
    tx.commit()?;
    Ok(rep)
}

/// Ułatwia testy: liczba wierszy w tabeli.
#[cfg(test)]
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mem() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrate(&mut c).unwrap();
        c
    }

    /// Kopia w formacie eksportowanym przez prototyp HTML.
    fn fixture() -> String {
        r#"{
          "artists": {
            "mata": {"id":"mata","name":"Mata","pill":"KIM JEST: raper","notes":"nie wspominać o X","hints":"z Warszawy",
                     "dossier":"dossier mata","sources":[{"title":"Źródło","uri":"https://x.pl"}],
                     "proposal":{"pill":"KIM JEST: raper z Warszawy","dossier":"d2","sources":[],"at":1700000500000},
                     "createdAt":1700000000000,"updatedAt":1700000100000,"researchedAt":1700000050000},
            "zolc": {"id":"zolc","name":"Żółć","pill":"","notes":"","hints":"","dossier":"","sources":[],"proposal":null,
                     "createdAt":1700000000000,"updatedAt":1700000000000,"researchedAt":null}
          },
          "tracks": [
            {"id":"t1","title":"RETRO","artist":"Mata","artistId":"mata","feat":"prod. X","releaseDate":"2025-05-01",
             "context":"TikTok","lyrics":"tekst","fileName":"retro.wav","createdAt":1700001000000,
             "analysis":{"primary_genre":"rap","bpm":120},
             "pitch":{"pitch":"Najnowszy pitch","pitch_pl":"PL","form":{"genres":["Hip-Hop"]},"angle":"kąt","facts_used":["pill: x"],"at":1700001300000,"engine":"claude","manual":true},
             "versions":[{"pitch":"Środkowy","at":1700001200000},{"pitch":"Pierwszy","pitch_pl":"P","at":1700001100000}]},
            {"id":"t2","title":"Bez pitchu","artist":"Żółć","artistId":"zolc","createdAt":1700002000000,"analysis":null,"pitch":null,"versions":[]}
          ],
          "rules":"MOJE ZASADY","examples":"Przykład 1\n---\nPrzykład 2","lang":"pl"
        }"#
        .to_string()
    }

    #[test]
    fn imports_artists_tracks_versions_rules_and_lang() {
        let mut c = mem();
        let r = import_json(&mut c, &fixture()).unwrap();
        assert_eq!(
            (
                r.artists_new,
                r.artists_merged,
                r.tracks,
                r.tracks_skipped,
                r.pitches
            ),
            (2, 0, 2, 0, 3)
        );
        assert!(r.rules_updated && r.examples_updated && r.lang_set);

        let id = ensure_artist(&c, "Mata").unwrap();
        let d = crate::db::artists::detail(&c, id).unwrap().unwrap();
        assert_eq!(d.pill, "KIM JEST: raper");
        assert_eq!(d.manager_notes, "nie wspominać o X");
        assert_eq!(d.research_hints, "z Warszawy");
        assert_eq!(d.sources.len(), 1);
        assert_eq!(d.proposal.unwrap().pill, "KIM JEST: raper z Warszawy");

        let t = crate::db::repo::get_track(&c, 1).unwrap().unwrap();
        assert_eq!(
            (t.title.as_str(), t.featuring.as_str()),
            ("RETRO", "prod. X")
        );
        assert_eq!(t.release_date.as_deref(), Some("2025-05-01"));
        assert_eq!(t.created_at, 1_700_001_000_000);
        assert_eq!(t.analysis.unwrap()["primary_genre"], "rap");
        // kolejność: Pierwszy → Środkowy → Najnowszy (aktualny)
        let texts: Vec<_> = t.pitches.iter().rev().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["Pierwszy", "Środkowy", "Najnowszy pitch"]);
        let cur = t.pitches.iter().find(|p| p.is_current).unwrap();
        assert_eq!(cur.text, "Najnowszy pitch");
        assert!(cur.is_manual_edit);
        assert_eq!(cur.engine, "claude");
        assert_eq!(cur.form["genres"][0], "Hip-Hop");
        assert_eq!(t.pitches.iter().filter(|p| p.is_current).count(), 1);

        assert_eq!(rules::current(&c, "rules"), "MOJE ZASADY");
        assert_eq!(
            rules::current(&c, "examples"),
            "Przykład 1\n---\nPrzykład 2"
        );
        assert_eq!(
            crate::settings::get(&c, "pitch_lang").as_deref(),
            Some("pl")
        );
    }

    #[test]
    fn reimport_is_idempotent() {
        let mut c = mem();
        import_json(&mut c, &fixture()).unwrap();
        let r = import_json(&mut c, &fixture()).unwrap();
        assert_eq!(
            (r.tracks, r.tracks_skipped, r.pitches, r.proposals),
            (0, 2, 0, 0)
        );
        assert!(!r.rules_updated && !r.examples_updated);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM tracks"), 2);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM pitches"), 3);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM pill_proposals"), 1);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM artist_sources"), 1);
    }

    #[test]
    fn existing_artist_is_never_overwritten_pill_becomes_proposal() {
        let mut c = mem();
        let id = ensure_artist(&c, "Mata").unwrap();
        crate::db::artists::update(&c, id, "MOJA PIGUŁKA", "", "").unwrap(); // notatki puste
        let r = import_json(&mut c, &fixture()).unwrap();
        assert_eq!((r.artists_new, r.artists_merged), (1, 1));
        let d = crate::db::artists::detail(&c, id).unwrap().unwrap();
        assert_eq!(d.pill, "MOJA PIGUŁKA"); // bez zmian
        assert_eq!(d.manager_notes, "nie wspominać o X"); // puste pole uzupełnione
        // importowana pigułka + propozycja z prototypu = propozycje do porównania
        assert_eq!(r.proposals, 2);
        let pending: i64 = count(
            &c,
            "SELECT COUNT(*) FROM pill_proposals WHERE status='pending'",
        );
        assert_eq!(pending, 2);
    }

    #[test]
    fn untouched_prototype_default_rules_do_not_replace_newer_defaults() {
        let mut c = mem();
        let before = rules::current(&c, "rules");
        let v = json!({"artists":{}, "tracks":[], "rules": crate::db::DEFAULT_RULES_V1_TEXT, "examples":"", "lang":"en"});
        let r = import_json(&mut c, &v.to_string()).unwrap();
        assert!(!r.rules_updated);
        assert_eq!(rules::current(&c, "rules"), before);
    }

    #[test]
    fn rejects_foreign_or_broken_files() {
        let mut c = mem();
        assert!(import_json(&mut c, "nie json").is_err());
        assert!(import_json(&mut c, "[]").is_err());
        assert!(import_json(&mut c, r#"{"artists":{}}"#).is_err());
        assert!(import_json(&mut c, r#"{"artists":[],"tracks":[]}"#).is_err());
        assert_eq!(count(&c, "SELECT COUNT(*) FROM tracks"), 0);
    }

    #[test]
    fn track_with_unknown_artist_is_skipped_not_fatal() {
        let mut c = mem();
        let v = json!({"artists":{}, "tracks":[{"title":"X","createdAt":1}]});
        let r = import_json(&mut c, &v.to_string()).unwrap();
        assert_eq!((r.tracks, r.tracks_skipped), (0, 1));
    }
}
