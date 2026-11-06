use std::path::Path;
use std::sync::{LazyLock, Mutex};

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

pub mod artists;
pub mod history;
pub mod pitches;
pub mod repo;
pub mod rules;
pub mod slug;

/// Domyślne zasady pitchu (port 1:1 z prototypu, `DEFAULT_RULES`).
pub const DEFAULT_RULES: &str = include_str!("default_rules.txt");

/// Poprzednie domyślne wersje zasad (v1 z prototypu, v2 „opis utworu”). Jeśli użytkownik
/// ich nie zmieniał, przy starcie dostaje obecną domyślną jako kolejną wersję
/// (stare zostają w historii).
pub(crate) const OLD_DEFAULT_RULES: [&str; 2] = [
    include_str!("default_rules_v1.txt"),
    include_str!("default_rules_v2.txt"),
];

/// Tekst domyślnych zasad z prototypu (v1) — do rozpoznania „nietkniętych” zasad przy imporcie.
#[cfg(test)]
pub(crate) const DEFAULT_RULES_V1_TEXT: &str = OLD_DEFAULT_RULES[0];

/// Czy podany tekst zasad to jedna z domyślnych wersji (stara lub obecna).
pub fn is_default_rules(text: &str) -> bool {
    let t = text.trim();
    t == DEFAULT_RULES.trim() || OLD_DEFAULT_RULES.iter().any(|o| o.trim() == t)
}

/// Liczba migracji = wersja schematu (`PRAGMA user_version`) tej wersji aplikacji.
pub const SCHEMA_VERSION: u32 = 1;

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
    // Uaktualnienie nietkniętych zasad ze starej wersji domyślnej do obecnej.
    let current: String = conn.query_row(
        "SELECT body FROM rules WHERE kind='rules' ORDER BY version DESC LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    if OLD_DEFAULT_RULES
        .iter()
        .any(|old| old.trim() == current.trim())
    {
        conn.execute(
            "INSERT INTO rules (kind, version, body, created_at)
             SELECT 'rules', MAX(version) + 1, ?1, ?2 FROM rules WHERE kind='rules'",
            (DEFAULT_RULES, now),
        )?;
    }
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
    fn schema_version_matches_applied_migrations() {
        let c = mem();
        let v: u32 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
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
    fn untouched_old_default_rules_are_upgraded_once() {
        for old in OLD_DEFAULT_RULES {
            let mut c = mem();
            c.execute("UPDATE rules SET body=?1 WHERE kind='rules'", [old])
                .unwrap();
            migrate(&mut c).unwrap();
            migrate(&mut c).unwrap(); // drugi start nie dubluje
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM rules WHERE kind='rules'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(n, 2);
            assert_eq!(rules::current(&c, "rules"), DEFAULT_RULES);
        }
    }

    #[test]
    fn user_edited_rules_are_never_overwritten() {
        let mut c = mem();
        c.execute("UPDATE rules SET body='MOJE ZASADY' WHERE kind='rules'", [])
            .unwrap();
        migrate(&mut c).unwrap();
        assert_eq!(rules::current(&c, "rules"), "MOJE ZASADY");
    }

    #[test]
    fn new_default_describes_the_track_not_the_artist() {
        assert!(DEFAULT_RULES.contains("NIE OPISUJ ARTYSTY"));
        assert!(DEFAULT_RULES.contains("LIMIT: maksymalnie 500 znaków ze spacjami"));
        for old in OLD_DEFAULT_RULES {
            assert_ne!(DEFAULT_RULES.trim(), old.trim());
        }
        // struktura jak w pitchach wzorcowych
        assert!(DEFAULT_RULES.contains("Produced by X"));
        assert!(DEFAULT_RULES.contains("Bez BPM, tonacji, znaczników czasu"));
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
