//! Klucze API żyją wyłącznie w sejfie systemowym (Windows Credential Manager /
//! macOS Keychain). Nigdy w bazie, plikach ani logach; UI może tylko zapytać,
//! czy klucz istnieje (`has_secret`) — odczyt jest dostępny tylko dla Rusta.

use crate::error::{AppError, AppResult};

const SERVICE: &str = "pl.repitch.soundvice-pitchlab";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Secret {
    Gemini,
    Anthropic,
}

impl Secret {
    fn user(self) -> &'static str {
        match self {
            Secret::Gemini => "gemini_api_key",
            Secret::Anthropic => "anthropic_api_key",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Secret::Gemini => "Gemini",
            Secret::Anthropic => "Anthropic",
        }
    }
}

fn entry(s: Secret) -> AppResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, s.user()).map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn set(s: Secret, value: &str) -> AppResult<()> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::Other("Klucz jest pusty.".into()));
    }
    entry(s)?
        .set_password(v)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn get(s: Secret) -> AppResult<String> {
    match entry(s)?.get_password() {
        Ok(v) if !v.trim().is_empty() => Ok(v),
        Ok(_) | Err(keyring::Error::NoEntry) => Err(AppError::NoKey(s.label())),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

pub fn has(s: Secret) -> bool {
    get(s).is_ok()
}

pub fn delete(s: Secret) -> AppResult<()> {
    match entry(s)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}
