use serde_json::{Value, json};

use super::http;
use crate::error::{AppError, AppResult};
use crate::secrets::{self, Secret};

pub const URL: &str = "https://api.anthropic.com/v1/messages";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

/// Krótkie zapytanie sprawdzające klucz Anthropic i wybrany model.
pub async fn ping(model: &str) -> AppResult<String> {
    let key = secrets::get(Secret::Anthropic)?;
    let c = http::client();
    let body = json!({
        "model": model,
        "max_tokens": 16,
        "messages": [{"role":"user","content":"Reply with one word: OK"}]
    });
    let res = http::send("Anthropic", || {
        c.post(URL)
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
    })
    .await?;
    let v: Value = res.json().await?;
    v["content"][0]["text"]
        .as_str()
        .map(|t| t.trim().chars().take(20).collect())
        .ok_or_else(|| AppError::ModelOutput("pusta odpowiedź Claude".into()))
}
