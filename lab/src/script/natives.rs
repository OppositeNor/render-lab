//! The lab's natives for Lamlet: `Pure` functions that check a name a storyboard gives against what the lab and
//! the investigation hold, so a wrong one is an error at the storyboard's line, before any speech. Each returns
//! the name it checked. They are the package `rl` (`declarations`, its Lamlet stubs), which render_lab's Lamlet
//! package imports for its checked attributes (`callout`, `imageTitle`, `point`, `formulaSaid`, …).
//!
//! | Native | Checks |
//! |---|---|
//! | `rl::source key` | a key of the investigation's source index (a callout) |
//! | `rl::text key` | a key of the investigation's package's text table, or the lab's |
//! | `rl::point key` | an annotation point the investigation publishes |
//! | `rl::formula tex` | the LaTeX typesets within the lab's limits (`crate::formula::check`) |
//!
//! They need an investigation (`render-lab --investigation <id> lamlet …`); without one, each fails with a
//! message saying so.

use crate::investigation::Investigation;
use lamlet::embed::{Engine, Error};

/// The stub package's name, and its natives' id prefix.
pub const PACKAGE: &str = "rl";

/// The investigation the natives check against, or an error naming the native that needs it.
fn chosen(
    investigation: Option<&'static dyn Investigation>,
    native: &str,
) -> Result<&'static dyn Investigation, String> {
    investigation.ok_or_else(|| format!("rl::{native} checks against an investigation: run `render-lab --investigation <id> lamlet …`"))
}

/// Registers the natives on `engine`, checking against `investigation`.
pub fn register(
    engine: &mut Engine,
    investigation: Option<&'static dyn Investigation>,
) -> Result<(), Error> {
    let r = engine.natives_mut();
    r.function("rl/source", move |_, (key,): (String,)| {
        let i = chosen(investigation, "source")?;
        if i.sources().entries.contains_key(&key) {
            Ok(key)
        } else {
            Err(format!("no source-index key `{key}` in {}'s source index", i.path()))
        }
    })
    .pure()
    .doc("Fn([String], String). `key`, a key of the investigation's source index (a callout's), or an error.")
    .register()?;
    r.function("rl/text", move |_, (key,): (String,)| {
        let i = chosen(investigation, "text")?;
        let package = i.package().unwrap_or("render_lab");
        if crate::i18n::try_package_text(package, &key).is_some() || crate::i18n::try_package_text("render_lab", &key).is_some() {
            Ok(key)
        } else {
            Err(format!("no text `{key}` in {package}'s text table (text/en.ftl) or the lab's"))
        }
    })
    .pure()
    .doc("Fn([String], String). `key`, a key of the investigation's package's text table or the lab's, or an error.")
    .register()?;
    r.function("rl/point", move |_, (key,): (String,)| {
        let i = chosen(investigation, "point")?;
        use crate::utility::annotations::AnnotationKind;
        let points: Vec<&str> = i.annotations().iter().filter(|a| matches!(a.kind, AnnotationKind::Point)).map(|a| a.key).collect();
        if points.contains(&key.as_str()) {
            Ok(key)
        } else {
            Err(format!("no point `{key}` that {} publishes (one of: {})", i.path(), points.join(", ")))
        }
    })
    .pure()
    .doc("Fn([String], String). `key`, an annotation point the investigation publishes, or an error.")
    .register()?;
    r.function("rl/formula", move |_, (tex,): (String,)| {
        chosen(investigation, "formula")?;
        crate::formula::check(&tex, false)
            .map(|_| tex.clone())
            .map_err(|e| format!("the formula {tex:?}: {e}"))
    })
    .pure()
    .doc("Fn([String], String). `tex`, LaTeX that typesets within the lab's limits, or an error.")
    .register()?;
    Ok(())
}

/// The stub package that declares the natives.
pub fn declarations() -> Result<String, String> {
    let mut engine = Engine::new();
    register(&mut engine, None).map_err(|e| e.render())?;
    engine
        .natives_mut()
        .declarations(PACKAGE)
        .map_err(|e| e.render())
}
