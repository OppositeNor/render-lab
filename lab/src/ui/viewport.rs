//! The viewer's header (its scene tabs and controls), the edges a drag
//! resizes, and what is drawn over the viewport: labels, the formula card, the
//! comparison's wipe, a pane's title, and a source's callout.

use super::*;

pub(super) fn draw_header(ink: &Ink, input: &UiInput, chrome: &mut Chrome, ctx: &egui::Context) {
    let l = &input.layout;
    let engine = sources::index();
    // The engine's version and commit, then its package's copyright and license
    // line (`credit-text`), fitted beside what the left of the header holds.
    let mut right = format!("{} {} · {}", investigation::current().wording().engine_tag, engine.engine.version, &engine.engine.commit[..8]);
    if let Some(credit) = crate::resources::credit(investigation::current().package().unwrap_or("render_lab")) {
        right = format!("{right} · {credit}");
    }
    let measure = |t: &str| rich_text::measure(ink, t, 24.0, FontFamily::Proportional).x;
    let left = match chrome {
        // The chapter's tag, in bold as it is drawn below.
        Chrome::Recording { timeline, frame } => {
            let chapter = timeline.chapter_at(*frame);
            rich_text::measure(ink, &timeline.chapter_tag(chapter), 24.0, bold()).x
        }
        Chrome::Viewer(_) => {
            let investigation = investigation::current();
            let title = measure(&format!("RENDER LAB · {}", input.state.demo.title().to_uppercase()));
            let scenes = investigation.scenes();
            title + if scenes.len() > 1 { 28.0 + scenes.iter().map(|s| measure(s.title()) + 24.0).sum::<f32>() } else { 0.0 }
        }
    };
    let right = ink.fit_right(&right, (l.canvas.x - 80.0 - left - 24.0).max(200.0), 24.0, FontFamily::Proportional);
    ink.text(pos2(l.canvas.x - 40.0, 26.0), Align2::RIGHT_TOP, &right, 24.0, FontFamily::Proportional, palette::text_faint());
    match chrome {
        Chrome::Recording { timeline, frame } => {
            let chapter = timeline.chapter_at(*frame);
            let (_, beat) = timeline.beat_at(*frame);
            let tag = timeline.chapter_tag(chapter);
            ink.text(pos2(40.0, 20.0), Align2::LEFT_TOP, &tag, 24.0, bold(), palette::accent());
            let headline = ink.fit_right(&beat.headline, l.canvas.x - 80.0, 38.0, bold());
            ink.text(pos2(38.0, 52.0), Align2::LEFT_TOP, headline, 38.0, bold(), palette::text());
        }
        Chrome::Viewer(viewer) => {
            let title = ink.text(pos2(40.0, 14.0), Align2::LEFT_TOP, format!("RENDER LAB · {}", input.state.demo.title().to_uppercase()), 24.0, bold(), palette::accent());
            ink.font(24.0, FontFamily::Proportional);
            // The scene tabs, on the title's line, when the demonstrations show more than one scene.
            let investigation = investigation::current();
            let shown = investigation.demo_scene(input.state.demo);
            let scenes = investigation.scenes();
            if scenes.len() > 1 {
                egui::Area::new(egui::Id::unique("viewer-scenes")).fixed_pos(pos2(title.max.x + 28.0, 8.0)).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for scene in scenes {
                            if ui.selectable_label(scene == shown, egui::RichText::new(scene.title()).strong()).clicked() && scene != shown {
                                viewer.actions.push(ViewerAction::SelectDemo(investigation.scene_demos(scene)[0]));
                            }
                        }
                    });
                });
            }
            let response = egui::Area::new(egui::Id::unique("viewer-controls")).fixed_pos(pos2(36.0, 50.0)).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // The shown scene's demonstrations.
                    for (i, demo) in investigation.scene_demos(shown).iter().enumerate() {
                        let selected = input.state.demo == *demo && !viewer.timeline_mode;
                        if ui.selectable_label(selected, format!("{} {}", i + 1, investigation.demo_tab(*demo))).clicked() {
                            viewer.actions.push(ViewerAction::SelectDemo(*demo));
                        }
                    }
                    ui.separator();
                    if ui.button(if viewer.playing { tr("ui-pause") } else { tr("ui-play") }).clicked() {
                        viewer.actions.push(ViewerAction::TogglePlay);
                    }
                    if ui.button(tr("ui-step")).clicked() {
                        viewer.actions.push(ViewerAction::Step);
                    }
                    if ui.button(tr("ui-reset")).clicked() {
                        viewer.actions.push(ViewerAction::Reset);
                    }
                    if viewer.timeline.is_some() && ui.selectable_label(viewer.timeline_mode, tr("ui-timeline")).clicked() {
                        viewer.actions.push(ViewerAction::ToggleTimeline);
                    }
                    ui.separator();
                    if ui.button(tr("ui-home")).on_hover_text(tr("ui-home_hint")).clicked() {
                        viewer.actions.push(ViewerAction::Home);
                    }
                });
            });
            if input.scene_type == SceneType::ThreeD && !(viewer.timeline_mode && viewer.playing) {
                draw_layout_handles(ctx, ink, input, viewer);
            }
            viewer.ui_has_pointer = ctx.egui_wants_pointer_input() || response.response.hovered();
            // The status fills what the controls leave of the row.
            let room = l.canvas.x - 40.0 - response.response.rect.max.x - 24.0;
            if room > 60.0 {
                let status = ink.fit_right(&viewer.status, room, 24.0, FontFamily::Proportional);
                ink.text(pos2(l.canvas.x - 40.0, 62.0), Align2::RIGHT_TOP, status, 24.0, FontFamily::Proportional, palette::text_dim());
            }
        }
    }
}

/// The viewer's draggable edges: between the viewport and the panel (its
/// width), and above the dock (its height). A drag sets the size, its end
/// writes the layout, and a double-click puts that size back to its default.
pub(super) fn draw_layout_handles(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let l = &input.layout;
    let code = matches!(input.state.panel, PanelMode::Code | PanelMode::Knowledge | PanelMode::Dag | PanelMode::Resources);
    let mut handles: Vec<(LayoutPart, Rect, egui::CursorIcon)> = Vec::new();
    // The panel's edge, when the panel shows beside the viewport.
    if l.viewport.max.x + 1.0 < l.panel.min.x {
        let part = if code { LayoutPart::CodePanel } else { LayoutPart::Panel };
        handles.push((part, Rect::from_min_max(pos2(l.viewport.max.x, l.content.min.y), pos2(l.panel.min.x, l.content.max.y)), egui::CursorIcon::ResizeHorizontal));
    }
    handles.push((LayoutPart::Dock, Rect::from_min_max(pos2(l.captions.min.x, l.content.max.y - 2.0), pos2(l.captions.max.x, l.captions.min.y + 2.0)), egui::CursorIcon::ResizeVertical));
    for (part, rect, cursor) in handles {
        let id = egui::Id::unique(match part {
            LayoutPart::Panel => "layout-panel-edge",
            LayoutPart::CodePanel => "layout-code-edge",
            LayoutPart::Dock => "layout-dock-edge",
        });
        egui::Area::new(id).fixed_pos(rect.min).order(egui::Order::Background).show(ctx, |ui| {
            let (_, response) = ui.allocate_exact_size(rect.size(), egui::Sense::click_and_drag());
            let response = response.on_hover_text(tr("ui-resize_hint"));
            if response.hovered() || response.dragged() {
                ctx.set_cursor_icon(cursor);
                let stroke = Stroke::new(3.0, if response.dragged() { palette::accent() } else { palette::border() });
                match part {
                    LayoutPart::Dock => ink.painter.hline(rect.x_range().shrink(24.0), rect.center().y, stroke),
                    _ => ink.painter.vline(rect.center().x, rect.y_range().shrink(24.0), stroke),
                };
            }
            if response.double_clicked() {
                viewer.actions.push(ViewerAction::ResizeLayout(part, None));
                viewer.actions.push(ViewerAction::SaveLayout);
            } else if response.dragged() && let Some(p) = response.interact_pointer_pos() {
                let size = match part {
                    LayoutPart::Dock => l.captions.max.y - p.y - 6.0,
                    _ => l.panel.max.x - p.x - 12.0,
                };
                viewer.actions.push(ViewerAction::ResizeLayout(part, Some(size)));
            }
            if response.drag_stopped() {
                viewer.actions.push(ViewerAction::SaveLayout);
            }
        });
    }
}

pub(super) fn draw_labels(ink: &Ink, input: &UiInput, content: &FrameContent, obstacles: &[Rect], report: &mut UiReport) {
    let l = &input.layout;
    let projector = crate::camera::Projector::new(&content.uniforms.camera);
    let bounds = l.viewport.shrink(6.0);
    let mut placed: Vec<Rect> = Vec::new();
    let blocked = |r: &Rect| obstacles.iter().any(|o| o.intersects(*r));
    // Pinned text: exactly where it is put (a chart's names), so it neither moves nor needs a leader line.
    for t in &content.draw.texts {
        let Some((px, _)) = projector.project(t.at) else { continue };
        let at = l.viewport_pixel_to_canvas(px);
        if !l.viewport.contains(at) {
            continue;
        }
        let size = rich_text::measure(ink, &t.text, 24.0, FontFamily::Proportional);
        let min = at - vec2(size.x * t.align[0], size.y * t.align[1]);
        ink.text(min, Align2::LEFT_TOP, &t.text, 24.0, FontFamily::Proportional, t.color);
        placed.push(Rect::from_min_size(min, size));
    }
    for label in &content.labels {
        let Some((px, _)) = projector.project(label.anchor) else { continue };
        let anchor = l.viewport_pixel_to_canvas(px);
        if !l.viewport.contains(anchor) {
            continue;
        }
        let size = rich_text::measure(ink, &label.text, 24.0, FontFamily::Proportional) + vec2(22.0, 10.0);
        let near = [vec2(20.0, -size.y - 14.0), vec2(20.0, 14.0), vec2(-size.x - 20.0, -size.y - 14.0), vec2(-size.x - 20.0, 14.0), vec2(-size.x * 0.5, -size.y - 30.0), vec2(-size.x * 0.5, 30.0), vec2(36.0, -size.y * 0.5), vec2(-size.x - 36.0, -size.y * 0.5), vec2(20.0, -size.y - 60.0), vec2(20.0, 60.0)];
        // Farther rings for crowded anchors; the leader line keeps the link visible.
        let mut candidates: Vec<Rect> = near.iter().map(|o| Rect::from_min_size(anchor + *o, size)).collect();
        for ring in [2.0f32, 3.2] {
            for k in 0..12 {
                let angle = k as f32 / 12.0 * std::f32::consts::TAU;
                let d = vec2(angle.cos(), angle.sin()) * (size.y + 28.0) * ring;
                candidates.push(Rect::from_center_size(anchor + d + vec2(angle.cos() * size.x * 0.5, 0.0), size));
            }
        }
        if obstacles.iter().any(|o| o.contains(anchor)) {
            continue;
        }
        let score = |r: &Rect| {
            let overlap: f32 = placed.iter().chain(obstacles).map(|p| p.intersect(*r)).filter(|i| i.is_positive()).map(|i| i.area()).sum();
            let outside = if bounds.contains_rect(*r) { 0.0 } else { 1e6 };
            overlap + outside
        };
        let best = candidates.iter().copied().min_by(|a, b| score(a).total_cmp(&score(b))).unwrap();
        let rect = translate_inside(best, bounds);
        if placed.iter().any(|p| p.intersects(rect)) || blocked(&rect) {
            report.label_overlaps += 1;
        }
        if !bounds.contains_rect(rect) {
            report.labels_outside += 1;
        }
        placed.push(rect);
        let painter = ink.painter;
        let edge = rect.clamp(anchor);
        painter.line_segment([anchor, edge], Stroke::new(2.0, label.color));
        painter.circle_filled(anchor, 4.0, label.color);
        painter.rect_filled(rect, CornerRadius::same(6), Color32::from_rgba_premultiplied(9, 22, 36, 235));
        painter.rect_filled(Rect::from_min_max(rect.min, pos2(rect.min.x + 5.0, rect.max.y)), CornerRadius::same(2), label.color);
        painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
        rich_text::text(ink, pos2(rect.min.x + 13.0, rect.center().y), Align2::LEFT_CENTER, &label.text, 24.0, FontFamily::Proportional, palette::text());
        report.labels.push(PlacedLabel { text: label.text.clone(), anchor: [anchor.x, anchor.y], rect: [rect.min.x, rect.min.y, rect.max.x, rect.max.y] });
    }
}

pub(super) fn translate_inside(r: Rect, bounds: Rect) -> Rect {
    let mut d = vec2(0.0, 0.0);
    if r.min.x < bounds.min.x {
        d.x = bounds.min.x - r.min.x;
    } else if r.max.x > bounds.max.x {
        d.x = bounds.max.x - r.max.x;
    }
    if r.min.y < bounds.min.y {
        d.y = bounds.min.y - r.min.y;
    } else if r.max.y > bounds.max.y {
        d.y = bounds.max.y - r.max.y;
    }
    r.translate(d)
}

/// The source callout in the viewport's bottom-left corner, raised above any
/// inset it would overlap.
/// The narration's formula card (`ShotState::formula`) in the viewport's
/// top-right corner: the formula, with term `term` lit to `lit` in its tag's
/// colour for it.
pub(super) fn draw_formula_card(ink: &Ink, l: &Layout, tag: &str, term: usize, lit: f32, insets: &[Rect]) -> Option<Rect> {
    let (latex, display, colors) = rich_text::first_formula(tag)?;
    let f = formula::get(&latex, formula::FORMULA_PX, display).map_err(|e| eprintln!("formula card {latex:?}: {e}")).ok()?;
    let pad = formula::PAD as f32;
    let ink_size = vec2(f.width as f32, f.height as f32) - Vec2::splat(2.0 * pad);
    let size = ink_size + vec2(36.0, 28.0);
    let mut rect = Rect::from_min_size(pos2(l.viewport.max.x - 16.0 - size.x, l.viewport.min.y + 16.0), size);
    // Below whatever already sits in that corner (an inset, the callout).
    while let Some(inset) = insets.iter().find(|i| i.intersects(rect)) {
        rect = rect.translate(vec2(0.0, inset.max.y + 8.0 - rect.min.y));
    }
    crate::paint::overlay_card(ink.painter, rect, 10, palette::border());
    let lit = (term > 0 && lit > 0.0).then(|| (rich_text::Lit { term, weight: lit, color: colors.get(term - 1).copied().unwrap_or(palette::accent()) }, palette::text_dim()));
    rich_text::draw_formula(ink, rect.min + vec2(18.0 - pad, 14.0 - pad), &latex, formula::FORMULA_PX, display, &f, 1.0, palette::text(), lit);
    Some(rect.expand(4.0))
}

/// A view's title at the middle of its top edge (the corners hold the insets,
/// the formula card, and the callout); beside an inset it would cover when
/// the top edge has room, else below it. Returns the area it covers.
/// An experiment's comparison over the views: the wipe's line, the
/// reference's border beside the live view, and which view is which.
pub(super) fn draw_compare(ink: &Ink, view: Rect, compare: CompareOverlay, insets: &[Rect]) -> Vec<Rect> {
    use crate::experiment::Compare;
    let (a, b) = (tr("experiments-label_reference"), tr("experiments-label_live"));
    match compare.mode {
        Compare::Off => Vec::new(),
        Compare::Wipe => {
            let x = view.min.x + view.width() * compare.split.clamp(0.0, 1.0);
            ink.painter.vline(x, view.y_range(), Stroke::new(2.0, palette::accent()));
            let left = Rect::from_min_max(view.min, pos2(x, view.max.y));
            let right = Rect::from_min_max(pos2(x, view.min.y), view.max);
            let mut out = Vec::new();
            if left.width() > 120.0 {
                out.push(draw_pane_title(ink, left, a, insets));
            }
            if right.width() > 120.0 {
                let mut avoid = insets.to_vec();
                avoid.extend(out.iter().copied());
                out.push(draw_pane_title(ink, right, b, &avoid));
            }
            out
        }
        Compare::SideBySide => {
            ink.painter.rect_stroke(compare.reference.expand(1.0), CornerRadius::same(10), Stroke::new(1.5, palette::border()), StrokeKind::Outside);
            vec![draw_pane_title(ink, compare.reference, a, &[]), draw_pane_title(ink, view, b, insets)]
        }
        Compare::Difference => vec![draw_pane_title(ink, view, &tr_with("experiments-label_difference", args!(gain = format!("{:.0}", compare.gain))), insets)],
        Compare::Reference => vec![draw_pane_title(ink, view, a, insets)],
    }
}

pub(super) fn draw_pane_title(ink: &Ink, view: Rect, title: &str, insets: &[Rect]) -> Rect {
    let size = ink.measure(title, 24.0, FontFamily::Proportional) + vec2(28.0, 14.0);
    let mut rect = Rect::from_min_size(pos2(view.center().x - size.x * 0.5, view.min.y + 16.0), size);
    while let Some(inset) = insets.iter().find(|i| i.intersects(rect)) {
        let beside = Rect::from_min_size(pos2(inset.max.x + 12.0, rect.min.y), size);
        rect = if beside.max.x <= view.max.x - 16.0 && !insets.iter().any(|i| i.intersects(beside)) { beside } else { rect.translate(vec2(0.0, inset.max.y + 8.0 - rect.min.y)) };
    }
    crate::paint::overlay_card(ink.painter, rect, 8, palette::border());
    ink.text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, title, 24.0, FontFamily::Proportional, palette::text());
    rect.expand(4.0)
}

pub(super) fn draw_callout(ink: &Ink, l: &Layout, key: &str, insets: &[Rect]) -> Option<Rect> {
    let index = sources::index();
    let entry = index.entries.get(key)?;
    // Within the viewport: the symbol alone when the file name does not fit beside it.
    let room = l.viewport.width() - 32.0 - 28.0;
    let mut text = format!("{} · {}", entry.file_name(), entry.short_symbol());
    if ink.measure(&text, 24.0, FontFamily::Monospace).x > room {
        text = ink.fit_right(&entry.short_symbol(), room, 24.0, FontFamily::Monospace);
    }
    let size = ink.measure(&text, 24.0, FontFamily::Monospace) + vec2(28.0, 14.0);
    let mut rect = Rect::from_min_size(pos2(l.viewport.min.x + 16.0, l.viewport.max.y - 16.0 - size.y), size);
    for inset in insets {
        if rect.intersects(*inset) {
            rect = rect.translate(vec2(0.0, inset.min.y - 8.0 - rect.max.y));
        }
    }
    crate::paint::overlay_card(ink.painter, rect, 8, palette::accent());
    ink.text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, text, 24.0, FontFamily::Monospace, palette::accent());
    Some(rect.expand(4.0))
}
