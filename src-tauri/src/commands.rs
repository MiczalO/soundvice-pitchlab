use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::ai::{anthropic, gemini};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::secrets::{self, Secret};
use crate::settings;

#[tauri::command]
pub fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn set_secret(name: Secret, value: String) -> AppResult<()> {
    secrets::set(name, &value)
}

#[tauri::command]
pub fn has_secret(name: Secret) -> bool {
    secrets::has(name)
}

#[tauri::command]
pub fn delete_secret(name: Secret) -> AppResult<()> {
    secrets::delete(name)
}

#[tauri::command]
pub fn get_settings(db: State<Db>) -> AppResult<HashMap<String, String>> {
    let conn =
        db.0.lock()
            .map_err(|_| AppError::Db("blokada bazy".into()))?;
    settings::get_all(&conn)
}

#[tauri::command]
pub fn set_setting(db: State<Db>, key: String, value: String) -> AppResult<()> {
    let conn =
        db.0.lock()
            .map_err(|_| AppError::Db("blokada bazy".into()))?;
    settings::set(&conn, &key, &value)
}

#[tauri::command]
pub async fn list_gemini_models() -> AppResult<Vec<String>> {
    gemini::list_models().await
}

#[derive(Serialize)]
pub struct ConnectionReport {
    pub gemini: Option<String>,
    pub claude: Option<String>,
}

/// Sprawdza Gemini (zawsze) i Claude (jeśli silnik pitchu = claude).
#[tauri::command]
pub async fn test_connection(
    gemini_model: Option<String>,
    claude_model: Option<String>,
    check_claude: bool,
) -> AppResult<ConnectionReport> {
    let gm = gemini_model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| gemini::FALLBACK_MODEL.into());
    let g = gemini::ping(&gm).await?;
    let c = if check_claude {
        let cm = claude_model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| anthropic::DEFAULT_MODEL.into());
        Some(anthropic::ping(&cm).await?)
    } else {
        None
    };
    Ok(ConnectionReport {
        gemini: Some(g),
        claude: c,
    })
}
