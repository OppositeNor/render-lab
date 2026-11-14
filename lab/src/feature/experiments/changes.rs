//! The Experiments panel's changed values: what differs from the demonstration's defaults.

use crate::controls::{Dock, readout};
use crate::i18n::{args, tr, tr_with};
use crate::palette;
use crate::ui::ViewerAction;
use crate::values::Entry;
use egui::{RichText, Ui};

/// The experiment group: the values changed from the demonstration's defaults, each with its reset (the first
/// eight), and Reset all.
pub(super) fn changed_group(d: &mut Dock, ui: &mut Ui, changed: &[(&'static Entry, &'static str)]) {
    ui.set_max_width(330.0);
    ui.label(
        RichText::new(tr_with("experiments-changed", args!(count = changed.len())))
            .color(palette::text_dim()),
    );
    if changed.is_empty() {
        ui.label(RichText::new(tr("experiments-no_changes")).color(palette::text_faint()));
    }
    for (entry, channel) in changed.iter().take(8) {
        ui.horizontal(|ui| {
            let name = if entry.components.is_empty() {
                entry.label_text().to_string()
            } else {
                format!(
                    "{} {}",
                    entry.label_text(),
                    channel.rsplit('.').next().unwrap_or(channel)
                )
            };
            let value = d
                .state
                .try_get(channel)
                .map(|v| readout(v.f32() as f64))
                .unwrap_or_default();
            if ui.small_button("↺").clicked() {
                d.reset(channel);
            }
            ui.label(RichText::new(format!("{name} = {value}")).color(palette::text()));
        });
    }
    if changed.len() > 8 {
        ui.label(
            RichText::new(tr_with(
                "experiments-more",
                args!(count = changed.len() - 8),
            ))
            .color(palette::text_faint()),
        );
    }
    if ui
        .add_enabled(
            !changed.is_empty(),
            egui::Button::new(tr("experiments-reset_all")),
        )
        .clicked()
    {
        d.actions.push(ViewerAction::ResetParameters);
    }
}
