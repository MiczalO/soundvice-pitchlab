use std::path::Path;
use std::sync::{LazyLock, Mutex};

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

pub mod artists;
pub mod pitches;
pub mod repo;
pub mod rules;
pub mod slug;

/// Domyślne zasady pitchu (port 1:1 z prototypu, `DEFAULT_RULES`).
pub const DEFAULT_RULES: &str = include_str!("default_rules.txt");

static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(vec![M::up(include_str!("migrations/0001_init.sql"))]));

pub struct Db(pub Mutex<Connection>);

pub fn migrate(conn: &mut Connection) -> Result<(), rusqlite_migration::Error> {
    conn.pragma_update(None, "foreign_keys", "ON").ok();
    MIGRATIONS.to_latest(conn)?;
    seed_defaults(conn).map_err(rusqlite_migration::Error::from)?;
    Ok(())
}

/// Wstawia domyślne zasady, jeśli jeszcze ich nie ma (idempotentne).
fn seed_defaults(conn: &Connection) -> rusqlite::Result<()> {
    let now = now_ms();
    conn.execute(
        "INSERT OR IGNORE INTO rules (kind, version, body, created_at) VALUES ('rules', 1, ?1, ?2)",
        (DEFAULT_RULES, now),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO rules (kind, version, body, created_at) VALUES ('examples', 1, '', ?1)",
        (now,),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('pitch_lang', 'en')",
        (),
    )?;
    Ok(())
}

pub fn open(path: &Path) -> Result<Db, Box<dyn std::error::Error>> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&mut conn)?;
    Ok(Db(Mutex::new(conn)))
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Data (UTC) w formacie RRRR-MM-DD z milisekund epoki — algorytm „civil from days”.
pub fn ymd(ms: i64) -> String {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymd_formats_known_dates() {
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(951_782_400_000), "2000-02-29");
        assert_eq!(ymd(1_762_387_200_000), "2025-11-06");
        assert_eq!(ymd(-86_400_000), "1969-12-31");
    }

    fn mem() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c).unwrap();
        c
    }

    #[test]
    fn migrations_are_valid() {
        MIGRATIONS.validate().unwrap();
    }

    #[test]
    fn migrate_creates_schema_and_seeds_rules() {
        let c = mem();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM rules WHERE kind='rules'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 1);
        let body: String = c
            .query_row("SELECT body FROM rules WHERE kind='rules'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(body.contains("LIMIT: maksymalnie 500 znaków"));
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut c = mem();
        migrate(&mut c).unwrap();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM rules", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2); // rules + examples, bez duplikatów
    }

    #[test]
    fn artist_slug_is_unique() {
        let c = mem();
        let ins = "INSERT INTO artists (name, slug, created_at, updated_at) VALUES (?1, ?2, 0, 0)";
        c.execute(ins, ("Żółć", "zolc")).unwrap();
        assert!(c.execute(ins, ("Zolc", "zolc")).is_err());
    }

    #[test]
    fn deleting_artist_cascades() {
        let c = mem();
        c.execute(
            "INSERT INTO artists (name, slug, created_at, updated_at) VALUES ('A','a',0,0)",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO tracks (artist_id, created_at) VALUES (1, 0)",
            [],
        )
        .unwrap();
        c.execute("DELETE FROM artists WHERE id=1", []).unwrap();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
