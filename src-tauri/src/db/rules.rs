//! Zasady pitchu i wzorcowe pitche (wersjonowane; aktualna = najwyższy `version`).

use rusqlite::Connection;

use super::now_ms;
use crate::error::AppResult;

pub fn current(conn: &Connection, kind: &str) -> String {
    conn.query_row(
        "SELECT body FROM rules WHERE kind=?1 ORDER BY version DESC LIMIT 1",
        [kind],
        |r| r.get(0),
    )
    .unwrap_or_default()
}

/// Zapisuje nową wersję treści (poprzednie zostają w historii).
pub fn save(conn: &Connection, kind: &str, body: &str) -> AppResult<()> {
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM rules WHERE kind=?1",
        [kind],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO rules (kind, version, body, created_at) VALUES (?1,?2,?3,?4)",
        (kind, next, body, now_ms()),
    )?;
    Ok(())
}

/// „Dodaj do wzorców”: dopisuje pitch na końcu, oddzielony linią `---`.
pub fn append_example(conn: &Connection, text: &str) -> AppResult<()> {
    let old = current(conn, "examples");
    let body = if old.trim().is_empty() {
        text.trim().to_string()
    } else {
        format!("{}\n---\n{}", old.trim(), text.trim())
    };
    save(conn, "examples", &body)
}

#[derive(serde::Serialize)]
pub struct RuleVersion {
    pub id: i64,
    pub version: i64,
    pub body: String,
    pub created_at: i64,
}

/// Historia wersji (najnowsze pierwsze).
pub fn versions(conn: &Connection, kind: &str) -> AppResult<Vec<RuleVersion>> {
    let mut st = conn.prepare(
        "SELECT id, version, body, created_at FROM rules WHERE kind=?1 ORDER BY version DESC",
    )?;
    let rows = st
        .query_map([kind], |r| {
            Ok(RuleVersion {
                id: r.get(0)?,
                version: r.get(1)?,
                body: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Zapis z formularza: nowa wersja tylko wtedy, gdy treść się zmieniła.
pub fn save_if_changed(conn: &Connection, kind: &str, body: &str) -> AppResult<bool> {
    if current(conn, kind) == body {
        return Ok(false);
    }
    save(conn, kind, body)?;
    Ok(true)
}

/// Przywraca starszą wersję jako nową aktualną (historia nie jest kasowana).
pub fn restore(conn: &Connection, version_id: i64) -> AppResult<()> {
    let (kind, body): (String, String) = conn
        .query_row(
            "SELECT kind, body FROM rules WHERE id=?1",
            [version_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| crate::error::AppError::Other("Nie ma takiej wersji.".into()))?;
    save_if_changed(conn, &kind, &body)?;
    Ok(())
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
    fn defaults_are_seeded_and_new_versions_win() {
        let c = mem();
        assert!(current(&c, "rules").contains("LIMIT: maksymalnie 500 znaków"));
        save(&c, "rules", "NOWE ZASADY").unwrap();
        assert_eq!(current(&c, "rules"), "NOWE ZASADY");
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM rules WHERE kind='rules'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 2); // stara wersja zachowana
    }

    #[test]
    fn save_if_changed_restore_and_versions() {
        let c = mem();
        let default = current(&c, "rules");
        assert!(!save_if_changed(&c, "rules", &default).unwrap());
        assert!(save_if_changed(&c, "rules", "B").unwrap());
        assert!(save_if_changed(&c, "rules", "C").unwrap());
        let v = versions(&c, "rules").unwrap();
        assert_eq!(
            v.iter().map(|x| x.body.as_str()).collect::<Vec<_>>(),
            vec!["C", "B", default.as_str()]
        );
        restore(&c, v[2].id).unwrap(); // przywrócenie domyślnych
        assert_eq!(current(&c, "rules"), default);
        assert_eq!(versions(&c, "rules").unwrap().len(), 4);
        assert!(restore(&c, 99999).is_err());
    }

    #[test]
    fn examples_are_appended_with_separator() {
        let c = mem();
        append_example(&c, "pierwszy").unwrap();
        append_example(&c, " drugi ").unwrap();
        assert_eq!(current(&c, "examples"), "pierwszy\n---\ndrugi");
    }
}
