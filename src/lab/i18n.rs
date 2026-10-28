//! Language of the on-screen text. The recorder takes it from the timeline
//! (`"locale": "zh-CN"`), the viewer from the timeline it plays. Identifiers, field
//! names, formulas, and technical terms stay in English in every locale;
//! only the prose around them is translated.
//!
//! Text is named by key. Each layer keeps its text in `text/en.json` and
//! `text/zh-CN.json` beside its code (the lab's in `src/lab/text/`) and reads
//! it through a [`Table`]: `tr("viewer.playing")`, or
//! `tr_with("ui.skip_back", args!(seconds = 10))` for text with values.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Locale {
    #[default]
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(locale: Locale) {
    CURRENT.store(locale as u8, Ordering::Relaxed);
}

pub fn current() -> Locale {
    if CURRENT.load(Ordering::Relaxed) == Locale::ZhCn as u8 { Locale::ZhCn } else { Locale::En }
}

pub fn zh() -> bool {
    current() == Locale::ZhCn
}

/// One layer's on-screen text: a JSON object of key → text for each locale
/// (`text/en.json` and `text/zh-CN.json` beside the layer's code), read on
/// first use. Text may name values in braces (`{seconds}`), which
/// [`Table::format`] fills; `{{` and `}}` are literal braces. build.rs checks
/// that every locale has the same keys and values, and that every key the
/// layer's code names exists.
pub struct Table {
    sources: [&'static str; 2],
    texts: OnceLock<[HashMap<String, String>; 2]>,
}

impl Table {
    /// The texts of `Locale::En` and `Locale::ZhCn`.
    pub const fn new(en: &'static str, zh: &'static str) -> Table {
        Table { sources: [en, zh], texts: OnceLock::new() }
    }

    /// The text of `key` in the current locale.
    pub fn get(&'static self, key: &str) -> &'static str {
        let texts = self.texts.get_or_init(|| self.sources.map(|s| serde_json::from_str(s).expect("a text table is a JSON object of strings")));
        texts[current() as usize].get(key).unwrap_or_else(|| panic!("no text for {key:?}"))
    }

    /// The text of `key` with its values filled in (`args!`).
    pub fn format(&'static self, key: &str, args: &[(&str, &dyn Display)]) -> String {
        fill(self.get(key), args)
    }
}

/// `{name}` replaced by the value of `name`.
fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
        } else {
            let end = tail.find('}').filter(|_| tail.starts_with('{')).unwrap_or_else(|| panic!("unbalanced brace in {text:?}"));
            let name = &tail[1..end];
            let value = args.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no value for {{{name}}} in {text:?}")).1;
            out.push_str(&value.to_string());
            rest = &tail[end + 1..];
        }
    }
    out.push_str(rest);
    out
}

/// The values of a text's placeholders: `args!(seconds = 10, mip = m)`.
macro_rules! args {
    ($($name:ident = $value:expr),+ $(,)?) => {
        &[$((stringify!($name), &$value as &dyn std::fmt::Display)),+]
    };
}
pub(crate) use args;

/// The shared lab's text.
static TEXT: Table = Table::new(include_str!("text/en.json"), include_str!("text/zh-CN.json"));

/// The lab's text of `key` in the current locale.
pub fn tr(key: &'static str) -> &'static str {
    TEXT.get(key)
}

/// The lab's text of `key`, with its values (`args!`).
pub fn tr_with(key: &'static str, args: &[(&str, &dyn Display)]) -> String {
    TEXT.format(key, args)
}


/// A scene object's display name, or a card name that starts with one
/// (`rotated box +X` → `旋转的盒子 +X`).
pub fn object_name(name: &str) -> String {
    if !zh() {
        return name.to_string();
    }
    let objects = [
        ("floor", tr("object.floor")),
        ("back wall", tr("object.back_wall")),
        ("left wall", tr("object.left_wall")),
        ("right wall", tr("object.right_wall")),
        ("emissive panel", tr("object.emissive_panel")),
        ("sphere", tr("object.sphere")),
        ("rotated box", tr("object.rotated_box")),
        ("small box", tr("object.small_box")),
        ("teapot", tr("object.teapot")),
        ("terrain", tr("object.terrain")),
        ("sun panel", tr("object.sun_panel")),
        ("none", tr("object.none")),
    ];
    for (en, zh) in objects {
        if let Some(rest) = name.strip_prefix(en) {
            if rest.is_empty() || rest.starts_with(' ') {
                return format!("{zh}{rest}");
            }
        }
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The locale is global: tests that set it run one at a time.
    static LOCALE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn object_names() {
        let _locale = LOCALE.lock().unwrap();
        set(Locale::ZhCn);
        assert_eq!(object_name("rotated box +X"), "旋转的盒子 +X");
        assert_eq!(object_name("small box"), "小盒子");
        assert_eq!(object_name("floor"), "地板");
        set(Locale::En);
        assert_eq!(object_name("floor"), "floor");
    }

    #[test]
    fn tables() {
        let _locale = LOCALE.lock().unwrap();
        static TABLE: Table = Table::new(r#"{"skip": "back {seconds} s", "brace": "{{x}} = {x}"}"#, r#"{"skip": "后退 {seconds} 秒", "brace": "{{x}} = {x}"}"#);
        set(Locale::En);
        assert_eq!(TABLE.format("skip", args!(seconds = 10)), "back 10 s");
        assert_eq!(TABLE.format("brace", args!(x = 1.5)), "{x} = 1.5");
        set(Locale::ZhCn);
        assert_eq!(TABLE.get("skip"), "后退 {seconds} 秒");
        assert_eq!(TABLE.format("skip", args!(seconds = 20)), "后退 20 秒");
        set(Locale::En);
    }
}
