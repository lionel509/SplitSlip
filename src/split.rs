use std::path::Path;

use crate::model::{People, Receipt, Result, Split};

pub fn load_people(path: &Path) -> Result<People> {
    let _ = path;
    todo!("U2 impl: load_people")
}

/// Err naming unknown initials; asserts sum of totals == receipt total.
pub fn split(r: &Receipt, people: &People, source: &str) -> Result<Split> {
    let _ = (r, people, source);
    todo!("U2 impl: split")
}

pub fn render_items(r: &Receipt) -> String {
    let _ = r;
    todo!("U2 impl: render_items")
}

pub fn render_people(s: &Split) -> String {
    let _ = s;
    todo!("U2 impl: render_people")
}

pub fn save(s: &Split, path: &Path) -> Result<()> {
    let _ = (s, path);
    todo!("U2 impl: save")
}

pub fn load(path: &Path) -> Result<Split> {
    let _ = path;
    todo!("U2 impl: load")
}
