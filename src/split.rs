use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::model::{dollars, Cents, People, PersonTotal, Receipt, Result, Share, Split};

pub fn load_people(path: &Path) -> Result<People> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Err naming unknown initials; asserts sum of totals == receipt total.
pub fn split(r: &Receipt, people: &People, source: &str) -> Result<Split> {
    // Who pays for an item: the people who wrote their initials beside it, or me.
    let payers_of = |written: &[String]| -> Vec<String> {
        if written.is_empty() {
            vec![people.me.clone()]
        } else {
            written.to_vec()
        }
    };

    let mut unknown: BTreeSet<String> = BTreeSet::new();
    for it in &r.items {
        for p in payers_of(&it.written) {
            if p != people.me && !people.people.contains_key(&p) {
                unknown.insert(p);
            }
        }
    }
    if !unknown.is_empty() {
        let list: Vec<String> = unknown.into_iter().collect();
        return Err(format!(
            "unknown initials {} on the receipt; add them to people.json",
            list.join(", ")
        ));
    }

    let mut shares: BTreeMap<String, Vec<Share>> = BTreeMap::new();
    for it in &r.items {
        let payers = payers_of(&it.written);
        let net = it.price + it.coupon + it.tax;
        let n = payers.len() as i64;
        let base = net.div_euclid(n);
        let rem = net.rem_euclid(n);
        let label = if n > 1 {
            format!("{} (1/{n})", it.name)
        } else {
            it.name.clone()
        };
        // Remainder cents go to the first-listed payers, one each.
        for (k, p) in payers.iter().enumerate() {
            let amount = base + if (k as i64) < rem { 1 } else { 0 };
            shares.entry(p.clone()).or_default().push(Share {
                item: label.clone(),
                amount,
            });
        }
    }

    let mut initials: BTreeSet<String> = people.people.keys().cloned().collect();
    initials.insert(people.me.clone());
    let mut out_people = Vec::new();
    for key in initials {
        let (name, email) = match people.people.get(&key) {
            Some(p) => (p.name.clone(), p.email.clone()),
            None => (key.clone(), String::new()),
        };
        let person_shares = shares.remove(&key).unwrap_or_default();
        let total: Cents = person_shares.iter().map(|s| s.amount).sum();
        out_people.push(PersonTotal {
            initials: key,
            name,
            email,
            shares: person_shares,
            total,
            invoice_id: None,
            status: None,
            sent_at: None,
        });
    }

    let sum: Cents = out_people.iter().map(|p| p.total).sum();
    if sum != r.total {
        return Err(format!(
            "shares sum to {} but receipt total is {}",
            dollars(sum),
            dollars(r.total)
        ));
    }
    Ok(Split {
        source: source.to_string(),
        me: people.me.clone(),
        receipt_total: r.total,
        people: out_people,
    })
}

pub fn render_items(r: &Receipt) -> String {
    let mut lines = vec![format!(
        "{:<6} {:<30} {:>8} {:>8} {:>7}  {}",
        "#", "item", "price", "coupon", "tax", "who"
    )];
    for it in &r.items {
        let no = it.item_no.clone().unwrap_or_else(|| "-".to_string());
        let coupon = if it.coupon == 0 {
            "-".to_string()
        } else {
            dollars(it.coupon)
        };
        let who = if it.written.is_empty() {
            "-".to_string()
        } else {
            it.written.join(", ")
        };
        lines.push(format!(
            "{:<6} {:<30} {:>8} {:>8} {:>7}  {}",
            no,
            it.name,
            dollars(it.price),
            coupon,
            dollars(it.tax),
            who
        ));
    }
    lines.push(format!(
        "Subtotal {}  Tax {}  Total {}",
        dollars(r.subtotal),
        dollars(r.tax),
        dollars(r.total)
    ));
    lines.join("\n")
}

pub fn render_people(s: &Split) -> String {
    let mut lines = Vec::new();
    for p in &s.people {
        let email = if p.email.is_empty() {
            "(me)"
        } else {
            p.email.as_str()
        };
        let status = p.status.clone().unwrap_or_else(|| "-".to_string());
        lines.push(format!(
            "{}  {:<12} {:>8}  {}  {}",
            p.initials,
            p.name,
            dollars(p.total),
            email,
            status
        ));
    }
    let sum: Cents = s.people.iter().map(|p| p.total).sum();
    let verdict = if sum == s.receipt_total { "OK" } else { "MISMATCH" };
    lines.push(format!(
        "Sum of shares {} = receipt total {} {}",
        dollars(sum),
        dollars(s.receipt_total),
        verdict
    ));
    lines.join("\n")
}

pub fn save(s: &Split, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    let mut text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    text.push('\n');
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn load(path: &Path) -> Result<Split> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}
