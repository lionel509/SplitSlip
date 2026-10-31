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
