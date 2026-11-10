//! One view for the code the lab shows: a pseudo-code listing (Python,
//! src/pseudocode.rs; the Code tab's panel) and a pass's shader source (WGSL,
//! src/shader_source.rs; beside the frame graph). Code is sections, each an
//! optional header (where it comes from) over its lines, a line lit or not;
//! each language has its tokens, coloured alike (keywords, names, numbers,
//! text, comments). Lines wrap within the view (a long one continues deeper
//! than its indentation), or run on and the view scrolls both ways; a view
//! that does not scroll counts the lines it leaves out. The pseudo-code's
//! block style (src/ui/code.rs `BlockPainter`) draws its statements' text with
//! `draw_text`.

use crate::palette;
use crate::text::Ink;
use egui::{Align2, CornerRadius, FontFamily, Pos2, Rect, Vec2, pos2, vec2};

pub use crate::pseudocode::Token;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Python,
    Wgsl,
}

/// One line of code, lit (a story's highlight) or not.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub lit: bool,
}

/// A run of lines under an optional header (a file and line), strong (the
/// code the view is about) or not (what it uses, dimmer).
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub header: Option<String>,
    pub lines: Vec<Line>,
    pub strong: bool,
}

/// A line's tokens for colouring.
pub fn tokens(language: Language, line: &str) -> Vec<(Token, &str)> {
    match language {
        Language::Python => crate::pseudocode::tokens(line),
        Language::Wgsl => wgsl_tokens(line),
    }
}

/// WGSL tokens of one line: keywords, types (as names), numbers, attributes
/// (as text), and a `//` comment to its end.
fn wgsl_tokens(line: &str) -> Vec<(Token, &str)> {
    const KEYWORDS: &[&str] = &[
        "fn",
        "let",
        "var",
        "const",
        "override",
        "struct",
        "alias",
        "return",
        "if",
        "else",
        "for",
        "while",
        "loop",
        "break",
        "continue",
        "continuing",
        "switch",
        "case",
        "default",
        "discard",
        "true",
        "false",
    ];
    const TYPES: &[&str] = &[
        "f32",
        "f16",
        "u32",
        "i32",
        "bool",
        "vec2",
        "vec3",
        "vec4",
        "mat2x2",
        "mat2x3",
        "mat2x4",
        "mat3x2",
        "mat3x3",
        "mat3x4",
        "mat4x2",
        "mat4x3",
        "mat4x4",
        "array",
        "atomic",
        "ptr",
        "sampler",
        "sampler_comparison",
    ];
    let mut out = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        // Each token in turn: a comment takes the rest of the line; an attribute, a number, or a word its own run.
        if rest.starts_with("//") {
            out.push((Token::Comment, rest));
            break;
        }
        let first = rest.chars().next().expect("not empty");
        let word = |s: &str| {
            s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(s.len())
        };
        let (kind, len) = if first == '@' {
            (Token::Text, 1 + word(&rest[1..]))
        } else if first.is_ascii_digit() {
            (
                Token::Number,
                rest.find(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_'))
                    .unwrap_or(rest.len()),
            )
        } else if first.is_alphabetic() || first == '_' {
            let n = word(rest);
            let w = &rest[..n];
            (
                if KEYWORDS.contains(&w) {
                    Token::Keyword
                } else if TYPES.contains(&w) || w.starts_with("texture_") {
                    Token::Name
                } else {
                    Token::Plain
                },
                n,
            )
        } else {
            (Token::Plain, first.len_utf8())
        };
        out.push((kind, &rest[..len]));
        rest = &rest[len..];
    }
    out
}

/// A row of a wrapped line: its characters `start..stop`, and its indent
/// (in characters).
pub type Row = (usize, usize, usize);

/// A code line's rows within `columns`: each row's character range and the
/// column it starts at. A long line breaks after its last space or comma that
/// fits, and continues four columns deeper than its indentation.
pub fn wrap(text: &str, columns: usize) -> Vec<Row> {
    let chars: Vec<char> = text.chars().collect();
    let deeper = chars.iter().take_while(|c| **c == ' ').count() + 4;
    let mut rows = Vec::new();
    let (mut start, mut indent) = (0, 0);
    while chars.len() - start > columns - indent {
        // A row breaks after the last space or comma that fits; the rows after the first are indented deeper.
        let limit = start + (columns - indent).max(1);
        let stop = (start + 1..=limit)
            .rev()
            .find(|&b| matches!(chars[b - 1], ' ' | ','))
            .unwrap_or(limit);
        rows.push((start, stop, indent));
        start = stop;
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
        indent = deeper.min(columns / 2);
    }
    rows.push((start, chars.len(), indent));
    rows
}

/// A code line in syntax colours, on its rows (`wrap`) from `at`: `cell` a
/// character's width and a row's height; `dim`, fainter.
pub fn draw_text(
    ink: &Ink,
    language: Language,
    text: &str,
    rows: &[Row],
    at: Pos2,
    cell: Vec2,
    dim: bool,
) {
    let mut col = 0usize;
    for (kind, token) in tokens(language, text) {
        let color = match kind {
            Token::Keyword => palette::code_keyword(),
            Token::Name => palette::code_name(),
            Token::Number => palette::code_number(),
            Token::Text => palette::code_string(),
            Token::Comment => palette::text_faint(),
            Token::Plain => palette::text(),
        };
        let color = if dim {
            color.gamma_multiply(0.55)
        } else {
            color
        };
        let end = col + token.chars().count();
        // The token's part on each row it reaches.
        for (row, &(start, stop, indent)) in rows.iter().enumerate() {
            let (from, to) = (col.max(start), end.min(stop));
            if from >= to {
                continue;
            }
            let part: String = token.chars().skip(from - col).take(to - from).collect();
            if !part.trim().is_empty() {
                let pos = pos2(
                    at.x + (indent + from - start) as f32 * cell.x,
                    at.y + row as f32 * cell.y,
                );
                ink.text(
                    pos,
                    Align2::LEFT_TOP,
                    part,
                    24.0,
                    FontFamily::Monospace,
                    color,
                );
            }
        }
        col = end;
    }
}

/// A view that scrolls (the wheel, both ways; a drag on its bar, or a press
/// beside the bar's thumb, which brings the thumb there): kept under `id`,
/// back at its top when `key` changes (another pass).
pub struct Scroll<'a> {
    pub ctx: &'a egui::Context,
    pub id: egui::Id,
    pub key: &'a str,
}

/// A row's height.
pub const LINE: f32 = 28.0;

/// A section laid out: its header, and each of its lines' rows.
type LaidSection<'a> = (Option<&'a String>, Vec<Vec<Row>>);

/// The view's scroll offset, kept in egui's memory for `s.key` (and where a drag holds the bar's thumb): the
/// wheel over `rect`, a press or a drag on the bar's `track`, clamped to the content (`height` high,
/// `content_w` wide); and whether the bar is lit (dragged or hovered).
fn scrolled(
    s: &Scroll,
    rect: Rect,
    (height, content_w): (f32, f32),
    (track, thumb_of): (Rect, &dyn Fn(f32) -> Rect),
) -> (Vec2, bool) {
    // The offset, and where on the thumb a drag holds it (none while not dragging).
    let (mut offset, key, mut grab) = s
        .ctx
        .data(|d| d.get_temp::<(Vec2, String, Option<f32>)>(s.id))
        .unwrap_or_default();
    if key != s.key {
        (offset, grab) = (Vec2::ZERO, None);
    }
    let (hover, pressed, down) = s.ctx.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
        )
    });
    let bar = height > rect.height();
    if hover.is_some_and(|p| rect.contains(p)) {
        offset -= s.ctx.input(|i| i.smooth_scroll_delta);
    }
    if bar
        && pressed
        && let Some(p) = hover.filter(|p| track.contains(*p))
    {
        let thumb = thumb_of(offset.y);
        // On the thumb, held where it is pressed; beside it, the thumb's middle comes there.
        grab = Some(if thumb.y_range().contains(p.y) {
            p.y - thumb.min.y
        } else {
            thumb.height() / 2.0
        });
    }
    if !down || !bar {
        grab = None;
    }
    if let (Some(g), Some(p)) = (grab, hover) {
        offset.y = (p.y - g - rect.min.y) * height / rect.height();
    }
    offset = vec2(
        offset.x.clamp(0.0, (content_w - rect.width()).max(0.0)),
        offset.y.clamp(0.0, (height - rect.height()).max(0.0)),
    );
    let bar_lit = bar && (grab.is_some() || hover.is_some_and(|p| track.contains(p)));
    if bar_lit {
        s.ctx.set_cursor_icon(if grab.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Grab
        });
    }
    s.ctx
        .data_mut(|d| d.insert_temp(s.id, (offset, s.key.to_string(), grab)));
    (offset, bar_lit)
}

/// Draws `sections` of `language` code in `rect`: each header, then its
/// lines, a lit line on a band with an accent bar, the others dimmed while
/// any is lit (and those of a section that is not strong). `wrap` breaks long
/// lines within the view; else they run on. Without `scroll`, the lines that
/// do not fit are left out, and their number returned.
pub fn draw(
    ink: &Ink,
    rect: Rect,
    language: Language,
    sections: &[Section],
    wrap_lines: bool,
    scroll: Option<Scroll>,
) -> usize {
    let column = ink.measure("0", 24.0, FontFamily::Monospace).x;
    let columns = if wrap_lines {
        ((rect.width() / column).floor() as usize).max(8)
    } else {
        usize::MAX / 2
    };
    // Each section's rows, and the content's size.
    let laid: Vec<LaidSection> = sections
        .iter()
        .map(|s| {
            (
                s.header.as_ref(),
                s.lines.iter().map(|l| wrap(&l.text, columns)).collect(),
            )
        })
        .collect();
    let header_height = 32.0;
    let gap = 14.0;
    let height: f32 = laid
        .iter()
        .map(|(h, rows)| {
            h.map_or(0.0, |_| header_height)
                + rows.iter().map(|r| r.len() as f32 * LINE).sum::<f32>()
                + gap
        })
        .sum();
    let width = sections
        .iter()
        .flat_map(|s| s.lines.iter())
        .map(|l| l.text.chars().count() as f32 * column)
        .fold(0.0, f32::max)
        + 20.0;
    // The bar along the right edge: its thumb from the top of the view in proportion to the
    // offset, as long as the view is of the content; `track`, where a press lands on it.
    let thumb_of = |offset: f32| {
        Rect::from_min_size(
            pos2(
                rect.max.x - 5.0,
                rect.min.y + rect.height() * offset / height,
            ),
            vec2(4.0, rect.height() * rect.height() / height),
        )
    };
    let track = Rect::from_x_y_ranges(rect.max.x - 14.0..=rect.max.x + 4.0, rect.y_range());
    let content_w = if wrap_lines {
        rect.width()
    } else {
        width
    };
    let (offset, bar_lit) = match &scroll {
        Some(s) => scrolled(s, rect, (height, content_w), (track, &thumb_of)),
        None => (Vec2::ZERO, false),
    };
    // A lit band reaches 10 points past the code on either side, and 2 above it.
    let overflow = draw_sections(
        ink,
        rect,
        (language, sections, &laid),
        offset,
        scroll.is_some(),
    );
    // Where the view is in the code: a thin bar along the right edge, brighter under the pointer.
    if scroll.is_some() && height > rect.height() {
        ink.painter.rect_filled(
            thumb_of(offset.y),
            CornerRadius::same(2),
            if bar_lit {
                palette::text_dim()
            } else {
                palette::text_faint()
            },
        );
    }
    overflow
}

/// The sections' headers and lines at `offset` into the content, clipped to `rect` (a lit band reaches 10
/// points past the code on either side, and 2 above it); without scrolling (`scrolls`), the lines that do not
/// fit left out, and their number returned.
fn draw_sections(
    ink: &Ink,
    rect: Rect,
    (language, sections, laid): (Language, &[Section], &[LaidSection]),
    offset: Vec2,
    scrolls: bool,
) -> usize {
    let column = ink.measure("0", 24.0, FontFamily::Monospace).x;
    let any_lit = sections.iter().any(|s| s.lines.iter().any(|l| l.lit));
    let (header_height, gap) = (32.0, 14.0);
    let clipped = ink.painter.with_clip_rect(rect.expand2(vec2(12.0, 2.0)));
    let cink = Ink::new(&clipped);
    let mut overflow = 0;
    let mut y = rect.min.y - offset.y;
    let x = rect.min.x - offset.x;
    for (section, (header, rows)) in sections.iter().zip(laid) {
        if let Some(h) = header {
            if scrolls || y + header_height <= rect.max.y {
                cink.text(
                    pos2(x, y),
                    Align2::LEFT_TOP,
                    h.as_str(),
                    24.0,
                    FontFamily::Proportional,
                    palette::accent(),
                );
            }
            y += header_height;
        }
        for (line, rows) in section.lines.iter().zip(rows) {
            // A line that does not fit (without scrolling) is counted as cut; one in view is drawn, lit with a band.
            let h = rows.len() as f32 * LINE;
            if !scrolls && y + h > rect.max.y {
                overflow += 1;
                continue;
            }
            if y + h >= rect.min.y && y <= rect.max.y {
                if line.lit {
                    let band = Rect::from_min_size(
                        pos2(rect.min.x - 10.0, y - 2.0),
                        vec2(rect.width() + 20.0, h),
                    );
                    clipped.rect_filled(band, CornerRadius::same(4), palette::highlight_row());
                    clipped.rect_filled(
                        Rect::from_min_size(pos2(rect.min.x - 10.0, y + 1.0), vec2(4.0, h - 6.0)),
                        CornerRadius::same(2),
                        palette::accent(),
                    );
                }
                let dim = (any_lit && !line.lit) || !section.strong;
                draw_text(
                    &cink,
                    language,
                    &line.text,
                    rows,
                    pos2(x, y),
                    vec2(column, LINE),
                    dim,
                );
            }
            y += h;
        }
        y += gap;
    }
    ink.min_size
        .set(ink.min_size.get().min(cink.min_size.get()));
    overflow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgsl_tokens_colour_what_they_are() {
        let kinds: Vec<(Token, &str)> = tokens(
            Language::Wgsl,
            "@compute fn f(x: vec3<u32>) { let a = 1.5e3; } // done",
        )
        .into_iter()
        .filter(|(k, _)| *k != Token::Plain)
        .collect();
        assert_eq!(
            kinds,
            [
                (Token::Text, "@compute"),
                (Token::Keyword, "fn"),
                (Token::Name, "vec3"),
                (Token::Name, "u32"),
                (Token::Keyword, "let"),
                (Token::Number, "1.5e3"),
                (Token::Comment, "// done")
            ]
        );
    }

    #[test]
    fn dragging_the_bar_scrolls() {
        let area = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let view = Rect::from_min_size(pos2(20.0, 20.0), vec2(600.0, 400.0));
        let sections = [Section {
            header: None,
            lines: (0..100)
                .map(|i| Line {
                    text: format!("let a{i} = {i};"),
                    lit: false,
                })
                .collect(),
            strong: true,
        }];
        let ctx = egui::Context::default();
        let id = egui::Id::unique("test-code-view-scroll");
        let frame = |events: Vec<egui::Event>| -> f32 {
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(area),
                    ..Default::default()
                },
                |ui| {
                    let painter = ui.painter().clone();
                    draw(
                        &Ink::new(&painter),
                        view,
                        Language::Wgsl,
                        &sections,
                        false,
                        Some(Scroll {
                            ctx: ui.ctx(),
                            id,
                            key: "pass",
                        }),
                    );
                },
            );
            ctx.data(|d| d.get_temp::<(Vec2, String, Option<f32>)>(id))
                .expect("kept")
                .0
                .y
        };
        let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let thumb = pos2(view.max.x - 3.0, view.min.y + 5.0);
        frame(vec![egui::Event::PointerMoved(thumb)]);
        assert_eq!(frame(vec![button(thumb, true)]), 0.0);
        let content = 100.0 * LINE + 14.0;
        let dragged = frame(vec![egui::Event::PointerMoved(thumb + vec2(0.0, 100.0))]);
        assert!(
            (dragged - 100.0 * content / view.height()).abs() < 1.0,
            "{dragged}"
        );
        frame(vec![button(thumb + vec2(0.0, 100.0), false)]);
        // Released, the pointer moves on without it.
        assert_eq!(
            frame(vec![egui::Event::PointerMoved(thumb + vec2(0.0, 200.0))]),
            dragged
        );
        // A press at the bottom of the bar brings the thumb to the end.
        let end = pos2(view.max.x - 3.0, view.max.y - 1.0);
        frame(vec![egui::Event::PointerMoved(end)]);
        assert_eq!(frame(vec![button(end, true)]), content - view.height());
    }

    #[test]
    fn a_long_line_wraps_deeper() {
        let rows = wrap("    let value = first_thing, second_thing", 20);
        assert!(rows.len() > 1 && rows[1].2 == 8);
    }
}
