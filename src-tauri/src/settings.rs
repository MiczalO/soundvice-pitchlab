//! Ustawienia nie-tajne (tabela `settings`). Klucze API tu nie trafiają.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// Dozwolone klucze ustawień — cokolwiek innego jest odrzucane.
pub const ALLOWED: &[&str] = &[
    "gemini_model",
    "gemini_model_research",
    "engine",
    "claude_model",
    "theme",
    "pitch_lang",
    "onboarded",
    "gemini_models_cache",
    "gemini_model_status",
];

pub fn get_all(conn: &Connection) -> AppResult<HashMap<String, String>> {
    let mut st = conn.prepare("SELECT key, value FROM settings")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = HashMap::new();
    for row in rows {
        let (k, v) = row?;
        out.insert(k, v);
    }
    Ok(out)
}

pub fn get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
        r.get(0)
    })
    .ok()
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    if !ALLOWED.contains(&key) {
        return Err(AppError::Other(format!("Nieznane ustawienie: {key}")));
    }
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        (key, value),
    )?;
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
    fn set_get_roundtrip_and_upsert() {
        let c = mem();
        set(&c, "engine", "claude").unwrap();
        set(&c, "engine", "gemini").unwrap();
        assert_eq!(get(&c, "engine").as_deref(), Some("gemini"));
        assert_eq!(
            get_all(&c).unwrap().get("pitch_lang").map(String::as_str),
            Some("en")
        );
    }

    #[test]
    fn rejects_unknown_and_secret_keys() {
        let c = mem();
        assert!(set(&c, "gemini_api_key", "AIza...").is_err());
        assert!(set(&c, "whatever", "x").is_err());
    }
}
