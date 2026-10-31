use std::fs;
use std::path::{Path, PathBuf};

use splitslip::http::{base64, Fake, Request, Response};
use splitslip::model::{InvoiceSummary, PersonTotal, Split};
use splitslip::paypal::{invoice_all, rollup, remind_due, PayPal};
use splitslip::receipt::{from_vision, parse_cents};
use splitslip::split::{load_people, split};
use splitslip::vision::load_vision_json;

const BASE: &str = "https://api-m.sandbox.paypal.com";

fn fx(name: &str) -> String {
    let p: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name);
    fs::read_to_string(p).unwrap()
}

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn pp<'a>(http: &'a Fake) -> PayPal<'a> {
    PayPal {
        base: BASE.into(),
        client_id: "id".into(),
        secret: "secret".into(),
        currency: "USD".into(),
        http,
    }
}

fn weekend_split() -> Split {
    let people = load_people(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/people.json")).unwrap();
    let v = load_vision_json(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/weekend.vision.json")).unwrap();
    let r = from_vision(&v).unwrap();
    split(&r, &people, "weekend").unwrap()
}

fn person<'a>(s: &'a Split, who: &str) -> &'a PersonTotal {
    s.people.iter().find(|p| p.initials == who).unwrap()
}

#[test]
fn token_uses_basic_auth() {
    let fake = Fake::new(vec![Response { status: 200, body: fx("paypal/token.json") }]);
    let tok = pp(&fake).token().unwrap();
    assert_eq!(tok, "A21AAtest");
    let calls = fake.calls.borrow();
    assert!(calls[0].url.ends_with("/v1/oauth2/token"), "{}", calls[0].url);
    assert_eq!(
        header(&calls[0], "Authorization"),
        Some(format!("Basic {}", base64(b"id:secret")).as_str())
    );
    assert!(calls[0].body.as_deref().unwrap().contains("grant_type=client_credentials"));
}

#[test]
fn invoice_body_shape() {
    let fake = Fake::new(vec![]);
    let s = weekend_split();
    let k = person(&s, "K");
    let body = pp(&fake).invoice_body(k, "note");
    assert_eq!(body["detail"]["currency_code"], "USD");
    assert_eq!(body["detail"]["payment_term"]["term_type"], "DUE_ON_RECEIPT");
    assert_eq!(
        body["primary_recipients"][0]["billing_info"]["email_address"],
        "kai@example.com"
    );
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty());
    let mut sum = 0;
    for it in items {
        assert_eq!(it["quantity"], "1");
        assert_eq!(it["unit_of_measure"], "AMOUNT");
        sum += parse_cents(it["unit_amount"]["value"].as_str().unwrap()).unwrap();
    }
    assert_eq!(sum, 1251);
}

#[test]
fn create_returns_id_with_prefer_header() {
    let fake = Fake::new(vec![Response { status: 201, body: fx("paypal/create.json") }]);
    let s = weekend_split();
    let id = pp(&fake).create_invoice("TOK", person(&s, "K"), "note").unwrap();
    assert_eq!(id, "INV2-TEST-0001");
    let calls = fake.calls.borrow();
    assert!(calls[0].url.ends_with("/v2/invoicing/invoices"), "{}", calls[0].url);
    assert_eq!(header(&calls[0], "Prefer"), Some("return=representation"));
    assert_eq!(header(&calls[0], "Authorization"), Some("Bearer TOK"));
}

#[test]
fn send_accepts_200_and_202() {
    let fake = Fake::new(vec![
        Response { status: 200, body: String::new() },
        Response { status: 202, body: String::new() },
    ]);
    let p = pp(&fake);
    p.send_invoice("TOK", "INV-1").unwrap();
    p.send_invoice("TOK", "INV-2").unwrap();
}

#[test]
fn send_errors_on_422_with_body() {
    let fake = Fake::new(vec![Response {
        status: 422,
        body: r#"{"name":"VALIDATION_ERROR"}"#.into(),
    }]);
    let err = pp(&fake).send_invoice("TOK", "INV-1").unwrap_err();
    assert!(err.contains("422"), "{err}");
    assert!(err.contains("VALIDATION_ERROR"), "{err}");
}

#[test]
fn remind_204_ok() {
    let fake = Fake::new(vec![Response { status: 204, body: String::new() }]);
    pp(&fake).remind("TOK", "INV-1", "friendly nudge").unwrap();
    let calls = fake.calls.borrow();
    assert!(calls[0].url.ends_with("/INV-1/remind"), "{}", calls[0].url);
    assert!(calls[0].body.as_deref().unwrap().contains("send_to_invoicer"));
}

#[test]
fn get_status_reads_status() {
    let fake = Fake::new(vec![Response { status: 200, body: fx("paypal/get-sent.json") }]);
    assert_eq!(pp(&fake).get_status("TOK", "INV-1").unwrap(), "SENT");
}

#[test]
fn record_payment_body() {
    let fake = Fake::new(vec![Response { status: 200, body: String::new() }]);
    pp(&fake).record_payment("TOK", "INV-1", 1251).unwrap();
    let calls = fake.calls.borrow();
    assert!(calls[0].url.ends_with("/INV-1/payments"), "{}", calls[0].url);
    let body: serde_json::Value = serde_json::from_str(calls[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(body["method"], "CASH");
    assert_eq!(body["amount"]["value"], "12.51");
    assert_eq!(body["amount"]["currency_code"], "USD");
}

#[test]
fn list_parses_items() {
    let fake = Fake::new(vec![Response { status: 200, body: fx("paypal/list.json") }]);
    let items = pp(&fake).list_invoices("TOK").unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].id, "INV2-A");
    assert_eq!(items[0].status, "PAID");
    assert_eq!(items[0].total, 2413);
    assert_eq!(items[0].email, "kai@example.com");
    assert_eq!(items[0].invoice_date, "2026-11-07");
}

#[test]
fn invoice_all_skips_me_and_records_ids() {
    let fake = Fake::new(vec![
        Response { status: 201, body: fx("paypal/create.json") },
        Response { status: 200, body: String::new() },
        Response { status: 201, body: fx("paypal/create.json") },
        Response { status: 202, body: String::new() },
    ]);
    let mut s = weekend_split();
    invoice_all(&pp(&fake), "TOK", &mut s, "note", 1_000_000).unwrap();
    assert_eq!(fake.calls.borrow().len(), 4);
    let k = person(&s, "K");
    assert_eq!(k.invoice_id.as_deref(), Some("INV2-TEST-0001"));
    assert_eq!(k.status.as_deref(), Some("SENT"));
    assert_eq!(k.sent_at, Some(1_000_000));
    let m = person(&s, "M");
    assert_eq!(m.invoice_id.as_deref(), Some("INV2-TEST-0001"));
    assert_eq!(person(&s, "L").invoice_id, None);
}

#[test]
fn remind_due_only_unpaid_and_old() {
    let fake = Fake::new(vec![
        Response { status: 200, body: fx("paypal/get-sent.json") },
        Response { status: 200, body: fx("paypal/get-sent.json") },
        Response { status: 204, body: String::new() },
    ]);
    let now = 10_000_000i64;
    let mut s = weekend_split();
    for p in s.people.iter_mut() {
        match p.initials.as_str() {
            "K" => {
                p.invoice_id = Some("INV-K".into());
                p.status = Some("SENT".into());
                p.sent_at = Some(now - 4 * 86400);
            }
            "M" => {
                p.invoice_id = Some("INV-M".into());
                p.status = Some("SENT".into());
                p.sent_at = Some(now - 86400);
            }
            _ => {}
        }
    }
    let reminded = remind_due(&pp(&fake), "TOK", &mut s, now, 3).unwrap();
    assert_eq!(reminded, vec!["INV-K".to_string()]);
    let calls = fake.calls.borrow();
    assert_eq!(calls.len(), 3);
    assert!(calls[2].url.ends_with("/INV-K/remind"), "{}", calls[2].url);
}

#[test]
fn rollup_groups_by_email_and_filters_month() {
    let items = vec![
        InvoiceSummary {
            id: "INV2-A".into(),
            status: "PAID".into(),
            total: 2413,
            email: "kai@example.com".into(),
            invoice_date: "2026-11-07".into(),
        },
        InvoiceSummary {
            id: "INV2-B".into(),
            status: "SENT".into(),
            total: 1151,
            email: "mo@example.com".into(),
            invoice_date: "2026-11-07".into(),
        },
        InvoiceSummary {
            id: "INV2-C".into(),
            status: "PAID".into(),
            total: 900,
            email: "kai@example.com".into(),
            invoice_date: "2026-10-02".into(),
        },
    ];
    let nov = rollup(&items, Some("2026-11"));
    assert_eq!(nov.len(), 2);
    assert_eq!(nov[0].email, "kai@example.com");
    assert_eq!((nov[0].count, nov[0].paid, nov[0].open), (1, 2413, 0));
    assert_eq!(nov[1].email, "mo@example.com");
    assert_eq!((nov[1].count, nov[1].paid, nov[1].open), (1, 0, 1151));

    let all = rollup(&items, None);
    assert_eq!(all[0].email, "kai@example.com");
    assert_eq!((all[0].count, all[0].paid, all[0].open), (2, 3313, 0));
}

#[test]
fn failed_send_keeps_the_draft_id_and_resend_creates_nothing() {
    let fake = Fake::new(vec![
        Response { status: 201, body: r#"{"id":"INV-K"}"#.into() },
        Response { status: 500, body: r#"{"name":"INTERNAL_SERVICE_ERROR"}"#.into() },
    ]);
    let mut s = weekend_split();
    assert!(invoice_all(&pp(&fake), "TOK", &mut s, "note", 1).is_err());
    let k = person(&s, "K");
    assert_eq!(k.invoice_id.as_deref(), Some("INV-K"));
    assert_eq!(k.status.as_deref(), Some("DRAFT"));
    assert_eq!(k.sent_at, None);

    // Second run: PayPal still shows the draft, so only the send is repeated.
    let fake = Fake::new(vec![
        Response { status: 200, body: r#"{"id":"INV-K","status":"DRAFT"}"#.into() },
        Response { status: 202, body: String::new() },
        Response { status: 201, body: fx("paypal/create.json") },
        Response { status: 202, body: String::new() },
    ]);
    invoice_all(&pp(&fake), "TOK", &mut s, "note", 2).unwrap();
    let calls = fake.calls.borrow();
    assert_eq!(calls[1].method, "POST");
    assert!(calls[1].url.ends_with("/INV-K/send"), "{}", calls[1].url);
    let creates = calls.iter().filter(|c| c.url.ends_with("/v2/invoicing/invoices")).count();
    assert_eq!(creates, 1, "only M needed a new invoice");
    assert_eq!(person(&s, "K").status.as_deref(), Some("SENT"));
    assert_eq!(person(&s, "K").sent_at, Some(2));
}

#[test]
fn zero_total_person_is_not_invoiced() {
    let fake = Fake::new(vec![
        Response { status: 201, body: fx("paypal/create.json") },
        Response { status: 202, body: String::new() },
        Response { status: 201, body: fx("paypal/create.json") },
        Response { status: 202, body: String::new() },
    ]);
    let mut s = weekend_split();
    let mut zed = person(&s, "K").clone();
    zed.initials = "Z".into();
    zed.name = "Zed".into();
    zed.email = "zed@example.com".into();
    zed.shares.clear();
    zed.total = 0;
    s.people.push(zed);
    invoice_all(&pp(&fake), "TOK", &mut s, "note", 1).unwrap();
    assert_eq!(fake.calls.borrow().len(), 4, "K and M only");
    assert_eq!(person(&s, "Z").invoice_id, None);
}

#[test]
fn remind_skips_cancelled_and_waits_between_reminders() {
    let fake = Fake::new(vec![
        Response { status: 200, body: r#"{"status":"CANCELLED"}"#.into() },
        Response { status: 200, body: r#"{"status":"SENT"}"#.into() },
        Response { status: 204, body: String::new() },
        Response { status: 200, body: r#"{"status":"CANCELLED"}"#.into() },
        Response { status: 200, body: r#"{"status":"SENT"}"#.into() },
    ]);
    let now = 10_000_000i64;
    let mut s = weekend_split();
    for p in s.people.iter_mut() {
        match p.initials.as_str() {
            "K" => {
                p.invoice_id = Some("INV-K".into());
                p.sent_at = Some(now - 4 * 86400);
            }
            "M" => {
                p.invoice_id = Some("INV-M".into());
                p.sent_at = Some(now - 4 * 86400);
            }
            _ => {}
        }
    }
    let first = remind_due(&pp(&fake), "TOK", &mut s, now, 3).unwrap();
    assert_eq!(first, vec!["INV-M".to_string()]);
    // A day later nothing is due: the last reminder was only a day ago.
    let second = remind_due(&pp(&fake), "TOK", &mut s, now + 86400, 3).unwrap();
    assert!(second.is_empty(), "{second:?}");
    let calls = fake.calls.borrow();
    assert_eq!(calls.len(), 5);
    assert!(calls[2].url.ends_with("/INV-M/remind"), "{}", calls[2].url);
}

#[test]
fn draft_that_paypal_already_sent_gets_sent_at_and_is_reminded() {
    let now = 10_000_000i64;
    let fake = Fake::new(vec![
        Response { status: 200, body: r#"{"status":"SENT"}"#.into() },
        // remind_due refreshes K and M, then reminds both.
        Response { status: 200, body: r#"{"status":"SENT"}"#.into() },
        Response { status: 200, body: r#"{"status":"SENT"}"#.into() },
        Response { status: 204, body: String::new() },
        Response { status: 204, body: String::new() },
    ]);
    let mut s = weekend_split();
    for p in s.people.iter_mut() {
        match p.initials.as_str() {
            "K" => {
                p.invoice_id = Some("INV-K".into());
                p.status = Some("DRAFT".into());
            }
            "M" => {
                p.invoice_id = Some("INV-M".into());
                p.status = Some("SENT".into());
                p.sent_at = Some(now);
            }
            _ => {}
        }
    }
    // The first send reached PayPal but its reply was lost: K is still a stored DRAFT.
    invoice_all(&pp(&fake), "TOK", &mut s, "note", now).unwrap();
    let k = person(&s, "K");
    assert_eq!(k.status.as_deref(), Some("SENT"));
    assert_eq!(k.sent_at, Some(now));
    assert_eq!(fake.calls.borrow().len(), 1, "one status GET, no create and no send");

    // Four days later remind_due must see K, or the draft is never chased.
    let reminded = remind_due(&pp(&fake), "TOK", &mut s, now + 4 * 86400, 3).unwrap();
    assert_eq!(reminded.len(), 2, "{reminded:?}");
    assert!(reminded.contains(&"INV-K".to_string()), "{reminded:?}");
}

#[test]
fn remind_includes_unpaid_invoices() {
    let now = 10_000_000i64;
    let fake = Fake::new(vec![
        Response { status: 200, body: r#"{"status":"UNPAID"}"#.into() },
        Response { status: 204, body: String::new() },
    ]);
    let mut s = weekend_split();
    for p in s.people.iter_mut() {
        if p.initials == "K" {
            p.invoice_id = Some("INV-K".into());
            p.sent_at = Some(now - 4 * 86400);
        }
    }
    let reminded = remind_due(&pp(&fake), "TOK", &mut s, now, 3).unwrap();
    assert_eq!(reminded, vec!["INV-K".to_string()]);
    assert!(fake.calls.borrow()[1].url.ends_with("/INV-K/remind"));
}

#[test]
fn rollup_does_not_count_cancelled_as_open() {
    let items = vec![InvoiceSummary {
        id: "INV2-X".into(),
        status: "CANCELLED".into(),
        total: 500,
        email: "mo@example.com".into(),
        invoice_date: "2026-11-07".into(),
    }];
    let rows = rollup(&items, None);
    assert_eq!((rows[0].count, rows[0].paid, rows[0].open), (1, 0, 0));
}
#[test]
fn deleted_invoice_does_not_block_other_reminders() {
    let now = 1_800_000_000;
    let mut s = weekend_split();
    for p in s.people.iter_mut() {
        match p.initials.as_str() {
            "K" => {
                p.invoice_id = Some("INV-DEL".into());
                p.status = Some("SENT".into());
                p.sent_at = Some(now - 9 * 86400);
            }
            "M" => {
                p.invoice_id = Some("INV-M".into());
                p.status = Some("SENT".into());
                p.sent_at = Some(now - 4 * 86400);
            }
            _ => {}
        }
    }
    let ids: Vec<String> = s.people.iter().filter_map(|p| p.invoice_id.clone()).collect();
    let k_first = ids[0] == "INV-DEL";
    let mut rs = vec![
        Response { status: 404, body: r#"{"name":"INVOICE_NOT_FOUND"}"#.into() },
        Response { status: 200, body: r#"{"status":"SENT"}"#.into() },
        Response { status: 204, body: String::new() },
    ];
    if !k_first {
        rs.swap(0, 1);
    }
    let fake = Fake::new(rs);
    let reminded = remind_due(&pp(&fake), "TOK", &mut s, now, 3).unwrap();
    assert_eq!(reminded, vec!["INV-M".to_string()]);
}

#[test]
fn deleted_draft_does_not_block_later_invoices() {
    let mut s = weekend_split();
    for p in s.people.iter_mut() {
        if p.initials == "K" {
            p.invoice_id = Some("INV-DEL".into());
            p.status = Some("DRAFT".into());
        }
    }
    let fake = Fake::new(vec![
        Response { status: 404, body: r#"{"name":"INVOICE_NOT_FOUND"}"#.into() },
        Response { status: 201, body: r#"{"id":"INV-NEW"}"#.into() },
        Response { status: 202, body: String::new() },
    ]);
    let res = invoice_all(&pp(&fake), "TOK", &mut s, "note", 1_800_000_000);
    assert!(res.is_err());
    assert!(s.people.iter().any(|p| p.invoice_id.as_deref() == Some("INV-NEW")));
}

#[test]
fn stored_draft_seen_sent_by_remind_gets_reminded() {
    let sent = || Response { status: 200, body: r#"{"status":"SENT"}"#.into() };
    let fake = Fake::new(vec![sent(), sent(), Response { status: 204, body: String::new() }]);
    let t0 = 10_000_000i64;
    let mut s = weekend_split();
    for p in s.people.iter_mut() {
        if p.initials == "K" {
            p.invoice_id = Some("INV-K".into());
            p.status = Some("DRAFT".into());
        }
    }
    assert!(remind_due(&pp(&fake), "TOK", &mut s, t0, 3).unwrap().is_empty());
    assert_eq!(person(&s, "K").sent_at, Some(t0));
    let r = remind_due(&pp(&fake), "TOK", &mut s, t0 + 4 * 86400, 3).unwrap();
    assert_eq!(r, vec!["INV-K".to_string()]);
}
