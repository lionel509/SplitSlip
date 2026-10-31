use std::path::Path;
use std::process::Command;

use splitslip::http::{base64, Fake, Request, Response, Transport};

fn req(url: &str) -> Request {
    Request {
        method: "GET".into(),
        url: url.into(),
        headers: vec![],
        body: None,
    }
}

#[test]
fn base64_vectors() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn fake_records_calls_and_pops_in_order() {
    let fake = Fake::new(vec![
        Response { status: 200, body: "a".into() },
        Response { status: 201, body: "b".into() },
    ]);
    let r1 = fake.send(&req("https://x/1")).unwrap();
    let r2 = fake.send(&req("https://x/2")).unwrap();
    assert_eq!((r1.status, r1.body.as_str()), (200, "a"));
    assert_eq!((r2.status, r2.body.as_str()), (201, "b"));
    let calls = fake.calls.borrow();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, "https://x/1");
    assert_eq!(calls[1].url, "https://x/2");
}

#[test]
fn fake_errors_when_queue_empty() {
    let fake = Fake::new(vec![]);
    let err = fake.send(&req("https://x/none")).unwrap_err();
    assert!(err.contains("no reply queued"), "{err}");
    assert!(err.contains("https://x/none"), "{err}");
}

#[test]
fn gitignore_covers_private_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ignored = |p: &str| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["check-ignore", "-q", p])
            .status()
            .expect("git on PATH")
            .success()
    };
    assert!(ignored("receipts/x.jpg"), "receipts/ must be ignored");
    assert!(ignored("out/x.json"), "out/ must be ignored");
    assert!(ignored("people.json"), "people.json must be ignored");
    assert!(ignored(".env"), ".env must be ignored");
    assert!(!ignored(".env.example"), ".env.example must be tracked");
}
