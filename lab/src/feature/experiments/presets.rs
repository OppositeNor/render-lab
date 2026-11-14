//! The Experiments panel's experiment files: saved under a name, listed, and loaded.

use super::{Asked, ExperimentView};
use crate::i18n::tr;
use crate::palette;
use egui::{RichText, Ui};

/// The presets group: a name to save the values under, and the presets saved and shipped, each loaded by a click.
pub(super) fn presets_group(ui: &mut Ui, view: &ExperimentView, asked: &mut Vec<Asked>) {
    ui.set_max_width(330.0);
    let name_id = egui::Id::unique("experiments-preset-name");
    let mut name = ui
        .data(|data| data.get_temp::<String>(name_id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::TextEdit::singleline(&mut name)
                    .hint_text(tr("experiments-preset_name"))
                    .desired_width(200.0),
            )
            .changed()
        {
            ui.data_mut(|data| data.insert_temp(name_id, name.clone()));
        }
        if ui
            .add_enabled(
                !name.trim().is_empty(),
                egui::Button::new(tr("experiments-save")),
            )
            .on_hover_text(tr("experiments-save_hint"))
            .clicked()
        {
            asked.push(Asked::SavePreset(name.trim().to_string()));
        }
    });
    for (i, preset) in view.presets.iter().enumerate() {
        let shipped = matches!(preset.source, crate::experiment::Source::Shipped(_));
        let label = if shipped {
            format!("{} · {}", preset.name, tr("experiments-shipped"))
        } else {
            preset.name.clone()
        };
        if ui
            .button(label)
            .on_hover_text(tr("experiments-load_hint"))
            .clicked()
        {
            asked.push(Asked::LoadPreset(i));
        }
    }
    if let Some(message) = &view.message {
        ui.add(egui::Label::new(RichText::new(message).color(palette::text_dim())).wrap());
    }
}
