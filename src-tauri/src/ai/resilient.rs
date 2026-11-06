//! Odporne wywołania modeli: przy przeciążeniu (503), braku odpowiedzi lub błędach
//! sieci próbujemy do skutku — z rosnącą przerwą, z przełączaniem na inny model z puli
//! i widocznym postępem. Całość jest anulowalna (przycisk „Zatrzymaj").

use std::future::Future;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

/// Tyle rund z rzędu zakończonych samym limitem zapytań, po których się poddajemy.
const MAX_RATE_LIMIT_ROUNDS: u32 = 4;

/// Po tylu minutach nieprzerwanych porażek poddajemy się z ostatnim błędem.
pub const MAX_TOTAL: Duration = Duration::from_secs(15 * 60);

/// Błędy, które mogą zniknąć same (przeciążenie, sieć, timeout, 5xx, nieczytelna odpowiedź).
pub fn is_retryable(e: &AppError) -> bool {
    match e {
        AppError::Overloaded(_) | AppError::Timeout | AppError::Network => true,
        // Model bywa niedeterministyczny: nieczytelną odpowiedź warto powtórzyć.
        AppError::ModelOutput(_) => true,
        // Chwilowy limit zapytań (np. na minutę) mija sam; brak przydziału — nie.
        AppError::RateLimited(..) => !e.is_hard_rate_limit(),
        AppError::Api { status, .. } => matches!(status, 500 | 502 | 504),
        _ => false,
    }
}

/// Przerwa przed rundą `round` (1, 2, 3…): 5, 10, 20, 40, potem 45 s.
pub fn backoff(round: u32) -> Duration {
    let secs = 5u64 << (round.saturating_sub(1)).min(3);
    Duration::from_secs(secs.min(45))
}

/// Woła `call(model)` po kolei dla modeli z puli, aż któryś się powiedzie.
/// - błąd przejściowy → następny model, a po całej rundzie przerwa i kolejna runda,
/// - błąd dotyczący konkretnego modelu (brak przydziału, wycofany) → model wypada z puli,
/// - zły klucz, anulowanie → natychmiast przerywa.
///
/// Zwraca wynik i nazwę modelu, który zadziałał.
pub async fn run<T, F, Fut>(
    models: &[String],
    cancel: &CancellationToken,
    max_total: Duration,
    wait: impl Fn(u32) -> Duration,
    progress: &(dyn Fn(&str) + Send + Sync),
    call: F,
) -> AppResult<(T, String)>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = AppResult<T>>,
{
    let mut pool: Vec<String> = Vec::new();
    for m in models {
        if !pool.contains(m) {
            pool.push(m.clone());
        }
    }
    if pool.is_empty() {
        return Err(AppError::Other(
            "Nie wybrano żadnego modelu. Wybierz model w Ustawieniach.".into(),
        ));
    }
    let started = Instant::now();
    let mut round = 0u32;
    let mut rate_rounds = 0u32;
    let mut last: Option<AppError> = None;
    loop {
        let mut i = 0;
        while i < pool.len() {
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let model = pool[i].clone();
            if round > 0 || i > 0 {
                progress(&format!("próbuję {model}…"));
            }
            match call(model.clone()).await {
                Ok(v) => return Ok((v, model)),
                Err(e @ (AppError::Cancelled | AppError::BadKey(_) | AppError::NoKey(_))) => {
                    return Err(e);
                }
                Err(e) if is_retryable(&e) => {
                    last = Some(e);
                    i += 1;
                }
                Err(e) => {
                    // Problem tego modelu (np. brak przydziału, wycofany) — pomijamy go.
                    last = Some(e);
                    pool.remove(i);
                }
            }
        }
        if pool.is_empty() || started.elapsed() >= max_total {
            return Err(last.unwrap_or(AppError::Timeout));
        }
        // Same limity zapytań rundę po rundzie: dajmy im szansę, ale nie w nieskończoność.
        if matches!(last, Some(AppError::RateLimited(..))) {
            rate_rounds += 1;
            if rate_rounds >= MAX_RATE_LIMIT_ROUNDS {
                return Err(last.unwrap_or(AppError::Timeout));
            }
        } else {
            rate_rounds = 0;
        }
        round += 1;
        let pause = wait(round);
        progress(&format!(
            "Gemini jest przeciążony — próba {round} nie wyszła, następna za {} s. Możesz zatrzymać.",
            pause.as_secs().max(1)
        ));
        tokio::select! {
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
            _ = tokio::time::sleep(pause) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn models(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    fn fast(_: u32) -> Duration {
        Duration::from_millis(1)
    }
    fn quiet(_: &str) {}

    #[tokio::test]
    async fn retries_overload_until_success() {
        let n = AtomicU32::new(0);
        let (v, m) = run(
            &models(&["a"]),
            &CancellationToken::new(),
            MAX_TOTAL,
            fast,
            &quiet,
            |_| {
                let c = n.fetch_add(1, Ordering::SeqCst);
                async move {
                    if c < 3 {
                        Err(AppError::Overloaded("Gemini"))
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await
        .unwrap();
        assert_eq!((v, m.as_str()), (42, "a"));
        assert_eq!(n.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn falls_back_to_next_model_in_same_round() {
        let used = Mutex::new(Vec::new());
        let (_, m) = run(
            &models(&["a", "b"]),
            &CancellationToken::new(),
            MAX_TOTAL,
            fast,
            &quiet,
            |m| {
                used.lock().unwrap().push(m.clone());
                async move {
                    if m == "a" {
                        Err(AppError::Overloaded("Gemini"))
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(m, "b");
        assert_eq!(*used.lock().unwrap(), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn drops_models_with_quota_or_retired_errors() {
        let calls = Mutex::new(Vec::new());
        let (_, m) = run(
            &models(&["pro", "old", "flash"]),
            &CancellationToken::new(),
            MAX_TOTAL,
            fast,
            &quiet,
            |m| {
                calls.lock().unwrap().push(m.clone());
                async move {
                    match m.as_str() {
                        "pro" => Err(AppError::RateLimited(
                            "Gemini",
                            "Ten model nie ma przydziału".into(),
                        )),
                        "old" => Err(AppError::Api {
                            service: "Gemini",
                            status: 404,
                            message: "x".into(),
                        }),
                        _ => Ok(()),
                    }
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(m, "flash");
        assert_eq!(*calls.lock().unwrap(), vec!["pro", "old", "flash"]);
    }

    #[tokio::test]
    async fn transient_rate_limit_is_retried_then_succeeds() {
        let n = AtomicU32::new(0);
        let (v, _) = run(
            &models(&["a"]),
            &CancellationToken::new(),
            MAX_TOTAL,
            fast,
            &quiet,
            |_| {
                let c = n.fetch_add(1, Ordering::SeqCst);
                async move {
                    if c < 2 {
                        Err(AppError::RateLimited("Gemini", "Odczekaj chwilę".into()))
                    } else {
                        Ok(7)
                    }
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(v, 7);
    }

    #[tokio::test]
    async fn endless_transient_rate_limit_gives_up_after_a_few_rounds() {
        let n = AtomicU32::new(0);
        let e = run(
            &models(&["a"]),
            &CancellationToken::new(),
            MAX_TOTAL,
            fast,
            &quiet,
            |_| {
                n.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(AppError::RateLimited("Gemini", "Odczekaj chwilę".into())) }
            },
        )
        .await
        .unwrap_err();
        assert_eq!(e.code(), "rate_limited");
        assert_eq!(n.load(Ordering::SeqCst), MAX_RATE_LIMIT_ROUNDS);
    }

    #[tokio::test]
    async fn gives_up_when_every_model_is_unusable() {
        let e = run(
            &models(&["a", "b"]),
            &CancellationToken::new(),
            MAX_TOTAL,
            fast,
            &quiet,
            |_| async {
                Err::<(), _>(AppError::RateLimited(
                    "Gemini",
                    "Ten model nie ma przydziału".into(),
                ))
            },
        )
        .await
        .unwrap_err();
        assert_eq!(e.code(), "rate_limited");
    }

    #[tokio::test]
    async fn bad_key_and_other_hard_errors_stop_immediately() {
        let n = AtomicU32::new(0);
        let e = run(
            &models(&["a", "b"]),
            &CancellationToken::new(),
            MAX_TOTAL,
            fast,
            &quiet,
            |_| {
                n.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(AppError::BadKey("Gemini")) }
            },
        )
        .await
        .unwrap_err();
        assert_eq!(e.code(), "bad_key");
        assert_eq!(n.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn gives_up_after_max_total_with_last_error() {
        let e = run(
            &models(&["a"]),
            &CancellationToken::new(),
            Duration::from_millis(20),
            fast,
            &quiet,
            |_| async { Err::<(), _>(AppError::Overloaded("Gemini")) },
        )
        .await
        .unwrap_err();
        assert_eq!(e.code(), "overloaded");
    }

    #[tokio::test]
    async fn cancel_interrupts_waiting_between_rounds() {
        let cancel = CancellationToken::new();
        let c2 = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            c2.cancel();
        });
        let e = run(
            &models(&["a"]),
            &cancel,
            MAX_TOTAL,
            |_| Duration::from_secs(60),
            &quiet,
            |_| async { Err::<(), _>(AppError::Overloaded("Gemini")) },
        )
        .await
        .unwrap_err();
        assert_eq!(e.code(), "cancelled");
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        let secs: Vec<u64> = (1..=6).map(|r| backoff(r).as_secs()).collect();
        assert_eq!(secs, vec![5, 10, 20, 40, 40, 40]);
        assert!(backoff(100).as_secs() <= 45);
    }

    #[test]
    fn retryable_classification() {
        assert!(is_retryable(&AppError::Overloaded("G")));
        assert!(is_retryable(&AppError::Timeout));
        assert!(is_retryable(&AppError::Network));
        assert!(is_retryable(&AppError::ModelOutput("x".into())));
        assert!(is_retryable(&AppError::Api {
            service: "G",
            status: 500,
            message: String::new()
        }));
        assert!(!is_retryable(&AppError::Api {
            service: "G",
            status: 404,
            message: String::new()
        }));
        assert!(is_retryable(&AppError::RateLimited(
            "G",
            "Odczekaj chwilę".into()
        )));
        assert!(!is_retryable(&AppError::RateLimited(
            "G",
            "Ten model nie ma przydziału".into()
        )));
        assert!(!is_retryable(&AppError::RateLimited(
            "G",
            "Wyczerpany dzienny limit".into()
        )));
        assert!(!is_retryable(&AppError::BadKey("G")));
    }
}
