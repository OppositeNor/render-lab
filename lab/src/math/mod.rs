//! The math canvas: what a math scene (a package's `Math` hook, packages/render_lab/language/runtime.lam) draws
//! in the middle of the frame instead of the 3D scene, on a `kind=2d math=<name>` beat of a story or for a
//! demonstration whose file names one (`math = "toyMath"`).
//!
//! The canvas is a plane in canvas units, 8 high and as wide as the frame's aspect makes it, its origin at the
//! centre and +y up (manim's frame); a scene's `frame` mob moves and zooms the part the frame shows. A scene is
//! built in Lamlet by the library `render_lab_math` (packages/render_lab/language/math.lam), which lays out by the
//! sizes measured here (`tex_box`, `text_width`), and the lab draws its flat list of mobs (`draw`).

use crate::formula::{self, Formula};
use egui::{Color32, Pos2, Rect, Vec2, vec2};
use std::sync::Arc;

mod draw;
mod layout;
pub use draw::draw;
pub use layout::{LAYOUTS, layout as graph_layout};

/// The canvas's height in canvas units when no scene sets its frame.
pub const FRAME_HEIGHT: f32 = 8.0;

/// One thing a math scene draws (runtime.lam's `Mob`), decoded and checked before any is drawn.
#[derive(Clone, Debug)]
pub enum Mob {
    /// A part of a formula: its LaTeX, which part (`Part`), where the whole formula's centre is, its em in canvas
    /// units, its colour (alpha in it), and how much of the part is written, from the left (`lo`, `hi`: the
    /// part's box about `at`, the wipe runs across it).
    Tex {
        latex: String,
        part: Part,
        at: glam::Vec2,
        size: f32,
        color: Color32,
        wipe: f32,
        lo: glam::Vec2,
        hi: glam::Vec2,
    },
    /// A text centred on `at`, an em `size` canvas units high.
    Text {
        text: String,
        at: glam::Vec2,
        size: f32,
        color: Color32,
    },
    /// A rectangle, filled and outlined (`width` pixels at 1080p), its corners rounded by `radius` canvas units.
    Rect {
        lo: glam::Vec2,
        hi: glam::Vec2,
        fill: Color32,
        stroke: Color32,
        width: f32,
        radius: f32,
    },
    /// A path through `points`, drawn up to the fraction `drawn` of its length.
    Path {
        points: Vec<glam::Vec2>,
        color: Color32,
        width: f32,
        drawn: f32,
        arrow: bool,
        dashed: bool,
    },
    /// The area between two lines of as many points.
    Band {
        top: Vec<glam::Vec2>,
        bottom: Vec<glam::Vec2>,
        color: Color32,
    },
    Dot {
        at: glam::Vec2,
        radius: f32,
        color: Color32,
    },
    /// The part of the canvas the frame shows: its centre and its height.
    Frame { center: glam::Vec2, height: f32 },
}

/// Which part of a formula a `Mob::Tex` draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// The whole formula.
    Whole,
    /// Its ink outside the `\lit{n}{…}` terms.
    Rest,
    /// Term n (1, 2, …).
    Term(usize),
}

impl Part {
    /// The part a scene names: -1 the whole, 0 the rest, n term n.
    pub fn of(n: i64) -> Result<Part, String> {
        match n {
            -1 => Ok(Part::Whole),
            0 => Ok(Part::Rest),
            n if n > 0 => Ok(Part::Term(n as usize)),
            n => Err(format!(
                "no part {n} of a formula (-1 the whole, 0 the rest, n a term)"
            )),
        }
    }
}

/// What a math scene drew at a frame: its mobs, in drawing order.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub mobs: Vec<Mob>,
}

impl Scene {
    /// The part of the canvas the frame shows: the last `frame` mob's, or the canvas's centre, 8 high.
    pub fn frame(&self) -> (glam::Vec2, f32) {
        self.mobs
            .iter()
            .rev()
            .find_map(|m| match m {
                Mob::Frame { center, height } => Some((*center, height.max(0.01))),
                _ => None,
            })
            .unwrap_or((glam::Vec2::ZERO, FRAME_HEIGHT))
    }
}

/// Where the canvas lands in a rectangle of the screen: canvas units to points, y flipped.
#[derive(Clone, Copy, Debug)]
pub struct Canvas {
    pub rect: Rect,
    pub center: glam::Vec2,
    /// Points per canvas unit.
    pub scale: f32,
}

impl Canvas {
    /// The canvas over `rect`, showing `height` canvas units about `center`.
    pub fn new(rect: Rect, center: glam::Vec2, height: f32) -> Canvas {
        Canvas {
            rect,
            center,
            scale: rect.height() / height,
        }
    }

    pub fn to_screen(&self, p: glam::Vec2) -> Pos2 {
        let d = (p - self.center) * self.scale;
        self.rect.center() + vec2(d.x, -d.y)
    }

    /// The canvas units visible across the rectangle.
    pub fn width(&self) -> f32 {
        self.rect.width() / self.scale
    }
}

/// The math scene the state's demonstration shows in place of its 3D scene (its file's `math`), if any.
pub fn demo_scene(state: &crate::shot::ShotState) -> Option<&'static str> {
    crate::investigation::current()
        .demo_specs()?
        .specs
        .iter()
        .find(|s| s.id == state.demo)?
        .math
}

// ---------------------------------------------------------------------------------------------------------
// Measuring

/// The size formulas are measured at (their boxes are in em, so the size only sets the precision).
const MEASURE_PX: f32 = formula::FORMULA_PX;

/// A formula's measured box: the bitmap's size, and each part's ink box (index 0 the rest, n term n), in em,
/// about the bitmap's centre, +y up; an empty part's box is at the centre with no size.
#[derive(Clone, Debug, PartialEq)]
pub struct TexBox {
    pub width: f32,
    pub height: f32,
    pub parts: Vec<(glam::Vec2, glam::Vec2)>,
}

/// The ink of the formula outside its terms: its coverage less the most any term covers.
pub fn rest_mask(f: &Formula) -> Vec<u8> {
    f.coverage
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let terms = f.terms.iter().map(|t| t[i]).max().unwrap_or(0);
            c.saturating_sub(terms)
        })
        .collect()
}

/// The box of a mask's ink (coverage above a quarter), in pixels: min and max corners, or None when it has none.
fn ink_box(mask: &[u8], width: u32) -> Option<(Vec2, Vec2)> {
    let mut lo = vec2(f32::INFINITY, f32::INFINITY);
    let mut hi = vec2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (i, &c) in mask.iter().enumerate() {
        // A pixel counts toward the box when it is more than faintly covered.
        if c > 64 {
            let (x, y) = ((i as u32 % width) as f32, (i as u32 / width) as f32);
            lo = lo.min(vec2(x, y));
            hi = hi.max(vec2(x + 1.0, y + 1.0));
        }
    }
    lo.x.is_finite().then_some((lo, hi))
}

/// The measured box of `latex`, typeset for display.
pub fn tex_box(latex: &str) -> Result<TexBox, String> {
    let f = formula::get(latex, MEASURE_PX, true)?;
    let half = vec2(f.width as f32, f.height as f32) * 0.5;
    // Pixels from the top left to em about the centre, +y up.
    let em = |p: Vec2| glam::Vec2::new(p.x - half.x, half.y - p.y) / MEASURE_PX;
    let boxed = |mask: &[u8]| match ink_box(mask, f.width) {
        Some((lo, hi)) => (em(vec2(lo.x, hi.y)), em(vec2(hi.x, lo.y))),
        None => (glam::Vec2::ZERO, glam::Vec2::ZERO),
    };
    let mut parts = vec![boxed(&rest_mask(&f))];
    parts.extend(f.terms.iter().map(|t| boxed(t)));
    Ok(TexBox {
        width: f.width as f32 / MEASURE_PX,
        height: f.height as f32 / MEASURE_PX,
        parts,
    })
}

/// The width of a line of text in em, in the lab's proportional font (Chinese in its fallback).
pub fn text_width(text: &str) -> f32 {
    use egui::epaint::text::{Fonts, TextOptions};
    thread_local! {
        static FONTS: std::cell::RefCell<Option<Fonts>> = const { std::cell::RefCell::new(None) };
    }
    const PX: f32 = 100.0;
    FONTS.with(|cell| {
        let mut cell = cell.borrow_mut();
        let fonts = cell.get_or_insert_with(|| {
            Fonts::new(TextOptions::default(), crate::text::font_definitions())
        });
        let galley = fonts.with_pixels_per_point(1.0).layout_no_wrap(
            text.to_string(),
            egui::FontId::proportional(PX),
            Color32::WHITE,
        );
        galley.size().x / PX
    })
}

/// `count` numbers in [0, 1) drawn in proportion to `masses` (even bins across [0, 1)), seeded: each a uniform
/// number (a seeded xorshift) through the inverse of the masses' running total, linear within its bin.
pub fn samples_from(masses: &[f64], seed: u64, count: usize) -> Result<Vec<f64>, String> {
    if masses.iter().any(|m| m.is_nan() || *m < 0.0) {
        return Err("a distribution's masses are not negative".into());
    }
    let total: f64 = masses.iter().sum();
    if total <= 0.0 {
        return Err("a distribution's masses are not all 0".into());
    }
    let mut cdf = Vec::with_capacity(masses.len());
    let mut run = 0.0;
    for m in masses {
        run += m / total;
        cdf.push(run);
    }
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let bins = masses.len() as f64;
    Ok((0..count)
        .map(|_| {
            // xorshift64*, then the first bin whose running total reaches it, and the place within that bin.
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let u = (state.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64;
            let i = cdf.partition_point(|c| *c < u).min(masses.len() - 1);
            let before = if i == 0 {
                0.0
            } else {
                cdf[i - 1]
            };
            let within = ((u - before) / (cdf[i] - before).max(1e-12)).clamp(0.0, 1.0);
            (i as f64 + within) / bins
        })
        .collect())
}

/// The formula `latex` typeset for drawing at about `px` pixels an em: sizes are rounded to twelfths of an octave
/// (about 6 %), so a zoom typesets each size once, and the drawing scales the rest of the way.
pub fn typeset_near(latex: &str, px: f32) -> Result<(Arc<Formula>, f32), String> {
    let steps = (px.max(1.0).log2() * 12.0).round();
    let near = (steps / 12.0).exp2();
    Ok((formula::get(latex, near, true)?, near))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn a_formula_s_parts_are_measured_where_they_are() {
        let b = tex_box(r"\lit{1}{a} + \lit{2}{b}").unwrap();
        assert_eq!(b.parts.len(), 3);
        let (a, plus, bb) = (b.parts[1], b.parts[0], b.parts[2]);
        // a, then +, then b, left to right, on one line about the centre.
        assert!(
            a.1.x <= plus.0.x + 0.05 && plus.1.x <= bb.0.x + 0.05,
            "{b:?}"
        );
        assert!(a.0.x < 0.0 && bb.1.x > 0.0);
        assert!(a.0.y < a.1.y && a.1.y > 0.0 && a.0.y < 0.3);
        assert!(b.width > bb.1.x - a.0.x);
    }

    #[test]
    fn samples_follow_their_masses() {
        let s = samples_from(&[1.0, 3.0], 7, 4000).unwrap();
        let right = s.iter().filter(|x| **x >= 0.5).count() as f64 / s.len() as f64;
        assert!((right - 0.75).abs() < 0.03, "{right}");
        assert!(s.iter().all(|x| (0.0..1.0).contains(x)));
        assert_eq!(s, samples_from(&[1.0, 3.0], 7, 4000).unwrap());
        assert!(samples_from(&[0.0, 0.0], 1, 1).is_err());
    }

    #[test]
    fn text_is_measured_in_em() {
        let one = text_width("m");
        let three = text_width("mmm");
        assert!(one > 0.3 && one < 1.5, "{one}");
        assert!((three - 3.0 * one).abs() < 0.05, "{one} {three}");
        assert!(text_width("中") > 0.5);
    }

    #[test]
    fn the_canvas_is_y_up_about_its_centre() {
        let c = Canvas::new(
            Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 800.0)),
            glam::Vec2::ZERO,
            8.0,
        );
        assert_eq!(c.to_screen(glam::Vec2::ZERO), pos2(800.0, 400.0));
        assert_eq!(c.to_screen(glam::Vec2::new(1.0, 1.0)), pos2(900.0, 300.0));
        assert_eq!(c.width(), 16.0);
    }
}
