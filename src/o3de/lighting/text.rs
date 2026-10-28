//! The investigation's on-screen text, by key, in `text/en.json` and
//! `text/zh-CN.json` (see `crate::lab::i18n::Table`).

use crate::lab::i18n::Table;

static TEXT: Table = Table::new("o3de/lighting/text");

/// The text of `key` in the current locale.
pub fn tr(key: &'static str) -> &'static str {
    TEXT.get(key)
}

/// The text of a key its data names (a `-text` field).
pub fn tr_key(key: &'static str) -> &'static str {
    TEXT.get(key)
}
