//! The investigation's on-screen text, by key, in `text/en.json` and
//! `text/zh-CN.json` (see `lab::i18n::Table`).

use lab::i18n::Table;

static TEXT: Table = Table::new("godot_sdfgi.text");

/// The text of `key` in the current locale.
pub fn tr(key: &'static str) -> &'static str {
    TEXT.get(key)
}

/// The text of `key`, with its values (`args!`).
pub fn tr_with(key: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    TEXT.format(key, args)
}

/// The text of a key its data names (a `-text` field).
pub fn tr_key(key: &'static str) -> &'static str {
    TEXT.get(key)
}
