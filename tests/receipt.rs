use std::path::{Path, PathBuf};

use splitslip::model::{Receipt, Vision};
use splitslip::receipt::{allocate_tax, build, from_vision, parse_cents, reconcile};
use splitslip::vision::load_vision_json;

fn fx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

fn weekend() -> Vision {
    load_vision_json(&fx("weekend.vision.json")).unwrap()
}

fn built() -> Receipt {
    build(&weekend()).unwrap()
}

#[test]
fn parse_cents_handles_formats() {
    assert_eq!(parse_cents("7.99").unwrap(), 799);
    assert_eq!(parse_cents("-3.00").unwrap(), -300);
    assert_eq!(parse_cents("3.00-").unwrap(), -300);
    assert_eq!(parse_cents("$1,234.56").unwrap(), 123456);
    assert!(parse_cents("abc").is_err());
}

#[test]
fn build_attaches_coupon_by_item_no() {
    let r = built();
    assert_eq!(r.items.len(), 5);
    let towel = &r.items[1];
    assert_eq!(towel.item_no.as_deref(), Some("1002"));
    assert_eq!(towel.price, 2199);
    assert_eq!(towel.coupon, -400);
    assert_eq!(r.items[0].coupon, 0);
    assert_eq!(r.subtotal, 4545);
    assert_eq!(r.tax, 270);
    assert_eq!(r.total, 4815);
}

#[test]
fn build_errors_on_orphan_coupon() {
    let v = load_vision_json(&fx("orphan-coupon.vision.json")).unwrap();
    let err = build(&v).unwrap_err();
    assert!(err.contains("1099"), "{err}");
}

#[test]
fn build_errors_when_total_missing() {
    let mut v = weekend();
    v.lines.retain(|l| l.kind != "total");
    let err = build(&v).unwrap_err();
    assert!(err.contains("total"), "{err}");
}

#[test]
fn taxable_only_for_flag_a() {
    let r = built();
    let flags: Vec<bool> = r.items.iter().map(|i| i.taxable).collect();
    assert_eq!(flags, vec![false, true, false, true, false]);
}

#[test]
fn allocate_tax_rides_taxable_lines_and_foots() {
    let mut r = built();
    allocate_tax(&mut r).unwrap();
    assert_eq!(r.items[1].tax, 159);
    assert_eq!(r.items[3].tax, 111);
    for (i, item) in r.items.iter().enumerate() {
        if i != 1 && i != 3 {
            assert_eq!(item.tax, 0, "item {i} is not taxable");
        }
    }
    let sum: i64 = r.items.iter().map(|i| i.tax).sum();
    assert_eq!(sum, 270);
}

#[test]
fn tax_with_no_taxable_items_is_error() {
    let mut v = weekend();
    for l in v.lines.iter_mut() {
        l.tax_flag = None;
    }
    let mut r = build(&v).unwrap();
    assert_eq!(r.tax, 270);
    assert!(allocate_tax(&mut r).is_err());
}

#[test]
fn reconcile_ok_on_weekend() {
    let r = from_vision(&weekend()).unwrap();
    reconcile(&r).unwrap();
}

#[test]
fn reconcile_err_names_both_numbers() {
    let v = load_vision_json(&fx("bad-total.vision.json")).unwrap();
    let err = from_vision(&v).unwrap_err();
    assert!(err.contains("48.16"), "{err}");
    assert!(err.contains("48.15"), "{err}");
}
