//! Math scenes: the `Math` hooks a package's scripts define (runtime.lam), their mobs decoded for the canvas
//! (src/math/), and the natives the library `render_lab_math` measures by.

use super::overlays::{field, number, text};
use super::*;
use crate::math::{Mob, Part, Scene};

/// The resource that holds the math library.
pub const MATH_LIBRARY: &str = "render_lab.script/math";
/// The math library's Lamlet package.
pub const MATH_PACKAGE: &str = "render_lab_math";

crate::lamlet_record! {
    /// A box on the canvas: its lower left and upper right corners.
    pub struct BoxRecord {
        pub lo: V2 = "lo",
        pub hi: V2 = "hi",
    }
}

crate::lamlet_record! {
    /// A formula's measured box (`crate::math::TexBox`), in em.
    pub struct TexBoxRecord {
        pub width: f64 = "width",
        pub height: f64 = "height",
        pub parts: Vec<BoxRecord> = "parts",
    }
}

/// Registers the natives the math library measures by (`rl_runtime/tex-box`, `rl_runtime/text-width`).
pub(super) fn register_math_natives(engine: &mut Engine) -> Result<(), lamlet::embed::Error> {
    let r = engine.natives_mut();
    r.function("rl_runtime/tex-box", |_, (latex,): (String,)| {
        let b = crate::math::tex_box(&latex)?;
        Ok(TexBoxRecord {
            width: b.width as f64,
            height: b.height as f64,
            parts: b
                .parts
                .into_iter()
                .map(|(lo, hi)| BoxRecord { lo: V2(lo), hi: V2(hi) })
                .collect(),
        })
    })
    .doc("Fn([String], {height: Float, parts: List({hi: Vec2, lo: Vec2}), width: Float}). The formula `latex` typeset for display, measured in em: its bitmap's size, and each part's ink box about the bitmap's centre, +y up (index 0 its ink outside the `\\lit{n}{…}` terms, n term n; an empty part is a box of no size at the centre).")
    .register()?;
    r.function("rl_runtime/text-width", |_, (text, key): (String, bool)| {
        let shown = if key { text_of(&text)? } else { text };
        Ok(crate::math::text_width(&shown) as f64)
    })
    .doc("Fn([String, Bool], Float). The width in em of a line of text, written out or (`key`) the package's text of that key, in the viewer's language.")
    .register()?;
    Ok(())
}

/// The package's text `key` in the viewer's language, or the lab's.
fn text_of(key: &str) -> Result<String, String> {
    let package = investigation::current().package().unwrap_or("render_lab");
    crate::i18n::try_package_text(package, key)
        .or_else(|| crate::i18n::try_package_text("render_lab", key))
        .map(str::to_string)
        .ok_or_else(|| format!("no text `{key}` in {package}'s text table or the lab's"))
}

fn vec2_of(d: &Data, label: &str) -> Result<glam::Vec2, String> {
    let p = field(d, label)?;
    Ok(glam::Vec2::new(number(p, "x")?, number(p, "y")?))
}

fn points(d: &Data, label: &str) -> Result<Vec<glam::Vec2>, String> {
    let Data::List(items) = field(d, label)? else {
        return Err(format!("`{label}` is not a list of points"));
    };
    items
        .iter()
        .map(|p| Ok(glam::Vec2::new(number(p, "x")?, number(p, "y")?)))
        .collect()
}

fn int(d: &Data, label: &str) -> Result<i64, String> {
    match field(d, label)? {
        Data::Int(n) => Ok(*n),
        other => Err(format!("`{label}` is {other:?}, not an Int")),
    }
}

/// A colour field (`#rrggbb` or a theme colour's name) with an alpha field, premultiplied.
fn tinted(d: &Data, color: &str, alpha: &str) -> Result<Color32, String> {
    let c = super::overlays::color_named(text(d, color)?)?;
    Ok(c.gamma_multiply(number(d, alpha)?.clamp(0.0, 1.0)))
}

/// The mobs a math scene returned, each checked.
fn mobs(list: &Data) -> Result<Vec<Mob>, String> {
    let Data::List(items) = list else {
        return Err("a math scene returns a list".into());
    };
    items
        .iter()
        .map(|item| {
            let Data::Variant(tag, d) = item else {
                return Err(format!("{item:?} is not a Mob"));
            };
            mob(tag, d)
        })
        .collect()
}

fn mob(tag: &str, d: &Data) -> Result<Mob, String> {
    Ok(match tag {
        "tex" => Mob::Tex {
            latex: text(d, "latex")?.to_string(),
            part: Part::of(int(d, "part")?)?,
            at: vec2_of(d, "at")?,
            size: number(d, "size")?,
            color: tinted(d, "color", "alpha")?,
            wipe: number(d, "wipe")?,
            lo: vec2_of(d, "lo")?,
            hi: vec2_of(d, "hi")?,
        },
        "text" => {
            let written = text(d, "text")?;
            let key = matches!(field(d, "key")?, Data::Bool(true));
            Mob::Text {
                text: if key {
                    text_of(written)?
                } else {
                    written.to_string()
                },
                at: vec2_of(d, "at")?,
                size: number(d, "size")?,
                color: tinted(d, "color", "alpha")?,
            }
        }
        "rect" => Mob::Rect {
            lo: vec2_of(d, "lo")?,
            hi: vec2_of(d, "hi")?,
            fill: tinted(d, "fill", "fillAlpha")?,
            stroke: tinted(d, "stroke", "strokeAlpha")?,
            width: number(d, "width")?,
            radius: number(d, "radius")?,
        },
        "path" => Mob::Path {
            points: points(d, "points")?,
            color: tinted(d, "color", "alpha")?,
            width: number(d, "width")?,
            drawn: number(d, "drawn")?,
            arrow: matches!(field(d, "arrow")?, Data::Bool(true)),
            dashed: matches!(field(d, "dashed")?, Data::Bool(true)),
        },
        "band" => Mob::Band {
            top: points(d, "top")?,
            bottom: points(d, "bottom")?,
            color: tinted(d, "color", "alpha")?,
        },
        "dot" => Mob::Dot {
            at: vec2_of(d, "at")?,
            radius: number(d, "radius")?,
            color: tinted(d, "color", "alpha")?,
        },
        "frame" => Mob::Frame {
            center: vec2_of(d, "center")?,
            height: number(d, "height")?,
        },
        other => return Err(format!("no mob `{other}`")),
    })
}

/// The scripts' math scenes, by name.
pub fn math_scenes() -> Vec<String> {
    with_runtime(|rt| rt.maths.keys().cloned().collect())
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// What the math scene `name` draws for `state` at `time` seconds: its mobs, or what it last drew when the call
/// fails in the viewer (a recording or a selftest fails instead, `set_strict`); None when the scripts have no
/// such scene.
pub fn math_scene(name: &str, state: &ShotState, time: f32) -> Option<Scene> {
    let drawn = with_runtime(|rt| {
        let kind = Kind::Math(name.to_string());
        let label = kind.to_string();
        // A scene the scripts lack fails a recording; the viewer shows the message.
        if rt.hook(&kind).is_none() {
            let known: Vec<&str> = rt.maths.keys().map(String::as_str).collect();
            let known = crate::story_refs::names_or_none(&known);
            rt.failed(
                &label,
                format!("the scripts have no math scene `{name}` (theirs: {known})"),
            );
            return None;
        }
        let result = rt
            .call(&kind, vec![Value::Float(time as f64)], state, None, None)
            .and_then(|d| mobs(&d));
        match result {
            Ok(mobs) => {
                rt.succeeded(&label);
                let scene = Scene { mobs };
                rt.last_math.insert(name.to_string(), scene.clone());
                Some(scene)
            }
            Err(e) => {
                rt.failed(&label, e);
                rt.last_math.get(name).cloned()
            }
        }
    });
    match drawn {
        Ok(scene) => scene.flatten(),
        Err(e) => {
            if STRICT.load(Ordering::Relaxed) {
                panic!("the scripts of {}: {e}", investigation::current().path());
            }
            None
        }
    }
}
