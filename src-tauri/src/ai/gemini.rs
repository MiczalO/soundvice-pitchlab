use std::path::Path;

use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

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
    "transcribe",
    "banana",
];

/// Filtr i ranking z prototypu (`loadModels`), z poprawką: stabilne przed „preview”.
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
    // Modele „preview/exp” bywają przeciążone lub bez przydziału w darmowym planie,
    // więc wszystkie stabilne idą przed nimi (domyślnie wybierany jest pierwszy z listy).
    let preview = if n.contains("preview") || n.contains("exp") {
        10_000.0
    } else {
        0.0
    };
    tier * 1000.0 - version * 10.0 + preview
}

/// Element `parts` zapytania generateContent.
#[derive(Debug, Clone)]
pub enum Part {
    Text(String),
    Inline { mime: String, data_b64: String },
    File { mime: String, uri: String },
}

impl Part {
    fn to_json(&self) -> Value {
        match self {
            Part::Text(t) => json!({ "text": t }),
            Part::Inline { mime, data_b64 } => {
                json!({ "inline_data": { "mime_type": mime, "data": data_b64 } })
            }
            Part::File { mime, uri } => {
                json!({ "file_data": { "mime_type": mime, "file_uri": uri } })
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GenOpts {
    pub json: bool,
    pub search: bool,
    pub temperature: f64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Source {
    pub title: String,
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct Generated {
    pub text: String,
    pub sources: Vec<Source>,
    pub queries: Vec<String>,
}

/// Przerywa przyszłość po anulowaniu joba.
pub async fn cancellable<T>(
    cancel: &CancellationToken,
    fut: impl std::future::Future<Output = AppResult<T>>,
) -> AppResult<T> {
    tokio::select! {
        _ = cancel.cancelled() => Err(AppError::Cancelled),
        r = fut => r,
    }
}

pub async fn generate(
    model: &str,
    parts: &[Part],
    opts: GenOpts,
    cancel: &CancellationToken,
) -> AppResult<Generated> {
    let key = key()?;
    let c = http::client();
    let url = format!(
        "{BASE}/v1beta/models/{}:generateContent",
        model.trim_start_matches("models/")
    );
    let mut gen_cfg = json!({ "temperature": opts.temperature });
    if opts.json {
        gen_cfg["responseMimeType"] = json!("application/json");
    }
    let mut body = json!({
        "contents": [{ "role": "user", "parts": parts.iter().map(Part::to_json).collect::<Vec<_>>() }],
        "generationConfig": gen_cfg,
    });
    if opts.search {
        body["tools"] = json!([{ "google_search": {} }]);
    }
    // Wyszukiwarka Google ma osobny przydział (zwykle tylko płatny plan): 429 przy niej
    // oznacza brak przydziału, więc nie czekamy ani nie ponawiamy.
    let data: Value = cancellable(cancel, async {
        let res = http::send_with("Gemini", !opts.search, || {
            c.post(&url).header("x-goog-api-key", &key).json(&body)
        })
        .await
        .map_err(|e| match e {
            AppError::RateLimited(svc, _) if opts.search => {
                AppError::RateLimited(svc, NO_SEARCH_QUOTA.into())
            }
            other => other,
        })?;
        Ok(res.json::<Value>().await?)
    })
    .await?;
    parse_generated(&data)
}

/// Komunikat przy 429 na wyszukiwarce Google (traktowany jak brak przydziału).
pub const NO_SEARCH_QUOTA: &str = "Wyszukiwarka Google w Gemini nie ma przydziału dla tego klucza     (zwykle wymaga płatnego planu — włącz rozliczenia w Google AI Studio).     Do tego czasu pigułkę możesz napisać ręcznie w karcie Artyści.";

pub fn parse_generated(data: &Value) -> AppResult<Generated> {
    let cand = data["candidates"]
        .get(0)
        .filter(|c| !c.is_null())
        .ok_or_else(|| {
            let reason = data["promptFeedback"]["blockReason"]
                .as_str()
                .map(|r| format!(" (blokada: {r})"))
                .unwrap_or_default();
            AppError::ModelOutput(format!("Gemini nie zwrócił odpowiedzi{reason}"))
        })?;
    let text: String = cand["content"]["parts"]
        .as_array()
        .map(|p| p.iter().filter_map(|x| x["text"].as_str()).collect())
        .unwrap_or_default();
    let mut sources: Vec<Source> = Vec::new();
    for ch in cand["groundingMetadata"]["groundingChunks"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(uri) = ch["web"]["uri"].as_str()
            && !sources.iter().any(|s| s.uri == uri)
        {
            sources.push(Source {
                title: ch["web"]["title"].as_str().unwrap_or(uri).to_string(),
                uri: uri.to_string(),
            });
        }
    }
    let queries = cand["groundingMetadata"]["webSearchQueries"]
        .as_array()
        .map(|q| {
            q.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(Generated {
        text,
        sources,
        queries,
    })
}

#[derive(Debug, Clone)]
pub struct UploadedFile {
    pub uri: String,
    pub mime: String,
}

/// Files API: resumable upload + oczekiwanie na stan ACTIVE.
pub async fn upload_file(
    path: &Path,
    mime: &str,
    display_name: &str,
    cancel: &CancellationToken,
    progress: &(dyn Fn(&str) + Send + Sync),
) -> AppResult<UploadedFile> {
    let key = key()?;
    let c = http::client();
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|e| AppError::Other(format!("Błąd pliku: {e}")))?
        .len();
    let name: String = display_name.chars().take(120).collect();

    let start = cancellable(cancel, async {
        http::send("Gemini", || {
            c.post(format!("{BASE}/upload/v1beta/files"))
                .header("x-goog-api-key", &key)
                .header("X-Goog-Upload-Protocol", "resumable")
                .header("X-Goog-Upload-Command", "start")
                .header("X-Goog-Upload-Header-Content-Length", size.to_string())
                .header("X-Goog-Upload-Header-Content-Type", mime)
                .json(&json!({ "file": { "display_name": name } }))
        })
        .await
    })
    .await?;
    let upload_url = start
        .headers()
        .get("x-goog-upload-url")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .ok_or_else(|| {
            AppError::Other(
                "Files API nie zwróciło adresu wysyłki. Zmniejsz plik (np. MP3 320 kbps) i spróbuj ponownie."
                    .into(),
            )
        })?;

    progress("wysyłam plik do Gemini…");
    let info: Value = cancellable(cancel, async {
        let f = tokio::fs::File::open(path)
            .await
            .map_err(|e| AppError::Other(format!("Błąd pliku: {e}")))?;
        let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(f));
        let res = c
            .post(&upload_url)
            .header("X-Goog-Upload-Offset", "0")
            .header("X-Goog-Upload-Command", "upload, finalize")
            .header("Content-Length", size.to_string())
            .body(body)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let text = res.text().await.unwrap_or_default();
            return Err(http::classify("Gemini", status, &text));
        }
        Ok(res.json::<Value>().await?)
    })
    .await?;

    let file = &info["file"];
    let fname = file["name"].as_str().unwrap_or_default().to_string();
    let mut state = file["state"].as_str().unwrap_or("PROCESSING").to_string();
    let mut uri = file["uri"].as_str().unwrap_or_default().to_string();
    let mut out_mime = file["mimeType"].as_str().unwrap_or(mime).to_string();
    for _ in 0..90 {
        if state != "PROCESSING" {
            break;
        }
        progress("Gemini przetwarza plik…");
        tokio::select! {
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
            _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
        }
        let v: Value = cancellable(cancel, async {
            let res = http::send("Gemini", || {
                c.get(format!("{BASE}/v1beta/{fname}"))
                    .header("x-goog-api-key", &key)
            })
            .await?;
            Ok(res.json::<Value>().await?)
        })
        .await?;
        state = v["state"].as_str().unwrap_or("PROCESSING").to_string();
        if let Some(u) = v["uri"].as_str() {
            uri = u.to_string();
        }
        if let Some(m) = v["mimeType"].as_str() {
            out_mime = m.to_string();
        }
    }
    match state.as_str() {
        "ACTIVE" if !uri.is_empty() => Ok(UploadedFile {
            uri,
            mime: out_mime,
        }),
        "FAILED" => Err(AppError::Other(
            "Gemini nie przetworzył pliku audio.".into(),
        )),
        _ => Err(AppError::Timeout),
    }
}

/// Wynik sondowania modelu: czy da się z niego realnie skorzystać.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Probe {
    pub model: String,
    /// ok | retired | quota | overloaded | timeout | error
    pub status: &'static str,
}

/// Mapowanie odpowiedzi HTTP na status modelu (lista modeli z API nie mówi,
/// czy model jest dostępny dla danego klucza i planu).
pub fn probe_status(http: u16) -> &'static str {
    match http {
        200..=299 => "ok",
        404 => "retired",
        429 => "quota",
        503 | 529 => "overloaded",
        _ => "error",
    }
}

/// Równolegle wysyła jedno malutkie zapytanie do każdego modelu (bez ponowień,
/// 25 s na model) i zwraca statusy w kolejności wejścia.
pub async fn probe_models(models: Vec<String>) -> AppResult<Vec<Probe>> {
    let key = key()?;
    let c = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|_| AppError::Network)?;
    let body = json!({
        "contents": [{"role": "user", "parts": [{"text": "Odpowiedz jednym słowem: OK"}]}],
        "generationConfig": {"temperature": 0}
    });
    let tasks: Vec<_> = models
        .into_iter()
        .map(|model| {
            let (c, key, body) = (c.clone(), key.clone(), body.clone());
            tokio::spawn(async move {
                let url = format!("{BASE}/v1beta/models/{model}:generateContent");
                let status = match c
                    .post(&url)
                    .header("x-goog-api-key", &key)
                    .json(&body)
                    .send()
                    .await
                {
                    Err(_) => "timeout",
                    Ok(res) => probe_status(res.status().as_u16()),
                };
                Probe { model, status }
            })
        })
        .collect();
    let mut out = Vec::with_capacity(tasks.len());
    for t in tasks {
        if let Ok(p) = t.await {
            out.push(p);
        }
    }
    Ok(out)
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
    fn parses_grounded_response() {
        let d = json!({"candidates":[{"content":{"parts":[{"text":"Ala "},{"text":"ma kota"}]},
            "groundingMetadata":{"groundingChunks":[
                {"web":{"uri":"https://a.pl","title":"A"}},
                {"web":{"uri":"https://a.pl","title":"A"}},
                {"web":{"uri":"https://b.pl"}}],
              "webSearchQueries":["q1"]}}]});
        let g = parse_generated(&d).unwrap();
        assert_eq!(g.text, "Ala ma kota");
        assert_eq!(g.sources.len(), 2);
        assert_eq!(g.sources[1].title, "https://b.pl");
        assert_eq!(g.queries, vec!["q1"]);
    }

    #[test]
    fn blocked_prompt_is_readable_error() {
        let e = parse_generated(&json!({"promptFeedback":{"blockReason":"SAFETY"}})).unwrap_err();
        assert!(e.to_string().contains("SAFETY"));
    }

    #[test]
    fn probe_status_mapping() {
        assert_eq!(probe_status(200), "ok");
        assert_eq!(probe_status(404), "retired");
        assert_eq!(probe_status(429), "quota");
        assert_eq!(probe_status(503), "overloaded");
        assert_eq!(probe_status(500), "error");
    }

    #[test]
    fn drops_transcribe_and_banana_models() {
        let r = filter_and_rank(v(&[
            "gemini-3.5-transcribe",
            "gemini-nano-banana-2.1",
            "gemini-3.5-flash",
        ]));
        assert_eq!(r, v(&["gemini-3.5-flash"]));
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

    #[test]
    fn stable_models_come_before_any_preview() {
        let r = filter_and_rank(v(&[
            "gemini-3.1-pro-preview",
            "gemini-2.5-flash",
            "gemini-2.5-pro",
            "gemini-3.0-flash-preview",
        ]));
        assert_eq!(
            r,
            v(&[
                "gemini-2.5-pro",
                "gemini-2.5-flash",
                "gemini-3.1-pro-preview",
                "gemini-3.0-flash-preview",
            ])
        );
    }
}
