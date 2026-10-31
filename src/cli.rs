use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{from_env, load_dotenv, require, Config};
use crate::http::{Transport, Ureq};
use crate::model::{dollars, Cents, Result};
use crate::paypal::{self, PayPal};
use crate::{receipt, split, vision};

/// Options that take a value; their value is never read as a positional argument.
const VALUE_OPTS: &[&str] = &["--people", "--out", "--days", "--month"];

/// Exit codes: 0 ok, 1 usage, 2 error (printed as "error: ..." on stderr).
pub fn run(args: Vec<String>) -> i32 {
    load_dotenv(Path::new(".env"));
    let Some((cmd, rest)) = args.split_first() else {
        print!("{}", usage());
        return 1;
    };
    let result = match cmd.as_str() {
        "vision" => cmd_vision(rest),
        "scan" => cmd_scan(rest),
        "invoice" => cmd_invoice(rest),
        "status" => cmd_status(rest),
        "remind" => cmd_remind(rest),
        "mark-paid" => cmd_mark_paid(rest),
        "rollup" => cmd_rollup(rest),
        "help" | "-h" | "--help" => {
            print!("{}", usage());
            return 0;
        }
        other => {
            eprintln!("error: unknown command '{other}'");
            eprint!("{}", usage());
            return 1;
        }
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

pub fn usage() -> String {
    "\
splitslip: split a warehouse-club receipt to the cent and invoice each person through PayPal

usage:
  splitslip vision <image>                         read the receipt with Gemini, print Vision JSON (needs GEMINI_API_KEY)
  splitslip scan <image | vision.json> [--people people.json] [--out out/<stem>.json]
                                                   split the receipt and save the split (a .json input needs no key)
  splitslip invoice <split.json> [--yes]           create and send one PayPal invoice per person (asks first)
  splitslip status <split.json>                    refresh invoice statuses from PayPal
  splitslip remind <split.json> [--days 3]         remind people whose invoice is unpaid and older than --days
  splitslip mark-paid <split.json> <initials>      record a cash payment on that person's invoice
  splitslip rollup [--month YYYY-MM]               totals per person across all invoices

keys come from .env in the current folder (see .env.example)
"
    .to_string()
}

/// Arguments that are not options or option values.
fn positionals(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip_value = false;
    for a in args {
        if skip_value {
            skip_value = false;
        } else if VALUE_OPTS.contains(&a.as_str()) {
            skip_value = true;
        } else if !a.starts_with("--") {
            out.push(a.clone());
        }
    }
    out
}

fn opt(args: &[String], name: &str) -> Result<Option<String>> {
    match args.iter().position(|a| a == name) {
        None => Ok(None),
        Some(i) => args
            .get(i + 1)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{name} needs a value")),
    }
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn one_positional(args: &[String], what: &str) -> Result<PathBuf> {
    positionals(args)
        .into_iter()
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing {what}; see `splitslip help`"))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn http() -> Ureq {
    Ureq {
        timeout: Duration::from_secs(90),
    }
}

/// PayPal client; checks both keys before any network call.
fn paypal_client<'a>(cfg: &Config, http: &'a dyn Transport) -> Result<PayPal<'a>> {
    Ok(PayPal {
        base: cfg.paypal_base.clone(),
        client_id: require(&cfg.paypal_client_id, "PAYPAL_CLIENT_ID")?,
        secret: require(&cfg.paypal_secret, "PAYPAL_CLIENT_SECRET")?,
        currency: cfg.currency.clone(),
        http,
    })
}

fn stem_of(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "receipt".to_string());
    stem.strip_suffix(".vision").unwrap_or(&stem).to_string()
}

fn cmd_vision(args: &[String]) -> Result<()> {
    let image = one_positional(args, "image path")?;
    let cfg = from_env();
    let key = require(&cfg.gemini_key, "GEMINI_API_KEY")?;
    let v = vision::read(&image, &key, &http(), Duration::from_secs(30))?;
    let json = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    println!("{json}");
    Ok(())
}

fn cmd_scan(args: &[String]) -> Result<()> {
    let input = one_positional(args, "image or vision.json path")?;
    let people_path = opt(args, "--people")?.unwrap_or_else(|| "people.json".to_string());
    let stem = stem_of(&input);
    let out = match opt(args, "--out")? {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from("out").join(format!("{stem}.json")),
    };

    let v = if input.to_string_lossy().to_ascii_lowercase().ends_with(".json") {
        vision::load_vision_json(&input)?
    } else {
        let cfg = from_env();
        let key = require(&cfg.gemini_key, "GEMINI_API_KEY")?;
        vision::read(&input, &key, &http(), Duration::from_secs(30))?
    };

    let r = receipt::from_vision(&v)?;
    let people = split::load_people(Path::new(&people_path))?;
    let s = split::split(&r, &people, &stem)?;
    println!("{}", split::render_items(&r));
    println!();
    println!("{}", split::render_people(&s));
    split::save(&s, &out)?;
    println!();
    println!("saved {}", out.display());
    Ok(())
}

fn cmd_invoice(args: &[String]) -> Result<()> {
    let path = one_positional(args, "split.json path")?;
    let mut s = split::load(&path)?;
    let cfg = from_env();
    let client = http();
    let pp = paypal_client(&cfg, &client)?;

    let pending: Vec<_> = s
        .people
        .iter()
        .filter(|p| p.initials != s.me && p.invoice_id.is_none())
        .collect();
    if pending.is_empty() {
        println!("Nothing to send: every invoice already exists.");
        return Ok(());
    }
    let total: Cents = pending.iter().map(|p| p.total).sum();
    println!("{}", split::render_people(&s));
    println!();
    if !has_flag(args, "--yes") {
        print!(
            "Send {} invoices totalling {}? [y/N] ",
            pending.len(),
            dollars(total)
        );
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        if !matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("Not sent.");
            return Ok(());
        }
    }

    let token = pp.token()?;
    let note = format!("Your share of the warehouse run \u{2014} {}", s.source);
    let res = paypal::invoice_all(&pp, &token, &mut s, &note, now());
    // Save first: ids already created must survive a failure partway through.
    split::save(&s, &path)?;
    res?;
    println!("{}", split::render_people(&s));
    Ok(())
}

fn cmd_status(args: &[String]) -> Result<()> {
    let path = one_positional(args, "split.json path")?;
    let mut s = split::load(&path)?;
    let cfg = from_env();
    let client = http();
    let pp = paypal_client(&cfg, &client)?;
    let token = pp.token()?;
    let res = paypal::refresh(&pp, &token, &mut s);
    split::save(&s, &path)?;
    res?;
    println!("{}", split::render_people(&s));
    Ok(())
}

fn cmd_remind(args: &[String]) -> Result<()> {
    let path = one_positional(args, "split.json path")?;
    let days: i64 = match opt(args, "--days")? {
        Some(d) => d
            .parse()
            .map_err(|_| format!("--days expects a whole number, got '{d}'"))?,
        None => 3,
    };
    let mut s = split::load(&path)?;
    let cfg = from_env();
    let client = http();
    let pp = paypal_client(&cfg, &client)?;
    let token = pp.token()?;
    let res = paypal::remind_due(&pp, &token, &mut s, now(), days);
    split::save(&s, &path)?;
    let reminded = res?;
    if reminded.is_empty() {
        println!("No reminders due.");
    } else {
        println!("Reminded: {}", reminded.join(", "));
    }
    println!("{}", split::render_people(&s));
    Ok(())
}

fn cmd_mark_paid(args: &[String]) -> Result<()> {
    let pos = positionals(args);
    let path = pos
        .first()
        .map(PathBuf::from)
        .ok_or("missing split.json path; see `splitslip help`")?;
    let initials = pos
        .get(1)
        .ok_or("missing initials; usage: mark-paid <split.json> <initials>")?
        .to_uppercase();
    let mut s = split::load(&path)?;
    let person = s
        .people
        .iter()
        .find(|p| p.initials == initials)
        .ok_or_else(|| format!("no person '{initials}' in {}", path.display()))?;
    let id = person
        .invoice_id
        .clone()
        .ok_or_else(|| format!("{initials} has no invoice yet; run `invoice` first"))?;
    let amount = person.total;

    let cfg = from_env();
    let client = http();
    let pp = paypal_client(&cfg, &client)?;
    let token = pp.token()?;
    pp.record_payment(&token, &id, amount)?;
    let res = paypal::refresh(&pp, &token, &mut s);
    split::save(&s, &path)?;
    res?;
    println!("Recorded {} paid by {initials}.", dollars(amount));
    println!("{}", split::render_people(&s));
    Ok(())
}

fn cmd_rollup(args: &[String]) -> Result<()> {
    let month = opt(args, "--month")?;
    if let Some(m) = &month {
        let ok = m.len() == 7 && m.as_bytes()[4] == b'-';
        if !ok {
            return Err(format!("--month expects YYYY-MM, got '{m}'"));
        }
    }
    let cfg = from_env();
    let client = http();
    let pp = paypal_client(&cfg, &client)?;
    let token = pp.token()?;
    let items = pp.list_invoices(&token)?;
    let rows = paypal::rollup(&items, month.as_deref());
    if rows.is_empty() {
        println!("No invoices.");
        return Ok(());
    }
    println!("{:<30} {:>5} {:>10} {:>10}", "email", "count", "paid", "open");
    let (mut count, mut paid, mut open) = (0usize, 0i64, 0i64);
    for r in &rows {
        println!(
            "{:<30} {:>5} {:>10} {:>10}",
            r.email,
            r.count,
            dollars(r.paid),
            dollars(r.open)
        );
        count += r.count;
        paid += r.paid;
        open += r.open;
    }
    println!(
        "{:<30} {:>5} {:>10} {:>10}",
        "Total",
        count,
        dollars(paid),
        dollars(open)
    );
    Ok(())
}
