//! The Code tab: a listing as text or as blocks (its loops and branches
//! holding their bodies, each row coloured by what it does).

use super::*;

/// A Python pseudo-code listing: the engine function it follows, then the
/// code with the highlighted tags' lines lit.
pub(crate) fn draw_code(ink: &Ink, rect: Rect, listing: &crate::pseudocode::Listing, highlight: &[String], report: &mut UiReport) {
    let painter = ink.painter;
    let x = rect.min.x + 22.0;
    let width = rect.width() - 44.0;
    let mut y = rect.min.y + 16.0;
    report.panel_code = Some(listing.name.to_string());
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("ui-python_pseudo_code"), 30.0, bold(), palette::text());
    y += 42.0;
    let title = ink.fit_right(listing.title(), width, 24.0, FontFamily::Proportional);
    ink.text(pos2(x, y), Align2::LEFT_TOP, title, 24.0, FontFamily::Proportional, palette::accent());
    y += 32.0;
    if let Some(entry) = sources::index().entries.get(&listing.source) {
        let engine = investigation::current().wording().engine_short;
        let one_line = format!("≈ {engine}: {} · {}", entry.short_symbol(), entry.file_name());
        let lines = if ink.measure(&one_line, 24.0, FontFamily::Proportional).x <= width {
            vec![one_line]
        } else {
            vec![format!("≈ {engine}: {}", entry.short_symbol()), format!("    {}:{}", entry.file_name(), entry.line)]
        };
        for text in lines {
            let text = ink.fit_right(&text, width, 24.0, FontFamily::Proportional);
            ink.text(pos2(x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, palette::text_faint());
            y += 30.0;
        }
        y += 4.0;
    }
    painter.line_segment([pos2(x, y), pos2(rect.max.x - 22.0, y)], Stroke::new(1.0, palette::border()));
    y += 12.0;
    let column = ink.measure("0", 24.0, FontFamily::Monospace).x;
    if crate::pseudocode::style() == crate::pseudocode::Style::Blocks {
        draw_blocks(ink, Rect::from_min_max(pos2(x - 10.0, y), pos2(x - 10.0 + width + 20.0, rect.max.y - 12.0)), listing, highlight, column, report);
        return;
    }
    // The text style: the code view (src/code_view.rs), its lines wrapped, the highlighted ones lit.
    use crate::code_view;
    let lines = listing.lines.iter().map(|l| code_view::Line { text: l.text.clone(), lit: listing.lit(l, highlight) }).collect();
    let sections = [code_view::Section { header: None, lines, strong: true }];
    report.code_overflow += code_view::draw(ink, Rect::from_min_max(pos2(x, y), pos2(x + width, rect.max.y - 12.0)), code_view::Language::Python, &sections, true, None);
}

/// The code panel's width that shows `listing` as `draw_code` lays it out, in the listing style in use,
/// without wrapping a line or shortening its header: what a recording gives it (Layout::with_panel_width
/// keeps it inside the canvas), at most CODE_PANEL_WIDTH. `ctx`'s fonts measure it.
pub fn code_panel_fit(ctx: &egui::Context, listing: &crate::pseudocode::Listing) -> f32 {
    let measure = |text: &str, size: f32, family: FontFamily| ctx.fonts_mut(|f| f.layout(text.to_string(), egui::FontId::new(size, family), Color32::WHITE, f32::INFINITY).size().x);
    let column = measure("0", 24.0, FontFamily::Monospace);
    // The header: its heading, the listing's title, and its source on two lines at most.
    let mut inner = measure(tr("ui-python_pseudo_code"), 30.0, bold()).max(measure(listing.title(), 24.0, FontFamily::Proportional));
    if let Some(entry) = sources::index().entries.get(&listing.source) {
        let engine = investigation::current().wording().engine_short;
        for text in [format!("≈ {engine}: {}", entry.short_symbol()), format!("    {}:{}", entry.file_name(), entry.line)] {
            inner = inner.max(measure(&text, 24.0, FontFamily::Proportional));
        }
    }
    let chars = |t: &str| t.chars().count() as f32;
    let code = if crate::pseudocode::style() == crate::pseudocode::Style::Blocks {
        // As BlockPainter::stack: each bar's lines from their first's indent, a body an arm further in.
        fn width(blocks: &[crate::pseudocode::Block], listing: &crate::pseudocode::Listing, column: f32, dispatch: &mut Option<f32>) -> f32 {
            use crate::pseudocode::Block;
            let indent = |t: &str| t.chars().take_while(|c| *c == ' ').count();
            let bar = |lines: &std::ops::Range<usize>| {
                let base = indent(&listing.lines[lines.start].text);
                let widest = listing.lines[lines.clone()].iter().map(|l| l.text.chars().count() - indent(&l.text).min(base)).max().unwrap_or(0);
                BLOCK_EDGE + BLOCK_TEXT + 4.0 + widest as f32 * column + 1.0
            };
            blocks.iter().map(|block| match block {
                Block::Hat { lines } => dispatch.take().unwrap_or_else(|| bar(lines)),
                Block::Stmt { lines } | Block::Cap { lines } => bar(lines),
                Block::Note { line } => BLOCK_EDGE + BLOCK_TEXT + listing.lines[*line].text.trim_start().chars().count() as f32 * column,
                Block::Gap => 0.0,
                Block::C { arms } => arms.iter().map(|a| bar(&a.head).max(BLOCK_ARM + width(&a.body, listing, column, dispatch))).fold(0.0, f32::max),
            }).fold(0.0, f32::max)
        }
        let mut dispatch = listing.dispatch().map(|d| measure(d, 24.0, bold()) + BLOCK_EDGE + BLOCK_TEXT + 8.0);
        // draw_blocks is given 20 more than the header.
        width(&crate::pseudocode::blocks(listing), listing, column, &mut dispatch) - 20.0
    } else {
        // code_view::draw wraps a line longer than the columns its width holds.
        listing.lines.iter().map(|l| chars(&l.text)).fold(0.0, f32::max) * column + 1.0
    };
    (inner.max(code) + 44.0).ceil().min(CODE_PANEL_WIDTH)
}

/// The block style's spacing, tighter for a listing that would not fit.
#[derive(Clone, Copy)]
pub(super) struct BlockSpacing {
    /// Above and below a block's text.
    pub(super) pad: f32,
    /// Between stacked blocks.
    pub(super) gap: f32,
    /// A loop's or a branch's foot.
    pub(super) foot: f32,
    /// A blank line.
    pub(super) blank: f32,
    /// Above a hat's text.
    pub(super) hat: f32,
}

/// From the roomiest; a listing takes the first that fits.
pub(super) const BLOCK_SPACINGS: [BlockSpacing; 5] = [
    BlockSpacing { pad: 4.0, gap: 3.0, foot: 12.0, blank: 12.0, hat: 6.0 },
    BlockSpacing { pad: 2.0, gap: 2.0, foot: 10.0, blank: 8.0, hat: 4.0 },
    BlockSpacing { pad: 1.0, gap: 1.0, foot: 8.0, blank: 4.0, hat: 2.0 },
    BlockSpacing { pad: 0.0, gap: 1.0, foot: 6.0, blank: 2.0, hat: 0.0 },
    // A full panel: blocks touch, told apart by their notches.
    BlockSpacing { pad: 0.0, gap: 0.0, foot: 4.0, blank: 2.0, hat: 0.0 },
];

/// Draws (or, with `draw` false, measures) a listing as Scratch-like blocks
/// (`pseudocode::blocks`): each statement a bar coloured by what it does, its
/// coloured edge on the left; loops and branches C-shaped around their bodies;
/// a function's `def` (or its `# dispatch:`) a hat on top.
pub(super) struct BlockPainter<'a, 'b> {
    pub(super) ink: &'a Ink<'b>,
    pub(super) listing: &'a crate::pseudocode::Listing,
    pub(super) highlight: &'a [String],
    pub(super) any_lit: bool,
    pub(super) column: f32,
    pub(super) spacing: BlockSpacing,
    pub(super) bottom: f32,
    pub(super) draw: bool,
    /// Lines left out below `bottom`.
    pub(super) overflow: usize,
    /// A hat is drawn: the dispatch is the first function's.
    pub(super) hatted: bool,
}

pub(super) const BLOCK_ROW: f32 = 28.0;
/// A block's coloured edge, and the space before its text.
pub(super) const BLOCK_EDGE: f32 = 4.0;
pub(super) const BLOCK_TEXT: f32 = 12.0;
/// A loop's arm, left of its body.
pub(super) const BLOCK_ARM: f32 = 18.0;

impl BlockPainter<'_, '_> {
    pub(super) fn color(kind: crate::pseudocode::Kind) -> Color32 {
        use crate::pseudocode::Kind;
        match kind {
            Kind::Control => palette::code_keyword(),
            Kind::Read => palette::code_name(),
            Kind::Write => palette::code_number(),
            Kind::Math => palette::code_string(),
            Kind::Statement => palette::text_dim(),
        }
    }

    pub(super) fn lit(&self, lines: &std::ops::Range<usize>) -> bool {
        self.listing.lines[lines.clone()].iter().any(|l| self.listing.lit(l, self.highlight))
    }

    /// Each line's text, indented from the first's, and its rows within `width`.
    pub(super) fn rows(&self, lines: &std::ops::Range<usize>, width: f32) -> Vec<(String, Vec<crate::code_view::Row>)> {
        let indent = |t: &str| t.chars().take_while(|c| *c == ' ').count();
        let base = indent(&self.listing.lines[lines.start].text);
        let columns = (((width - BLOCK_EDGE - BLOCK_TEXT - 4.0) / self.column).floor() as usize).max(8);
        self.listing.lines[lines.clone()]
            .iter()
            .map(|l| {
                let text: String = l.text.chars().skip(indent(&l.text).min(base)).collect();
                let rows = crate::code_view::wrap(&text, columns);
                (text, rows)
            })
            .collect()
    }

    /// The fill of a block of `color`: lit, dimmed beside lit lines, or plain.
    pub(super) fn fill(&self, color: Color32, lit: bool) -> Color32 {
        color.gamma_multiply(if lit { 0.5 } else if self.any_lit { 0.2 } else { 0.34 })
    }

    pub(super) fn edge(&self, color: Color32, lit: bool) -> Color32 {
        if self.any_lit && !lit { color.gamma_multiply(0.6) } else { color }
    }

    /// One bar: `lines` on a block of `color`; its height.
    pub(super) fn bar(&mut self, at: Pos2, width: f32, lines: std::ops::Range<usize>, color: Color32, shape: BarShape) -> f32 {
        let rows = self.rows(&lines, width);
        let count: usize = rows.iter().map(|(_, r)| r.len()).sum();
        let hat = matches!(shape, BarShape::Hat(_));
        let top = self.spacing.pad + if hat { self.spacing.hat } else { 0.0 };
        let dispatch = match shape {
            BarShape::Hat(Some(_)) => 1,
            _ => count,
        };
        let height = top + self.spacing.pad + BLOCK_ROW * dispatch as f32;
        if !self.draw {
            return height;
        }
        if at.y + height > self.bottom {
            self.overflow += lines.len();
            return height;
        }
        let painter = self.ink.painter;
        let lit = self.lit(&lines);
        let rect = Rect::from_min_size(at, vec2(width, height));
        let radius = match shape {
            BarShape::Hat(_) => CornerRadius { nw: 14, ne: 14, sw: 3, se: 3 },
            BarShape::Head => CornerRadius { nw: 4, ne: 4, sw: 0, se: 4 },
            BarShape::Middle => CornerRadius { nw: 0, ne: 4, sw: 0, se: 4 },
            BarShape::Plain | BarShape::Cap => CornerRadius::same(4),
        };
        painter.rect_filled(rect, radius, self.fill(color, lit));
        painter.rect_filled(Rect::from_min_size(at, vec2(BLOCK_EDGE, height)), CornerRadius { nw: radius.nw, sw: radius.sw, ne: 0, se: 0 }, self.edge(color, lit));
        // The notch a stacked block hangs from.
        if !hat && !matches!(shape, BarShape::Middle) {
            let (l, t) = (at.x + BLOCK_EDGE + 10.0, at.y);
            painter.add(egui::Shape::convex_polygon(vec![pos2(l, t), pos2(l + 22.0, t), pos2(l + 18.0, t + 4.0), pos2(l + 4.0, t + 4.0)], palette::panel(), Stroke::NONE));
        }
        // Nothing follows a cap: its foot is solid.
        if matches!(shape, BarShape::Cap) {
            painter.rect_filled(Rect::from_min_max(pos2(at.x, rect.max.y - 3.0), rect.max), CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, self.edge(color, lit));
        }
        if lit {
            painter.rect_stroke(rect, radius, Stroke::new(2.0, palette::accent()), StrokeKind::Inside);
        }
        let dim = self.any_lit && !lit;
        let x = at.x + BLOCK_EDGE + BLOCK_TEXT;
        let mut y = at.y + top;
        if let BarShape::Hat(Some(text)) = shape {
            let text = self.ink.fit_right(text, width - BLOCK_EDGE - BLOCK_TEXT - 8.0, 24.0, bold());
            let color = if dim { palette::text().gamma_multiply(0.55) } else { palette::text() };
            self.ink.text(pos2(x, y), Align2::LEFT_TOP, text, 24.0, bold(), color);
            return height;
        }
        for (text, rows) in &rows {
            crate::code_view::draw_text(self.ink, crate::code_view::Language::Python, text, rows, pos2(x, y), vec2(self.column, BLOCK_ROW), dim);
            y += BLOCK_ROW * rows.len() as f32;
        }
        height
    }

    /// A loop's or a branch's column left of a body, or its foot.
    pub(super) fn frame(&mut self, rect: Rect, color: Color32, lit: bool, radius: CornerRadius) {
        if !self.draw || rect.height() <= 0.0 {
            return;
        }
        let rect = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.max.y.min(self.bottom)));
        if rect.height() <= 0.0 {
            return;
        }
        let painter = self.ink.painter;
        painter.rect_filled(rect, radius, self.fill(color, lit));
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(BLOCK_EDGE, rect.height())), CornerRadius { nw: 0, ne: 0, sw: radius.sw, se: 0 }, self.edge(color, lit));
    }

    /// `blocks` stacked from `at`; their height.
    pub(super) fn stack(&mut self, blocks: &[crate::pseudocode::Block], at: Pos2, width: f32) -> f32 {
        use crate::pseudocode::Block;
        let mut y = at.y;
        for (i, block) in blocks.iter().enumerate() {
            if i > 0 {
                y += self.spacing.gap;
            }
            let at = pos2(at.x, y);
            y += match block {
                Block::Hat { lines } => {
                    let dispatch = if self.hatted { None } else { self.listing.dispatch().map(str::to_string) };
                    self.hatted = true;
                    self.bar(at, width, lines.clone(), palette::accent(), BarShape::Hat(dispatch.as_deref()))
                }
                Block::Stmt { lines } => self.bar(at, width, lines.clone(), Self::color(self.listing.lines[lines.start].kind()), BarShape::Plain),
                Block::Cap { lines } => self.bar(at, width, lines.clone(), Self::color(self.listing.lines[lines.start].kind()), BarShape::Cap),
                Block::Note { line } => {
                    if self.draw {
                        if y + BLOCK_ROW > self.bottom {
                            self.overflow += 1;
                        } else {
                            let text = self.listing.lines[*line].text.trim_start().to_string();
                            let rows = crate::code_view::wrap(&text, usize::MAX / 2);
                            crate::code_view::draw_text(self.ink, crate::code_view::Language::Python, &text, &rows, pos2(at.x + BLOCK_EDGE + BLOCK_TEXT, y), vec2(self.column, BLOCK_ROW), self.any_lit);
                        }
                    }
                    BLOCK_ROW
                }
                Block::Gap => self.spacing.blank,
                Block::C { arms } => {
                    let color = Self::color(self.listing.lines[arms[0].head.start].kind());
                    let lit = arms.iter().any(|a| self.lit(&a.head));
                    let mut cy = y;
                    for (k, arm) in arms.iter().enumerate() {
                        cy += self.bar(pos2(at.x, cy), width, arm.head.clone(), color, if k == 0 { BarShape::Head } else { BarShape::Middle });
                        let body = self.spacing.gap + self.stack(&arm.body, pos2(at.x + BLOCK_ARM, cy + self.spacing.gap), width - BLOCK_ARM) + self.spacing.gap;
                        self.frame(Rect::from_min_size(pos2(at.x, cy), vec2(BLOCK_ARM, body)), color, lit, CornerRadius::ZERO);
                        cy += body;
                    }
                    let foot = Rect::from_min_size(pos2(at.x, cy), vec2(width.min(BLOCK_ARM + 120.0), self.spacing.foot));
                    self.frame(foot, color, lit, CornerRadius { nw: 0, ne: 4, sw: 4, se: 4 });
                    cy + self.spacing.foot - y
                }
            };
        }
        y - at.y
    }
}

#[derive(Clone, Copy)]
pub(super) enum BarShape<'a> {
    /// A function's hat, with its dispatch's text if it has one.
    Hat(Option<&'a str>),
    /// A loop's or a branch's first head.
    Head,
    /// A branch's next head (`elif`, `else`).
    Middle,
    Plain,
    Cap,
}

/// The listing as blocks in `area`, with the roomiest spacing that fits;
/// lines below its bottom are counted in `report.code_overflow`.
pub(super) fn draw_blocks(ink: &Ink, area: Rect, listing: &crate::pseudocode::Listing, highlight: &[String], column: f32, report: &mut UiReport) {
    let (x, y, width, bottom) = (area.min.x, area.min.y, area.width(), area.max.y);
    let blocks = crate::pseudocode::blocks(listing);
    let any_lit = listing.lines.iter().any(|l| listing.lit(l, highlight));
    let mut painter = BlockPainter { ink, listing, highlight, any_lit, column, spacing: BLOCK_SPACINGS[0], bottom, draw: false, overflow: 0, hatted: false };
    for spacing in BLOCK_SPACINGS {
        painter.spacing = spacing;
        painter.hatted = false;
        if y + painter.stack(&blocks, pos2(x, y), width) <= bottom {
            break;
        }
    }
    painter.draw = true;
    painter.hatted = false;
    painter.stack(&blocks, pos2(x, y), width);
    report.code_overflow += painter.overflow;
}
