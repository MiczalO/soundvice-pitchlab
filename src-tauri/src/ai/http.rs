//! Wspólny klient HTTP: mapowanie błędów na `AppError`, jedno ponowienie po 429.

use std::time::Duration;

use reqwest::{RequestBuilder, Response, StatusCode};

use crate::error::{AppError, AppResult};

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .expect("reqwest client")
}

/// Wysyła żądanie; przy 429 czeka (Retry-After, domyślnie 20 s, max 60 s)
/// i ponawia dokładnie raz. Brak dalszych pętli.
pub async fn send(service: &'static str, make: impl Fn() -> RequestBuilder) -> AppResult<Response> {
    let mut res = make().send().await?;
    if res.status() == StatusCode::TOO_MANY_REQUESTS {
        let wait = retry_after(&res).unwrap_or(20).min(60);
        tokio::time::sleep(Duration::from_secs(wait)).await;
        res = make().send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::RateLimited(service));
        }
    }
    check(service, res).await
}

fn retry_after(res: &Response) -> Option<u64> {
    res.headers()
        .get("retry-after")?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

async fn check(service: &'static str, res: Response) -> AppResult<Response> {
    if res.status().is_success() {
        return Ok(res);
    }
    let status = res.status().as_u16();
    let body = res.text().await.unwrap_or_default();
    Err(classify(service, status, &body))
}

pub fn classify(service: &'static str, status: u16, body: &str) -> AppError {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| body.chars().take(300).collect());
    let key_problem = status == 401
        || body.contains("API_KEY_INVALID")
        || body.contains("API key not valid")
        || (status == 403 && body.contains("PERMISSION_DENIED"));
    if key_problem {
        return AppError::BadKey(service);
    }
    if status == 429 {
        return AppError::RateLimited(service);
    }
    AppError::Api {
        service,
        status,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_gemini_key_is_bad_key() {
        let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"reason":"API_KEY_INVALID"}]}}"#;
        assert_eq!(classify("Gemini", 400, body).code(), "bad_key");
    }

    #[test]
    fn anthropic_401_is_bad_key() {
        assert_eq!(classify("Anthropic", 401, "{}").code(), "bad_key");
    }

    #[test]
    fn other_errors_keep_message() {
        let e = classify("Gemini", 500, r#"{"error":{"message":"boom"}}"#);
        assert_eq!(e.code(), "api");
        assert!(e.to_string().contains("boom"));
    }
}
