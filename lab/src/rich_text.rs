//! Text with formulas in it. A string may hold tags, `{@ name=value …}`; the
//! lab draws the formula tags, `{@ formula="LaTeX"}`, typeset by
//! src/formula.rs, on the text's baseline and in the text's colour. Other
//! tags (a storyboard's cues) are the video builder's and are not drawn.
//!
//! Inside a quoted value only `\"` is an escape, so LaTeX keeps its
//! backslashes as written (`\\` is a line break). egui cannot place images in
//! a text layout, so a line is laid out run by run here.

use crate::formula::{self, Formula, FORMULA_PX, PAD};
use crate::text::Ink;
use egui::{pos2, vec2, Align2, Color32, FontFamily, FontId, Pos2, Rect, Vec2};
use std::sync::Arc;

/// A piece of rich text.
#[derive(Clone, Debug, PartialEq)]
pub enum Run {
    Text(String),
    /// `colors`: the term colours a readout lights its terms in (`colors="#rrggbb,…"`).
    Formula { latex: String, display: bool, colors: Vec<Color32> },
}

/// Whether a string holds any tag (strings without one keep the plain text path).
pub fn has_tags(text: &str) -> bool {
    text.contains("{@")
}

/// A tag's attributes, in order, from just after `{@` to its closing `}`.
/// Returns the attributes and the byte index past the tag.
pub fn parse_tag(text: &str, start: usize) -> Result<(Vec<(String, String)>, usize), String> {
    let bytes = text.as_bytes();
    let mut i = start;
    let mut attrs = Vec::new();
    loop {
        while bytes.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
            i += 1;
        }
        match bytes.get(i) {
            None => return Err(format!("unclosed tag: {}", &text[start.saturating_sub(2)..])),
            Some(b'}') => return Ok((attrs, i + 1)),
            _ => {}
        }
        let name_start = i;
        while bytes.get(i).is_some_and(|b| !b.is_ascii_whitespace() && *b != b'=' && *b != b'}') {
            i += 1;
        }
        let name = text[name_start..i].to_string();
        if bytes.get(i) != Some(&b'=') {
            // A bare word: a named cue (`{@intro}`), kept with an empty value.
            attrs.push((name, String::new()));
            continue;
        }
        i += 1;
        let value = if bytes.get(i) == Some(&b'"') {
            i += 1;
            let mut value = String::new();
            loop {
                match bytes.get(i) {
                    None => return Err(format!("unclosed quote in tag: {}", &text[start.saturating_sub(2)..])),
                    Some(b'"') => {
                        i += 1;
                        break;
                    }
                    Some(b'\\') if bytes.get(i + 1) == Some(&b'"') => {
                        value.push('"');
                        i += 2;
                    }
                    Some(_) => {
                        let c = text[i..].chars().next().unwrap();
                        value.push(c);
                        i += c.len_utf8();
                    }
                }
            }
            value
        } else {
            let value_start = i;
            while bytes.get(i).is_some_and(|b| !b.is_ascii_whitespace() && *b != b'}') {
                i += 1;
            }
            text[value_start..i].to_string()
        };
        attrs.push((name, value));
    }
}

/// The runs of a string: its text, and its formula tags. Other tags are dropped.
pub fn parse(text: &str) -> Result<Vec<Run>, String> {
    let mut runs = Vec::new();
    let mut plain = String::new();
    let mut rest = 0;
    while let Some(at) = text[rest..].find("{@").map(|i| rest + i) {
        plain.push_str(&text[rest..at]);
        let (attrs, end) = parse_tag(text, at + 2)?;
        if let Some((_, latex)) = attrs.iter().find(|(k, _)| k == "formula") {
            if latex != "off" {
                if !plain.is_empty() {
                    runs.push(Run::Text(std::mem::take(&mut plain)));
                }
                let display = attrs.iter().any(|(k, v)| k == "display" && v == "yes");
                let colors = attrs.iter().find(|(k, _)| k == "colors").map(|(_, v)| v.split(',').map(hex_color).collect::<Result<Vec<_>, _>>()).transpose()?.unwrap_or_default();
                runs.push(Run::Formula { latex: latex.clone(), display, colors });
            }
        }
        rest = end;
    }
    plain.push_str(&text[rest..]);
    if !plain.is_empty() {
        runs.push(Run::Text(plain));
    }
    Ok(runs)
}

fn hex_color(text: &str) -> Result<Color32, String> {
    let hex = text.trim().strip_prefix('#').filter(|h| h.len() == 6).ok_or_else(|| format!("colour {text:?} is not #rrggbb"))?;
    let v = u32::from_str_radix(hex, 16).map_err(|_| format!("colour {text:?} is not #rrggbb"))?;
    Ok(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// The first formula of a string, if it has one.
pub fn first_formula(text: &str) -> Option<(String, bool, Vec<Color32>)> {
    parse(text).ok()?.into_iter().find_map(|r| match r {
        Run::Formula { latex, display, colors } => Some((latex, display, colors)),
        Run::Text(_) => None,
    })
}

/// The formula size for text of `size`: never under FORMULA_PX, so its scripts keep the lab's minimum.
pub fn formula_size(size: f32) -> f32 {
    size.max(FORMULA_PX)
}

/// An egui texture of one of a formula's masks (`mask` 0: the whole formula;
/// `n`: term `n`), white with the mask as alpha, so drawing tints it.
fn texture(ctx: &egui::Context, latex: &str, size: f32, display: bool, f: &Formula, mask: usize) -> egui::TextureHandle {
    let id = egui::Id::unique(("formula", latex, size.to_bits(), display, mask));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let coverage = if mask == 0 { &f.coverage } else { &f.terms[mask - 1] };
    let rgba: Vec<u8> = coverage.iter().flat_map(|&a| [a, a, a, a]).collect();
    let image = egui::ColorImage::from_rgba_premultiplied([f.width as usize, f.height as usize], &rgba);
    let t = ctx.load_texture(format!("formula {mask}: {latex}"), image, egui::TextureOptions::LINEAR);
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
pub fn draw_formula(ink: &Ink, at: Pos2, latex: &str, size: f32, display: bool, f: &Formula, scale: f32, color: Color32, lit: Option<(Lit, Color32)>) -> Rect {
    let ctx = ink.painter.ctx();
    let rect = Rect::from_min_size(at, vec2(f.width as f32, f.height as f32) * scale);
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let base = texture(ctx, latex, size, display, f, 0);
    match lit.filter(|(l, _)| l.term >= 1 && l.term <= f.terms.len() && l.weight > 0.0) {
        Some((l, dim)) => {
            ink.painter.image(base.id(), rect, uv, lerp_color(color, dim, l.weight));
            let term = texture(ctx, latex, size, display, f, l.term);
            ink.painter.image(term.id(), rect, uv, l.color.gamma_multiply(l.weight));
        }
        None => {
            ink.painter.image(base.id(), rect, uv, color);
        }
    }
    ink.min_size.set(ink.min_size.get().min(f.min_glyph_px * scale));
    rect
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()), mix(a.a(), b.a()))
}

/// One laid-out run: its width, its ascent and descent about the line's
/// baseline, and what to draw.
enum Piece {
    Text { text: String, width: f32, ascent: f32, height: f32 },
    Formula { latex: String, display: bool, formula: Arc<Formula>, size: f32 },
    /// A formula that failed to typeset, shown as its LaTeX.
    Broken { text: String, width: f32, ascent: f32, height: f32 },
}

fn text_metrics(ink: &Ink, text: &str, size: f32, family: &FontFamily) -> (f32, f32, f32) {
    let galley = ink.painter.layout_no_wrap(text.to_string(), FontId::new(size, family.clone()), Color32::WHITE);
    let height = galley.size().y;
    let ascent = galley.rows.first().and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y)).unwrap_or(height * 0.8);
    (galley.size().x, ascent, height)
}

fn pieces(ink: &Ink, text: &str, size: f32, family: &FontFamily) -> Vec<Piece> {
    let runs = parse(text).unwrap_or_else(|e| vec![Run::Text(format!("{text} ({e})"))]);
    runs.into_iter()
        .map(|run| match run {
            Run::Text(t) => {
                let (width, ascent, height) = text_metrics(ink, &t, size, family);
                Piece::Text { text: t, width, ascent, height }
            }
            // In a line of text a formula is set inline, whatever its tag says
            // (display style is for the readout and the formula card).
            Run::Formula { latex, .. } => {
                let fsize = formula_size(size);
                match formula::get(&latex, fsize, false) {
                    Ok(formula) => Piece::Formula { latex, display: false, formula, size: fsize },
                    Err(e) => {
                        eprintln!("formula {latex:?}: {e}");
                        let (width, ascent, height) = text_metrics(ink, &latex, size, family);
                        Piece::Broken { text: latex, width, ascent, height }
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
            Piece::Text { width, ascent, height, .. } | Piece::Broken { width, ascent, height, .. } => (*width, *ascent, height - ascent),
            Piece::Formula { formula, .. } => {
                let pad = PAD as f32;
                (formula.width as f32 - 2.0 * pad, formula.baseline - pad, formula.height as f32 - pad - formula.baseline)
            }
        }
    }
}

/// The size of one line of rich text.
pub fn measure(ink: &Ink, text: &str, size: f32, family: FontFamily) -> Vec2 {
    if !has_tags(text) {
        return ink.measure(text, size, family);
    }
    let (w, a, d) = pieces(ink, text, size, &family).iter().map(Piece::extent).fold((0.0f32, 0.0f32, 0.0f32), |(w, a, d), (pw, pa, pd)| (w + pw, a.max(pa), d.max(pd)));
    vec2(w, a + d)
}

/// Draws one line of rich text at `pos`, aligned like `Ink::text`; returns its rect.
pub fn text(ink: &Ink, pos: Pos2, align: Align2, text: &str, size: f32, family: FontFamily, color: Color32) -> Rect {
    if !has_tags(text) {
        return ink.text(pos, align, text, size, family, color);
    }
    let pieces = pieces(ink, text, size, &family);
    let (width, ascent, descent) = pieces.iter().map(Piece::extent).fold((0.0f32, 0.0f32, 0.0f32), |(w, a, d), (pw, pa, pd)| (w + pw, a.max(pa), d.max(pd)));
    let rect = align.anchor_size(pos, vec2(width, ascent + descent));
    let baseline = rect.min.y + ascent;
    let mut x = rect.min.x;
    for piece in &pieces {
        let (w, a, _) = piece.extent();
        match piece {
            Piece::Text { text, .. } | Piece::Broken { text, .. } => {
                ink.text(pos2(x, baseline - a), Align2::LEFT_TOP, text, size, family.clone(), color);
            }
            Piece::Formula { latex, display, formula, size: fsize } => {
                let pad = PAD as f32;
                draw_formula(ink, pos2(x - pad, baseline - formula.baseline), latex, *fsize, *display, formula, 1.0, color, None);
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
    fn parses_formula_tags_and_drops_cues() {
        let runs = parse(r#"blend {@ hfRay=1 callout=x}with {@ formula="\alpha = \frac{1}{1+n}" say="alpha"}, done"#).unwrap();
        assert_eq!(
            runs,
            vec![Run::Text("blend with ".into()), Run::Formula { latex: r"\alpha = \frac{1}{1+n}".into(), display: false, colors: vec![] }, Run::Text(", done".into())]
        );
    }

    #[test]
    fn quoted_values_keep_backslashes() {
        let (attrs, end) = parse_tag(r#"{@ formula="a \\ \"b\"" display=yes}x"#, 2).unwrap();
        assert_eq!(attrs, vec![("formula".into(), r#"a \\ "b""#.into()), ("display".into(), "yes".into())]);
        assert_eq!(end, r#"{@ formula="a \\ \"b\"" display=yes}"#.len());
    }

    #[test]
    fn unclosed_tags_fail() {
        assert!(parse(r#"x {@ formula="a"#).is_err());
        assert!(parse("x {@ a=1").is_err());
    }
}
