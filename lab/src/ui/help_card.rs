//! The help card (F1, the toolbar's `?`): what the viewer accepts where it is, from the bindings' table
//! (src/bindings.rs): the viewport always, then a timeline, flying, a selected entity, the graph views, an
//! experiment's wipe, as they are shown, and the windows.

use super::{UiInput, ViewerAction, ViewerChrome};
use crate::bindings::{BINDINGS, Context, input_text};
use crate::i18n::tr;
use crate::palette;
use crate::shot::PanelMode;
use egui::{RichText, vec2};

/// The card's width in points.
const WIDTH: f32 = 380.0;

/// The contexts the viewer is in, in the card's order.
fn contexts(input: &UiInput, viewer: &ViewerChrome) -> Vec<Context> {
    let graph = matches!(
        input.state.panel,
        PanelMode::Graph | PanelMode::Dag | PanelMode::Resources | PanelMode::Knowledge
    ) || viewer.features.covered(input.state);
    let wipe = input
        .compare
        .is_some_and(|c| c.mode == crate::experiment::Compare::Wipe);
    [
        (Context::Viewport, true),
        (Context::Timeline, viewer.timeline_mode),
        (Context::Flying, true),
        (Context::Entity, viewer.selected.is_some()),
        (Context::Graph, graph),
        (Context::Wipe, wipe),
        (Context::Windows, true),
    ]
    .into_iter()
    .filter_map(|(c, shown)| shown.then_some(c))
    .collect()
}

/// A context's heading.
fn heading(context: Context) -> &'static str {
    match context {
        Context::Viewport => tr("help-context_viewport"),
        Context::Timeline => tr("help-context_timeline"),
        Context::Flying => tr("help-context_flying"),
        Context::Entity => tr("help-context_entity"),
        Context::Graph => tr("help-context_graph"),
        Context::Wipe => tr("help-context_wipe"),
        Context::Windows => tr("help-context_windows"),
    }
}

/// The card at the viewport's top right, while it is open: each context's bindings, and how it closes.
pub(super) fn draw_help_card(ctx: &egui::Context, input: &UiInput, viewer: &mut ViewerChrome) {
    if !viewer.help {
        return;
    }
    let view = input.layout.viewport;
    let at = egui::pos2(view.max.x - WIDTH - 16.0, view.min.y + 12.0);
    let shown = contexts(input, viewer);
    let mut close = false;
    egui::Area::new(egui::Id::unique("viewer-help-card"))
        .fixed_pos(at)
        .order(egui::Order::Foreground)
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(palette::panel_raised())
                .stroke(egui::Stroke::new(1.0, palette::border()))
                .corner_radius(8.0)
                .inner_margin(vec2(14.0, 10.0))
                .show(ui, |ui| {
                    ui.set_width(WIDTH - 28.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(tr("help-title"))
                                .strong()
                                .color(palette::text()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            close = ui.small_button("✕").clicked();
                        });
                    });
                    // Taller than the viewport (many contexts), the card scrolls.
                    egui::ScrollArea::vertical()
                        .id_salt("viewer-help-scroll")
                        .min_scrolled_height(view.height() - 80.0)
                        .max_height(view.height() - 80.0)
                        .show(ui, |ui| {
                            for context in shown {
                                context_rows(ui, context);
                            }
                        });
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr("help-close_hint")).color(palette::text_faint()));
                });
        });
    if close {
        viewer.actions.push(ViewerAction::ToggleHelp);
    }
}

/// A context's heading, and a row per binding of it: what the user does (one line), and what it does (wrapped).
fn context_rows(ui: &mut egui::Ui, context: Context) {
    ui.add_space(4.0);
    ui.label(RichText::new(heading(context)).color(palette::accent()));
    egui::Grid::new(("viewer-help", context as u8))
        .num_columns(2)
        .spacing(vec2(12.0, 2.0))
        .show(ui, |ui| {
            for b in BINDINGS.iter().filter(|b| b.context == context) {
                let input = RichText::new(input_text(b.input)).color(palette::text_dim());
                ui.add(egui::Label::new(input).extend());
                let does = RichText::new((b.does)()).color(palette::text());
                ui.add(egui::Label::new(does).wrap());
                ui.end_row();
            }
        });
}
