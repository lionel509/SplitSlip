use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::http::{base64, Request, Transport};
use crate::model::{Result, Vision};

pub const MODEL: &str = "gemini-2.5-flash";
pub const ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

/// Asks for the Vision JSON, strict JSON only.
pub fn prompt() -> String {
    concat!(
        "You are transcribing a warehouse-club store receipt photo. ",
        "Return ONLY strict JSON, no prose, no markdown fences, in exactly this shape: ",
        "{\"lines\":[{\"kind\":\"item|coupon|subtotal|tax|total|other\",\"item_no\":\"string or null\",",
        "\"text\":\"printed text of the line\",\"expanded\":\"plain-English name of the item or null\",",
        "\"price\":\"decimal string such as 7.99 or -4.00\",\"tax_flag\":\"the letter printed right of the price, or null\",",
        "\"written\":[\"initials handwritten beside the line, uppercase, or empty\"]}]}. ",
        "Rules: one object per printed line in receipt order. ",
        "A quantity line such as '2 @ 7.99' is folded into the item it belongs to, with price = the line total (15.98). ",
        "Coupon lines (for example TPD/1002 3.00-) have kind coupon and item_no = the item number they refer to, ",
        "with a negative price. Subtotal, tax and total lines carry only kind and price. ",
        "Use kind other for header, footer, card and member lines; do not transcribe names, addresses or member numbers. ",
        "Expand abbreviations in expanded (KS ORG EGGS 24CT -> Organic eggs, 24 count). ",
        "Copy handwritten initials beside a line into written, and leave it empty if there are none. ",
        "Prices are strings; never invent a line that is not printed."
    )
    .to_string()
}

/// jpg/jpeg -> image/jpeg, png -> image/png, webp -> image/webp, else Err.
pub fn mime_for(path: &Path) -> Result<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "png" => Ok("image/png"),
        "webp" => Ok("image/webp"),
        other => Err(format!(
            "unsupported image type '.{other}' for {}: use jpg, png or webp",
            path.display()
        )),
    }
}

pub fn request(image: &[u8], mime: &str, key: &str) -> Request {
    let body = json!({
        "contents": [{
            "parts": [
                { "text": prompt() },
                { "inline_data": { "mime_type": mime, "data": base64(image) } }
            ]
        }],
        "generationConfig": {
            "temperature": 0,
            "response_mime_type": "application/json"
        }
    });
    Request {
        method: "POST".into(),
        url: ENDPOINT.into(),
        headers: vec![
            ("x-goog-api-key".into(), key.into()),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: Some(body.to_string()),
    }
}

/// Strips ``` fences; {"error":{"message"}} -> Err with the message.
pub fn parse_response(body: &str) -> Result<Vision> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| format!("Gemini response is not JSON: {e}"))?;
    if let Some(msg) = v["error"]["message"].as_str() {
        return Err(format!("Gemini error: {msg}"));
    }
    let text = v["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| "Gemini response has no candidates[0].content.parts[0].text".to_string())?;
    let stripped = strip_fences(text);
    serde_json::from_str::<Vision>(stripped)
        .map_err(|e| format!("Gemini text is not the Vision JSON: {e}"))
}

fn strip_fences(text: &str) -> &str {
    let t = text.trim();
    let t = match t.strip_prefix("```") {
        Some(rest) => {
            let rest = rest.strip_prefix("json").unwrap_or(rest);
            rest.trim_start_matches(['\r', '\n'])
        }
        None => t,
    };
    t.trim_end().strip_suffix("```").unwrap_or(t).trim()
}

pub fn load_vision_json(path: &Path) -> Result<Vision> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str::<Vision>(strip_fences(&text))
        .map_err(|e| format!("{} is not Vision JSON: {e}", path.display()))
}

/// 429/503 -> sleep retry_wait, retry once; second failure -> Err "Gemini {status}: ...".
pub fn read(path: &Path, key: &str, http: &dyn Transport, retry_wait: Duration) -> Result<Vision> {
    let mime = mime_for(path)?;
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let req = request(&bytes, mime, key);
    let mut attempt = 0;
    loop {
        let resp = http.send(&req)?;
        if resp.status == 200 {
            return parse_response(&resp.body);
        }
        let retryable = resp.status == 429 || resp.status == 503;
        if retryable && attempt == 0 {
            attempt += 1;
            std::thread::sleep(retry_wait);
            continue;
        }
        let snippet: String = resp.body.chars().take(300).collect();
        return Err(format!("Gemini {}: {snippet}", resp.status));
    }
}
