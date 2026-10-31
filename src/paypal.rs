use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::http::{base64, Request, Response, Transport};
use crate::model::{dollars, Cents, InvoiceSummary, PersonTotal, Result, RollupRow, Split, PAID_STATUSES};

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

/// For each person != me with invoice_id None: create, send, set invoice_id, status SENT, sent_at now.
pub fn invoice_all(pp: &PayPal, token: &str, split: &mut Split, note: &str, now: i64) -> Result<()> {
    for i in 0..split.people.len() {
        if split.people[i].initials == split.me || split.people[i].invoice_id.is_some() {
            continue;
        }
        let id = pp.create_invoice(token, &split.people[i], note)?;
        pp.send_invoice(token, &id)?;
        let p = &mut split.people[i];
        p.invoice_id = Some(id);
        p.status = Some("SENT".into());
        p.sent_at = Some(now);
    }
    Ok(())
}

pub fn refresh(pp: &PayPal, token: &str, split: &mut Split) -> Result<()> {
    for p in split.people.iter_mut() {
        if let Some(id) = p.invoice_id.clone() {
            p.status = Some(pp.get_status(token, &id)?);
        }
    }
    Ok(())
}

/// Refresh, then remind unpaid invoices with now - sent_at >= days*86400; returns reminded ids.
pub fn remind_due(pp: &PayPal, token: &str, split: &mut Split, now: i64, days: i64) -> Result<Vec<String>> {
    refresh(pp, token, split)?;
    let mut reminded = Vec::new();
    for p in split.people.iter() {
        let (Some(id), Some(sent_at)) = (p.invoice_id.as_ref(), p.sent_at) else {
            continue;
        };
        let unpaid = !PAID_STATUSES.contains(&p.status.as_deref().unwrap_or(""));
        if unpaid && now - sent_at >= days * 86400 {
            pp.remind(token, id, REMIND_NOTE)?;
            reminded.push(id.clone());
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
        } else {
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
