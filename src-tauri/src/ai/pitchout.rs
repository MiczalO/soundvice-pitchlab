//! Odpowiedź modelu z pitchem: struktura, tolerancyjne parsowanie, liczenie znaków.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::json;
use crate::error::{AppError, AppResult};

/// Limit znaków pitchu w Spotify for Artists.
pub const LIMIT: usize = 500;

/// Pitch liczymy jak Spotify: ze spacjami, w znakach (nie bajtach).
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Modele czasem zwracają `null` zamiast pustego napisu/listy.
fn nz<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Form {
    #[serde(deserialize_with = "nz")]
    pub genres: Vec<String>,
    #[serde(deserialize_with = "nz")]
    pub moods: Vec<String>,
    #[serde(deserialize_with = "nz")]
    pub styles: Vec<String>,
    #[serde(deserialize_with = "nz")]
    pub instruments: Vec<String>,
    #[serde(deserialize_with = "nz")]
    pub culture: String,
    #[serde(deserialize_with = "nz")]
    pub song_type: String,
    #[serde(deserialize_with = "nz")]
    pub vocal_language: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PitchOut {
    #[serde(deserialize_with = "nz")]
    pub pitch: String,
    #[serde(deserialize_with = "nz")]
    pub pitch_pl: String,
    #[serde(deserialize_with = "nz")]
    pub form: Form,
    #[serde(deserialize_with = "nz")]
    pub angle: String,
    #[serde(deserialize_with = "nz")]
    pub facts_used: Vec<String>,
}

/// Parsuje i waliduje odpowiedź z pitchem (JSON w tekście, w blokach kodu itp.).
pub fn parse(text: &str) -> AppResult<PitchOut> {
    let v: Value = json::parse_loose(text)?;
    let mut out: PitchOut = serde_json::from_value(v)
        .map_err(|e| AppError::ModelOutput(format!("nieprawidłowa struktura pitchu: {e}")))?;
    out.pitch = out.pitch.trim().to_string();
    if out.pitch.is_empty() {
        return Err(AppError::ModelOutput("model nie zwrócił pitchu".into()));
    }
    Ok(out)
}

/// Odpowiedź na prośbę o skrócenie: sam `pitch` i opcjonalnie `pitch_pl`.
pub fn parse_shortened(text: &str) -> AppResult<(String, Option<String>)> {
    let v = json::parse_loose(text)?;
    let pitch = v["pitch"].as_str().unwrap_or("").trim().to_string();
    if pitch.is_empty() {
        return Err(AppError::ModelOutput("skrócony pitch jest pusty".into()));
    }
    let pl = v["pitch_pl"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok((pitch, pl))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_characters_with_spaces_not_bytes() {
        assert_eq!(char_count("Mata – zażółć"), 13);
        assert_eq!(char_count(&"a".repeat(500)), LIMIT);
    }

    #[test]
    fn parses_full_response_in_code_fence() {
        let t = "```json\n{\"pitch\":\"Hello world.\",\"pitch_pl\":\"Witaj świecie.\",\"form\":{\"genres\":[\"Hip-Hop\"],\"moods\":[\"Energetic\"],\"styles\":[],\"instruments\":[\"808\"],\"culture\":null,\"song_type\":\"Original\",\"vocal_language\":\"Polish\"},\"angle\":\"a\",\"facts_used\":[\"pill: x\"]}\n```";
        let p = parse(t).unwrap();
        assert_eq!(p.pitch, "Hello world.");
        assert_eq!(p.form.genres, vec!["Hip-Hop"]);
        assert_eq!(p.form.culture, ""); // null → pusty
        assert_eq!(p.facts_used, vec!["pill: x"]);
    }

    #[test]
    fn missing_optional_fields_are_ok_but_pitch_is_required() {
        assert_eq!(parse(r#"{"pitch":"x"}"#).unwrap().form, Form::default());
        assert_eq!(
            parse(r#"{"pitch":"  "}"#).unwrap_err().code(),
            "model_output"
        );
        assert_eq!(parse(r#"{"form":{}}"#).unwrap_err().code(), "model_output");
        assert_eq!(parse("nie json").unwrap_err().code(), "model_output");
    }

    #[test]
    fn shortened_response() {
        let (p, pl) = parse_shortened(r#"{"pitch":" krótko ","pitch_pl":""}"#).unwrap();
        assert_eq!(p, "krótko");
        assert!(pl.is_none());
        assert!(parse_shortened(r#"{"pitch":""}"#).is_err());
    }
}
