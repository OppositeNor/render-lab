//! Language of the on-screen text. The recorder takes it from the timeline
//! (`"locale": "zh-CN"`), the viewer from the timeline it plays. Identifiers, field
//! names, formulas, and technical terms stay in English in every locale;
//! only the prose around them is translated.
//!
//! Text is named by key. Each layer keeps its text in its package's
//! `text/en.json` and `text/zh-CN.json` (the lab's in packages/render_lab/text/)
//! and reads it through a [`Table`]: `tr("viewer.playing")`, or
//! `tr_with("ui.skip_back", args!(seconds = 10))` for text with values.

use crate::resources;
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

impl Locale {
    pub const ALL: [Locale; 2] = [Locale::En, Locale::ZhCn];

    /// Its name in timelines and the settings file (`en`, `zh-CN`).
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::ZhCn => "zh-CN",
        }
    }

    pub fn from_code(code: &str) -> Option<Locale> {
        Locale::ALL.into_iter().find(|l| l.code() == code)
    }

    /// Its name in itself, as a language picker shows it in every locale.
    pub fn name(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::ZhCn => "中文",
        }
    }
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
/// (`text/en.json` and `text/zh-CN.json` of the layer's package, the
/// resources `<prefix>/en` and `<prefix>/zh-cn`, `<prefix>` being
/// `<package>.text`), read on
/// first use. Text may name values in braces (`{seconds}`), which
/// [`Table::format`] fills; `{{` and `}}` are literal braces. build.rs checks
/// that every locale has the same keys and values, and that every key the
/// layer's code names exists.
pub struct Table {
    /// The resources' id without the locale (`o3de/lighting/text`).
    prefix: &'static str,
    texts: OnceLock<[HashMap<String, String>; 2]>,
}

impl Table {
    /// The texts of `Locale::En` and `Locale::ZhCn`: `<prefix>/en` and `<prefix>/zh-cn`.
    pub const fn new(prefix: &'static str) -> Table {
        Table { prefix, texts: OnceLock::new() }
    }

    /// The text of `key` in the current locale.
    pub fn get(&'static self, key: &str) -> &'static str {
        let texts = self.texts.get_or_init(|| {
            ["en", "zh-cn"].map(|locale| {
                let id = format!("{}/{locale}", self.prefix);
                serde_json::from_str(resources::get(&id).text()).unwrap_or_else(|e| panic!("{id}: a text table is a JSON object of strings: {e}"))
            })
        });
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
        } else if tail.starts_with("{@") {
            // A tag (src/rich_text.rs) is kept as it is.
            let (_, len) = crate::rich_text::parse_tag(tail, 2).unwrap_or_else(|e| panic!("{e} in {text:?}"));
            out.push_str(&tail[..len]);
            rest = &tail[len..];
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
#[macro_export]
macro_rules! args {
    ($($name:ident = $value:expr),+ $(,)?) => {
        &[$((stringify!($name), &$value as &dyn std::fmt::Display)),+]
    };
}
pub use crate::args;

/// The shared lab's text.
static TEXT: Table = Table::new("render_lab.text");

/// The lab's text of `key` in the current locale.
pub fn tr(key: &'static str) -> &'static str {
    TEXT.get(key)
}

/// The lab's text of a key its data names (a `-text` field: build.rs checks
/// those keys as it checks `tr`'s).
pub fn tr_key(key: &'static str) -> &'static str {
    TEXT.get(key)
}

/// The text of `key` in `package`'s table (`render_lab` is the lab's): what a
/// value's declaration names (src/values.rs). The package's build checks its
/// keys.
pub fn package_text(package: &str, key: &str) -> &'static str {
    if package == "render_lab" {
        return TEXT.get(key);
    }
    static TABLES: std::sync::Mutex<Vec<&'static Table>> = std::sync::Mutex::new(Vec::new());
    let prefix = format!("{package}.text");
    let table = {
        let mut tables = TABLES.lock().unwrap();
        match tables.iter().find(|t| t.prefix == prefix) {
            Some(t) => *t,
            None => {
                let t: &'static Table = Box::leak(Box::new(Table::new(Box::leak(prefix.into_boxed_str()))));
                tables.push(t);
                t
            }
        }
    };
    table.get(key)
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
    fn placeholders() {
        assert_eq!(fill("back {seconds} s", args!(seconds = 10)), "back 10 s");
        assert_eq!(fill("{{x}} = {x}", args!(x = 1.5)), "{x} = 1.5");
        assert_eq!(fill("后退 {seconds} 秒", args!(seconds = 20)), "后退 20 秒");
    }
}
