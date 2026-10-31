use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::http::{base64, Request, Response, Transport};
use crate::model::{
    dollars, Cents, InvoiceSummary, PersonTotal, Result, RollupRow, Split, PAID_STATUSES, VOID_STATUSES,
};

/// REST Invoicing v2. Sandbox base: https://api-m.sandbox.paypal.com
pub struct PayPal<'a> {
    pub base: String,
    pub client_id: String,
    pub secret: String,
    pub currency: String,
    pub http: &'a dyn Transport,
}

const REMIND_NOTE: &str = "Friendly reminder: your share of the split is still open.";

impl<'a> PayPal<'a> {
    fn send(&self, method: &str, path: &str, headers: Vec<(String, String)>, body: Option<String>) -> Result<Response> {
        let req = Request {
            method: method.into(),
            url: format!("{}{}", self.base, path),
            headers,
            body,
        };
        self.http.send(&req)
    }

    /// Any status outside `ok` becomes an Err carrying the first 300 chars of the body.
    fn expect(method: &str, path: &str, resp: Response, ok: &[u16]) -> Result<Response> {
        if ok.contains(&resp.status) {
            return Ok(resp);
        }
        let body: String = resp.body.chars().take(300).collect();
        Err(format!("PayPal {method} {path} -> {}: {body}", resp.status))
    }

    fn bearer(token: &str) -> (String, String) {
        ("Authorization".into(), format!("Bearer {token}"))
    }

    fn json_header() -> (String, String) {
        ("Content-Type".into(), "application/json".into())
    }

    pub fn token(&self) -> Result<String> {
        let path = "/v1/oauth2/token";
        let auth = format!("Basic {}", base64(format!("{}:{}", self.client_id, self.secret).as_bytes()));
        let headers = vec![
            ("Authorization".into(), auth),
            ("Content-Type".into(), "application/x-www-form-urlencoded".into()),
        ];
        let resp = Self::expect(
            "POST",
            path,
            self.send("POST", path, headers, Some("grant_type=client_credentials".into()))?,
            &[200],
        )?;
        let v: Value = serde_json::from_str(&resp.body).map_err(|e| format!("PayPal token: bad JSON: {e}"))?;
        v["access_token"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "PayPal token: no access_token in response".to_string())
    }

    pub fn invoice_body(&self, p: &PersonTotal, note: &str) -> Value {
        let items: Vec<Value> = p
            .shares
            .iter()
            .map(|s| {
                json!({
                    "name": s.item,
                    "quantity": "1",
                    "unit_amount": {"currency_code": self.currency, "value": dollars(s.amount)},
                    "unit_of_measure": "AMOUNT",
                })
            })
            .collect();
        json!({
            "detail": {
                "currency_code": self.currency,
                "note": note,
                "payment_term": {"term_type": "DUE_ON_RECEIPT"},
            },
            "primary_recipients": [{
                "billing_info": {"name": {"given_name": p.name}, "email_address": p.email},
            }],
            "items": items,
        })
    }

    /// POST /v2/invoicing/invoices with Prefer: return=representation; 201 -> .id
    pub fn create_invoice(&self, token: &str, p: &PersonTotal, note: &str) -> Result<String> {
        let path = "/v2/invoicing/invoices";
        let headers = vec![
            Self::bearer(token),
            Self::json_header(),
            ("Prefer".into(), "return=representation".into()),
        ];
        let body = self.invoice_body(p, note).to_string();
        let resp = Self::expect("POST", path, self.send("POST", path, headers, Some(body))?, &[201])?;
        let v: Value = serde_json::from_str(&resp.body).map_err(|e| format!("PayPal create: bad JSON: {e}"))?;
        v["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "PayPal create: no id in response".to_string())
    }

    /// 200 or 202 ok.
    pub fn send_invoice(&self, token: &str, id: &str) -> Result<()> {
        let path = format!("/v2/invoicing/invoices/{id}/send");
        let body = json!({"send_to_invoicer": true, "send_to_recipient": true}).to_string();
        let headers = vec![Self::bearer(token), Self::json_header()];
        Self::expect("POST", &path, self.send("POST", &path, headers, Some(body))?, &[200, 202])?;
        Ok(())
    }

    /// 204 ok.
    pub fn remind(&self, token: &str, id: &str, note: &str) -> Result<()> {
        let path = format!("/v2/invoicing/invoices/{id}/remind");
        let body = json!({"note": note, "send_to_invoicer": true, "send_to_recipient": true}).to_string();
        let headers = vec![Self::bearer(token), Self::json_header()];
        Self::expect("POST", &path, self.send("POST", &path, headers, Some(body))?, &[204])?;
        Ok(())
    }

    pub fn get_status(&self, token: &str, id: &str) -> Result<String> {
        let path = format!("/v2/invoicing/invoices/{id}");
        let resp = Self::expect("GET", &path, self.send("GET", &path, vec![Self::bearer(token)], None)?, &[200])?;
        let v: Value = serde_json::from_str(&resp.body).map_err(|e| format!("PayPal get: bad JSON: {e}"))?;
        v["status"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("PayPal get {id}: no status in response"))
    }

    /// POST .../payments, method CASH; 200 ok.
    pub fn record_payment(&self, token: &str, id: &str, amount: Cents) -> Result<()> {
        let path = format!("/v2/invoicing/invoices/{id}/payments");
        let body = json!({
            "method": "CASH",
            "amount": {"currency_code": self.currency, "value": dollars(amount)},
        })
        .to_string();
        let headers = vec![Self::bearer(token), Self::json_header()];
        Self::expect("POST", &path, self.send("POST", &path, headers, Some(body))?, &[200])?;
        Ok(())
    }

    pub fn list_invoices(&self, token: &str) -> Result<Vec<InvoiceSummary>> {
        let path = "/v2/invoicing/invoices?page=1&page_size=100&total_required=true";
        let resp = Self::expect("GET", path, self.send("GET", path, vec![Self::bearer(token)], None)?, &[200])?;
        let v: Value = serde_json::from_str(&resp.body).map_err(|e| format!("PayPal list: bad JSON: {e}"))?;
        let items = v["items"].as_array().cloned().unwrap_or_default();
        items
            .iter()
            .map(|it| {
                let id = it["id"].as_str().ok_or("PayPal list: item without id")?;
                Ok(InvoiceSummary {
                    id: id.to_string(),
                    status: it["status"].as_str().unwrap_or_default().to_string(),
                    total: parse_amount(it["amount"]["value"].as_str().unwrap_or("0"))?,
                    email: it["primary_recipients"][0]["billing_info"]["email_address"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    invoice_date: it["detail"]["invoice_date"].as_str().unwrap_or_default().to_string(),
                })
            })
            .collect()
    }
}

/// "24.13" -> 2413, "-4.00" -> -400, "7" -> 700.
fn parse_amount(s: &str) -> Result<Cents> {
    let neg = s.starts_with('-');
    let (whole, frac) = s.trim_start_matches('-').split_once('.').unwrap_or((s.trim_start_matches('-'), "0"));
    let whole: Cents = whole.parse().map_err(|_| format!("bad amount: {s}"))?;
    let frac: String = format!("{:0<2}", frac).chars().take(2).collect();
    let frac: Cents = frac.parse().map_err(|_| format!("bad amount: {s}"))?;
    let total = whole * 100 + frac;
    Ok(if neg { -total } else { total })
}

/// Someone gets an invoice when they owe money, are not the invoicer, and either have no invoice
/// yet or have one that was created but never sent (DRAFT).
pub fn needs_invoice(p: &PersonTotal, me: &str) -> bool {
    p.initials != me && p.total > 0 && (p.invoice_id.is_none() || p.status.as_deref() == Some("DRAFT"))
}

/// Create, then send. The id is stored as soon as PayPal returns it, so a failed send leaves a
/// DRAFT behind that the next run sends again instead of creating a second invoice.
pub fn invoice_all(pp: &PayPal, token: &str, split: &mut Split, note: &str, now: i64) -> Result<()> {
    let me = split.me.clone();
    let mut errs = Vec::new();
    for i in 0..split.people.len() {
        if !needs_invoice(&split.people[i], &me) {
            continue;
        }
        if let Err(e) = invoice_one(pp, token, split, i, note, now) {
            errs.push(format!("{}: {e}", split.people[i].initials));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs.join("; ").into()) }
}

fn invoice_one(pp: &PayPal, token: &str, split: &mut Split, i: usize, note: &str, now: i64) -> Result<()> {
    {
        let id = match split.people[i].invoice_id.clone() {
            Some(id) => {
                // A draft from an earlier run: PayPal may already have sent it.
                let status = pp.get_status(token, &id)?;
                if status != "DRAFT" {
                    // Sent by an earlier run whose reply was lost: remind_due needs sent_at to count from.
                    let p = &mut split.people[i];
                    p.sent_at.get_or_insert(now);
                    p.status = Some(status);
                    return Ok(());
                }
                id
            }
            None => {
                let id = pp.create_invoice(token, &split.people[i], note)?;
                let p = &mut split.people[i];
                p.invoice_id = Some(id.clone());
                p.status = Some("DRAFT".into());
                id
            }
        };
        pp.send_invoice(token, &id)?;
        let p = &mut split.people[i];
        p.status = Some("SENT".into());
        p.sent_at = Some(now);
    }
    Ok(())
}

/// Refresh every invoice; a failing one is skipped and reported, so it cannot block the rest.
pub fn refresh_each(pp: &PayPal, token: &str, split: &mut Split) -> Vec<String> {
    let mut errs = Vec::new();
    for p in split.people.iter_mut() {
        if let Some(id) = p.invoice_id.clone() {
            match pp.get_status(token, &id) {
                Ok(s) => p.status = Some(s),
                Err(e) => {
                    p.status = None; // unknown: never remind an invoice we could not read
                    errs.push(format!("{}: {e}", p.initials));
                }
            }
        }
    }
    errs
}

pub fn refresh(pp: &PayPal, token: &str, split: &mut Split) -> Result<()> {
    let errs = refresh_each(pp, token, split);
    if errs.is_empty() { Ok(()) } else { Err(errs.join("; ").into()) }
}

/// Refresh, then remind open invoices (SENT, PARTIALLY_PAID or UNPAID) that are due. The first reminder is
/// due days after sending, later ones days after the previous reminder. Returns reminded ids.
pub fn remind_due(pp: &PayPal, token: &str, split: &mut Split, now: i64, days: i64) -> Result<Vec<String>> {
    for e in refresh_each(pp, token, split) {
        eprintln!("splitslip: skipped {e}");
    }
    let mut reminded = Vec::new();
    for p in split.people.iter_mut() {
        let (Some(id), Some(sent_at)) = (p.invoice_id.clone(), p.sent_at) else {
            continue;
        };
        let open = matches!(p.status.as_deref(), Some("SENT") | Some("PARTIALLY_PAID") | Some("UNPAID"));
        let since = p.last_reminder.unwrap_or(sent_at);
        if open && now - since >= days * 86400 {
            pp.remind(token, &id, REMIND_NOTE)?;
            p.last_reminder = Some(now);
            reminded.push(id);
        }
    }
    Ok(reminded)
}

pub fn rollup(items: &[InvoiceSummary], month: Option<&str>) -> Vec<RollupRow> {
    // email -> (count, paid, open)
    let mut by_email: BTreeMap<&str, (usize, Cents, Cents)> = BTreeMap::new();
    for it in items {
        if let Some(m) = month {
            if !it.invoice_date.starts_with(m) {
                continue;
            }
        }
        let row = by_email.entry(it.email.as_str()).or_insert((0, 0, 0));
        row.0 += 1;
        if PAID_STATUSES.contains(&it.status.as_str()) {
            row.1 += it.total;
        } else if !VOID_STATUSES.contains(&it.status.as_str()) {
            row.2 += it.total;
        }
    }
    by_email
        .into_iter()
        .map(|(email, (count, paid, open))| RollupRow {
            email: email.to_string(),
            count,
            paid,
            open,
        })
        .collect()
}
