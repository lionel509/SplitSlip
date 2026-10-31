use crate::model::{Cents, Receipt, Result, Vision};

/// "7.99"->799, "-3.00"->-300, "3.00-"->-300, "$1,234.56"->123456, "abc"->Err
pub fn parse_cents(s: &str) -> Result<Cents> {
    let _ = s;
    todo!("U2 impl: parse_cents")
}

/// Items in order; coupon attaches to the LAST earlier item with the same item_no.
pub fn build(v: &Vision) -> Result<Receipt> {
    let _ = v;
    todo!("U2 impl: build")
}

/// Spread r.tax over taxable items (largest remainder, ties to earlier item).
pub fn allocate_tax(r: &mut Receipt) -> Result<()> {
    let _ = r;
    todo!("U2 impl: allocate_tax")
}

/// Err names both sides in dollars, e.g. "total 48.16 but lines sum to 48.15".
pub fn reconcile(r: &Receipt) -> Result<()> {
    let _ = r;
    todo!("U2 impl: reconcile")
}

pub fn from_vision(v: &Vision) -> Result<Receipt> {
    let _ = v;
    todo!("U2 impl: from_vision")
}
