use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use splitslip::http::{base64, Fake, Response};
use splitslip::vision::{mime_for, parse_response, read, request, ENDPOINT, MODEL};

fn fx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

fn gemini_body() -> String {
    fs::read_to_string(fx("gemini-response.json")).unwrap()
}

fn temp_image(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("vision");
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn request_targets_flash_with_inline_image() {
    let bytes = b"\xff\xd8\xff fake jpeg bytes";
    let req = request(bytes, "image/jpeg", "KEY123");
    assert_eq!(MODEL, "gemini-2.5-flash");
    assert!(ENDPOINT.contains("models/gemini-2.5-flash:generateContent"));
    assert_eq!(req.method, "POST");
    assert!(req.url.starts_with(ENDPOINT) || req.url.contains("gemini-2.5-flash:generateContent"));
    let key = req
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("x-goog-api-key"))
        .map(|(_, v)| v.as_str());
    assert_eq!(key, Some("KEY123"));
    let body: serde_json::Value = serde_json::from_str(req.body.as_deref().unwrap()).unwrap();
    let part = &body["contents"][0]["parts"][1]["inline_data"];
    assert_eq!(part["mime_type"], "image/jpeg");
    assert_eq!(part["data"], base64(bytes).as_str());
    assert_eq!(body["generationConfig"]["response_mime_type"], "application/json");
    assert_eq!(body["generationConfig"]["temperature"], 0);
}

#[test]
fn parse_response_strips_fences() {
    let v = parse_response(&gemini_body()).unwrap();
    assert_eq!(v.lines.len(), 9);
    assert_eq!(v.lines[0].item_no.as_deref(), Some("1001"));
    assert_eq!(v.lines[8].kind, "total");
}

#[test]
fn parse_response_surfaces_api_error() {
    let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key."}}"#;
    let err = parse_response(body).unwrap_err();
    assert!(err.contains("API key not valid"), "{err}");
}

#[test]
fn read_retries_once_on_429() {
    let img = temp_image("retry.jpg", b"jpg");
    let fake = Fake::new(vec![
        Response { status: 429, body: "{}".into() },
        Response { status: 200, body: gemini_body() },
    ]);
    let v = read(&img, "KEY", &fake, Duration::ZERO).unwrap();
    assert_eq!(v.lines.len(), 9);
    assert_eq!(fake.calls.borrow().len(), 2);
}

#[test]
fn read_fails_after_second_429() {
    let img = temp_image("twice.jpg", b"jpg");
    let fake = Fake::new(vec![
        Response { status: 429, body: "rate limited".into() },
        Response { status: 429, body: "rate limited".into() },
    ]);
    let err = read(&img, "KEY", &fake, Duration::ZERO).unwrap_err();
    assert!(err.contains("429"), "{err}");
    assert_eq!(fake.calls.borrow().len(), 2);
}

#[test]
fn mime_for_extensions() {
    assert_eq!(mime_for(Path::new("a.jpg")).unwrap(), "image/jpeg");
    assert_eq!(mime_for(Path::new("a.jpeg")).unwrap(), "image/jpeg");
    assert_eq!(mime_for(Path::new("a.png")).unwrap(), "image/png");
    assert_eq!(mime_for(Path::new("a.webp")).unwrap(), "image/webp");
    assert!(mime_for(Path::new("a.heic")).is_err());
}
