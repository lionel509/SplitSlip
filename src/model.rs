use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Cents = i64;
pub type Result<T> = std::result::Result<T, String>;

/// Cents to a dollars string: 4815 -> "48.15", -400 -> "-4.00".
pub fn dollars(c: Cents) -> String {
    {
        let sign = if c < 0 { "-" } else { "" };
        let a = c.unsigned_abs();
        format!("{sign}{}.{:02}", a / 100, a % 100)
    }
}

pub const TAXABLE_FLAGS: &[&str] = &["A"];
pub const PAID_STATUSES: &[&str] = &["PAID", "MARKED_AS_PAID", "PAID_EXTERNAL"];
/// Invoices that will never be paid: nothing is owed, nothing is reminded.
pub const VOID_STATUSES: &[&str] = &["CANCELLED", "REFUNDED", "MARKED_AS_REFUNDED"];

/// Gemini sometimes returns a number where the schema asks for a string (item_no 1001, price 7.99).
fn loose_string<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    use serde::Deserialize;
    Ok(match Option::<serde_json::Value>::deserialize(d)? {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s),
        Some(other) => Some(other.to_string()),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionLine {
    pub kind: String,
    #[serde(default, deserialize_with = "loose_string")]
    pub item_no: Option<String>,
    #[serde(default, deserialize_with = "loose_string")]
    pub text: Option<String>,
    #[serde(default, deserialize_with = "loose_string")]
    pub expanded: Option<String>,
    #[serde(default, deserialize_with = "loose_string")]
    pub price: Option<String>,
    #[serde(default, deserialize_with = "loose_string")]
    pub tax_flag: Option<String>,
    #[serde(default)]
    pub written: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vision {
    pub lines: Vec<VisionLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub item_no: Option<String>,
    pub name: String,
    pub price: Cents,
    pub coupon: Cents,
    pub taxable: bool,
    pub written: Vec<String>,
    pub tax: Cents,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Receipt {
    pub items: Vec<Item>,
    pub subtotal: Cents,
    pub tax: Cents,
    pub total: Cents,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct People {
    pub me: String,
    pub people: BTreeMap<String, Person>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Share {
    pub item: String,
    pub amount: Cents,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonTotal {
    pub initials: String,
    pub name: String,
    pub email: String,
    pub shares: Vec<Share>,
    pub total: Cents,
    #[serde(default)]
    pub invoice_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    /// unix seconds
    #[serde(default)]
    pub sent_at: Option<i64>,
    /// unix seconds of the last reminder sent
    #[serde(default)]
    pub last_reminder: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub source: String,
    pub me: String,
    pub receipt_total: Cents,
    /// sorted by initials, includes me
    pub people: Vec<PersonTotal>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvoiceSummary {
    pub id: String,
    pub status: String,
    pub total: Cents,
    pub email: String,
    pub invoice_date: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RollupRow {
    pub email: String,
    pub count: usize,
    pub paid: Cents,
    pub open: Cents,
}
