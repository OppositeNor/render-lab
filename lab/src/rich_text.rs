//! Text with formulas and code in it: plain text with Project Fluent's
//! function placeables, as the text tables (src/i18n.rs) and the video
//! builder's captions (tools/video/markup.py) write them. The lab draws
//! `{ FORMULA("LaTeX", display: "yes", colors: "#rrggbb,…") }`, typeset by
//! src/formula.rs, and `{ CODE("LightCullingTilePrepare") }`, a code name in
//! the monospace font on a soft chip, both on the text's baseline and in the
//! text's colour. Any other text, braces included, is drawn as it is.
//!
//! A string literal's escapes are Fluent's (`\\`, `\"`, `\u…`), so LaTeX's
//! backslashes are doubled (`\\` is a line break: `\\\\`). egui cannot place
//! images in a text layout, so a line is laid out run by run here.

use crate::formula::{self, FORMULA_PX, Formula, PAD};
use crate::text::Ink;
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Vec2, pos2, vec2};
use fluent_bundle::{FluentArgs, FluentValue};
use fluent_syntax::ast;
use std::sync::Arc;

/// A piece of rich text.
#[derive(Clone, Debug, PartialEq)]
pub enum Run {
    Text(String),
    /// `colors`: the term colours a readout lights its terms in (`colors: "#rrggbb,…"`).
    Formula {
        latex: String,
        display: bool,
        colors: Vec<Color32>,
    },
    /// A code name (`{ CODE("…") }`).
    Code(String),
}

/// The functions rich text calls; a text table's bundle gives each back as written.
pub const FUNCTIONS: [&str; 2] = ["FORMULA", "CODE"];

/// A code span's chip: its padding either side of the text, and its fill.
const CODE_PAD: f32 = 5.0;
fn code_fill(color: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (color.a() as f32 * 0.14) as u8,
    )
}

/// A Fluent string literal of `text`.
pub fn literal(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `{ FORMULA(…) }`: a formula as rich text writes it.
pub fn formula_text(latex: &str, display: bool, colors: &str) -> String {
    let mut args = vec![literal(latex)];
    if display {
        args.push("display: \"yes\"".into());
    }
    if !colors.is_empty() {
        args.push(format!("colors: {}", literal(colors)));
    }
    format!("{{ FORMULA({}) }}", args.join(", "))
}

/// A rich-text function called from a text table's message: the call as rich
/// text writes it, so the resolved message keeps its formulas and code names.
pub fn call(name: &str, positional: &[FluentValue], named: &FluentArgs) -> String {
    let text = |v: &FluentValue| match v {
        FluentValue::String(s) => s.to_string(),
        other => format!("{other:?}"),
    };
    let first = positional.first().map(text).unwrap_or_default();
    match name {
        "CODE" => format!("{{ CODE({}) }}", literal(&first)),
        _ => {
            let named: Vec<(&str, String)> = named.iter().map(|(k, v)| (k, text(v))).collect();
            let get = |key: &str| {
                named
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.as_str())
            };
            formula_text(
                &first,
                get("display") == Some("yes"),
                get("colors").unwrap_or(""),
            )
        }
    }
}

/// Where each rich-text call of `text` starts and ends (byte ranges), whether or not it parses.
fn calls(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = text[i..].find('{').map(|k| i + k) {
        // A brace starts a call only when a known function's name and `(` follow it.
        let rest = text[at + 1..].trim_start();
        if !FUNCTIONS.iter().any(|f| {
            rest.strip_prefix(f)
                .is_some_and(|r| r.trim_start().starts_with('('))
        }) {
            i = at + 1;
            continue;
        }
        // The placeable's end: its closing brace outside a string literal.
        let mut quoted = false;
        let mut chars = text[at + 1..].char_indices();
        let mut end = text.len();
        while let Some((k, c)) = chars.next() {
            match c {
                '\\' if quoted => {
                    chars.next();
                }
                '"' => quoted = !quoted,
                '}' if !quoted => {
                    end = at + 1 + k + 1;
                    break;
                }
                _ => {}
            }
        }
        out.push((at, end));
        i = end;
    }
    out
}

/// Whether a string holds any formula or code name (strings without one keep the plain text path).
pub fn is_rich(text: &str) -> bool {
    !calls(text).is_empty()
}

/// One call's run, parsed as Fluent parses the placeable.
fn run(placeable: &str) -> Result<Run, String> {
    let fail = |why: &str| format!("{why}: {placeable}");
    let source = format!("x = {placeable}\n");
    let resource = fluent_syntax::parser::parse(source.as_str()).map_err(|(_, e)| {
        fail(&format!(
            "not a Fluent placeable ({:?})",
            e.first().map(|e| &e.kind)
        ))
    })?;
    let Some(ast::Entry::Message(ast::Message {
        value: Some(pattern),
        ..
    })) = resource.body.first()
    else {
        return Err(fail("not a Fluent placeable"));
    };
    let [
        ast::PatternElement::Placeable {
            expression:
                ast::Expression::Inline(ast::InlineExpression::FunctionReference { id, arguments }),
        },
    ] = pattern.elements.as_slice()
    else {
        return Err(fail("not one function call"));
    };
    let string = |e: &ast::InlineExpression<&str>| match e {
        ast::InlineExpression::StringLiteral { value } => {
            Ok(fluent_syntax::unicode::unescape_unicode_to_string(value).into_owned())
        }
        _ => Err(fail("an argument is not a string")),
    };
    let [first] = arguments.positional.as_slice() else {
        return Err(fail("one positional argument"));
    };
    let first = string(first)?;
    let mut named = Vec::new();
    for a in &arguments.named {
        named.push((a.name.name, string(&a.value)?));
    }
    let get = |key: &str| {
        named
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    };
    match id.name {
        "CODE" if named.is_empty() => Ok(Run::Code(first)),
        "FORMULA"
            if named
                .iter()
                .all(|(k, _)| matches!(*k, "display" | "colors")) =>
        {
            let colors = get("colors")
                .map(|v| v.split(',').map(hex_color).collect::<Result<Vec<_>, _>>())
                .transpose()?
                .unwrap_or_default();
            Ok(Run::Formula {
                latex: first,
                display: get("display") == Some("yes"),
                colors,
            })
        }
        _ => Err(fail("unknown arguments")),
    }
}

/// The runs of a string: its text, its formulas, and its code names.
pub fn parse(text: &str) -> Result<Vec<Run>, String> {
    let mut runs = Vec::new();
    let mut rest = 0;
    for (start, end) in calls(text) {
        if start > rest {
            runs.push(Run::Text(text[rest..start].to_string()));
        }
        runs.push(run(&text[start..end])?);
        rest = end;
    }
    if rest < text.len() {
        runs.push(Run::Text(text[rest..].to_string()));
    }
    Ok(runs)
}

fn hex_color(text: &str) -> Result<Color32, String> {
    let hex = text
        .trim()
        .strip_prefix('#')
        .filter(|h| h.len() == 6)
        .ok_or_else(|| format!("colour {text:?} is not #rrggbb"))?;
    let v = u32::from_str_radix(hex, 16).map_err(|_| format!("colour {text:?} is not #rrggbb"))?;
    Ok(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// The first formula of a string, if it has one.
pub fn first_formula(text: &str) -> Option<(String, bool, Vec<Color32>)> {
    parse(text).ok()?.into_iter().find_map(|r| match r {
        Run::Formula {
            latex,
            display,
            colors,
        } => Some((latex, display, colors)),
        Run::Text(_) | Run::Code(_) => None,
    })
}

/// The formula size for text of `size`: never under FORMULA_PX, so its scripts keep the lab's minimum.
pub fn formula_size(size: f32) -> f32 {
    size.max(FORMULA_PX)
}

/// An egui texture of one of a formula's masks (`mask` 0: the whole formula;
/// `n`: term `n`), white with the mask as alpha, so drawing tints it.
/// A typeset formula: its LaTeX, its size in pixels, whether it is set as a
/// display (or inline), and what Typst made of it.
#[derive(Clone, Copy)]
pub struct Typeset<'a> {
    pub latex: &'a str,
    pub size: f32,
    pub display: bool,
    pub formula: &'a Formula,
}

fn texture(ctx: &egui::Context, typeset: &Typeset, mask: usize) -> egui::TextureHandle {
    let Typeset {
        latex,
        size,
        display,
        formula: f,
    } = *typeset;
    let id = egui::Id::unique(("formula", latex, size.to_bits(), display, mask));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let coverage = if mask == 0 {
        &f.coverage
    } else {
        &f.terms[mask - 1]
    };
    let rgba: Vec<u8> = coverage.iter().flat_map(|&a| [a, a, a, a]).collect();
    let image =
        egui::ColorImage::from_rgba_premultiplied([f.width as usize, f.height as usize], &rgba);
    let t = ctx.load_texture(
        format!("formula {mask}: {latex}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

/// The lit term of a drawn formula: its number, how lit it is (0..1), and its colour.
#[derive(Clone, Copy, Debug)]
pub struct Lit {
    pub term: usize,
    pub weight: f32,
    pub color: Color32,
}

/// Draws a typeset formula with its top-left corner (padding included) at
/// `at`, `scale` canvas units per pixel: in `color`, or dimmed to `dim` with
/// one term lit.
pub fn draw_formula(
    ink: &Ink,
    at: Pos2,
    typeset: &Typeset,
    scale: f32,
    color: Color32,
    lit: Option<(Lit, Color32)>,
) -> Rect {
    let f = typeset.formula;
    let ctx = ink.painter.ctx();
    let rect = Rect::from_min_size(at, vec2(f.width as f32, f.height as f32) * scale);
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let base = texture(ctx, typeset, 0);
    match lit.filter(|(l, _)| l.term >= 1 && l.term <= f.terms.len() && l.weight > 0.0) {
        Some((l, dim)) => {
            // A lit term: the whole formula dimmed toward `dim`, and the term's own mask over it in its colour.
            ink.painter.image(
                base.id(),
                rect,
                uv,
                crate::graph_view::lerp_color(color, dim, l.weight),
            );
            let term = texture(ctx, typeset, l.term);
            ink.painter
                .image(term.id(), rect, uv, l.color.gamma_multiply(l.weight));
        }
        None => {
            ink.painter.image(base.id(), rect, uv, color);
        }
    }
    ink.min_size
        .set(ink.min_size.get().min(f.min_glyph_px * scale));
    rect
}

/// One laid-out run: its width, its ascent and descent about the line's
/// baseline, and what to draw.
enum Piece {
    Text {
        text: String,
        width: f32,
        ascent: f32,
        height: f32,
    },
    Formula {
        latex: String,
        display: bool,
        formula: Arc<Formula>,
        size: f32,
    },
    /// A formula that failed to typeset, shown as its LaTeX.
    Broken {
        text: String,
        width: f32,
        ascent: f32,
        height: f32,
    },
    /// A code span: its text's metrics in the monospace font (the chip's padding not included).
    Code {
        text: String,
        width: f32,
        ascent: f32,
        height: f32,
    },
}

fn text_metrics(ink: &Ink, text: &str, size: f32, family: &FontFamily) -> (f32, f32, f32) {
    let galley = ink.painter.layout_no_wrap(
        text.to_string(),
        FontId::new(size, family.clone()),
        Color32::WHITE,
    );
    let height = galley.size().y;
    let ascent = galley
        .rows
        .first()
        .and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y))
        .unwrap_or(height * 0.8);
    (galley.size().x, ascent, height)
}

fn pieces(ink: &Ink, text: &str, size: f32, family: &FontFamily) -> Vec<Piece> {
    let runs = parse(text).unwrap_or_else(|e| vec![Run::Text(format!("{text} ({e})"))]);
    runs.into_iter()
        .map(|run| match run {
            Run::Text(t) => {
                let (width, ascent, height) = text_metrics(ink, &t, size, family);
                Piece::Text {
                    text: t,
                    width,
                    ascent,
                    height,
                }
            }
            Run::Code(t) => {
                let (width, ascent, height) = text_metrics(ink, &t, size, &FontFamily::Monospace);
                Piece::Code {
                    text: t,
                    width,
                    ascent,
                    height,
                }
            }
            // In a line of text a formula is set inline, whatever its tag says
            // (display style is for the readout and the formula card).
            Run::Formula { latex, .. } => {
                let fsize = formula_size(size);
                match formula::get(&latex, fsize, false) {
                    Ok(formula) => Piece::Formula {
                        latex,
                        display: false,
                        formula,
                        size: fsize,
                    },
                    Err(e) => {
                        eprintln!("formula {latex:?}: {e}");
                        let (width, ascent, height) = text_metrics(ink, &latex, size, family);
                        Piece::Broken {
                            text: latex,
                            width,
                            ascent,
                            height,
                        }
                    }
                }
            }
        })
        .collect()
}

impl Piece {
    /// Width, ascent above the baseline, and descent below it.
    fn extent(&self) -> (f32, f32, f32) {
        match self {
            Piece::Text {
                width,
                ascent,
                height,
                ..
            }
            | Piece::Broken {
                width,
                ascent,
                height,
                ..
            } => (*width, *ascent, height - ascent),
            Piece::Code {
                width,
                ascent,
                height,
                ..
            } => (width + 2.0 * CODE_PAD, *ascent, height - ascent),
            Piece::Formula { formula, .. } => {
                let pad = PAD as f32;
                (
                    formula.width as f32 - 2.0 * pad,
                    formula.baseline - pad,
                    formula.height as f32 - pad - formula.baseline,
                )
            }
        }
    }
}

/// The size of one line of rich text.
pub fn measure(ink: &Ink, text: &str, size: f32, family: FontFamily) -> Vec2 {
    if !is_rich(text) {
        return ink.measure(text, size, family);
    }
    let (w, a, d) = pieces(ink, text, size, &family)
        .iter()
        .map(Piece::extent)
        .fold((0.0f32, 0.0f32, 0.0f32), |(w, a, d), (pw, pa, pd)| {
            (w + pw, a.max(pa), d.max(pd))
        });
    vec2(w, a + d)
}

/// Draws one line of rich text at `pos`, aligned like `Ink::text`; returns its rect.
pub fn text(
    ink: &Ink,
    pos: Pos2,
    align: Align2,
    text: &str,
    size: f32,
    family: FontFamily,
    color: Color32,
) -> Rect {
    if !is_rich(text) {
        return ink.text(pos, align, text, size, family, color);
    }
    let pieces = pieces(ink, text, size, &family);
    let (width, ascent, descent) = pieces
        .iter()
        .map(Piece::extent)
        .fold((0.0f32, 0.0f32, 0.0f32), |(w, a, d), (pw, pa, pd)| {
            (w + pw, a.max(pa), d.max(pd))
        });
    let rect = align.anchor_size(pos, vec2(width, ascent + descent));
    let baseline = rect.min.y + ascent;
    let mut x = rect.min.x;
    for piece in &pieces {
        // The pieces sit on one baseline, left to right.
        let (w, a, _) = piece.extent();
        match piece {
            Piece::Text { text, .. } | Piece::Broken { text, .. } => {
                ink.text(
                    pos2(x, baseline - a),
                    Align2::LEFT_TOP,
                    text,
                    size,
                    family.clone(),
                    color,
                );
            }
            Piece::Formula {
                latex,
                display,
                formula,
                size: fsize,
            } => {
                let pad = PAD as f32;
                draw_formula(
                    ink,
                    pos2(x - pad, baseline - formula.baseline),
                    &Typeset {
                        latex,
                        size: *fsize,
                        display: *display,
                        formula,
                    },
                    1.0,
                    color,
                    None,
                );
            }
            Piece::Code {
                text,
                ascent,
                height,
                ..
            } => {
                let chip = Rect::from_min_size(
                    pos2(x + 1.0, baseline - ascent - 1.0),
                    vec2(w - 2.0, height + 2.0),
                );
                ink.painter
                    .rect_filled(chip, egui::CornerRadius::same(4), code_fill(color));
                ink.text(
                    pos2(x + CODE_PAD, baseline - ascent),
                    Align2::LEFT_TOP,
                    text,
                    size,
                    FontFamily::Monospace,
                    color,
                );
            }
        }
        x += w;
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formulas() {
        let runs = parse(r#"blend with { FORMULA("\\alpha = \\frac{1}{1+n}") }, done"#).unwrap();
        assert_eq!(
            runs,
            vec![
                Run::Text("blend with ".into()),
                Run::Formula {
                    latex: r"\alpha = \frac{1}{1+n}".into(),
                    display: false,
                    colors: vec![]
                },
                Run::Text(", done".into())
            ]
        );
        let runs =
            parse(r##"{ FORMULA("a \\\\ \"b\"", display: "yes", colors: "#ff0000,#00ff00") }"##)
                .unwrap();
        assert_eq!(
            runs,
            vec![Run::Formula {
                latex: r#"a \\ "b""#.into(),
                display: true,
                colors: vec![Color32::from_rgb(255, 0, 0), Color32::from_rgb(0, 255, 0)]
            }]
        );
    }

    #[test]
    fn parses_code_names_and_keeps_other_braces() {
        let runs =
            parse(r#"then { CODE("LightCullingTilePrepare") } adds {x} and { $y }"#).unwrap();
        assert_eq!(
            runs,
            vec![
                Run::Text("then ".into()),
                Run::Code("LightCullingTilePrepare".into()),
                Run::Text(" adds {x} and { $y }".into())
            ]
        );
        assert!(!is_rich("a {b} c"));
    }

    #[test]
    fn writes_what_it_reads() {
        let text = formula_text(r#"a \\ "b""#, true, "#ff0000");
        assert_eq!(
            text,
            r##"{ FORMULA("a \\\\ \"b\"", display: "yes", colors: "#ff0000") }"##
        );
        assert_eq!(
            parse(&text).unwrap(),
            vec![Run::Formula {
                latex: r#"a \\ "b""#.into(),
                display: true,
                colors: vec![Color32::from_rgb(255, 0, 0)]
            }]
        );
    }

    #[test]
    fn malformed_calls_fail() {
        assert!(parse(r#"x { FORMULA("a) }"#).is_err());
        assert!(parse(r#"x { FORMULA("a", size: "2") }"#).is_err());
        assert!(parse(r#"x { CODE(1) }"#).is_err());
    }
}
