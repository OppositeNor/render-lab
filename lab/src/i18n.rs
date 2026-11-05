//! Language of the on-screen text. The recorder takes it from the timeline
//! (`"locale": "zh-CN"`), the viewer from the timeline it plays. Identifiers, field
//! names, formulas, and technical terms stay in English in every locale;
//! only the prose around them is translated.
//!
//! Text is named by key. Each layer keeps its text in its package's
//! `text/en.ftl` and `text/zh-CN.ftl` (Project Fluent; the lab's in
//! packages/render_lab/text/) and reads it through a [`Table`]:
//! `tr("viewer-playing")`, or `tr_with("ui-skip_back", args!(seconds = 10))`
//! for a message with variables (`{ $seconds }`). A message may hold the rich
//! text's formulas and code names, `{ FORMULA("…") }` and `{ CODE("…") }`
//! (src/rich_text.rs), which it gives back as written.

use crate::resources;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// A language of the on-screen text: an entry of the locale registry,
/// render_lab's `locales.toml` (the resource `render_lab.locales`). The first
/// entry is English, the language every other falls back to last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Locale(u8);

/// A locale as the registry lists it.
struct Entry {
    code: String,
    name: String,
    /// The locales a missing key is looked up in, in order (English last, always).
    fallback: Vec<u8>,
    /// Whether its tables must have every key (`complete`), or may miss some (`draft`).
    complete: bool,
}

fn registry() -> &'static [Entry] {
    static REGISTRY: OnceLock<Vec<Entry>> = OnceLock::new();
    REGISTRY.get_or_init(|| parse_registry(resources::get("render_lab.locales").text()).unwrap_or_else(|e| panic!("render_lab.locales: {e}")))
}

/// The registry from `locales.toml`: `[[locale]]` tables of `code`, `name`,
/// `fallback` (codes; English is always last), and `status`.
fn parse_registry(text: &str) -> Result<Vec<Entry>, String> {
    let document = text.parse::<toml_edit::Document<String>>().map_err(|e| e.to_string())?;
    let tables = document.get("locale").and_then(|i| i.as_array_of_tables()).ok_or("no [[locale]]")?;
    let field = |t: &toml_edit::Table, k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_string).ok_or(format!("a locale without `{k}`"));
    let mut entries: Vec<(Entry, Vec<String>)> = Vec::new();
    for t in tables.iter() {
        let code = field(t, "code")?;
        let complete = match field(t, "status")?.as_str() {
            "complete" => true,
            "draft" => false,
            other => return Err(format!("{code}: status `{other}` is not complete or draft")),
        };
        let fallback = t.get("fallback").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
        entries.push((Entry { code, name: field(t, "name")?, fallback: Vec::new(), complete }, fallback));
    }
    if entries.first().is_none_or(|(e, _)| e.code != "en" || !e.complete) {
        return Err("the first locale is en, complete".into());
    }
    let codes: Vec<String> = entries.iter().map(|(e, _)| e.code.clone()).collect();
    let index = |c: &str| codes.iter().position(|k| k == c).map(|i| i as u8).ok_or(format!("unknown fallback `{c}`"));
    let mut out = Vec::new();
    for (mut entry, fallback) in entries {
        entry.fallback = fallback.iter().map(|c| index(c)).collect::<Result<_, _>>()?;
        if entry.code != "en" && entry.fallback.last() != Some(&0) {
            entry.fallback.push(0);
        }
        out.push(entry);
    }
    Ok(out)
}

impl Locale {
    /// English: the registry's first locale.
    pub const EN: Locale = Locale(0);

    /// Every registered locale, in the registry's order.
    pub fn all() -> Vec<Locale> {
        (0..registry().len() as u8).map(Locale).collect()
    }

    /// The locales a language picker offers: the complete ones.
    pub fn offered() -> Vec<Locale> {
        Locale::all().into_iter().filter(|l| l.entry().complete).collect()
    }

    fn entry(self) -> &'static Entry {
        &registry()[self.0 as usize]
    }

    /// Its name in timelines and the settings file (`en`, `zh-CN`).
    pub fn code(self) -> &'static str {
        &self.entry().code
    }

    pub fn from_code(code: &str) -> Option<Locale> {
        registry().iter().position(|e| e.code == code).map(|i| Locale(i as u8))
    }

    /// Its name in itself, as a language picker shows it in every locale.
    pub fn name(self) -> &'static str {
        &self.entry().name
    }

    /// The locale, then those a key missing from it is looked up in.
    fn chain(self) -> impl Iterator<Item = usize> {
        std::iter::once(self.0 as usize).chain(self.entry().fallback.iter().map(|&i| i as usize))
    }
}

impl Serialize for Locale {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for Locale {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Locale, D::Error> {
        let code = String::deserialize(d)?;
        Locale::from_code(&code).ok_or_else(|| serde::de::Error::custom(format!("unknown locale `{code}`")))
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(locale: Locale) {
    CURRENT.store(locale.0, Ordering::Relaxed);
}

pub fn current() -> Locale {
    Locale(CURRENT.load(Ordering::Relaxed))
}


/// One layer's on-screen text: a Fluent file for each registered locale
/// (`text/en.ftl`, `text/zh-CN.ftl`, … of the layer's package, the resources
/// `<prefix>/en`, `<prefix>/zh-cn`, …: the code in lower case, `<prefix>`
/// being `<package>.text`), read on first use. A key missing from a locale is
/// looked up along its fallback chain. A message may name values
/// (`{ $seconds }`), which [`Table::format`] fills. build.rs checks that every
/// complete locale has every key, that each key names the same variables in
/// every locale, and that every key the layer's code names exists.
pub struct Table {
    /// The resources' id without the locale (`o3de_lighting.text`).
    prefix: &'static str,
    /// Each locale's texts, in the registry's order; None for a draft locale without a file yet.
    texts: OnceLock<Vec<Option<Texts>>>,
}

/// One locale's messages: its bundle, and each message's text without values.
struct Texts {
    bundle: FluentBundle<FluentResource>,
    plain: HashMap<String, &'static str>,
}

impl Table {
    /// The table whose resources are `<prefix>/<locale>`.
    pub const fn new(prefix: &'static str) -> Table {
        Table { prefix, texts: OnceLock::new() }
    }

    /// The text of `key` in the current locale.
    pub fn get(&'static self, key: &str) -> &'static str {
        self.try_get(key).unwrap_or_else(|| panic!("no text for {key:?}"))
    }

    /// The text of `key` in the current locale (or the first of its fallbacks that has it), or None when the
    /// table has no such key.
    pub fn try_get(&'static self, key: &str) -> Option<&'static str> {
        self.find(key).map(|t| t.plain[key])
    }

    /// The texts of the current locale's chain that first have `key`.
    fn find(&'static self, key: &str) -> Option<&'static Texts> {
        let texts = self.get_or_load();
        current().chain().filter_map(|i| texts[i].as_ref()).find(|t| t.plain.contains_key(key))
    }

    /// Every key of the table (its English keys), sorted.
    pub fn keys(&'static self) -> Vec<&'static str> {
        let mut keys: Vec<&'static str> = self.get_or_load()[0].as_ref().map_or_else(Vec::new, |t| t.plain.keys().map(String::as_str).collect());
        keys.sort_unstable();
        keys
    }

    /// Whether `key` is in `locale`'s own file, not taken from a fallback.
    pub fn has_own(&'static self, locale: Locale, key: &str) -> bool {
        self.get_or_load()[locale.0 as usize].as_ref().is_some_and(|t| t.plain.contains_key(key))
    }

    fn get_or_load(&'static self) -> &'static [Option<Texts>] {
        self.texts.get_or_init(|| {
            Locale::all()
                .into_iter()
                .map(|locale| {
                    let id = format!("{}/{}", self.prefix, locale.code().to_lowercase());
                    match resources::find(&id) {
                        Some(r) => Some(load(&id, r.text(), locale.code())),
                        None if !locale.entry().complete => None,
                        None => panic!("no resource {id}: every complete locale has a text table"),
                    }
                })
                .collect()
        })
    }

    /// The text of `key` with its values filled in (`args!`).
    pub fn format(&'static self, key: &str, args: &[(&str, &dyn Display)]) -> String {
        format(self.find(key).unwrap_or_else(|| panic!("no text for {key:?}")), self.prefix, key, args)
    }
}

/// `key`'s message of `texts` with its values.
fn format(texts: &Texts, table: &str, key: &str, args: &[(&str, &dyn Display)]) -> String {
    let pattern = texts.bundle.get_message(key).and_then(|m| m.value()).unwrap_or_else(|| panic!("no text for {key:?}"));
    let mut values = FluentArgs::new();
    for (name, value) in args {
        values.set(*name, value_of(value.to_string()));
    }
    let mut errors = Vec::new();
    let text = texts.bundle.format_pattern(pattern, Some(&values), &mut errors).into_owned();
    assert!(errors.is_empty(), "{table}: {key}: {errors:?}");
    text
}

/// A value as Fluent takes it: a number when Fluent writes that number the same
/// way (so a selector can choose by it), else the string as it is.
fn value_of(text: String) -> FluentValue<'static> {
    match FluentValue::try_number(&text) {
        FluentValue::Number(n) if n.as_string() == text => FluentValue::Number(n),
        _ => FluentValue::String(text.into()),
    }
}

/// A locale's bundle from its Fluent file, with the rich text's functions,
/// and its messages' texts without values.
fn load(id: &str, source: &str, locale: &str) -> Texts {
    let resource = FluentResource::try_new(source.to_string()).unwrap_or_else(|(_, e)| panic!("{id}: not a Fluent file: {e:?}"));
    let keys: Vec<String> = resource
        .entries()
        .filter_map(|e| match e {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
            _ => None,
        })
        .collect();
    let mut bundle = FluentBundle::new_concurrent(vec![locale.parse().unwrap_or_else(|e| panic!("{locale}: {e}"))]);
    // The lab lays text out itself: no bidirectional isolation marks around values.
    bundle.set_use_isolating(false);
    for name in crate::rich_text::FUNCTIONS {
        bundle.add_function(name, move |positional, named| FluentValue::String(crate::rich_text::call(name, positional, named).into())).expect("each function once");
    }
    bundle.add_resource(resource).unwrap_or_else(|e| panic!("{id}: {e:?}"));
    let plain = keys
        .into_iter()
        .map(|key| {
            let pattern = bundle.get_message(&key).and_then(|m| m.value()).unwrap_or_else(|| panic!("{id}: {key} has no value"));
            // Without values, a message's variables read as Fluent writes a missing one (`{$seconds}`).
            let text: &'static str = Box::leak(bundle.format_pattern(pattern, None, &mut Vec::new()).into_owned().into_boxed_str());
            (key, text)
        })
        .collect();
    Texts { bundle, plain }
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
    package_table(package).get(key)
}

/// `package_text` with its values (`args!`).
pub fn package_text_with(package: &str, key: &str, args: &[(&str, &dyn Display)]) -> String {
    package_table(package).format(key, args)
}

/// Every key of `package`'s table, sorted.
pub fn package_keys(package: &str) -> Vec<&'static str> {
    package_table(package).keys()
}

/// The text tables of every package that has one, per locale (`render-lab i18n`): the keys the locale's own
/// table lacks, the texts equal to English's, and the characters the bundled fonts cannot draw (outside formulas,
/// which Typst typesets with fonts of its own). `problems` counts what a complete locale may not have.
pub fn report() -> Vec<serde_json::Value> {
    let ctx = egui::Context::default();
    crate::text::install_fonts(&ctx);
    let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
    let mut out = Vec::new();
    let saved = current();
    for package in crate::resources::packages().iter().map(|p| p.name.as_str()) {
        if resources::find(&format!("{package}.text/en")).is_none() {
            continue;
        }
        let table = package_table(package);
        let keys = table.keys();
        let english: Vec<&str> = keys.iter().map(|k| table.get_or_load()[0].as_ref().map_or("", |t| t.plain[*k])).collect();
        for locale in Locale::all() {
            set(locale);
            let missing: Vec<&str> = keys.iter().copied().filter(|k| !table.has_own(locale, k)).collect();
            let same: Vec<&str> = if locale == Locale::EN { Vec::new() } else { keys.iter().zip(&english).filter(|(k, en)| table.has_own(locale, k) && table.get(k) == **en).map(|(k, _)| *k).collect() };
            let mut unglyphed: Vec<(char, &str)> = Vec::new();
            // The package's own texts, then its source table's (src/sources.rs), in this locale.
            let sources: Vec<(&'static str, &'static str)> = if locale == Locale::EN { Vec::new() } else { source_table(package, locale).map_or_else(Vec::new, |t| t.plain.iter().map(|(k, v)| (k.as_str(), *v)).collect()) };
            let texts: Vec<(&str, &str)> = keys.iter().map(|k| (*k, table.get(k))).chain(sources.iter().copied()).collect();
            for (key, text) in texts {
                let plain: String = crate::rich_text::parse(text).unwrap_or_default().into_iter().filter_map(|r| match r {
                    crate::rich_text::Run::Text(t) | crate::rich_text::Run::Code(t) => Some(t),
                    crate::rich_text::Run::Formula { .. } => None,
                }).collect();
                for c in plain.chars().filter(|c| !c.is_control()) {
                    if !unglyphed.iter().any(|(u, _)| *u == c) && !ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(24.0), c)) {
                        unglyphed.push((c, key));
                    }
                }
            }
            let complete = locale.entry().complete;
            let problems = unglyphed.len() + if complete { missing.len() } else { 0 };
            out.push(serde_json::json!({
                "package": package,
                "locale": locale.code(),
                "status": if complete { "complete" } else { "draft" },
                "keys": keys.len(),
                "missing": missing,
                "sameAsEnglish": same.len(),
                "sameAsEnglishKeys": same,
                "sourceTexts": sources.len(),
                "unglyphed": unglyphed.iter().map(|(c, k)| serde_json::json!({"char": c.to_string(), "codepoint": format!("U+{:04X}", *c as u32), "key": k})).collect::<Vec<_>>(),
                "problems": problems,
            }));
        }
    }
    set(saved);
    out
}

/// The current locale's translation of a text of `package`'s source index
/// (src/sources.rs): its `text/sources/<code>.ftl`, the resource
/// `<package>.sources/<code in lower case>`, along the locale's fallback chain;
/// None in English, which is the index's own, or when no table has the key.
pub fn source_text(package: &str, key: &str) -> Option<&'static str> {
    current().chain().filter(|&i| i != 0).find_map(|i| source_table(package, Locale(i as u8)).and_then(|t| t.plain.get(key).copied()))
}

/// Every source text of `package` in the current locale's chain (first found
/// wins), sorted by key; empty in English.
pub fn source_texts(package: &str) -> Vec<(&'static str, &'static str)> {
    let mut out: std::collections::BTreeMap<&'static str, &'static str> = std::collections::BTreeMap::new();
    for i in current().chain().filter(|&i| i != 0) {
        if let Some(t) = source_table(package, Locale(i as u8)) {
            for (k, v) in &t.plain {
                out.entry(k.as_str()).or_insert(v);
            }
        }
    }
    out.into_iter().collect()
}

/// A package's source table in `locale`, read once (None without one).
fn source_table(package: &str, locale: Locale) -> Option<&'static Texts> {
    static TABLES: std::sync::Mutex<Vec<((String, Locale), Option<&'static Texts>)>> = std::sync::Mutex::new(Vec::new());
    let mut tables = TABLES.lock().unwrap();
    if let Some((_, t)) = tables.iter().find(|(k, _)| k.0 == package && k.1 == locale) {
        return *t;
    }
    let id = format!("{package}.sources/{}", locale.code().to_lowercase());
    let table = resources::find(&id).map(|r| &*Box::leak(Box::new(load(&id, r.text(), locale.code()))));
    tables.push(((package.to_string(), locale), table));
    table
}

/// `package_text`, or None when the package's table has no such key.
pub fn try_package_text(package: &str, key: &str) -> Option<&'static str> {
    package_table(package).try_get(key)
}

fn package_table(package: &str) -> &'static Table {
    if package == "render_lab" {
        return &TEXT;
    }
    static TABLES: std::sync::Mutex<Vec<&'static Table>> = std::sync::Mutex::new(Vec::new());
    let prefix = format!("{package}.text");
    let mut tables = TABLES.lock().unwrap();
    match tables.iter().find(|t| t.prefix == prefix) {
        Some(t) => t,
        None => {
            let t: &'static Table = Box::leak(Box::new(Table::new(Box::leak(prefix.into_boxed_str()))));
            tables.push(t);
            t
        }
    }
}

/// The lab's text of `key`, with its values (`args!`).
pub fn tr_with(key: &'static str, args: &[(&str, &dyn Display)]) -> String {
    TEXT.format(key, args)
}


/// A scene object's display name, or a card name that starts with one
/// (`rotated box +X` → `旋转的盒子 +X`).
pub fn object_name(name: &str) -> String {
    if current() == Locale::EN {
        return name.to_string();
    }
    let objects = [
        ("floor", tr("object-floor")),
        ("back wall", tr("object-back_wall")),
        ("left wall", tr("object-left_wall")),
        ("right wall", tr("object-right_wall")),
        ("emissive panel", tr("object-emissive_panel")),
        ("sphere", tr("object-sphere")),
        ("rotated box", tr("object-rotated_box")),
        ("small box", tr("object-small_box")),
        ("teapot", tr("object-teapot")),
        ("terrain", tr("object-terrain")),
        ("sun panel", tr("object-sun_panel")),
        ("none", tr("object-none")),
    ];
    for (en, zh) in objects {
        if let Some(rest) = name.strip_prefix(en) && (rest.is_empty() || rest.starts_with(' ')) {
            return format!("{zh}{rest}");
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
        set(Locale::from_code("zh-CN").unwrap());
        assert_eq!(object_name("rotated box +X"), "旋转的盒子 +X");
        assert_eq!(object_name("small box"), "小盒子");
        assert_eq!(object_name("floor"), "地板");
        set(Locale::EN);
        assert_eq!(object_name("floor"), "floor");
    }

    #[test]
    fn messages() {
        let t = load("test", "back = back { $seconds } s\nbraces = { \"{\" }x{ \"}\" } = { $x }\nlead = { \"  \" }= r g b\nlines =\n    one\n    { \"  \" }two\n", "en");
        assert_eq!(format(&t, "test", "back", args!(seconds = 10)), "back 10 s");
        assert_eq!(format(&t, "test", "back", args!(seconds = "0.50")), "back 0.50 s");
        assert_eq!(format(&t, "test", "braces", args!(x = 1.5)), "{x} = 1.5");
        assert_eq!(t.plain["lead"], "  = r g b");
        assert_eq!(t.plain["lines"], "one\n  two");
        assert_eq!(t.plain["back"], "back {$seconds} s");
    }

    #[test]
    fn plurals_and_rich_text() {
        let t = load("test", "n = { $n ->\n    [one] one cluster\n   *[other] { $n } clusters\n}\nzh = { $n } 个 cluster\n", "en");
        assert_eq!(format(&t, "test", "n", args!(n = 1)), "one cluster");
        assert_eq!(format(&t, "test", "n", args!(n = 3)), "3 clusters");
        assert_eq!(format(&t, "test", "zh", args!(n = 3)), "3 个 cluster");
        let f = r#"see { FORMULA("a \\cdot b", display: "yes") } and { CODE("Name") }"#;
        let t = load("test", &format!("f = {f}\n"), "en");
        assert_eq!(t.plain["f"], f);
    }
}
