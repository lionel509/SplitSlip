use std::path::{Path, PathBuf};

use splitslip::model::{People, Receipt, Split, Vision};
use splitslip::receipt::from_vision;
use splitslip::split::{load, load_people, render_people, save, split};
use splitslip::vision::load_vision_json;

fn fx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

fn people() -> People {
    load_people(&fx("people.json")).unwrap()
}

fn weekend_receipt() -> Receipt {
    let v: Vision = load_vision_json(&fx("weekend.vision.json")).unwrap();
    from_vision(&v).unwrap()
}

fn weekend_split() -> Split {
    split(&weekend_receipt(), &people(), "weekend").unwrap()
}

fn total_for(s: &Split, who: &str) -> i64 {
    s.people.iter().find(|p| p.initials == who).unwrap().total
}

#[test]
fn split_weekend_totals() {
    let s = weekend_split();
    let order: Vec<&str> = s.people.iter().map(|p| p.initials.as_str()).collect();
    assert_eq!(order, vec!["K", "L", "M"]);
    assert_eq!(total_for(&s, "K"), 1251);
    assert_eq!(total_for(&s, "L"), 2413);
    assert_eq!(total_for(&s, "M"), 1151);
    assert_eq!(s.receipt_total, 4815);
}

#[test]
fn unwritten_item_goes_to_me() {
    let s = weekend_split();
    let bulbs = |who: &str| -> i64 {
        s.people
            .iter()
            .find(|p| p.initials == who)
            .unwrap()
            .shares
            .iter()
            .filter(|sh| sh.item.starts_with("LED BULBS") || sh.item.starts_with("LED light bulbs"))
            .map(|sh| sh.amount)
            .sum()
    };
    assert_eq!(bulbs("L"), 1360);
    assert_eq!(bulbs("K"), 0);
    assert_eq!(bulbs("M"), 0);
}

#[test]
fn remainder_cents_go_to_first_listed() {
    let s = weekend_split();
    let eggs = |who: &str| -> i64 {
        s.people
            .iter()
            .find(|p| p.initials == who)
            .unwrap()
            .shares
            .iter()
            .filter(|sh| sh.item.starts_with("Organic eggs"))
            .map(|sh| sh.amount)
            .sum()
    };
    assert_eq!(eggs("L"), 400);
    assert_eq!(eggs("K"), 399);
}

#[test]
fn share_names_carry_fraction() {
    let s = weekend_split();
    let l = s.people.iter().find(|p| p.initials == "L").unwrap();
    assert!(
        l.shares.iter().any(|sh| sh.item == "Organic eggs, 24 count (1/2)"),
        "{:?}",
        l.shares
    );
}

#[test]
fn unknown_initials_is_error() {
    let v = load_vision_json(&fx("unknown-initials.vision.json")).unwrap();
    let r = from_vision(&v).unwrap();
    let err = split(&r, &people(), "unknown").unwrap_err();
    assert!(err.contains("Q"), "{err}");
}

#[test]
fn render_people_last_line() {
    let text = render_people(&weekend_split());
    assert_eq!(
        text.lines().last().unwrap(),
        "Sum of shares 48.15 = receipt total 48.15 OK"
    );
}

#[test]
fn save_load_roundtrip() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("split-roundtrip");
    let path = dir.join("out.json");
    let s = weekend_split();
    save(&s, &path).unwrap();
    assert_eq!(load(&path).unwrap(), s);
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    /// inclusive range
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

fn blank(kind: &str) -> splitslip::model::VisionLine {
    splitslip::model::VisionLine {
        kind: kind.into(),
        item_no: None,
        text: None,
        expanded: None,
        price: None,
        tax_flag: None,
        written: vec![],
    }
}

#[test]
fn property_every_cent_assigned_once() {
    use splitslip::model::dollars;
    let mut rng = Lcg(7);
    let people = people();
    let letters = ["K", "L", "M"];
    for case in 0..500 {
        let n_items = rng.range(1, 25) as usize;
        let mut lines = Vec::new();
        let mut subtotal: i64 = 0;
        let mut taxable_net: i64 = 0;
        for i in 0..n_items {
            let price = rng.range(1, 20000) as i64;
            let item_no = format!("{}", 1000 + i);
            let taxable = rng.range(0, 99) < 40;
            let mask = rng.range(0, 7);
            let written: Vec<String> = letters
                .iter()
                .enumerate()
                .filter(|(b, _)| mask & (1 << b) != 0)
                .map(|(_, l)| l.to_string())
                .collect();
            let mut item = blank("item");
            item.item_no = Some(item_no.clone());
            item.text = Some(format!("ITEM {i}"));
            item.expanded = Some(format!("Item {i}"));
            item.price = Some(dollars(price));
            item.tax_flag = if taxable { Some("A".into()) } else { None };
            item.written = written;
            lines.push(item);
            let mut net = price;
            if rng.range(0, 99) < 30 {
                let c = rng.range(1, price as u64) as i64;
                let mut coupon = blank("coupon");
                coupon.item_no = Some(item_no);
                coupon.text = Some(format!("TPD/{}", 1000 + i));
                coupon.price = Some(dollars(-c));
                lines.push(coupon);
                net -= c;
            }
            subtotal += net;
            if taxable {
                taxable_net += net;
            }
        }
        let tax = taxable_net * rng.range(0, 9) as i64 / 100;
        let total = subtotal + tax;
        let mut st = blank("subtotal");
        st.price = Some(dollars(subtotal));
        let mut tx = blank("tax");
        tx.price = Some(dollars(tax));
        let mut tt = blank("total");
        tt.price = Some(dollars(total));
        lines.push(st);
        lines.push(tx);
        lines.push(tt);
        let v = Vision { lines };

        let r = from_vision(&v).unwrap_or_else(|e| panic!("case {case}: from_vision: {e}"));
        let s = split(&r, &people, "property").unwrap_or_else(|e| panic!("case {case}: split: {e}"));

        let sum: i64 = s.people.iter().map(|p| p.total).sum();
        assert_eq!(sum, r.total, "case {case}: people sum to receipt total");
        let tax_sum: i64 = r.items.iter().map(|i| i.tax).sum();
        assert_eq!(tax_sum, r.tax, "case {case}: item tax foots");

        for p in &s.people {
            for sh in &p.shares {
                let item = r
                    .items
                    .iter()
                    .find(|it| {
                        sh.item == it.name || sh.item.starts_with(&format!("{} (", it.name))
                    })
                    .unwrap_or_else(|| panic!("case {case}: share {:?} has no item", sh.item));
                let allowed = if item.written.is_empty() {
                    p.initials == s.me
                } else {
                    item.written.contains(&p.initials)
                };
                assert!(allowed, "case {case}: {} paid for {:?}", p.initials, item.name);
            }
        }
    }
}
