//! Rejestr długich operacji: anulowanie + zdarzenia postępu dla UI.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::error::AppError;

#[derive(Default)]
pub struct Jobs {
    next: AtomicU64,
    tokens: Mutex<HashMap<u64, CancellationToken>>,
}

impl Jobs {
    pub fn start(&self) -> (u64, CancellationToken) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let token = CancellationToken::new();
        self.tokens.lock().unwrap().insert(id, token.clone());
        (id, token)
    }

    pub fn cancel(&self, id: u64) {
        if let Some(t) = self.tokens.lock().unwrap().get(&id) {
            t.cancel();
        }
    }

    pub fn finish(&self, id: u64) {
        self.tokens.lock().unwrap().remove(&id);
    }
}

#[derive(Clone, Copy, Serialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Active,
    Done,
    Skip,
    Err,
}

#[derive(Clone, Serialize)]
pub struct StepEvent {
    pub job_id: u64,
    pub step: &'static str,
    pub state: StepState,
    pub sub: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct DoneEvent {
    pub job_id: u64,
    pub track_id: Option<i64>,
    pub cancelled: bool,
    pub error: Option<AppError>,
}

pub fn emit_step(
    app: &AppHandle,
    job_id: u64,
    step: &'static str,
    state: StepState,
    sub: Option<String>,
) {
    let _ = app.emit(
        "job-step",
        StepEvent {
            job_id,
            step,
            state,
            sub,
        },
    );
}

pub fn emit_done(app: &AppHandle, ev: DoneEvent) {
    let _ = app.emit("job-done", ev);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_marks_only_that_job() {
        let jobs = Jobs::default();
        let (a, ta) = jobs.start();
        let (b, tb) = jobs.start();
        assert_ne!(a, b);
        jobs.cancel(a);
        assert!(ta.is_cancelled());
        assert!(!tb.is_cancelled());
        jobs.finish(a);
        jobs.cancel(a); // brak panic po zakończeniu
    }
}
