use crate::model::{dollars, Cents, Item, Receipt, Result, Vision, VisionLine, TAXABLE_FLAGS};

/// "7.99"->799, "-3.00"->-300, "3.00-"->-300, "$1,234.56"->123456, "abc"->Err
pub fn parse_cents(s: &str) -> Result<Cents> {
    let cleaned: String = s.trim().chars().filter(|c| *c != '$' && *c != ',').collect();
    let (negative, body) = if let Some(rest) = cleaned.strip_suffix('-') {
        (true, rest)
    } else if let Some(rest) = cleaned.strip_prefix('-') {
        (true, rest)
    } else {
        (false, cleaned.as_str())
    };
    let (whole, frac) = match body.split_once('.') {
        Some((w, f)) => (w, f),
        None => (body, ""),
    };
    let bad = || format!("not a price: {s:?}");
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    if frac.len() > 2 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let dollars_part: i64 = whole.parse().map_err(|_| bad())?;
    let cents_part: i64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>().map_err(|_| bad())? * 10,
        _ => frac.parse().map_err(|_| bad())?,
    };
    let value = dollars_part * 100 + cents_part;
    Ok(if negative { -value } else { value })
}

fn price_of(line: &VisionLine, what: &str) -> Result<Cents> {
    match &line.price {
        Some(p) => parse_cents(p).map_err(|e| format!("{what}: {e}")),
        None => Err(format!("{what} line has no price")),
    }
}

fn normalize_written(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let up = n.trim().to_uppercase();
        if !up.is_empty() && !out.contains(&up) {
            out.push(up);
        }
    }
    out
}

/// Items in order; coupon attaches to the LAST earlier item with the same item_no.
pub fn build(v: &Vision) -> Result<Receipt> {
    let mut items: Vec<Item> = Vec::new();
    let mut subtotal: Option<Cents> = None;
    let mut tax: Option<Cents> = None;
    let mut total: Option<Cents> = None;

    for line in &v.lines {
        match line.kind.as_str() {
            "item" => {
                let name = line
                    .expanded
                    .clone()
                    .or_else(|| line.text.clone())
                    .ok_or_else(|| "item line has no name".to_string())?;
                let price = price_of(line, &format!("item {name}"))?;
                let taxable = line
                    .tax_flag
                    .as_deref()
                    .map(|f| TAXABLE_FLAGS.contains(&f.trim().to_uppercase().as_str()))
                    .unwrap_or(false);
                items.push(Item {
                    item_no: line.item_no.clone(),
                    name,
                    price,
                    coupon: 0,
                    taxable,
                    written: normalize_written(&line.written),
                    tax: 0,
                });
            }
            "coupon" => {
                let item_no = line
                    .item_no
                    .clone()
                    .ok_or_else(|| "coupon line has no item number".to_string())?;
                // Coupons always reduce the price, whatever sign the slip shows.
                let amount = -price_of(line, &format!("coupon {item_no}"))?.abs();
                let idx = items
                    .iter()
                    .rposition(|it| it.item_no.as_deref() == Some(item_no.as_str()))
                    .ok_or_else(|| format!("coupon for item {item_no} has no matching item"))?;
                items[idx].coupon += amount;
            }
            "subtotal" => subtotal = Some(price_of(line, "subtotal")?),
            // A tax line without a price counts as zero tax.
            "tax" => {
                if line.price.is_some() {
                    tax = Some(price_of(line, "tax")?);
                }
            }
            "total" => total = Some(price_of(line, "total")?),
            _ => {}
        }
    }

    let subtotal = subtotal.ok_or_else(|| "receipt has no subtotal line".to_string())?;
    let total = total.ok_or_else(|| "receipt has no total line".to_string())?;
    Ok(Receipt {
        items,
        subtotal,
        tax: tax.unwrap_or(0),
        total,
    })
}

/// Spread r.tax over taxable items (largest remainder, ties to earlier item).
pub fn allocate_tax(r: &mut Receipt) -> Result<()> {
    for it in r.items.iter_mut() {
        it.tax = 0;
    }
    if r.tax == 0 {
        return Ok(());
    }
    let taxable: Vec<usize> = r
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| it.taxable)
        .map(|(i, _)| i)
        .collect();
    if taxable.is_empty() {
        return Err(format!("tax {} but no item is flagged taxable", dollars(r.tax)));
    }
    let weights: Vec<i128> = taxable
        .iter()
        .map(|&i| (r.items[i].price + r.items[i].coupon) as i128)
        .collect();
    let base: i128 = weights.iter().sum();
    if base <= 0 {
        return Err("taxable items net to zero, cannot spread tax".to_string());
    }

    let sign: i128 = if r.tax < 0 { -1 } else { 1 };
    let amount = (r.tax as i128).abs();
    let mut alloc: Vec<i128> = Vec::with_capacity(weights.len());
    let mut rems: Vec<(i128, usize)> = Vec::with_capacity(weights.len());
    for (k, w) in weights.iter().enumerate() {
        alloc.push(amount * w / base);
        rems.push((amount * w % base, k));
    }
    let leftover = amount - alloc.iter().sum::<i128>();
    rems.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, k) in rems.iter().take(leftover as usize) {
        alloc[*k] += 1;
    }
    for (k, &i) in taxable.iter().enumerate() {
        r.items[i].tax = (sign * alloc[k]) as Cents;
    }
    Ok(())
}

/// Err names both sides in dollars, e.g. "total 48.16 but lines sum to 48.15".
pub fn reconcile(r: &Receipt) -> Result<()> {
    let items_sum: Cents = r.items.iter().map(|it| it.price + it.coupon).sum();
    if items_sum != r.subtotal {
        return Err(format!(
            "subtotal {} but items sum to {}",
            dollars(r.subtotal),
            dollars(items_sum)
        ));
    }
    let lines_sum = r.subtotal + r.tax;
    if lines_sum != r.total {
        return Err(format!(
            "total {} but lines sum to {}",
            dollars(r.total),
            dollars(lines_sum)
        ));
    }
    let tax_sum: Cents = r.items.iter().map(|it| it.tax).sum();
    if tax_sum != r.tax {
        return Err(format!(
            "tax {} but item tax sums to {}",
            dollars(r.tax),
            dollars(tax_sum)
        ));
    }
    Ok(())
}

pub fn from_vision(v: &Vision) -> Result<Receipt> {
    let mut r = build(v)?;
    allocate_tax(&mut r)?;
    reconcile(&r)?;
    Ok(r)
}
