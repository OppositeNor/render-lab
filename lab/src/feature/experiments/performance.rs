//! The Experiments panel's performance: the frame's time and each pass's.

use super::ExperimentView;
use crate::controls::Dock;
use crate::i18n::{args, tr, tr_with};
use crate::palette;
use crate::shot::PanelMode;
use crate::ui::ViewerAction;
use egui::{RichText, Ui, vec2};

/// The performance group: the performance panel and the frame graph's tab, the frame's time and each pass's, and
/// whether the scene data is building.
pub(super) fn performance_group(d: &mut Dock, ui: &mut Ui, view: &ExperimentView) {
    // The performance panel on the right, or back to the demonstration's panel.
    let shown = d.state.panel == PanelMode::Performance;
    let label = if shown {
        tr("experiments-hide_panel")
    } else {
        tr("experiments-show_panel")
    };
    if ui
        .selectable_label(shown, label)
        .on_hover_text(tr("experiments-show_panel_hint"))
        .clicked()
    {
        let back = d.defaults.as_ref().map_or(PanelMode::Values, |s| s.panel);
        d.actions.push(ViewerAction::SetPanel(if shown {
            back
        } else {
            PanelMode::Performance
        }));
    }
    // The frame graph (src/graph/) has a tab of its own.
    if ui
        .selectable_label(false, tr("experiments-show_graph"))
        .on_hover_text(tr("experiments-show_graph_hint"))
        .clicked()
    {
        d.actions
            .push(ViewerAction::SelectTab(crate::feature::graph::TAB));
    }
    // While the reference renders, the frame holds it too: the times count both views.
    if view.reference && !view.paused {
        ui.label(
            RichText::new(tr("experiments-timings_include_reference")).color(palette::accent()),
        );
    }
    egui::Grid::new("experiment-timings")
        .num_columns(3)
        .spacing(vec2(10.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(tr("experiments-frame")).color(palette::text_dim()));
            ui.label(RichText::new(format!("{:.2} ms", view.frame_ms.0)).color(palette::text()));
            ui.label(
                RichText::new(format!("p95 {:.2}", view.frame_ms.1)).color(palette::text_faint()),
            );
            ui.end_row();
            for (pass, mean, p95) in &view.timings {
                ui.label(RichText::new(crate::ui::pass_label(pass)).color(palette::text_dim()));
                ui.label(RichText::new(format!("{mean:.3} ms")).color(palette::text()));
                ui.label(RichText::new(format!("p95 {p95:.3}")).color(palette::text_faint()));
                ui.end_row();
            }
        });
    if view.timings.is_empty() {
        ui.label(RichText::new(tr("experiments-no_timestamps")).color(palette::text_faint()));
    }
    if let Some(seconds) = view.rebuilding {
        ui.label(
            RichText::new(tr_with(
                "experiments-rebuilding",
                args!(seconds = format!("{seconds:.1}")),
            ))
            .color(palette::accent()),
        );
    }
}
