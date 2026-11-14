//! The Experiments tab's dock: every value of the investigation, filtered.

use crate::camera::OrbitCamera;
use crate::controls::Dock;
use crate::i18n::tr;
use crate::investigation;
use crate::palette;
use crate::values::{self, Entry, Reflect};
use egui::{RichText, Ui};

/// The Experiments tab's dock: every value of the investigation in
/// groups side by side (the engine's settings by label, the scene's, the
/// demonstrations' presentation, the camera), a long group in columns of
/// twelve rows; the first column has the filter, which keeps the matching ones.
pub(super) fn values(dock: &mut Dock, ui: &mut Ui) {
    let defaults = investigation::current().default_state(dock.state.demo, dock.input.statics);
    dock.defaults = Some(defaults);
    let (filter, shown_before) = filter_group(dock, ui);
    let all = values::list();
    let camera = OrbitCamera::schema();
    let is_camera = |e: &Entry| camera.iter().any(|c| std::ptr::eq(c, e));
    let needle = filter.trim().to_lowercase();
    let matches = |e: &Entry| {
        needle.is_empty()
            || [e.key, e.label_text(), e.doc, e.engine.unwrap_or("")]
                .iter()
                .any(|t| t.to_lowercase().contains(&needle))
    };
    let engine: Vec<&'static Entry> = all
        .iter()
        .copied()
        .filter(|e| e.engine.is_some() && matches(e))
        .collect();
    // A movable object's place is the Scene window's (src/entity.rs); its changes still show above.
    let scene: Vec<&'static Entry> = all
        .iter()
        .copied()
        .filter(|e| {
            e.engine.is_none()
                && !is_camera(e)
                && !e.labels.is_empty()
                && !e.toggle
                && !crate::entity::is_key(e.key)
                && matches(e)
        })
        .collect();
    let presentation: Vec<&'static Entry> = all
        .iter()
        .copied()
        .filter(|e| {
            e.engine.is_none() && !is_camera(e) && (e.labels.is_empty() || e.toggle) && matches(e)
        })
        .collect();
    if !engine.is_empty() {
        dock.group(
            ui,
            tr("experiments-engine_settings"),
            Some(tr("experiments-engine_settings_hint")),
            |d, ui| {
                engine_settings(d, ui, &engine);
            },
        );
    }
    let column = |d: &mut Dock,
                  ui: &mut Ui,
                  title: &str,
                  hint: &str,
                  id: &str,
                  entries: &[&'static Entry]| {
        if entries.is_empty() {
            return;
        }
        d.group(ui, title, Some(hint), |d, ui| {
            ui.horizontal_top(|ui| {
                for (i, chunk) in entries.chunks(12).enumerate() {
                    d.rows(ui, &format!("{id}-{i}"), |d, ui| {
                        for e in chunk {
                            d.value_rows(ui, e);
                        }
                    });
                    ui.add_space(16.0);
                }
            });
        });
    };
    column(
        dock,
        ui,
        tr("experiments-scene"),
        tr("experiments-scene_hint"),
        "experiment-scene",
        &scene,
    );
    let shown = shown_before || !needle.is_empty();
    if shown {
        column(
            dock,
            ui,
            tr("experiments-presentation"),
            tr("experiments-presentation_hint"),
            "experiment-presentation",
            &presentation,
        );
    }
    let cameras: Vec<&'static Entry> = camera
        .iter()
        .filter(|e| e.has_label("view") && matches(e))
        .collect();
    column(
        dock,
        ui,
        tr("controls-camera"),
        tr("controls-camera_hint"),
        "experiment-camera",
        &cameras,
    );
}

/// The filter's group: the filter (kept in egui's memory) and the presentation's toggle. The filter, and whether
/// the presentation was shown before this frame.
fn filter_group(dock: &mut Dock, ui: &mut Ui) -> (String, bool) {
    let filter_id = filter_id();
    let shown_id = egui::Id::unique("experiments-presentation");
    let mut filter = ui
        .data(|d| d.get_temp::<String>(filter_id))
        .unwrap_or_default();
    let mut toggle_presentation = false;
    let shown_before = ui.data(|d| d.get_temp::<bool>(shown_id)).unwrap_or(false);
    dock.group(
        ui,
        tr("controls-experiments"),
        Some(tr("experiments-experiment_hint")),
        |_, ui| {
            ui.set_max_width(260.0);
            if ui
                .add(
                    egui::TextEdit::singleline(&mut filter)
                        .hint_text(tr("experiments-filter"))
                        .desired_width(250.0),
                )
                .changed()
            {
                ui.data_mut(|data| data.insert_temp(filter_id, filter.clone()));
            }
            let label = if shown_before {
                tr("experiments-hide_presentation").to_string()
            } else {
                tr("experiments-presentation").to_string()
            };
            if ui
                .selectable_label(shown_before, label)
                .on_hover_text(tr("experiments-presentation_hint"))
                .clicked()
            {
                toggle_presentation = true;
            }
        },
    );
    if toggle_presentation {
        ui.data_mut(|d| d.insert_temp(shown_id, !shown_before));
    }
    (filter, shown_before)
}

/// The filter's place in egui's memory.
fn filter_id() -> egui::Id {
    egui::Id::unique("experiments-filter")
}

/// The dock's filter set to `key`, so the dock shows only the values it matches: the one of that key.
pub(super) fn show_only(ui: &Ui, key: &str) {
    ui.data_mut(|data| data.insert_temp(filter_id(), key.to_string()));
}

/// The engine's settings, a column per label (its package's name for the group, `controls-<label>`).
fn engine_settings(d: &mut Dock, ui: &mut Ui, engine: &[&'static Entry]) {
    let mut labels: Vec<&str> = Vec::new();
    for e in engine {
        let label = e.labels.first().copied().unwrap_or("");
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    ui.horizontal_top(|ui| {
        for label in labels {
            ui.vertical(|ui| {
                if !label.is_empty() {
                    // The package's name for the group (its dock's `controls-<label>`), else the label.
                    let package = engine
                        .iter()
                        .find(|e| e.labels.first().copied() == Some(label))
                        .map_or("render_lab", |e| e.package);
                    let title =
                        crate::i18n::try_package_text(package, &format!("controls-{label}"))
                            .unwrap_or(label);
                    ui.label(RichText::new(title).color(palette::text_faint()));
                }
                d.rows(ui, &format!("experiment-engine-{label}"), |d, ui| {
                    for e in engine
                        .iter()
                        .filter(|e| e.labels.first().copied().unwrap_or("") == label)
                    {
                        d.value_rows(ui, e);
                    }
                });
            });
            ui.add_space(16.0);
        }
    });
}
