//! The investigation's on-screen text, by key, in `text/en.json` and
//! `text/zh-CN.json` (see `crate::lab::i18n::Table`).

use crate::lab::i18n::Table;

static TEXT: Table = Table::new(include_str!("text/en.json"), include_str!("text/zh-CN.json"));

/// The text of `key` in the current locale.
pub fn tr(key: &'static str) -> &'static str {
    TEXT.get(key)
}
