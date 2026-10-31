use serde_json::Value;

use crate::http::Transport;
use crate::model::{Cents, InvoiceSummary, PersonTotal, Result, RollupRow, Split};

/// REST Invoicing v2. Sandbox base: https://api-m.sandbox.paypal.com
pub struct PayPal<'a> {
    pub base: String,
    pub client_id: String,
    pub secret: String,
    pub currency: String,
    pub http: &'a dyn Transport,
}

impl<'a> PayPal<'a> {
    pub fn token(&self) -> Result<String> {
        todo!("U4 impl: token")
    }

    pub fn invoice_body(&self, p: &PersonTotal, note: &str) -> Value {
        let _ = (p, note);
        todo!("U4 impl: invoice_body")
    }

    /// POST /v2/invoicing/invoices with Prefer: return=representation; 201 -> .id
    pub fn create_invoice(&self, token: &str, p: &PersonTotal, note: &str) -> Result<String> {
        let _ = (token, p, note);
        todo!("U4 impl: create_invoice")
    }

    /// 200 or 202 ok.
    pub fn send_invoice(&self, token: &str, id: &str) -> Result<()> {
        let _ = (token, id);
        todo!("U4 impl: send_invoice")
    }

    /// 204 ok.
    pub fn remind(&self, token: &str, id: &str, note: &str) -> Result<()> {
        let _ = (token, id, note);
        todo!("U4 impl: remind")
    }

    pub fn get_status(&self, token: &str, id: &str) -> Result<String> {
        let _ = (token, id);
        todo!("U4 impl: get_status")
    }

    /// POST .../payments, method CASH; 200 ok.
    pub fn record_payment(&self, token: &str, id: &str, amount: Cents) -> Result<()> {
        let _ = (token, id, amount);
        todo!("U4 impl: record_payment")
    }

    pub fn list_invoices(&self, token: &str) -> Result<Vec<InvoiceSummary>> {
        let _ = token;
        todo!("U4 impl: list_invoices")
    }
}

/// For each person != me with invoice_id None: create, send, set invoice_id, status SENT, sent_at now.
pub fn invoice_all(pp: &PayPal, token: &str, split: &mut Split, note: &str, now: i64) -> Result<()> {
    let _ = (pp, token, split, note, now);
    todo!("U4 impl: invoice_all")
}

pub fn refresh(pp: &PayPal, token: &str, split: &mut Split) -> Result<()> {
    let _ = (pp, token, split);
    todo!("U4 impl: refresh")
}

/// Refresh, then remind unpaid invoices with now - sent_at >= days*86400; returns reminded ids.
pub fn remind_due(pp: &PayPal, token: &str, split: &mut Split, now: i64, days: i64) -> Result<Vec<String>> {
    let _ = (pp, token, split, now, days);
    todo!("U4 impl: remind_due")
}

pub fn rollup(items: &[InvoiceSummary], month: Option<&str>) -> Vec<RollupRow> {
    let _ = (items, month);
    todo!("U4 impl: rollup")
}
