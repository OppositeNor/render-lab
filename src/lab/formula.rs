//! Formulas: LaTeX math typeset at run time into coverage masks, which the
//! lab tints with its own colours when it draws them (captions, readouts,
//! notes, labels, and the formula card; src/lab/rich_text.rs).
//!
//! MiTeX translates the LaTeX into Typst math, evaluated in MiTeX's own Typst
//! scope (assets/mitex/specs, Apache-2.0); Typst typesets it with its bundled
//! New Computer Modern fonts, and typst-render rasterizes it. The same LaTeX at
//! the same size always gives the same bytes, so frames stay deterministic.
//!
//! Terms are marked `\lit{n}{…}` (n = 1, 2, …). Besides the whole formula's
//! coverage, each term gets its own mask, so a drawing can dim the formula and
//! light one term.

use crate::lab::resources;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::layout::{Frame, FrameItem};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

/// The size formulas are drawn at, in pixels at 1080p: KaTeX's 28.4 px times
/// its stylesheet's 1.21, so the readouts keep their size, and scripts (70 %)
/// keep the lab's 24 px minimum.
pub const FORMULA_PX: f32 = 34.4;
/// The smallest glyph a formula may have, and the widest a display formula may be (px at 1080p).
pub const MIN_GLYPH_PX: f32 = 24.0;
pub const MAX_DISPLAY_WIDTH: u32 = 546;
/// The gap between the rows of a multi-line formula (`aligned`), in em: as
/// KaTeX spaces them, so the readouts keep their height.
const LEADING: f32 = 0.5;
/// Transparent pixels around the ink: Typst's frames do not always enclose it.
pub const PAD: u32 = 12;

/// A typeset formula: `width` × `height` coverage, row by row, its baseline
/// (pixels from the top), and one mask per `\lit` term.
pub struct Formula {
    pub width: u32,
    pub height: u32,
    pub baseline: f32,
    pub coverage: Vec<u8>,
    /// `terms[n - 1]` is the coverage of term `n`.
    pub terms: Vec<Vec<u8>>,
    /// The smallest glyph, in pixels.
    pub min_glyph_px: f32,
}

/// The formula of `latex` at `size_px`, typeset on first use and kept.
/// `display` lays it out on its own (fractions and sums full size, `aligned`
/// rows); otherwise it is laid out inline, on a text line.
pub fn get(latex: &str, size_px: f32, display: bool) -> Result<Arc<Formula>, String> {
    static CACHE: OnceLock<Mutex<HashMap<(String, u32, bool), Arc<Formula>>>> = OnceLock::new();
    let key = (latex.to_string(), size_px.to_bits(), display);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(f) = cache.lock().unwrap().get(&key) {
        return Ok(f.clone());
    }
    let formula = Arc::new(typeset(latex, size_px, display)?);
    cache.lock().unwrap().insert(key, formula.clone());
    Ok(formula)
}

/// Checks a formula against the lab's limits: it typesets, its glyphs are at
/// least `MIN_GLYPH_PX`, and a display formula is at most `MAX_DISPLAY_WIDTH` wide.
pub fn check(latex: &str, display: bool) -> Result<Arc<Formula>, String> {
    let f = get(latex, FORMULA_PX, display)?;
    if f.min_glyph_px < MIN_GLYPH_PX - 0.05 {
        return Err(format!("its smallest glyph is {:.1} px (< {MIN_GLYPH_PX}); avoid nested scripts", f.min_glyph_px));
    }
    let ink = f.width - 2 * PAD;
    if display && ink > MAX_DISPLAY_WIDTH {
        return Err(format!("{ink} px wide (> {MAX_DISPLAY_WIDTH}); break it over more lines"));
    }
    Ok(f)
}

/// Every formula tag in the strings of these JSON files, checked: one entry
/// per distinct formula, with its file, size, smallest glyph, and any error.
pub fn check_files(paths: &[std::path::PathBuf]) -> anyhow::Result<Vec<serde_json::Value>> {
    fn strings<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a str>) {
        match v {
            serde_json::Value::String(s) => out.push(s),
            serde_json::Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
            serde_json::Value::Object(o) => o.values().for_each(|x| strings(x, out)),
            _ => {}
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut report = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let mut all = Vec::new();
        strings(&json, &mut all);
        for s in all.into_iter().filter(|s| s.contains("{@")) {
            let runs = match crate::lab::rich_text::parse(s) {
                Ok(runs) => runs,
                Err(e) => {
                    report.push(serde_json::json!({"file": path, "text": s, "error": e}));
                    continue;
                }
            };
            for run in runs {
                let crate::lab::rich_text::Run::Formula { latex, display, .. } = run else { continue };
                if !seen.insert((latex.clone(), display)) {
                    continue;
                }
                let entry = match check(&latex, display) {
                    Ok(f) => serde_json::json!({"file": path, "latex": latex, "display": display, "size": [f.width - 2 * PAD, f.height - 2 * PAD], "minGlyphPx": f.min_glyph_px, "terms": f.terms.len(), "error": null}),
                    Err(e) => serde_json::json!({"file": path, "latex": latex, "display": display, "error": e}),
                };
                report.push(entry);
            }
        }
    }
    Ok(report)
}

struct FormulaWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    files: HashMap<FileId, Bytes>,
    main: FileId,
}

fn file_id(path: &str) -> FileId {
    RootedPath::new(VirtualRoot::Project, VirtualPath::new(path).expect("a valid virtual path")).intern()
}

/// MiTeX's Typst scope (assets/mitex/specs), at the paths its files import each other by.
const SPECS: [(&str, &str); 3] = [("/specs/mod.typ", "mitex/mod"), ("/specs/prelude.typ", "mitex/prelude"), ("/specs/latex/standard.typ", "mitex/latex/standard")];

impl FormulaWorld {
    fn new() -> FormulaWorld {
        let fonts: Vec<Font> = typst_assets::fonts().flat_map(|data| Font::iter(Bytes::new(data))).collect();
        let book = FontBook::from_fonts(&fonts);
        let files = SPECS.iter().map(|(path, spec)| (file_id(path), Bytes::from_string(resources::get(spec).text()))).collect();
        FormulaWorld { library: LazyHash::new(Library::default()), book: LazyHash::new(book), fonts, files, main: file_id("/main.typ") }
    }
}

impl World for FormulaWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        let bytes = self.file(id)?;
        Ok(Source::new(id, String::from_utf8(bytes.to_vec()).map_err(|_| FileError::InvalidUtf8)?))
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.get(&id).cloned().ok_or_else(|| FileError::NotFound(PathBuf::from(format!("{id:?}"))))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// The arguments of `\lit{n}{body}` starting at `at` (just past `\lit`): n, body, and the end.
fn lit_args(latex: &str, at: usize) -> Result<(u32, &str, usize), String> {
    let bytes = latex.as_bytes();
    let mut args = Vec::new();
    let mut j = at;
    for _ in 0..2 {
        if bytes.get(j) != Some(&b'{') {
            return Err("\\lit takes two arguments: \\lit{n}{…}".into());
        }
        let start = j + 1;
        let mut depth = 0;
        loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Some(_) => {}
                None => return Err("unbalanced braces in \\lit".into()),
            }
            j += 1;
        }
        args.push(&latex[start..j]);
        j += 1;
    }
    let n = args[0].trim().parse::<u32>().map_err(|_| format!("\\lit{{{}}}: the term is a number from 1", args[0]))?;
    Ok((n, args[1], j))
}

/// The LaTeX MiTeX is given: `\lit{n}{body}` becomes `{body}`, or with term
/// `lit` coloured red so its coverage can be told apart; a line break's
/// spacing argument (`\\[3pt]`), which MiTeX would print, is dropped; and a
/// braced operator (`N{\cdot}L`), which LaTeX sets as an ordinary symbol
/// without spaces, becomes `\mathord`, since MiTeX drops the braces.
/// Returns the LaTeX and the number of terms.
fn prepare(latex: &str, lit: Option<u32>) -> Result<(String, u32), String> {
    let mut out = String::new();
    let mut terms = 0;
    let latex = ["\\cdot", "-", "+"].iter().fold(latex.to_string(), |s, op| s.replace(&format!("{{{op}}}"), &format!("\\mathord{{{op}}}")));
    let mut rest = latex.as_str();
    loop {
        let lit_at = rest.find("\\lit{");
        let break_at = rest.find("\\\\[");
        match (lit_at, break_at) {
            (Some(i), b) if b.is_none_or(|b| i < b) => {
                out.push_str(&rest[..i]);
                let (n, body, end) = lit_args(rest, i + 4)?;
                let (body, inner) = prepare(body, lit)?;
                terms = terms.max(n).max(inner);
                if lit == Some(n) {
                    out.push_str(&format!("{{\\color{{red}}{{{body}}}}}"));
                } else {
                    out.push_str(&format!("{{{body}}}"));
                }
                rest = &rest[end..];
            }
            (_, Some(b)) => {
                out.push_str(&rest[..b + 2]);
                let close = rest[b..].find(']').ok_or("unclosed \\\\[ spacing")?;
                rest = &rest[b + close + 1..];
            }
            _ => break,
        }
    }
    out.push_str(rest);
    Ok((out, terms))
}

fn min_text_size(frame: &Frame, scale: f64, min: &mut f64) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(g) => min_text_size(&g.frame, scale * g.transform.sy.get(), min),
            FrameItem::Text(t) if t.glyphs.iter().any(|g| g.id != 0) => *min = min.min(t.size.to_pt() * scale),
            _ => {}
        }
    }
}

/// Premultiplied RGBA of the formula, its size, its baseline, and its smallest glyph.
fn render(world: &mut FormulaWorld, latex: &str, size_px: f32, display: bool) -> Result<(Vec<u8>, u32, u32, f32, f32), String> {
    let converted = mitex::convert_math(latex, None)?;
    // `\widehat` spans its whole argument, as in LaTeX.
    let scope = "mitex-scope + (hat: x => math.accent(x, \"\\u{302}\", size: 100%))";
    let equation = format!("math.equation(block: {display}, eval(\"$\" + read(\"/formula.txt\") + \"$\", scope: {scope}))");
    let main = format!(
        "#import \"/specs/mod.typ\": mitex-scope\n#set page(width: auto, height: auto, margin: {PAD}pt, fill: none)\n#set par(leading: {LEADING}em)\n#set text(size: {size_px}pt, fill: black, font: \"New Computer Modern\")\n#box({equation})\n"
    );
    world.files.insert(world.main, Bytes::from_string(main));
    world.files.insert(file_id("/formula.txt"), Bytes::from_string(converted.clone()));
    let document = typst::compile::<PagedDocument>(world)
        .output
        .map_err(|errors| format!("{} (Typst: {converted})", errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; ")))?;
    let page = document.pages().first().ok_or("no page")?;
    // The box holding the formula keeps its baseline.
    let baseline = page
        .frame
        .items()
        .find_map(|(pos, item)| match item {
            FrameItem::Group(g) if g.frame.has_baseline() => Some((pos.y + g.frame.baseline()).to_pt()),
            _ => None,
        })
        .unwrap_or(page.frame.height().to_pt());
    let mut min = f64::INFINITY;
    min_text_size(&page.frame, 1.0, &mut min);
    let pixmap = typst_render::render(page, &typst_render::RenderOptions { pixel_per_pt: typst::utils::Scalar::new(1.0), ..Default::default() });
    Ok((pixmap.data().to_vec(), pixmap.width(), pixmap.height(), baseline as f32, min as f32))
}

fn typeset(latex: &str, size_px: f32, display: bool) -> Result<Formula, String> {
    static WORLD: OnceLock<Mutex<FormulaWorld>> = OnceLock::new();
    let mut world = WORLD.get_or_init(|| Mutex::new(FormulaWorld::new())).lock().unwrap();
    let (plain, terms) = prepare(latex, None)?;
    let (rgba, width, height, baseline, min_glyph_px) = render(&mut world, &plain, size_px, display)?;
    let coverage = rgba.chunks_exact(4).map(|p| p[3]).collect();
    let mut term_masks = Vec::new();
    for n in 1..=terms {
        let (lit, _) = prepare(latex, Some(n))?;
        let (rgba, w, h, _, _) = render(&mut world, &lit, size_px, display)?;
        if (w, h) != (width, height) {
            return Err(format!("term {n} changes the formula's size"));
        }
        // Black ink has as much green as red; the lit term is red alone.
        term_masks.push(rgba.chunks_exact(4).map(|p| p[0].saturating_sub(p[1])).collect());
    }
    Ok(Formula { width, height, baseline, coverage, terms: term_masks, min_glyph_px })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_terms_and_spacing() {
        let (s, n) = prepare(r"a &= \lit{1}{b} \\[3pt] c &= \lit{2}{d_{\lit{3}{e}}}", Some(2)).unwrap();
        assert_eq!(n, 3);
        assert_eq!(s, r"a &= {b} \\ c &= {\color{red}{d_{{e}}}}");
    }

    #[test]
    fn typesets_deterministically() {
        let a = typeset(r"\alpha = \dfrac{1}{1 + \lit{1}{n}}", FORMULA_PX, true).unwrap();
        let b = typeset(r"\alpha = \dfrac{1}{1 + \lit{1}{n}}", FORMULA_PX, true).unwrap();
        assert_eq!(a.coverage, b.coverage);
        assert_eq!(a.terms.len(), 1);
        assert!(a.terms[0].iter().any(|&c| c > 128));
        assert!(a.min_glyph_px >= MIN_GLYPH_PX);
    }
}
