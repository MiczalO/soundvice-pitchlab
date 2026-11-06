use serde::Serialize;

/// Błąd przekazywany do UI: stabilny `code` + polski komunikat z instrukcją.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("Brak klucza API ({0}). Wpisz go w Ustawieniach.")]
    NoKey(&'static str),
    #[error(
        "Klucz API ({0}) został odrzucony. Sprawdź, czy został wklejony w całości i czy jest aktywny."
    )]
    BadKey(&'static str),
    #[error("Przekroczono limit zapytań ({0}). {1}")]
    RateLimited(&'static str, String),
    #[error(
        "{0} jest chwilowo przeciążony (to nie problem z kluczem ani z internetem). \
         Spróbuj ponownie za kilka minut albo wybierz w Ustawieniach inny model, \
         najlepiej stabilny (bez „preview”)."
    )]
    Overloaded(&'static str),
    #[error(
        "Brak połączenia z siecią lub serwer nie odpowiada. Sprawdź internet i spróbuj ponownie."
    )]
    Network,
    #[error("Upłynął czas oczekiwania na odpowiedź. Spróbuj ponownie.")]
    Timeout,
    #[error("{service} zwrócił błąd {status}: {message}")]
    Api {
        service: &'static str,
        status: u16,
        message: String,
    },
    #[error("Model zwrócił odpowiedź, której nie da się odczytać: {0}")]
    ModelOutput(String),
    #[error("Operacja została zatrzymana.")]
    Cancelled,
    #[error("Sejf systemowy: {0}")]
    Keychain(String),
    #[error("Baza danych: {0}")]
    Db(String),
    #[error("{0}")]
    Other(String),
}

impl AppError {
    /// Limit, który nie zniknie po chwili (brak przydziału w planie lub limit dzienny).
    pub fn is_hard_rate_limit(&self) -> bool {
        matches!(self, AppError::RateLimited(_, d)
            if d.contains("nie ma przydziału") || d.contains("dzienny limit"))
    }

    pub fn code(&self) -> &'static str {
        match self {
            AppError::NoKey(_) => "no_key",
            AppError::BadKey(_) => "bad_key",
            AppError::RateLimited(..) => "rate_limited",
            AppError::Overloaded(_) => "overloaded",
            AppError::Network => "network",
            AppError::Timeout => "timeout",
            AppError::Api { .. } => "api",
            AppError::ModelOutput(_) => "model_output",
            AppError::Cancelled => "cancelled",
            AppError::Keychain(_) => "keychain",
            AppError::Db(_) => "db",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            AppError::Timeout
        } else {
            AppError::Network
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_message() {
        let v = serde_json::to_value(AppError::NoKey("Gemini")).unwrap();
        assert_eq!(v["code"], "no_key");
        assert!(v["message"].as_str().unwrap().contains("Gemini"));
    }
}
