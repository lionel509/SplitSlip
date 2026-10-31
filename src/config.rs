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
pub fn load_dotenv(path: &Path) {
    let _ = path;
    todo!("U5 impl: load_dotenv")
}

/// Reads GEMINI_API_KEY, PAYPAL_CLIENT_ID, PAYPAL_CLIENT_SECRET, PAYPAL_BASE, PAYPAL_CURRENCY.
pub fn from_env() -> Config {
    todo!("U5 impl: from_env")
}

/// Err "{name} is not set; copy .env.example to .env and fill it in".
pub fn require(v: &Option<String>, name: &str) -> Result<String> {
    let _ = (v, name);
    todo!("U5 impl: require")
}
