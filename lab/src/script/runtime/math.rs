//! Math scenes: the `Math` hooks a package's scripts define (runtime.lam), their mobs decoded for the canvas
//! (src/math/), and the natives the library `render_lab_math` measures by.

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

/// Registers the natives the math library measures, lays out, and samples by (`rl_runtime/tex-box`,
/// `rl_runtime/text-width`, `rl_runtime/graph-layout`, `rl_runtime/samples-from`, `rl_runtime/uniform`).
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
    r.function(
        "rl_runtime/graph-layout",
        |_, (count, edges, layout, seed): (i64, Vec<(i64, i64)>, String, i64)| {
            let edges: Vec<(usize, usize)> = edges
                .into_iter()
                .map(|(a, b)| (a.max(0) as usize, b.max(0) as usize))
                .collect();
            let points = crate::math::graph_layout(count.max(0) as usize, &edges, &layout, seed as u64)?;
            Ok(points.iter().map(|p| V2(*p)).collect::<Vec<_>>())
        },
    )
    .doc("Fn([Int, List({first: Int, second: Int}), String, Int], List(Vec2)). Where each of `count` nodes goes in a graph layout (`circular`, `grid`, `bipartite`, `tree`, `layered`, `spring`; lab/src/math/layout.rs), from its edges (from, to) and, for `spring`, a seed: in [-1, 1] on both axes, +y up.")
    .register()?;
    r.function(
        "rl_runtime/samples-from",
        |_, (masses, seed, count): (Vec<f64>, i64, i64)| {
            crate::math::samples_from(&masses, seed as u64, count.max(0) as usize)
        },
    )
    .doc("Fn([List(Float), Int, Int], List(Float)). `count` numbers in [0, 1) drawn in proportion to `masses` (the masses of even bins across [0, 1), none negative, not all 0), seeded: the inverse of their running total, linear within a bin.")
    .register()?;
    r.function("rl_runtime/uniform", |_, (seed, i): (i64, i64)| {
        Ok(crate::math::uniform(seed as u64, i as u64))
    })
    .doc("Fn([Int, Int], Float). The `i`-th of a seed's uniform numbers in [0, 1) (splitmix64), neighbours unrelated.")
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

crate::lamlet_record! {
    /// runtime.lam's `_MathTex`, the parts the canvas draws.
    struct TexRecord {
        latex: String = "latex",
        part: i64 = "part",
        at: V2 = "at",
        size: Num = "size",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
        wipe: Num = "wipe",
        lo: V2 = "lo",
        hi: V2 = "hi",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathText`, the parts the canvas draws: its text written out, or (`key`) a text key.
    struct TextRecord {
        text: String = "text",
        key: bool = "key",
        at: V2 = "at",
        size: Num = "size",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathRect`, the parts the canvas draws.
    struct RectRecord {
        lo: V2 = "lo",
        hi: V2 = "hi",
        fill: ScriptColor = "fill",
        fill_alpha: Num = "fillAlpha",
        stroke: ScriptColor = "stroke",
        stroke_alpha: Num = "strokeAlpha",
        width: Num = "width",
        radius: Num = "radius",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathPath`, the parts the canvas draws.
    struct PathRecord {
        points: Vec<V2> = "points",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
        width: Num = "width",
        drawn: Num = "drawn",
        arrow: bool = "arrow",
        dashed: bool = "dashed",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathBand`, the parts the canvas draws.
    struct BandRecord {
        top: Vec<V2> = "top",
        bottom: Vec<V2> = "bottom",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathDot`, the parts the canvas draws.
    struct DotRecord {
        at: V2 = "at",
        radius: Num = "radius",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathFrame`.
    struct FrameRecord {
        center: V2 = "center",
        height: Num = "height",
    }
}

/// A colour with an alpha, premultiplied.
fn tinted(color: ScriptColor, alpha: Num) -> Color32 {
    color.color32().gamma_multiply(alpha.0.clamp(0.0, 1.0))
}

fn points(list: Vec<V2>) -> Vec<glam::Vec2> {
    list.into_iter().map(|p| p.0).collect()
}

/// runtime.lam's `Mob`, read by its type.
struct MobRecord(Mob);

impl lamlet::embed::FromLamlet for MobRecord {
    fn from_lamlet(v: &Value) -> Result<Self, String> {
        use super::typed::payload;
        let (tag, p) = super::typed::variant(v)?;
        Ok(MobRecord(match tag {
            "tex" => {
                let r: TexRecord = payload(tag, p)?;
                Mob::Tex {
                    latex: r.latex,
                    part: Part::of(r.part)?,
                    at: r.at.0,
                    size: r.size.0,
                    color: tinted(r.color, r.alpha),
                    wipe: r.wipe.0,
                    lo: r.lo.0,
                    hi: r.hi.0,
                }
            }
            "text" => {
                let r: TextRecord = payload(tag, p)?;
                Mob::Text {
                    // A key is looked up in the viewer's language; a written text is drawn as it is.
                    text: if r.key {
                        text_of(&r.text)?
                    } else {
                        r.text
                    },
                    at: r.at.0,
                    size: r.size.0,
                    color: tinted(r.color, r.alpha),
                }
            }
            "rect" => {
                let r: RectRecord = payload(tag, p)?;
                Mob::Rect {
                    lo: r.lo.0,
                    hi: r.hi.0,
                    fill: tinted(r.fill, r.fill_alpha),
                    stroke: tinted(r.stroke, r.stroke_alpha),
                    width: r.width.0,
                    radius: r.radius.0,
                }
            }
            "path" => {
                let r: PathRecord = payload(tag, p)?;
                Mob::Path {
                    points: points(r.points),
                    color: tinted(r.color, r.alpha),
                    width: r.width.0,
                    drawn: r.drawn.0,
                    arrow: r.arrow,
                    dashed: r.dashed,
                }
            }
            "band" => {
                let r: BandRecord = payload(tag, p)?;
                Mob::Band {
                    top: points(r.top),
                    bottom: points(r.bottom),
                    color: tinted(r.color, r.alpha),
                }
            }
            "dot" => {
                let r: DotRecord = payload(tag, p)?;
                Mob::Dot {
                    at: r.at.0,
                    radius: r.radius.0,
                    color: tinted(r.color, r.alpha),
                }
            }
            "frame" => {
                let r: FrameRecord = payload(tag, p)?;
                Mob::Frame {
                    center: r.center.0,
                    height: r.height.0,
                }
            }
            other => return Err(format!("no mob `{other}`")),
        }))
    }
}

/// The mobs a math scene returned, each checked.
fn mobs(list: &Value) -> Result<Vec<Mob>, String> {
    Ok(super::typed::items::<MobRecord>(list, "mob")?
        .into_iter()
        .map(|m| m.0)
        .collect())
}

/// The scripts' math scenes, by name.
pub fn math_scenes() -> Vec<String> {
    with_runtime(|rt| rt.maths.keys().cloned().collect())
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// What the math scene `name` draws for `state` at `time` seconds, its natives reading `frame` (what the
/// investigation's `build_frame` left, `frame_data`): its mobs, or what it last drew when the call fails in the
/// viewer (a recording or a selftest fails instead, `set_strict`); None when the scripts have no such scene.
pub fn math_scene(
    name: &str,
    state: &ShotState,
    frame: Option<&FrameContent>,
    statics: &Statics,
    time: f32,
) -> Option<Scene> {
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
            .call(
                &kind,
                vec![Value::Float(time as f64)],
                state,
                frame,
                Some(statics),
            )
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
