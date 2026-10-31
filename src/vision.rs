use std::path::Path;
use std::time::Duration;

use crate::http::{Request, Transport};
use crate::model::{Result, Vision};

pub const MODEL: &str = "gemini-2.5-flash";
pub const ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

/// Asks for the Vision JSON, strict JSON only.
pub fn prompt() -> String {
    todo!("U3 impl: prompt")
}

/// jpg/jpeg -> image/jpeg, png -> image/png, webp -> image/webp, else Err.
pub fn mime_for(path: &Path) -> Result<&'static str> {
    let _ = path;
    todo!("U3 impl: mime_for")
}

pub fn request(image: &[u8], mime: &str, key: &str) -> Request {
    let _ = (image, mime, key);
    todo!("U3 impl: request")
}

/// Strips ``` fences; {"error":{"message"}} -> Err with the message.
pub fn parse_response(body: &str) -> Result<Vision> {
    let _ = body;
    todo!("U3 impl: parse_response")
}

pub fn load_vision_json(path: &Path) -> Result<Vision> {
    let _ = path;
    todo!("U3 impl: load_vision_json")
}

/// 429/503 -> sleep retry_wait, retry once; second failure -> Err "Gemini {status}: ...".
pub fn read(path: &Path, key: &str, http: &dyn Transport, retry_wait: Duration) -> Result<Vision> {
    let _ = (path, key, http, retry_wait);
    todo!("U3 impl: read")
}
