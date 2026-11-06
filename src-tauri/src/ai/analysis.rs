//! Struktura analizy odsłuchu + walidacja odpowiedzi Gemini.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Analysis {
    pub primary_genre: String,
    pub subgenres: Vec<String>,
    pub genre_rationale: String,
    #[serde(deserialize_with = "lenient_f64")]
    pub bpm: Option<f64>,
    pub key: Option<String>,
    #[serde(deserialize_with = "lenient_f64")]
    pub energy: Option<f64>,
    pub moods: Vec<String>,
    pub instruments: Vec<String>,
    pub production_notes: String,
    pub vocals: Vocals,
    pub lyrics_themes: Vec<String>,
    pub lyrics_summary: String,
    pub hook: String,
    pub structure: Vec<Section>,
    pub standout_moments: Vec<Moment>,
    pub sound_alikes: Vec<String>,
    pub explicit_content: Option<bool>,
    pub editorial_angle: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Vocals {
    #[serde(rename = "type")]
    pub kind: String,
    pub language: String,
    pub delivery: String,
    pub features: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Section {
    pub t: String,
    pub section: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Moment {
    pub t: String,
    pub desc: String,
}

/// Modele czasem zwracają liczby jako napisy ("128", "128 BPM").
fn lenient_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let num: String = s
                .trim()
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            num.parse().ok()
        }
        _ => None,
    })
}

/// Waliduje JSON z modelu; zwraca czytelny błąd, gdy to nie jest analiza utworu.
pub fn validate(v: Value) -> AppResult<Analysis> {
    if !v.is_object() {
        return Err(AppError::ModelOutput(
            "oczekiwano obiektu JSON z analizą utworu".into(),
        ));
    }
    let a: Analysis = serde_json::from_value(v)
        .map_err(|e| AppError::ModelOutput(format!("nieprawidłowa struktura analizy: {e}")))?;
    if a.primary_genre.trim().is_empty() {
        return Err(AppError::ModelOutput(
            "w analizie brakuje pola primary_genre (gatunek)".into(),
        ));
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_full_and_lenient_numbers() {
        let a = validate(json!({
            "primary_genre": "boom bap",
            "bpm": "92 BPM",
            "energy": 7,
            "vocals": {"type": "rap", "language": "polski"},
            "structure": [{"t": "0:00", "section": "intro"}],
            "unknown_field": 1
        }))
        .unwrap();
        assert_eq!(a.bpm, Some(92.0));
        assert_eq!(a.energy, Some(7.0));
        assert_eq!(a.vocals.kind, "rap");
        assert_eq!(a.structure[0].section, "intro");
    }

    #[test]
    fn null_bpm_is_none() {
        let a = validate(json!({"primary_genre": "pop", "bpm": null})).unwrap();
        assert_eq!(a.bpm, None);
    }

    #[test]
    fn rejects_missing_genre_and_non_objects() {
        assert!(validate(json!({"bpm": 100})).is_err());
        assert!(validate(json!([1, 2])).is_err());
        assert!(validate(json!({"primary_genre": "x", "moods": "nie lista"})).is_err());
    }
}
