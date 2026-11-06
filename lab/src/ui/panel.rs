//! The struct panel: a readout's rows, the bytes of a shader struct as its
//! fields, and the cards of the sources and the notices.

use super::*;

pub fn rust_fields(name: &str) -> Vec<RustField> {
    use crate::layout::*;
    match name {
        "ObjectData" => ObjectData::fields(),
        "CameraData" => CameraData::fields(),
        "FrameUniforms" => FrameUniforms::fields(),
        _ => vec![],
    }
}

pub(super) fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
pub(super) fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub(super) fn fmt_f(v: f32) -> String {
    let v = if v.abs() < 5e-4 { 0.0 } else { v };
    format!("{v:.3}")
}

/// Vectors of three or four floats use two decimals so they fit the panel.
pub(super) fn fmt_f2(v: f32) -> String {
    let v = if v.abs() < 5e-3 { 0.0 } else { v };
    format!("{v:.2}")
}

pub(super) fn fmt_u(v: u32) -> String {
    if v == u32::MAX { "INVALID".into() } else { v.to_string() }
}

/// Displayed value lines of one field, read from the record bytes.
pub fn format_field(bytes: &[u8], field: &RustField) -> Vec<String> {
    match field.kind {
        FieldKind::F32(1) => vec![fmt_f(read_f32(bytes, field.offset))],
        FieldKind::F32(n) => {
            let f = if n >= 3 { fmt_f2 } else { fmt_f };
            vec![format!("({})", (0..n as usize).map(|i| f(read_f32(bytes, field.offset + 4 * i))).collect::<Vec<_>>().join(", "))]
        }
        FieldKind::U32(1) => vec![fmt_u(read_u32(bytes, field.offset))],
        FieldKind::U32(n) => vec![format!("({})", (0..n as usize).map(|i| fmt_u(read_u32(bytes, field.offset + 4 * i))).collect::<Vec<_>>().join(", "))],
        FieldKind::Mat3 | FieldKind::Mat4 => {
            let n = if field.kind == FieldKind::Mat3 { 3 } else { 4 };
            (0..n)
                .map(|r| (0..n).map(|c| format!("{:>7}", fmt_f(read_f32(bytes, field.offset + 16 * c + 4 * r)))).collect::<Vec<_>>().join(""))
                .collect()
        }
        FieldKind::Nested => vec!["…".into()],
    }
}

pub(super) fn draw_panel(ink: &Ink, input: &UiInput, content: &FrameContent, report: &mut UiReport) {
    let l = &input.layout;
    let state = input.state;
    if state.panel == PanelMode::Hidden || !l.panel_shown {
        return;
    }
    let painter = ink.painter;
    let rect = l.panel;
    crate::paint::panel_card(painter, rect, 12);
    if state.panel == PanelMode::Code {
        if let Some(listing) = state.code.as_deref().and_then(|c| investigation::current().listing(c)) {
            draw_code(ink, rect, listing, &state.highlight, report);
        }
        return;
    }
    if state.panel == PanelMode::Performance {
        if let Some(view) = input.performance {
            draw_performance(ink, rect, view, &state.highlight);
        }
        return;
    }
    let Some(record) = &content.panel_record else {
        if let Some(readout) = &content.readout {
            draw_readout(ink, rect, readout, &content.notes, report);
        }
        return;
    };
    let Some(layout) = input.statics.layouts.get(record.struct_name) else { return };
    let Some(decl) = input.statics.declarations.get(record.struct_name) else { return };
    report.panel_struct = Some(record.struct_name.to_string());
    let x = rect.min.x + 22.0;
    let width = rect.width() - 44.0;
    let mut y = rect.min.y + 16.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, record.struct_name, 30.0, mono_bold(), palette::text());
    ink.text(pos2(rect.max.x - 22.0, y + 4.0), Align2::RIGHT_TOP, format!("{} B / {}", layout.size, tr("ui-element")), 24.0, FontFamily::Proportional, palette::text_dim());
    y += 42.0;
    let title = ink.fit_right(&record.title, width, 24.0, FontFamily::Proportional);
    ink.text(pos2(x, y), Align2::LEFT_TOP, title, 24.0, FontFamily::Proportional, palette::accent());
    y += 32.0;
    if let Some(mapping) = sources::index().mapping(record.struct_name) {
        let text = ink.fit_right(&format!("≈ {}: {}", investigation::current().wording().engine_short, mapping.engine_short()), width, 24.0, FontFamily::Proportional);
        ink.text(pos2(x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, palette::text_faint());
        y += 34.0;
    }
    painter.line_segment([pos2(x, y), pos2(rect.max.x - 22.0, y)], Stroke::new(1.0, palette::border()));
    y += 10.0;
    let rust = rust_fields(record.struct_name);
    let highlighted = |name: &str| state.highlight.iter().any(|h| h == name);
    if state.panel == PanelMode::Memory {
        draw_memory(ink, input, record, layout, &rust, Rect::from_min_max(pos2(x, y), pos2(x + width, rect.max.y - 16.0)));
        return;
    }
    // Declaration rows with byte offsets and padding.
    let line = 27.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, format!("struct {} {{", record.struct_name), 24.0, FontFamily::Monospace, palette::text_dim());
    y += line;
    let pads = layout.padding();
    let mut cursor = 0usize;
    for (_, field_name, code) in &decl.fields {
        let Some(f) = layout.field(field_name) else { continue };
        if f.offset > cursor {
            ink.text(pos2(x + 8.0, y), Align2::LEFT_TOP, format!("{:>3}  ·· {} B padding", cursor, f.offset - cursor), 24.0, FontFamily::Monospace, palette::padding());
            y += line;
        }
        let row = Rect::from_min_size(pos2(x - 8.0, y - 2.0), vec2(width + 16.0, line));
        if highlighted(field_name) {
            painter.rect_filled(row, CornerRadius::same(4), palette::highlight_row());
            swatch(painter, Rect::from_min_size(pos2(x - 6.0, y + 2.0), vec2(6.0, line - 8.0)), &field_colors(record.struct_name, field_name));
        }
        let color = if highlighted(field_name) { palette::text() } else { palette::text_dim() };
        let text = ink.fit_right(&format!("{:>3}  {code}", f.offset), width - 8.0, 24.0, FontFamily::Monospace);
        ink.text(pos2(x + 8.0, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Monospace, color);
        y += line;
        cursor = f.offset + f.size;
    }
    if layout.size > cursor {
        ink.text(pos2(x + 8.0, y), Align2::LEFT_TOP, format!("{:>3}  ·· {} B padding", cursor, layout.size - cursor), 24.0, FontFamily::Monospace, palette::padding());
        y += line;
    }
    ink.text(pos2(x, y), Align2::LEFT_TOP, "}", 24.0, FontFamily::Monospace, palette::text_dim());
    y += line + 4.0;
    let _ = pads;
    if state.panel != PanelMode::Values {
        return;
    }
    // Values of the highlighted fields, read from the uploaded bytes.
    let bottom = rect.max.y - 14.0;
    for name in &state.highlight {
        let Some(field) = rust.iter().find(|f| f.name == name.as_str()) else { continue };
        let lines = format_field(&record.bytes, field);
        report.panel_values.push((name.clone(), lines.clone()));
        let needed = line * (lines.len() as f32 + if lines.len() > 1 { 1.0 } else { 0.0 });
        if y + needed > bottom {
            break;
        }
        if lines.len() == 1 {
            let one_line = format!("{name} = {}", lines[0]);
            if ink.measure(&one_line, 24.0, FontFamily::Monospace).x <= width {
                ink.text(pos2(x, y), Align2::LEFT_TOP, one_line, 24.0, FontFamily::Monospace, palette::text());
                y += line;
            } else {
                if y + 2.0 * line > bottom {
                    break;
                }
                ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{name} ="), 24.0, FontFamily::Monospace, palette::text());
                y += line;
                let value = ink.fit_right(&format!("  {}", lines[0]), width, 24.0, FontFamily::Monospace);
                ink.text(pos2(x, y), Align2::LEFT_TOP, value, 24.0, FontFamily::Monospace, palette::text());
                y += line;
            }
        } else {
            let storage = if matches!(field.kind, FieldKind::Mat3) { tr("ui-3_cols_16_b") } else { tr("ui-4_cols_16_b") };
            ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{name}  ({storage})"), 24.0, FontFamily::Monospace, palette::text());
            y += line;
            for row in &lines {
                ink.text(pos2(x, y), Align2::LEFT_TOP, row, 24.0, FontFamily::Monospace, palette::text());
                y += line;
            }
        }
    }
    for (note, color) in &content.notes {
        if y + line > bottom {
            break;
        }
        fitted_line(ink, pos2(x, y), note, width, *color);
        y += line;
    }
}

/// An investigation's readout (`FrameContent::readout`): its formula, names
/// on the left and values on the right, then the frame's notes.
pub(super) fn draw_readout(ink: &Ink, rect: Rect, readout: &crate::frame::Readout, notes: &[(String, Color32)], report: &mut UiReport) {
    let x = rect.min.x + 22.0;
    let width = rect.width() - 44.0;
    let mut y = rect.min.y + 16.0;
    let title = ink.fit_right(&readout.title, width, 30.0, bold());
    ink.text(pos2(x, y), Align2::LEFT_TOP, title, 30.0, bold(), palette::text());
    y += 42.0;
    if !readout.subtitle.is_empty() {
        let subtitle = ink.fit_right(&readout.subtitle, width, 24.0, FontFamily::Proportional);
        ink.text(pos2(x, y), Align2::LEFT_TOP, subtitle, 24.0, FontFamily::Proportional, palette::accent());
        y += 32.0;
    }
    ink.painter.line_segment([pos2(x, y), pos2(rect.max.x - 22.0, y)], Stroke::new(1.0, palette::border()));
    y += 10.0;
    let line = 29.0;
    let bottom = rect.max.y - 14.0;
    if let Some((latex, display, colors)) = readout.formula.as_deref().and_then(rich_text::first_formula) {
        match formula::get(&latex, formula::FORMULA_PX, display) {
            Ok(f) => {
                // Full size, unless the panel is too short for it and every row (the
                // viewer's panel is shorter than the recorder's). The padding around
                // the ink overlaps the panel's margin.
                // The ink with 4 px around it, as the readouts have always had.
                let pad = formula::PAD as f32 - 4.0;
                let size = vec2(f.width as f32, f.height as f32) - Vec2::splat(2.0 * pad);
                let rows: f32 = readout.rows.iter().map(|(n, v, _)| if n.is_empty() && v.is_empty() { 0.5 } else { 1.0 }).sum::<f32>() + notes.len() as f32;
                let room = bottom - y - 12.0 - 6.0 - rows * line;
                let scale = (width / size.x).min(room / size.y).clamp(0.5, 1.0);
                let lit = readout.formula_lit.map(|(term, weight)| (rich_text::Lit { term, weight, color: colors.get(term.wrapping_sub(1)).copied().unwrap_or(palette::accent()) }, palette::text_dim()));
                rich_text::draw_formula(ink, pos2(x, y) - Vec2::splat(pad * scale), &rich_text::Typeset { latex: &latex, size: formula::FORMULA_PX, display, formula: &f }, scale, palette::text(), lit);
                report.formula_scale = Some(scale);
                y += size.y * scale + 12.0;
            }
            Err(e) => {
                ink.text(pos2(x, y), Align2::LEFT_TOP, ink.fit_right(&format!("formula: {e}"), width, 24.0, FontFamily::Proportional), 24.0, FontFamily::Proportional, palette::MISMATCH);
                y += line;
            }
        }
    }
    for (i, (name, value, color)) in readout.rows.iter().enumerate() {
        if y + line > bottom {
            report.readout_overflow = readout.rows.len() - i + notes.len();
            return;
        }
        if name.is_empty() && value.is_empty() {
            y += line * 0.5;
            continue;
        }
        let value_width = ink.measure(value, 24.0, FontFamily::Monospace).x;
        fitted_line(ink, pos2(x, y), name, (width - value_width - 16.0).max(40.0), palette::text_dim());
        ink.text(pos2(rect.max.x - 22.0, y), Align2::RIGHT_TOP, value, 24.0, FontFamily::Monospace, *color);
        y += line;
    }
    y += 6.0;
    for (i, (note, color)) in notes.iter().enumerate() {
        if y + line > bottom {
            report.readout_overflow = notes.len() - i;
            return;
        }
        fitted_line(ink, pos2(x, y), note, width, *color);
        y += line;
    }
}

/// One line of 24 px text cut to `width`, or, holding a formula, drawn whole
/// (src/rich_text.rs).
pub(super) fn fitted_line(ink: &Ink, pos: Pos2, text: &str, width: f32, color: Color32) {
    if rich_text::is_rich(text) {
        rich_text::text(ink, pos, Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, color);
    } else {
        let text = ink.fit_right(text, width, 24.0, FontFamily::Proportional);
        ink.text(pos, Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, color);
    }
}

pub(super) fn draw_memory(ink: &Ink, input: &UiInput, record: &crate::frame::PanelRecord, layout: &crate::layout::ShaderStruct, rust: &[RustField], area: Rect) {
    let (x, width, bottom) = (area.min.x, area.width(), area.max.y);
    let mut y = area.min.y;
    let painter = ink.painter;
    let state = input.state;
    let rows = layout.size / 16;
    let cell = 15.0;
    let row_h = 28.0;
    let label_x = x + 68.0 + 16.0 * cell + 14.0;
    let pads = layout.padding();
    let is_pad = |byte: usize| pads.iter().any(|(o, s, _)| byte >= *o && byte < o + s);
    let field_at = |byte: usize| layout.fields.iter().position(|f| byte >= f.offset && byte < f.offset + f.size);
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("ui-byte_0_15"), 24.0, FontFamily::Monospace, palette::text_faint());
    y += row_h;
    for r in 0..rows {
        if y + row_h > bottom - 3.0 * row_h {
            break;
        }
        ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{:>4}", r * 16), 24.0, FontFamily::Monospace, palette::text_dim());
        let mut label = String::new();
        for b in 0..16 {
            let byte = r * 16 + b;
            let cell_rect = Rect::from_min_size(pos2(x + 68.0 + b as f32 * cell, y + 3.0), vec2(cell - 2.0, row_h - 8.0));
            let color = if is_pad(byte) {
                palette::padding()
            } else if let Some(fi) = field_at(byte) {
                let f = &layout.fields[fi];
                if label.is_empty() {
                    label = match f.matrix {
                        Some(_) => format!("{} c{}", f.name, (byte - f.offset) / 16),
                        None => f.name.clone(),
                    };
                }
                if state.highlight.iter().any(|h| h == &f.name) {
                    let colors = field_colors(record.struct_name, &f.name);
                    let column = f.matrix.map(|_| (byte - f.offset) / 16).unwrap_or(0);
                    colors[column.min(colors.len() - 1)]
                } else if fi % 2 == 0 {
                    palette::theme().field_even
                } else {
                    palette::theme().field_odd
                }
            } else {
                palette::padding()
            };
            painter.rect_filled(cell_rect, CornerRadius::same(2), color);
        }
        let text = ink.fit_right(&label, x + width - label_x, 24.0, FontFamily::Monospace);
        ink.text(pos2(label_x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Monospace, palette::text_dim());
        y += row_h;
    }
    y += 8.0;
    let pad_total: usize = pads.iter().map(|p| p.1).sum();
    ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{} B {} + {} B padding", layout.size - pad_total, tr("ui-data"), pad_total), 24.0, FontFamily::Proportional, palette::text());
    y += row_h + 2.0;
    for name in &state.highlight {
        if let (Some(f), Some(rf)) = (layout.field(name), rust.iter().find(|r| r.name == name.as_str())) {
            if y + row_h > bottom {
                break;
            }
            let text = match rf.kind {
                FieldKind::Mat3 => format!("{name}: {} B = {}", f.size, tr("ui-3_columns_16_b")),
                FieldKind::Mat4 => format!("{name}: {} B = {}", f.size, tr("ui-4_columns_16_b")),
                _ => format!("{name}: offset {}, {} B", f.offset, f.size),
            };
            let text = ink.fit_right(&text, width, 24.0, FontFamily::Proportional);
            ink.text(pos2(x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, field_colors(record.struct_name, name)[0]);
            y += row_h;
        }
    }
}

pub(super) fn draw_source_card(ink: &Ink, input: &UiInput) {
    let l = &input.layout;
    let painter = ink.painter;
    let index = sources::index();
    let rect = l.content;
    crate::paint::panel_card(painter, rect, 14);
    let x = rect.min.x + 40.0;
    let width = rect.width() - 80.0;
    let mut y = rect.min.y + 28.0;
    let heading = tr_with("ui-read_it_in", args!(engine = index.engine.name, version = index.engine.version, chapter = input.chapter_number));
    ink.text(pos2(x, y), Align2::LEFT_TOP, heading, 36.0, bold(), palette::text());
    y += 50.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{} {} · commit {}", tr("ui-paths_are_relative_to"), investigation::current().wording().checkout_dir, &index.engine.commit[..12]), 24.0, FontFamily::Proportional, palette::text_dim());
    y += 46.0;
    for key in input.beat_sources {
        let Some(entry) = index.entries.get(key) else { continue };
        if y > rect.max.y - 100.0 {
            break;
        }
        ink.text(pos2(x, y), Align2::LEFT_TOP, &entry.symbol, 28.0, mono_bold(), palette::accent());
        y += 38.0;
        let path = ink.fit_left(&format!("{}:{}", entry.path, entry.line), width, 24.0, FontFamily::Monospace);
        ink.text(pos2(x, y), Align2::LEFT_TOP, path, 24.0, FontFamily::Monospace, palette::text());
        y += 32.0;
        y = ink.wrapped(pos2(x, y), width, entry.inspect_text(key), 24.0, FontFamily::Proportional, palette::text_dim()) + 22.0;
    }
}

/// The legal notices of the investigation's package (`[[notice]]`): each
/// notice, and the file of its terms, which comes with the package.
pub(super) fn draw_notices_card(ink: &Ink, input: &UiInput) {
    let l = &input.layout;
    let painter = ink.painter;
    let rect = l.content;
    crate::paint::panel_card(painter, rect, 14);
    let x = rect.min.x + 40.0;
    let width = rect.width() - 80.0;
    let mut y = rect.min.y + 28.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("ui-notices_title"), 36.0, bold(), palette::text());
    y += 64.0;
    let package = investigation::current().package().unwrap_or("render_lab");
    for (text, license) in crate::resources::notices(package) {
        if y > rect.max.y - 90.0 {
            break;
        }
        y = ink.wrapped(pos2(x, y), width, &text, 28.0, FontFamily::Proportional, palette::text()) + 8.0;
        if let Some(file) = license.as_ref().and_then(|f| f.file_name()).map(|f| f.to_string_lossy().into_owned()) {
            ink.text(pos2(x, y), Align2::LEFT_TOP, tr_with("ui-notice_terms", args!(file = file)), 24.0, FontFamily::Proportional, palette::text_dim());
            y += 32.0;
        }
        y += 24.0;
    }
}
