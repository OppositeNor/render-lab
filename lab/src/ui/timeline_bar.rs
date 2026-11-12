//! Under the viewport: the captions, the notices of a reload or a skip, the
//! timeline's bar with its speed menu and preview, and the footer.

use super::*;

/// A recording's caption: a subtitle over the bottom of the picture (`Layout::for_video`), each line on a
/// dark card, the last line's foot at the band's foot. Returns where it is.
pub(super) fn draw_captions(
    ink: &Ink,
    l: &Layout,
    timeline: &Timeline,
    frame: u64,
) -> Option<Rect> {
    let (_, beat) = timeline.beat_at(frame);
    let caption = beat.caption_at(frame)?;
    let lines: Vec<&str> = caption.text.lines().collect();
    let sizes: Vec<Vec2> = lines
        .iter()
        .map(|line| rich_text::measure(ink, line, 34.0, FontFamily::Proportional))
        .collect();
    let mut y = l.captions.max.y - sizes.iter().map(|s| s.y.max(40.0) + 16.0).sum::<f32>();
    let mut area = Rect::NOTHING;
    for (line, size) in lines.iter().zip(&sizes) {
        let height = size.y.max(40.0) + 12.0;
        let card = Rect::from_center_size(
            pos2(l.captions.center().x, y + height / 2.0),
            vec2(size.x + 36.0, height),
        );
        ink.painter
            .rect_filled(card, CornerRadius::same(8), crate::paint::overlay_fill(190));
        rich_text::text(
            ink,
            pos2(card.center().x, card.center().y),
            Align2::CENTER_CENTER,
            line,
            34.0,
            FontFamily::Proportional,
            palette::text(),
        );
        area = area.union(card);
        y += height + 4.0;
    }
    Some(area)
}

/// A caption's lines with a shadow, 48 px apart, or more where a line holds a taller formula.
pub(super) fn draw_caption_lines(ink: &Ink, l: &Layout, text: &str) {
    let mut y = l.captions.min.y + 18.0;
    for line in text.lines() {
        rich_text::text(
            ink,
            pos2(l.captions.center().x + 1.5, y + 2.0),
            Align2::CENTER_TOP,
            line,
            34.0,
            FontFamily::Proportional,
            palette::theme().shadow,
        );
        let rect = rich_text::text(
            ink,
            pos2(l.captions.center().x, y),
            Align2::CENTER_TOP,
            line,
            34.0,
            FontFamily::Proportional,
            palette::text(),
        );
        y += 48.0f32.max(rect.height() + 8.0);
    }
}

/// The Captions tab: the paused caption at the size it plays, and the one
/// before it, dimmer and smaller, above it for context.
pub fn draw_caption_tab(ink: &Ink, rect: Rect, previous: Option<&str>, caption: Option<&str>) {
    let painter = ink.painter.with_clip_rect(rect);
    let ink = Ink::new(&painter);
    let Some(caption) = caption else {
        ink.text(
            rect.center(),
            Align2::CENTER_CENTER,
            tr("ui-no_caption"),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        return;
    };
    let mut y = rect.min.y + 14.0;
    for line in previous.into_iter().flat_map(str::lines) {
        let r = rich_text::text(
            &ink,
            pos2(rect.center().x, y),
            Align2::CENTER_TOP,
            line,
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        y += 34.0f32.max(r.height() + 6.0);
    }
    y += 10.0;
    for line in caption.lines() {
        let r = rich_text::text(
            &ink,
            pos2(rect.center().x, y),
            Align2::CENTER_TOP,
            line,
            32.0,
            FontFamily::Proportional,
            palette::text(),
        );
        y += 44.0f32.max(r.height() + 8.0);
    }
}

pub(super) fn draw_caption_text(ink: &Ink, l: &Layout, caption: Option<&str>) {
    let Some(caption) = caption else { return };
    draw_caption_lines(ink, l, caption);
}

/// How far the timeline just skipped, in a card on the viewport's left
/// (back) or right (forward) side, fading out.
/// The last live reload of the shaders, over the top of the viewport: the
/// files read again, or (until a reload succeeds) why it failed.
pub(super) fn draw_reload_notice(ink: &Ink, l: &Layout, text: &str, failed: bool) {
    let width = (l.viewport.width() - 48.0).max(200.0);
    let galley = ink.painter.layout(
        text.to_string(),
        egui::FontId::monospace(24.0),
        if failed {
            palette::MISMATCH
        } else {
            palette::text()
        },
        width - 28.0,
    );
    let card = Rect::from_min_size(
        pos2(l.viewport.min.x + 24.0, l.viewport.min.y + 16.0),
        vec2(
            galley.size().x + 28.0,
            galley.size().y.min(8.0 * 30.0) + 20.0,
        ),
    );
    ink.painter.rect_filled(
        card,
        CornerRadius::same(10),
        palette::panel().gamma_multiply(0.92),
    );
    ink.painter.rect_stroke(
        card,
        CornerRadius::same(10),
        Stroke::new(
            1.5,
            if failed {
                palette::MISMATCH
            } else {
                palette::accent()
            },
        ),
        StrokeKind::Inside,
    );
    ink.painter.with_clip_rect(card.shrink(4.0)).galley(
        card.min + vec2(14.0, 10.0),
        galley,
        palette::text(),
    );
    ink.min_size.set(ink.min_size.get().min(24.0));
}

pub(super) fn draw_skip_notice(ink: &Ink, l: &Layout, seconds: i64, opacity: f32) {
    let text = if seconds < 0 {
        format!("◀◀  {}", tr_with("ui-skip_back", args!(seconds = -seconds)))
    } else {
        format!(
            "{}  ▶▶",
            tr_with("ui-skip_forward", args!(seconds = seconds))
        )
    };
    let size = ink.measure(&text, 34.0, bold()) + vec2(56.0, 28.0);
    let x = if seconds < 0 {
        l.viewport.min.x + l.viewport.width() * 0.2
    } else {
        l.viewport.max.x - l.viewport.width() * 0.2
    };
    let card = Rect::from_center_size(pos2(x, l.viewport.center().y), size);
    ink.painter.rect_filled(
        card,
        CornerRadius::same(12),
        palette::panel().gamma_multiply(0.85 * opacity),
    );
    ink.painter.rect_stroke(
        card,
        CornerRadius::same(12),
        Stroke::new(1.5, palette::accent().gamma_multiply(opacity)),
        StrokeKind::Inside,
    );
    ink.text(
        card.center(),
        Align2::CENTER_CENTER,
        text,
        34.0,
        bold(),
        palette::text().gamma_multiply(opacity),
    );
}

/// The narration's volume at the timeline bar's right end: its width.
pub(super) const BAR_VOLUME: f32 = 300.0;
/// The speed menu before it: its width.
pub(super) const BAR_SPEED: f32 = 96.0;

/// The speeds a timeline plays at, its narration too (its pitch kept).
pub const SPEEDS: [f32; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 4.0];

/// A speed as the menu shows it: `1.25x`.
pub fn speed_label(speed: f32) -> String {
    format!("{}x", speed)
}

/// The speed menu: its button shows the speed, its list `SPEEDS`; the speed picked, if one was.
pub fn speed_menu(ui: &mut egui::Ui, id: &str, speed: f32, width: f32) -> (Option<f32>, Rect) {
    let mut picked = None;
    let r = egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(speed_label(speed))
        .show_ui(ui, |ui| {
            for s in SPEEDS {
                if ui
                    .selectable_label((s - speed).abs() < 1e-4, speed_label(s))
                    .clicked()
                {
                    picked = Some(s);
                }
            }
        });
    let rect = r.response.rect;
    (picked.filter(|s| (s - speed).abs() >= 1e-4), rect)
}

/// The viewer's timeline: a play/pause button, the time, a bar with the
/// chapters that seeks where it is clicked or dragged, and the narration's volume.
pub(super) fn draw_timeline_bar(
    ctx: &egui::Context,
    ink: &Ink,
    l: &Layout,
    viewer: &mut ViewerChrome,
) {
    let Some((frame, total)) = viewer.timeline else {
        return;
    };
    let painter = ink.painter;
    let button = Rect::from_min_size(
        pos2(l.footer.min.x, l.footer.min.y + 4.0),
        vec2(150.0, 40.0),
    );
    let bar = Rect::from_min_max(
        pos2(button.max.x + 24.0, l.footer.min.y + 18.0),
        pos2(
            l.footer.max.x - BAR_VOLUME - BAR_SPEED - 36.0,
            l.footer.min.y + 30.0,
        ),
    );
    let speed = Rect::from_min_max(
        pos2(bar.max.x + 24.0, l.footer.min.y + 4.0),
        pos2(bar.max.x + 24.0 + BAR_SPEED, l.footer.min.y + 44.0),
    );
    let volume = Rect::from_min_max(
        pos2(speed.max.x + 12.0, l.footer.min.y + 4.0),
        pos2(l.footer.max.x, l.footer.min.y + 44.0),
    );
    // In front of the dock, so the bar keeps its hover preview while the timeline is paused.
    // Not kept on screen: its controls are placed at the footer's rectangles, so an area
    // moved up to fit would grow over the dock every frame and take its pointer.
    let response = egui::Area::new(egui::Id::unique("timeline-bar"))
        .order(egui::Order::Foreground)
        .fixed_pos(l.footer.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_min_size(l.footer.size());
            let play = ui.put(
                button,
                egui::Button::new(if viewer.playing {
                    tr("ui-bar_pause")
                } else {
                    tr("ui-bar_play")
                }),
            );
            if play.clicked() {
                viewer.actions.push(ViewerAction::TogglePlay);
            }
            let hit = ui.allocate_rect(bar.expand2(vec2(0.0, 10.0)), egui::Sense::click_and_drag());
            if (hit.clicked() || hit.dragged())
                && let Some(p) = hit.interact_pointer_pos()
            {
                viewer
                    .actions
                    .push(ViewerAction::Seek((p.x - bar.min.x) / bar.width()));
            }
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(speed)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    let (picked, rect) =
                        speed_menu(ui, "bar:speed", viewer.speed, BAR_SPEED - 16.0);
                    viewer.widgets.push(("bar:speed".into(), rect));
                    if let Some(s) = picked {
                        viewer.actions.push(ViewerAction::SetSpeed(s));
                    }
                },
            );
            bar_volume(ui, volume, viewer);
            hit.hover_pos()
                .map(|p| ((p.x - bar.min.x) / bar.width()).clamp(0.0, 1.0))
        });
    viewer.ui_has_pointer |= response.response.hovered();
    viewer.widgets.push(("timeline-bar".into(), bar));
    painter.rect_filled(bar, CornerRadius::same(4), palette::theme().track);
    for &c in &viewer.chapters {
        let x = bar.min.x + bar.width() * c;
        painter.rect_filled(
            Rect::from_min_size(pos2(x, bar.min.y - 6.0), vec2(2.0, bar.height() + 12.0)),
            CornerRadius::ZERO,
            palette::text_faint(),
        );
    }
    let share = frame as f32 / total.max(1) as f32;
    let x = bar.min.x + bar.width() * share;
    painter.rect_filled(
        Rect::from_min_max(bar.min, pos2(x, bar.max.y)),
        CornerRadius::same(4),
        palette::accent(),
    );
    painter.circle_filled(pos2(x, bar.center().y), 10.0, palette::accent());
    if let Some(share) = response.inner {
        draw_bar_preview(ctx, bar, share, total, viewer);
    }
}

/// The narration's volume in `volume`: its slider and its mute button.
fn bar_volume(ui: &mut egui::Ui, volume: Rect, viewer: &mut ViewerChrome) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(volume)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            let mut narration = viewer.narration;
            let r = crate::volume::slider(ui, &mut narration, 32.0, BAR_VOLUME - 150.0);
            viewer.widgets.push(("bar:narration".into(), r.rail));
            viewer.widgets.push(("bar:narration-mute".into(), r.mute));
            if r.changed || r.committed {
                viewer
                    .actions
                    .push(ViewerAction::SetNarration(narration, r.committed));
            }
        },
    );
}

/// The section under the pointer on the timeline bar: its chapter, the
/// beat's headline, and the time, in a card above the bar, with the beat's
/// span lit on the bar.
pub(super) fn draw_bar_preview(
    ctx: &egui::Context,
    bar: Rect,
    share: f32,
    total: u64,
    viewer: &mut ViewerChrome,
) {
    let Some(i) = viewer.sections.iter().rposition(|s| s.start <= share) else {
        return;
    };
    let section = &viewer.sections[i];
    let end = viewer.sections.get(i + 1).map_or(1.0, |s| s.start);
    // Above every panel and the dock.
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::unique("timeline-preview"),
    ));
    let ink = Ink::new(&painter);
    let span = Rect::from_min_max(
        pos2(bar.min.x + bar.width() * section.start, bar.min.y - 3.0),
        pos2(bar.min.x + bar.width() * end, bar.max.y + 3.0),
    );
    painter.rect_filled(
        span,
        CornerRadius::same(3),
        Color32::from_rgba_unmultiplied(255, 255, 255, 60),
    );
    let x = bar.min.x + bar.width() * share;
    painter.rect_filled(
        Rect::from_min_size(
            pos2(x - 1.0, bar.min.y - 8.0),
            vec2(2.0, bar.height() + 16.0),
        ),
        CornerRadius::ZERO,
        palette::text(),
    );

    let fps = viewer.fps.max(1) as u64;
    let clock = |frame: u64| {
        let s = frame / fps;
        format!("{}:{:02}", s / 60, s % 60)
    };
    let at = (share as f64 * total as f64) as u64;
    let chapter = tr_with(
        "ui-bar_chapter",
        args!(chapter = section.chapter, title = section.chapter_title),
    );
    let width = 720.0f32;
    let chapter_line = ink.fit_right(&chapter, width - 120.0, 24.0, bold());
    let headline = ink.fit_right(&section.headline, width, 26.0, bold());
    let size = vec2(width + 36.0, 108.0);
    let left = (x - size.x / 2.0).clamp(bar.min.x - 150.0, bar.max.x - size.x);
    let card = Rect::from_min_size(pos2(left, bar.min.y - 26.0 - size.y), size);
    painter.rect_filled(card, CornerRadius::same(12), palette::theme().card);
    painter.rect_stroke(
        card,
        CornerRadius::same(12),
        Stroke::new(1.5, palette::accent()),
        StrokeKind::Inside,
    );
    ink.text(
        card.min + vec2(18.0, 14.0),
        Align2::LEFT_TOP,
        &chapter_line,
        24.0,
        bold(),
        palette::accent(),
    );
    ink.text(
        pos2(card.max.x - 18.0, card.min.y + 14.0),
        Align2::RIGHT_TOP,
        clock(at),
        24.0,
        FontFamily::Monospace,
        palette::text_dim(),
    );
    ink.text(
        card.min + vec2(18.0, 54.0),
        Align2::LEFT_TOP,
        &headline,
        26.0,
        bold(),
        palette::text(),
    );
    viewer.hover = Some(format!("{chapter} | {} | {}", section.headline, clock(at)));
}

/// The engine source entries of a demonstration: symbol, path, and line.
pub fn draw_source_list(ink: &Ink, rect: Rect, demo: DemoId) {
    let index = sources::index();
    let wording = investigation::current().wording();
    let x = rect.min.x + 20.0;
    let mut y = rect.min.y + 10.0;
    // An engine's entries are paths in its checkout; a paper's are its sections and pages.
    let heading = if index.paper.is_some() {
        tr_with(
            "ui-paper_list_heading",
            args!(engine = wording.engine_short, demo = demo.title()),
        )
    } else {
        tr_with(
            "ui-source_list_heading",
            args!(
                engine = wording.engine_short,
                demo = demo.title(),
                checkout = wording.checkout_dir
            ),
        )
    };
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        heading,
        24.0,
        bold(),
        palette::accent(),
    );
    y += 34.0;
    let keys = index.demos.get(demo.key()).cloned().unwrap_or_default();
    for key in keys.iter().take(4) {
        let entry = index.entry(key);
        let symbol_width = 520.0;
        let symbol = ink.fit_right(
            &entry.short_symbol(),
            symbol_width - 16.0,
            24.0,
            mono_bold(),
        );
        ink.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            symbol,
            24.0,
            mono_bold(),
            palette::text(),
        );
        let path = ink.fit_left(
            &entry.location(),
            rect.width() - symbol_width - 40.0,
            24.0,
            FontFamily::Monospace,
        );
        ink.text(
            pos2(x + symbol_width, y),
            Align2::LEFT_TOP,
            path,
            24.0,
            FontFamily::Monospace,
            palette::text_dim(),
        );
        y += 29.0;
    }
}

pub(super) fn draw_footer(ink: &Ink, l: &Layout, timeline: Option<(&&Timeline, u64)>) {
    let painter = ink.painter;
    let bar = Rect::from_min_size(
        pos2(l.footer.min.x, l.footer.min.y + 2.0),
        vec2(l.footer.width(), 4.0),
    );
    painter.rect_filled(bar, CornerRadius::same(2), palette::theme().track);
    if let Some((timeline, frame)) = timeline {
        let total = timeline.total_frames.max(1) as f32;
        for c in &timeline.chapters {
            let x = bar.min.x + bar.width() * c.start_frame as f32 / total;
            painter.rect_filled(
                Rect::from_min_size(pos2(x, bar.min.y - 5.0), vec2(2.0, 14.0)),
                CornerRadius::ZERO,
                palette::text_faint(),
            );
        }
        let x = bar.min.x + bar.width() * frame as f32 / total;
        painter.rect_filled(
            Rect::from_min_max(bar.min, pos2(x, bar.max.y)),
            CornerRadius::same(2),
            palette::accent(),
        );
        painter.circle_filled(pos2(x, bar.center().y), 7.0, palette::accent());
    }
    ink.text(
        pos2(l.footer.min.x, l.footer.min.y + 16.0),
        Align2::LEFT_TOP,
        investigation::current().footer(),
        24.0,
        FontFamily::Proportional,
        palette::text_faint(),
    );
}
