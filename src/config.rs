use std::path::Path;

use crate::model::Result;

pub struct Config {
    pub gemini_key: Option<String>,
    pub paypal_client_id: Option<String>,
    pub paypal_secret: Option<String>,
    /// default https://api-m.sandbox.paypal.com
    pub paypal_base: String,
    /// default USD
    pub currency: String,
}

/// KEY=VALUE lines, '#' comments, quotes stripped; sets only vars not already set; silent if missing.
/// A variable that is set but empty counts as unset, so `GEMINI_API_KEY=` in the shell does not block `.env`.
pub fn load_dotenv(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let quoted = value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')));
        let value = if quoted { &value[1..value.len() - 1] } else { value };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        let already = std::env::var(key).map(|v| !v.is_empty()).unwrap_or(false);
        if !already {
            std::env::set_var(key, value);
        }
    }
}

/// Reads GEMINI_API_KEY, PAYPAL_CLIENT_ID, PAYPAL_CLIENT_SECRET, PAYPAL_BASE, PAYPAL_CURRENCY.
pub fn from_env() -> Config {
    let get = |k: &str| {
        std::env::var(k)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    Config {
        gemini_key: get("GEMINI_API_KEY"),
        paypal_client_id: get("PAYPAL_CLIENT_ID"),
        paypal_secret: get("PAYPAL_CLIENT_SECRET"),
        paypal_base: get("PAYPAL_BASE")
            .unwrap_or_else(|| "https://api-m.sandbox.paypal.com".to_string())
            .trim_end_matches('/')
            .to_string(),
        currency: get("PAYPAL_CURRENCY").unwrap_or_else(|| "USD".to_string()),
    }
}

/// Err "{name} is not set; copy .env.example to .env and fill it in".
pub fn require(v: &Option<String>, name: &str) -> Result<String> {
    match v {
        Some(s) if !s.trim().is_empty() => Ok(s.clone()),
        _ => Err(format!(
            "{name} is not set; copy .env.example to .env and fill it in"
        )),
    }
}
