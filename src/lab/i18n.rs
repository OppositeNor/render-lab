//! Language of the on-screen text. The recorder takes it from the timeline
//! (`"locale": "zh-CN"`), the viewer from the timeline it plays. Identifiers, field
//! names, formulas, and technical terms stay in English in every locale;
//! only the prose around them is translated.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

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

/// The English or the Chinese text, by the current locale.
pub fn t(en: &'static str, zh: &'static str) -> &'static str {
    if self::zh() { zh } else { en }
}

const OBJECTS: [(&str, &str); 10] = [
    ("floor", "地板"),
    ("back wall", "后墙"),
    ("left wall", "左墙"),
    ("right wall", "右墙"),
    ("emissive panel", "发光面板"),
    ("sphere", "球体"),
    ("rotated box", "旋转的盒子"),
    ("small box", "小盒子"),
    ("teapot", "茶壶"),
    ("none", "无"),
];

/// A scene object's display name, or a card name that starts with one
/// (`rotated box +X` → `旋转的盒子 +X`).
pub fn object_name(name: &str) -> String {
    if !zh() {
        return name.to_string();
    }
    for (en, zh) in OBJECTS {
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

    #[test]
    fn object_names() {
        set(Locale::ZhCn);
        assert_eq!(object_name("rotated box +X"), "旋转的盒子 +X");
        assert_eq!(object_name("small box"), "小盒子");
        assert_eq!(object_name("floor"), "地板");
        set(Locale::En);
        assert_eq!(object_name("floor"), "floor");
    }
}
