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
    fn examples_are_appended_with_separator() {
        let c = mem();
        append_example(&c, "pierwszy").unwrap();
        append_example(&c, " drugi ").unwrap();
        assert_eq!(current(&c, "examples"), "pierwszy\n---\ndrugi");
    }
}
