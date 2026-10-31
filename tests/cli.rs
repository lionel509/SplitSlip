use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use splitslip::config::load_dotenv;

const BIN: &str = env!("CARGO_BIN_EXE_splitslip");

fn fx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn scan_offline_prints_and_writes() {
    let dir = scratch("scan");
    let out = dir.join("out.json");
    let o = Command::new(BIN)
        .current_dir(&dir)
        .arg("scan")
        .arg(fx("weekend.vision.json"))
        .arg("--people")
        .arg(fx("people.json"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&o.stderr));
    assert!(stdout.contains("Sum of shares 48.15 = receipt total 48.15 OK"), "{stdout}");
    assert!(
        stdout.lines().any(|l| l.starts_with("K") && l.contains("12.51")),
        "{stdout}"
    );
    let saved: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(saved["people"].as_array().unwrap().len(), 3);
}

#[test]
fn scan_bad_total_exits_2() {
    let dir = scratch("bad-total");
    let o = Command::new(BIN)
        .current_dir(&dir)
        .arg("scan")
        .arg(fx("bad-total.vision.json"))
        .arg("--people")
        .arg(fx("people.json"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(stderr.contains("48.16"), "{stderr}");
}

#[test]
fn invoice_without_keys_exits_2() {
    let dir = scratch("no-keys");
    let out = dir.join("out.json");
    let made = Command::new(BIN)
        .current_dir(&dir)
        .arg("scan")
        .arg(fx("weekend.vision.json"))
        .arg("--people")
        .arg(fx("people.json"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(made.status.code(), Some(0));
    let o = Command::new(BIN)
        .env_clear()
        .current_dir(&dir)
        .arg("invoice")
        .arg(&out)
        .arg("--yes")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(stderr.contains("PAYPAL_CLIENT_ID"), "{stderr}");
}

#[test]
fn no_args_prints_usage_exit_1() {
    let dir = scratch("usage");
    let o = Command::new(BIN).current_dir(&dir).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stdout).contains("scan"));
}

#[test]
fn dotenv_sets_unset_only() {
    let dir = scratch("dotenv");
    let file = dir.join(".env");
    fs::write(
        &file,
        "# comment line\nSPLITSLIP_TEST_PRESET=from_file\nSPLITSLIP_TEST_NEW=\"quoted\"\n",
    )
    .unwrap();
    std::env::set_var("SPLITSLIP_TEST_PRESET", "kept");
    std::env::remove_var("SPLITSLIP_TEST_NEW");
    load_dotenv(&file);
    assert_eq!(std::env::var("SPLITSLIP_TEST_PRESET").unwrap(), "kept");
    assert_eq!(std::env::var("SPLITSLIP_TEST_NEW").unwrap(), "quoted");
}

/// Runs the binary with `stdin` piped in and the given env (cleared first).
fn run_with_stdin(dir: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(BIN)
        .env_clear()
        .envs(env.iter().copied())
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn scan_weekend(dir: &Path, out: &Path) {
    let o = Command::new(BIN)
        .current_dir(dir)
        .arg("scan")
        .arg(fx("weekend.vision.json"))
        .arg("--people")
        .arg(fx("people.json"))
        .arg("--out")
        .arg(out)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn rescan_refuses_to_wipe_invoice_ids() {
    let dir = scratch("rescan");
    let out = dir.join("out.json");
    scan_weekend(&dir, &out);
    let mut v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    for p in v["people"].as_array_mut().unwrap() {
        if p["initials"] == "K" {
            p["invoice_id"] = "INV-K-SENT".into();
        }
    }
    fs::write(&out, v.to_string()).unwrap();
    let o = Command::new(BIN)
        .current_dir(&dir)
        .arg("scan")
        .arg(fx("weekend.vision.json"))
        .arg("--people")
        .arg(fx("people.json"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("already has invoices"));
    assert!(fs::read_to_string(&out).unwrap().contains("INV-K-SENT"));
}

#[test]
fn invoice_sends_only_after_yes_or_y() {
    let dir = scratch("typed-yes");
    let out = dir.join("out.json");
    scan_weekend(&dir, &out);
    let keys = [
        ("PAYPAL_CLIENT_ID", "id"),
        ("PAYPAL_CLIENT_SECRET", "secret"),
        ("PAYPAL_BASE", "http://127.0.0.1:9"),
    ];
    let no = run_with_stdin(&dir, &["invoice", out.to_str().unwrap()], "n\n", &keys);
    assert_eq!(no.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&no.stdout).contains("Not sent."));
    let eof = run_with_stdin(&dir, &["invoice", out.to_str().unwrap()], "", &keys);
    assert!(String::from_utf8_lossy(&eof.stdout).contains("Not sent."));
    // "y" passes the gate and reaches the network, which is closed on port 9.
    let yes = run_with_stdin(&dir, &["invoice", out.to_str().unwrap()], "y\n", &keys);
    assert_eq!(yes.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&yes.stdout).contains("Not sent."));
    assert!(String::from_utf8_lossy(&yes.stderr).contains("transport error"));
}

#[test]
fn people_flag_accepts_equals_form() {
    let dir = scratch("equals");
    let arg = format!("--people={}", fx("people.json").display());
    let o = Command::new(BIN)
        .current_dir(&dir)
        .arg("scan")
        .arg(fx("weekend.vision.json"))
        .arg(&arg)
        .arg("--out")
        .arg(dir.join("out.json"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn blank_email_is_not_shown_as_me() {
    let dir = scratch("blank-email");
    let people = dir.join("people.json");
    fs::write(
        &people,
        r#"{"me":"L","people":{"L":{"name":"Lee","email":""},"K":{"name":"Kai","email":"kai@example.com"},"M":{"name":"Mo","email":""}}}"#,
    )
    .unwrap();
    let o = Command::new(BIN)
        .current_dir(&dir)
        .arg("scan")
        .arg(fx("weekend.vision.json"))
        .arg("--people")
        .arg(&people)
        .arg("--out")
        .arg(dir.join("out.json"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert!(stdout.lines().any(|l| l.starts_with("L") && l.contains("(me)")), "{stdout}");
    assert!(stdout.lines().any(|l| l.starts_with("M") && l.contains("(no email)")), "{stdout}");
}

#[test]
fn mark_paid_on_a_paid_invoice_makes_no_call() {
    let dir = scratch("already-paid");
    let out = dir.join("out.json");
    scan_weekend(&dir, &out);
    let mut v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    for p in v["people"].as_array_mut().unwrap() {
        if p["initials"] == "K" {
            p["invoice_id"] = "INV-K".into();
            p["status"] = "PAID".into();
        }
    }
    fs::write(&out, v.to_string()).unwrap();
    let o = Command::new(BIN)
        .env_clear()
        .current_dir(&dir)
        .arg("mark-paid")
        .arg(&out)
        .arg("K")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("already paid"));
}