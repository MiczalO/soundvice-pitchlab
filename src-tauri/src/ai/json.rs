//! Tolerancyjny parser JSON z odpowiedzi modeli (port `parseJSONLoose` z prototypu).

use serde_json::Value;

use crate::error::{AppError, AppResult};

pub fn parse_loose(txt: &str) -> AppResult<Value> {
    let mut s = txt.trim();
    if s.is_empty() {
        return Err(AppError::ModelOutput("pusta odpowiedź modelu".into()));
    }
    if let Some(inner) = fenced(s) {
        s = inner;
    }
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        return Ok(v);
    }
    if let (Some(a), Some(b)) = (s.find('{'), s.rfind('}'))
        && b > a
        && let Ok(v) = serde_json::from_str::<Value>(&s[a..=b])
    {
        return Ok(v);
    }
    let preview: String = s.chars().take(400).collect();
    Err(AppError::ModelOutput(format!(
        "model nie zwrócił poprawnego JSON:\n{preview}"
    )))
}

/// Zawartość pierwszego bloku ``` (opcjonalnie z `json`).
fn fenced(s: &str) -> Option<&str> {
    let start = s.find("```")?;
    let rest = &s[start + 3..];
    let rest = rest
        .strip_prefix("json")
        .or_else(|| rest.strip_prefix("JSON"))
        .unwrap_or(rest);
    let end = rest.find("```")?;
    Some(rest[..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_json() {
        assert_eq!(parse_loose(r#"{"a":1}"#).unwrap()["a"], 1);
    }

    #[test]
    fn fenced_json() {
        let t = "Oto wynik:\n```json\n{\"pitch\": \"x\"}\n```\nGotowe.";
        assert_eq!(parse_loose(t).unwrap()["pitch"], "x");
    }

    #[test]
    fn json_surrounded_by_noise() {
        let t = "Jasne! {\"a\": {\"b\": 2}} Mam nadzieję, że pomogło.";
        assert_eq!(parse_loose(t).unwrap()["a"]["b"], 2);
    }

    #[test]
    fn empty_and_garbage_fail_with_model_output() {
        assert_eq!(parse_loose("  ").unwrap_err().code(), "model_output");
        let e = parse_loose("brak jsona tutaj").unwrap_err();
        assert_eq!(e.code(), "model_output");
        assert!(e.to_string().contains("brak jsona"));
    }
}
