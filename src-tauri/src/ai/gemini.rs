use serde_json::{Value, json};

use super::http;
use crate::error::{AppError, AppResult};
use crate::secrets::{self, Secret};

pub const BASE: &str = "https://generativelanguage.googleapis.com";
pub const FALLBACK_MODEL: &str = "gemini-2.5-pro";

fn key() -> AppResult<String> {
    secrets::get(Secret::Gemini)
}

/// Lista modeli Gemini z `generateContent`, posortowana: najnowsze „pro” na górze.
pub async fn list_models() -> AppResult<Vec<String>> {
    let key = key()?;
    let c = http::client();
    let res = http::send("Gemini", || {
        c.get(format!("{BASE}/v1beta/models?pageSize=200"))
            .header("x-goog-api-key", &key)
    })
    .await?;
    let data: Value = res.json().await?;
    let names = data["models"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|m| {
                    m["supportedGenerationMethods"]
                        .as_array()
                        .is_some_and(|g| g.iter().any(|x| x == "generateContent"))
                })
                .filter_map(|m| m["name"].as_str())
                .map(|n| n.trim_start_matches("models/").to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(filter_and_rank(names))
}

const EXCLUDED: &[&str] = &[
    "embed",
    "image",
    "tts",
    "live",
    "aqa",
    "robotics",
    "computer-use",
];

/// Port filtra i `score()` z prototypu (`loadModels`).
pub fn filter_and_rank(names: Vec<String>) -> Vec<String> {
    let mut list: Vec<String> = names
        .into_iter()
        .filter(|n| {
            let l = n.to_lowercase();
            l.contains("gemini") && !EXCLUDED.iter().any(|x| l.contains(x))
        })
        .collect();
    list.sort_by(|a, b| score(a).total_cmp(&score(b)).then_with(|| a.cmp(b)));
    list.dedup();
    list
}

fn score(n: &str) -> f64 {
    let tier = if n.contains("pro") {
        0.0
    } else if n.contains("flash") && !n.contains("flash-lite") {
        1.0
    } else {
        2.0
    };
    let version = n
        .split_once("gemini-")
        .map(|(_, r)| {
            r.chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<String>()
        })
        .and_then(|v| v.trim_end_matches('.').parse::<f64>().ok())
        .unwrap_or(0.0);
    let preview = if n.contains("preview") || n.contains("exp") {
        5.0
    } else {
        0.0
    };
    tier * 1000.0 - version * 10.0 + preview
}

/// Krótkie zapytanie sprawdzające klucz i model.
pub async fn ping(model: &str) -> AppResult<String> {
    let key = key()?;
    let c = http::client();
    let url = format!(
        "{BASE}/v1beta/models/{}:generateContent",
        model.trim_start_matches("models/")
    );
    let body = json!({
        "contents":[{"role":"user","parts":[{"text":"Odpowiedz jednym słowem: OK"}]}],
        "generationConfig":{"temperature":0}
    });
    let res = http::send("Gemini", || {
        c.post(&url).header("x-goog-api-key", &key).json(&body)
    })
    .await?;
    let v: Value = res.json().await?;
    v["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|t| t.trim().chars().take(20).collect())
        .ok_or_else(|| AppError::ModelOutput("pusta odpowiedź Gemini".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn filters_non_text_models() {
        let r = filter_and_rank(v(&[
            "gemini-2.5-pro",
            "gemini-embedding-001",
            "gemini-2.5-flash-image",
            "gemini-live-2.5-flash",
            "gemini-2.5-flash-preview-tts",
            "gemma-3-27b",
        ]));
        assert_eq!(r, v(&["gemini-2.5-pro"]));
    }

    #[test]
    fn newest_pro_first_then_flash_then_lite() {
        let r = filter_and_rank(v(&[
            "gemini-2.5-flash-lite",
            "gemini-2.5-flash",
            "gemini-2.5-pro",
            "gemini-3.0-pro",
        ]));
        assert_eq!(
            r,
            v(&[
                "gemini-3.0-pro",
                "gemini-2.5-pro",
                "gemini-2.5-flash",
                "gemini-2.5-flash-lite"
            ])
        );
    }

    #[test]
    fn preview_ranks_below_stable_same_version() {
        let r = filter_and_rank(v(&["gemini-2.5-pro-preview-05-06", "gemini-2.5-pro"]));
        assert_eq!(r[0], "gemini-2.5-pro");
    }
}
