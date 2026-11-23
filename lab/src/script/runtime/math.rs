//! Math scenes: the `Math` hooks a package's scripts define (runtime.lam), their mobs decoded for the canvas
//! (src/math/), and the natives the library `render_lab_math` measures by.

use super::*;
use crate::math::{
    Mob, Part, Scene,
    check::{Probe, Role},
};

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
        id: String = "id",
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
        id: String = "id",
        text: String = "text",
        key: bool = "key",
        at: V2 = "at",
        size: Num = "size",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
        lo: V2 = "lo",
        hi: V2 = "hi",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathRect`, the parts the canvas draws.
    struct RectRecord {
        id: String = "id",
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
        id: String = "id",
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
        id: String = "id",
        top: Vec<V2> = "top",
        bottom: Vec<V2> = "bottom",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_MathDot`, the parts the canvas draws.
    struct DotRecord {
        id: String = "id",
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

/// runtime.lam's `Mob`, read by its type: what the canvas draws, and what the motion check measures of it (none for
/// the frame).
struct MobRecord {
    mob: Mob,
    probe: Option<Probe>,
}

/// The ink box of a polyline or of a band's lines.
fn bounds(points: &[glam::Vec2]) -> (glam::Vec2, glam::Vec2) {
    points.iter().fold(
        (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    )
}

/// The first `drawn` fraction (by length) of a polyline, as the canvas draws it.
fn drawn_part(points: &[glam::Vec2], drawn: f32) -> Vec<glam::Vec2> {
    if drawn >= 1.0 || points.len() < 2 {
        return points.to_vec();
    }
    let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total * drawn.max(0.0);
    let mut out = vec![points[0]];
    for w in points.windows(2) {
        // Each segment is kept whole while the length left covers it, and cut where it runs out.
        let d = w[0].distance(w[1]);
        if d >= left {
            out.push(w[0] + (w[1] - w[0]) * (left / d.max(1e-6)));
            break;
        }
        out.push(w[1]);
        left -= d;
    }
    out
}

/// What the check measures of a mob named `id` of `kind`.
fn probe(
    id: String,
    kind: &'static str,
    (lo, hi): (glam::Vec2, glam::Vec2),
    color: ScriptColor,
) -> Probe {
    let c = color.color32();
    Probe {
        id,
        kind,
        lo,
        hi,
        alpha: 1.0,
        coverage: 1.0,
        rgb: [c.r(), c.g(), c.b()].map(|v| v as f32 / 255.0),
        em: None,
        fine_print: false,
        formula: None,
        points: Vec::new(),
        rest: false,
        fill: 0.0,
        role: match color {
            ScriptColor::Text => Some(Role::Text),
            ScriptColor::TextDim => Some(Role::Dim),
            ScriptColor::TextFaint => Some(Role::Faint),
            ScriptColor::Accent => Some(Role::Accent),
            ScriptColor::Rgb(_) => None,
        },
    }
}

/// A `tex` mob, and what the check measures of it.
fn read_tex(r: TexRecord) -> Result<MobRecord, String> {
    let fine = ["\\frac", "_", "^", "\\sum", "\\int"]
        .iter()
        .any(|m| r.latex.contains(m));
    // A formula is known by its LaTeX: a transform carries its parts apart, each to its own place.
    let formula = r.latex.clone();
    let probe = Probe {
        alpha: r.alpha.0,
        coverage: r.wipe.0.clamp(0.0, 1.0),
        em: Some(r.size.0),
        fine_print: fine,
        formula: Some(formula),
        // Only a formula with terms has a rest apart from them (its box the whole formula's); one without is whole.
        rest: r.part == 0 && r.latex.contains("\\lit"),
        ..probe(r.id, "tex", (r.at.0 + r.lo.0, r.at.0 + r.hi.0), r.color)
    };
    let mob = Mob::Tex {
        latex: r.latex,
        part: Part::of(r.part)?,
        at: r.at.0,
        size: r.size.0,
        color: tinted(r.color, r.alpha),
        wipe: r.wipe.0,
        lo: r.lo.0,
        hi: r.hi.0,
    };
    Ok(MobRecord {
        mob,
        probe: Some(probe),
    })
}

/// A `text` mob, and what the check measures of it.
fn read_text(r: TextRecord) -> Result<MobRecord, String> {
    let probe = Probe {
        alpha: r.alpha.0,
        em: Some(r.size.0),
        ..probe(r.id, "text", (r.lo.0, r.hi.0), r.color)
    };
    let mob = Mob::Text {
        // A key is looked up in the viewer's language; a written text is drawn as it is.
        text: if r.key {
            text_of(&r.text)?
        } else {
            r.text
        },
        at: r.at.0,
        size: r.size.0,
        color: tinted(r.color, r.alpha),
    };
    Ok(MobRecord {
        mob,
        probe: Some(probe),
    })
}

/// A `rect` mob, and what the check measures of it.
fn read_rect(r: RectRecord) -> Result<MobRecord, String> {
    // A rectangle shows by its fill, and by its outline when it has one.
    let outline = if r.width.0 > 0.0 {
        r.stroke_alpha.0
    } else {
        0.0
    };
    let probe = Probe {
        alpha: r.fill_alpha.0.max(outline),
        fill: r.fill_alpha.0,
        ..probe(
            r.id,
            "rect",
            (r.lo.0.min(r.hi.0), r.lo.0.max(r.hi.0)),
            r.fill,
        )
    };
    let mob = Mob::Rect {
        lo: r.lo.0,
        hi: r.hi.0,
        fill: tinted(r.fill, r.fill_alpha),
        stroke: tinted(r.stroke, r.stroke_alpha),
        width: r.width.0,
        radius: r.radius.0,
    };
    Ok(MobRecord {
        mob,
        probe: Some(probe),
    })
}

/// A `path` mob, and what the check measures of it.
fn read_path(r: PathRecord) -> Result<MobRecord, String> {
    let points = points(r.points);
    let probe = Probe {
        alpha: r.alpha.0,
        coverage: r.drawn.0.clamp(0.0, 1.0),
        points: drawn_part(&points, r.drawn.0),
        ..probe(r.id, "path", bounds(&points), r.color)
    };
    let mob = Mob::Path {
        points,
        color: tinted(r.color, r.alpha),
        width: r.width.0,
        drawn: r.drawn.0,
        arrow: r.arrow,
        dashed: r.dashed,
    };
    Ok(MobRecord {
        mob,
        probe: Some(probe),
    })
}

/// A `band` mob, and what the check measures of it.
fn read_band(r: BandRecord) -> Result<MobRecord, String> {
    let (top, bottom) = (points(r.top), points(r.bottom));
    let all: Vec<glam::Vec2> = top.iter().chain(&bottom).copied().collect();
    let probe = Probe {
        alpha: r.alpha.0,
        ..probe(r.id, "band", bounds(&all), r.color)
    };
    let mob = Mob::Band {
        top,
        bottom,
        color: tinted(r.color, r.alpha),
    };
    Ok(MobRecord {
        mob,
        probe: Some(probe),
    })
}

/// A `dot` mob, and what the check measures of it.
fn read_dot(r: DotRecord) -> Result<MobRecord, String> {
    let d = glam::Vec2::splat(r.radius.0);
    let probe = Probe {
        alpha: r.alpha.0,
        fill: r.alpha.0,
        ..probe(r.id, "dot", (r.at.0 - d, r.at.0 + d), r.color)
    };
    let mob = Mob::Dot {
        at: r.at.0,
        radius: r.radius.0,
        color: tinted(r.color, r.alpha),
    };
    Ok(MobRecord {
        mob,
        probe: Some(probe),
    })
}

impl lamlet::embed::FromLamlet for MobRecord {
    fn from_lamlet(v: &Value) -> Result<Self, String> {
        use super::typed::payload;
        let (tag, p) = super::typed::variant(v)?;
        match tag {
            "tex" => read_tex(payload(tag, p)?),
            "text" => read_text(payload(tag, p)?),
            "rect" => read_rect(payload(tag, p)?),
            "path" => read_path(payload(tag, p)?),
            "band" => read_band(payload(tag, p)?),
            "dot" => read_dot(payload(tag, p)?),
            "frame" => {
                let r: FrameRecord = payload(tag, p)?;
                let mob = Mob::Frame {
                    center: r.center.0,
                    height: r.height.0,
                };
                Ok(MobRecord { mob, probe: None })
            }
            other => Err(format!("no mob `{other}`")),
        }
    }
}

/// The mobs a math scene returned, each checked.
fn mobs(list: &Value) -> Result<Vec<Mob>, String> {
    Ok(super::typed::items::<MobRecord>(list, "mob")?
        .into_iter()
        .map(|m| m.mob)
        .collect())
}

/// What the motion check measures of the math scene `name` for `state` at `time` seconds (src/math/check.rs): the
/// mobs it draws, as probes, and the part of the canvas its frame shows. Its natives read no frame (the check
/// renders none); an error is the scene's, never an earlier frame.
pub fn math_probe(
    name: &str,
    state: &ShotState,
    statics: &Statics,
    time: f32,
) -> Result<(Vec<Probe>, (glam::Vec2, f32)), String> {
    let result = with_runtime(|rt| {
        let kind = Kind::Math(name.to_string());
        if rt.hook(&kind).is_none() {
            return Err(format!("the scripts have no math scene `{name}`"));
        }
        let value = rt.call(
            &kind,
            vec![Value::Float(time as f64)],
            state,
            None,
            Some(statics),
        )?;
        let records = super::typed::items::<MobRecord>(&value, "mob")?;
        let frame = Scene {
            mobs: records.iter().map(|r| r.mob.clone()).collect(),
        }
        .frame();
        Ok((records.into_iter().filter_map(|r| r.probe).collect(), frame))
    })?;
    result.unwrap_or_else(|| Err("the investigation has no scripts".into()))
}

/// The scripts' math scenes, by name.
pub fn math_scenes() -> Vec<String> {
    with_runtime(|rt| rt.maths.keys().cloned().collect())
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// What the math scene `name` draws for `state` at `time` seconds, its natives reading `frame` (what the
/// investigation's `build_frame` left, `frame_data`): its mobs, or the error drawn in their place when the call fails in the
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
                Some(Scene { mobs })
            }
            Err(e) => {
                // The viewer shows the error where the scene would be, never an earlier frame of it.
                let shown = Scene::failed(&format!("{label}: {e}"));
                rt.failed(&label, e);
                Some(shown)
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
