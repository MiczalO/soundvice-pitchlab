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
    send_with(service, true, make).await
}

/// To samo co `send`, ale `retry_429 = false` zwraca 429 od razu (bez czekania i ponowienia) —
/// dla wywołań, w których limit zwykle oznacza brak przydziału (np. wyszukiwarka Google).
pub async fn send_with(
    service: &'static str,
    retry_429: bool,
    make: impl Fn() -> RequestBuilder,
) -> AppResult<Response> {
    let mut res = make().send().await?;
    if retry_429 && res.status() == StatusCode::TOO_MANY_REQUESTS {
        let wait = retry_after(&res).unwrap_or(20).min(60);
        let body = res.text().await.unwrap_or_default();
        // Brak przydziału / limit dzienny: ponawianie nic nie da, więc nie czekamy.
        if is_hard_quota(&body) {
            return Err(classify(service, 429, &body));
        }
        tokio::time::sleep(Duration::from_secs(wait)).await;
        res = make().send().await?;
    }
    check(service, res).await
}

/// Limit, który nie zniknie po kilkunastu sekundach.
fn is_hard_quota(body: &str) -> bool {
    if body.contains("PerMinute") {
        return false; // limit minutowy mija sam — jedno ponowienie ma sens
    }
    body.contains("limit: 0") || body.contains("PerDay")
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
    // 503 (Gemini „high demand”) i 529 (Anthropic „overloaded”): przeciążenie po stronie dostawcy.
    if status == 503 || status == 529 {
        return AppError::Overloaded(service);
    }
    if status == 429 {
        let hint = if body.contains("limit: 0") {
            "Ten model nie ma przydziału w Twoim planie (darmowy plan zwykle nie obejmuje modeli Pro). \
             Wybierz model flash w Ustawieniach albo włącz płatny plan."
        } else if body.contains("PerDay") {
            "Wyczerpany dzienny limit. Spróbuj jutro albo zmień model lub plan."
        } else {
            "Odczekaj chwilę i spróbuj ponownie albo zmień model."
        };
        let google: String = message.chars().take(200).collect();
        return AppError::RateLimited(service, format!("{hint} (odpowiedź: {google})"));
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
    fn zero_quota_explains_plan_and_keeps_google_message() {
        let body = r#"{"error":{"code":429,"message":"Quota exceeded for metric ... limit: 0, model: gemini-3.1-pro"}}"#;
        let e = classify("Gemini", 429, body);
        assert_eq!(e.code(), "rate_limited");
        let m = e.to_string();
        assert!(m.contains("nie ma przydziału"), "{m}");
        assert!(m.contains("limit: 0"), "{m}");
        assert!(is_hard_quota(body));
    }

    #[test]
    fn zero_or_daily_quota_is_hard_but_generic_or_per_minute_is_not() {
        assert!(is_hard_quota("Quota exceeded ... limit: 0, model: x"));
        assert!(is_hard_quota(
            "... GenerateRequestsPerDayPerProjectPerModel-FreeTier"
        ));
        assert!(!is_hard_quota(
            "You exceeded your current quota, please check your plan"
        ));
        assert!(!is_hard_quota(
            "... limit: 0 ... GenerateRequestsPerMinutePerProjectPerModel"
        ));
    }

    #[test]
    fn transient_429_suggests_waiting() {
        let e = classify(
            "Gemini",
            429,
            r#"{"error":{"message":"Resource exhausted"}}"#,
        );
        assert!(e.to_string().contains("Odczekaj chwilę"));
        assert!(!is_hard_quota("Resource exhausted"));
    }

    #[test]
    fn high_demand_503_is_overloaded_not_generic() {
        let body = r#"{"error":{"code":503,"message":"This model is currently experiencing high demand.","status":"UNAVAILABLE"}}"#;
        let e = classify("Gemini", 503, body);
        assert_eq!(e.code(), "overloaded");
        assert!(e.to_string().contains("przeciążony"));
        assert_eq!(classify("Anthropic", 529, "{}").code(), "overloaded");
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
