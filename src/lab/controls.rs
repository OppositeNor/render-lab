//! The interactive viewer's dock: sliders for what the current demonstration
//! shows, "Play animation" and "Reset parameters" buttons, and tabs with the
//! demonstration's pseudo-code and engine source. Every control sends a
//! `ViewerAction`; the viewer applies it to its `ShotState`, so the sliders
//! always show the state that is rendered. The recorder never draws the dock.
//!
//! `Dock` is the kit an investigation builds its Sliders tab from
//! (`Investigation::dock`): groups, slider rows, record indices, the camera,
//! the record browser, and checkboxes for overlay parameters.

use crate::lab::camera::OrbitCamera;
use crate::lab::i18n::{args, tr, tr_with};
use crate::lab::investigation;
use crate::lab::palette;
use crate::lab::shot::{PanelMode, RecordRef, ShotState};
use crate::lab::ui::{self, DockTab, Ink, UiInput, ViewerAction, ViewerChrome};
use egui::{pos2, vec2, Color32, CornerRadius, FontId, Rect, RichText, Stroke, StrokeKind, Ui};
use std::ops::RangeInclusive;

/// Width of the dock's button column.
const BUTTONS: f32 = 300.0;
/// Width of a slider's rail (the value box follows it).
pub const SLIDER_WIDTH: f32 = 120.0;
pub const AXES: [(&str, Color32); 3] = [("X", palette::AXIS_X), ("Y", palette::AXIS_Y), ("Z", palette::AXIS_Z)];

#[derive(Clone, Copy)]
pub enum Unit {
    Metres,
    Degrees,
    Factor,
    Plain,
    Index,
}

/// The records the Layouts demonstration can show: struct, element count,
/// and the record reference of element `i`.
type RecordKind = (&'static str, usize, fn(u32) -> RecordRef);

pub struct Dock<'a> {
    pub state: &'a ShotState,
    pub input: &'a UiInput<'a>,
    playing: bool,
    /// Parameters the demonstration's animation drives.
    animated: Vec<&'static str>,
    pub actions: Vec<ViewerAction>,
    widgets: Vec<(String, Rect)>,
}

pub fn draw_dock(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let rect = input.layout.captions;
    let painter = ink.painter;
    painter.rect_filled(rect, CornerRadius::same(10), palette::PANEL);
    painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, palette::BORDER), StrokeKind::Inside);
    let inner = rect.shrink2(vec2(16.0, 10.0));
    let divider = inner.min.x + BUTTONS + 16.0;
    painter.vline(divider, rect.y_range().shrink(12.0), Stroke::new(1.0, palette::BORDER));
    let demo = input.state.demo;
    let mut dock = Dock {
        state: input.state,
        input,
        playing: viewer.playing,
        animated: investigation::current().animation(demo, 0.0).into_iter().map(|p| p.0).collect(),
        actions: Vec::new(),
        widgets: Vec::new(),
    };
    let tab = viewer.tab;
    egui::Area::new(egui::Id::unique("viewer-dock")).fixed_pos(inner.min).fade_in(false).show(ctx, |ui| {
        ui.set_width(inner.width());
        ui.set_clip_rect(inner);
        // Labels keep their width; anything too wide shows as overflow, not as wrapped text.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let spacing = ui.spacing_mut();
        spacing.button_padding = vec2(10.0, 3.0);
        spacing.item_spacing = vec2(8.0, 4.0);
        spacing.slider_width = SLIDER_WIDTH;
        spacing.icon_width = 22.0;
        spacing.icon_width_inner = 12.0;
        spacing.icon_spacing = 8.0;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(BUTTONS);
                dock.buttons(ui, tab);
            });
            ui.add_space((divider + 16.0 - ui.cursor().min.x).max(0.0));
            if tab != DockTab::Sources {
                // Each direction scrolls once the groups overflow it (as of last frame), so a
                // tall group's lower rows stay reachable; with one direction, the wheel drives it.
                let room = vec2(inner.max.x - ui.cursor().min.x, inner.max.y - ui.cursor().min.y);
                let overflow_id = egui::Id::unique("viewer-dock-overflow");
                let overflow = ctx.data(|d| d.get_temp::<[bool; 2]>(overflow_id)).unwrap_or([true, false]);
                let output = egui::ScrollArea::new(overflow).id_salt("viewer-dock-scroll").max_width(room.x).max_height(room.y).show(ui, |ui| {
                    ui.horizontal_top(|ui| if tab == DockTab::Code { dock.code(ui) } else { investigation::current().dock(&mut dock, ui) });
                });
                ctx.data_mut(|d| d.insert_temp(overflow_id, [output.content_size.x > room.x + 0.5, output.content_size.y > room.y + 0.5]));
            }
        });
    });
    if tab == DockTab::Sources {
        ui::draw_source_list(ink, Rect::from_min_max(pos2(divider, rect.min.y), rect.max), demo);
    }
    viewer.actions.extend(dock.actions);
    viewer.widgets = dock.widgets;
}

impl Dock<'_> {
    fn buttons(&mut self, ui: &mut Ui, tab: DockTab) {
        ui.horizontal(|ui| {
            for (value, label) in [(DockTab::Parameters, tr("controls.sliders")), (DockTab::Code, tr("controls.code")), (DockTab::Sources, tr("controls.source"))] {
                if ui.selectable_label(tab == value, label).clicked() {
                    self.actions.push(ViewerAction::SelectTab(value));
                }
            }
        });
        let full = vec2(BUTTONS, 0.0);
        let label = if self.playing { tr("controls.stop_animation") } else { tr("controls.play_animation") };
        let play = ui.add(egui::Button::new(label).min_size(full));
        if play.clicked() {
            self.actions.push(ViewerAction::TogglePlay);
        }
        play.on_hover_text(tr("controls.play_animation_hint"));
        let reset = ui.add(egui::Button::new(tr("controls.reset_parameters")).min_size(full));
        if reset.on_hover_text(tr("controls.reset_parameters_hint")).clicked() {
            self.actions.push(ViewerAction::ResetParameters);
        }
    }

    /// The Code tab: this demonstration's listings, and one button per step
    /// (tag) that lights its lines in the panel.
    fn code(&mut self, ui: &mut Ui) {
        let demo = self.state.demo;
        let listings: Vec<_> = investigation::current().listings().iter().filter(|l| l.demo.as_deref() == Some(demo.key())).collect();
        if listings.is_empty() {
            ui.label(RichText::new(tr("controls.no_listing")).color(palette::TEXT_DIM));
            return;
        }
        let engine = investigation::current().wording().engine_short;
        let hint = tr_with("controls.listing_hint", args!(engine = engine));
        self.group(ui, tr("controls.listing"), Some(&hint), |d, ui| {
            for listing in &listings {
                let selected = d.state.code.as_deref() == Some(listing.name);
                if ui.selectable_label(selected, listing.title()).clicked() {
                    d.actions.push(ViewerAction::SetCode(listing.name));
                }
            }
        });
        let Some(listing) = self.state.code.as_deref().and_then(|c| investigation::current().listing(c)) else { return };
        let mut tags: Vec<&str> = Vec::new();
        for line in &listing.lines {
            for tag in &line.tags {
                if !tags.contains(&tag.as_str()) {
                    tags.push(tag);
                }
            }
        }
        self.group(ui, tr("controls.steps"), Some(tr("controls.steps_hint")), |d, ui| {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(520.0);
                for tag in tags {
                    let lit = d.state.highlight.iter().any(|h| h == tag);
                    if ui.selectable_label(lit, tag).clicked() {
                        d.actions.push(ViewerAction::SetHighlight(if lit { Vec::new() } else { vec![tag.to_string()] }));
                    }
                }
            });
        });
    }

    /// Any element of any record buffer, its struct panel mode, and the highlighted field.
    pub fn layouts(&mut self, ui: &mut Ui) {
        let statics = self.input.statics;
        #[cfg(commercial)]
        let fx = &statics.fixtures;
        // Without src/commercial there are no distance fields or cards.
        #[cfg(commercial)]
        let [voxels, bricks, cards, pages] = [fx.distance_field.voxels.len(), fx.distance_field.bricks.len(), fx.surface_cache.cards.len(), fx.surface_cache.pages.len()];
        #[cfg(not(commercial))]
        let [voxels, bricks, cards, pages] = [0; 4];
        let content = self.input.content;
        let kinds: [RecordKind; 9] = [
            ("ObjectData", statics.scene.objects.len(), |i| RecordRef::Object { index: i }),
            ("VoxelSample", voxels, |i| RecordRef::Voxel { index: i }),
            ("BrickEntry", bricks, |i| RecordRef::Brick { index: i }),
            ("CardData", cards, |i| RecordRef::Card { index: i }),
            ("PageEntry", pages, |i| RecordRef::Page { index: i }),
            ("RayDebugData", 1, |_| RecordRef::Ray),
            ("CameraData", 1, |_| RecordRef::Camera),
            ("ProbeRecord", content.map_or(0, |c| c.probes.len()), |i| RecordRef::Probe { index: i }),
            ("RadianceSample", content.map_or(0, |c| c.radiance.len()), |i| RecordRef::Radiance { index: i }),
        ];
        let (kind, index) = match self.state.record {
            RecordRef::Object { index } => (0, index),
            RecordRef::Voxel { index } => (1, index),
            RecordRef::Brick { index } => (2, index),
            RecordRef::Card { index } => (3, index),
            RecordRef::Page { index } => (4, index),
            RecordRef::Ray => (5, 0),
            RecordRef::Camera => (6, 0),
            RecordRef::Probe { index } => (7, index),
            RecordRef::Radiance { index } => (8, index),
            RecordRef::None => (3, 0),
        };
        let (name, count, make) = kinds[kind];
        self.group(ui, tr("controls.record"), None, |d, ui| {
            egui::ComboBox::from_id_salt("struct").selected_text(name).width(250.0).show_ui(ui, |ui| {
                for (other, _, make) in kinds {
                    if ui.selectable_label(other == name, other).clicked() && other != name {
                        d.actions.push(ViewerAction::SetRecord(make(0)));
                    }
                }
            });
            if count > 1 {
                d.rows(ui, "record", |d, ui| d.index(ui, "index", Some(index), count, make));
            }
        });
        self.group(ui, tr("controls.struct_panel"), None, |d, ui| {
            for (mode, label) in [(PanelMode::Declaration, tr("controls.declaration")), (PanelMode::Values, tr("controls.values")), (PanelMode::Memory, tr("controls.memory"))] {
                if ui.selectable_label(d.state.panel == mode, label).clicked() {
                    d.actions.push(ViewerAction::SetPanel(mode));
                }
            }
        });
        let fields: Vec<String> = statics.layouts.get(name).map(|s| s.fields.iter().map(|f| f.name.clone()).collect()).unwrap_or_default();
        let none = tr("controls.none");
        self.group(ui, tr("controls.highlight"), None, |d, ui| {
            let current = d.state.highlight.first().cloned();
            egui::ComboBox::from_id_salt("highlight").selected_text(current.clone().unwrap_or_else(|| none.to_string())).width(250.0).show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), none).clicked() {
                    d.actions.push(ViewerAction::SetHighlight(Vec::new()));
                }
                for field in &fields {
                    if ui.selectable_label(current.as_ref() == Some(field), field.as_str()).clicked() {
                        d.actions.push(ViewerAction::SetHighlight(vec![field.clone()]));
                    }
                }
            });
        });
    }

    pub fn camera(&mut self, ui: &mut Ui) {
        let c = self.state.camera;
        let hint = tr("controls.camera_hint");
        self.group(ui, tr("controls.camera"), Some(hint), |d, ui| {
            d.rows(ui, "camera", |d, ui| {
                let wrapped = (c.yaw + 180.0).rem_euclid(360.0) - 180.0;
                let set = |d: &mut Self, camera: OrbitCamera| d.actions.push(ViewerAction::SetCamera(camera));
                if let Some(yaw) = d.number(ui, "cam.yaw", "yaw", palette::TEXT, wrapped, -180.0..=180.0, Unit::Degrees) {
                    set(d, OrbitCamera { yaw, ..c });
                }
                if let Some(pitch) = d.number(ui, "cam.pitch", "pitch", palette::TEXT, c.pitch, -85.0..=85.0, Unit::Degrees) {
                    set(d, OrbitCamera { pitch, ..c });
                }
                if let Some(distance) = d.number(ui, "cam.distance", tr("controls.distance"), palette::TEXT, c.distance, 0.4..=15.0, Unit::Metres) {
                    set(d, OrbitCamera { distance, ..c });
                }
                if let Some(fov) = d.number(ui, "cam.fov", "fov", palette::TEXT, c.fov, 20.0..=90.0, Unit::Degrees) {
                    set(d, OrbitCamera { fov, ..c });
                }
            })
        });
    }

    /// Checkboxes for overlay parameters (0 hidden, 1 shown), four per column.
    pub fn show(&mut self, ui: &mut Ui, toggles: &[(&'static str, &str)]) {
        self.group(ui, tr("controls.show"), None, |d, ui| {
            ui.horizontal_top(|ui| {
                for (i, column) in toggles.chunks(4).enumerate() {
                    if i > 0 {
                        ui.add_space(20.0);
                    }
                    ui.vertical(|ui| {
                        for (param, label) in column {
                            d.toggle(ui, param, label);
                        }
                    });
                }
            });
        });
    }

    /// One checkbox for an overlay parameter (0 hidden, 1 shown).
    pub fn toggle(&mut self, ui: &mut Ui, param: &'static str, label: &str) {
        let mut on = self.state.p(param) > 0.5;
        let r = ui.checkbox(&mut on, RichText::new(label).color(self.color(param, palette::TEXT)));
        self.widgets.push((param.to_string(), r.rect));
        if r.changed() {
            self.actions.push(ViewerAction::SetParam(param, if on { 1.0 } else { 0.0 }));
        }
    }

    pub fn group(&mut self, ui: &mut Ui, title: &str, hint: Option<&str>, add: impl FnOnce(&mut Self, &mut Ui)) {
        ui.vertical(|ui| {
            let heading = ui.label(RichText::new(title).font(FontId::new(24.0, ui::bold())).color(palette::ACCENT));
            if let Some(hint) = hint {
                heading.on_hover_text(hint);
            }
            add(self, ui);
        });
        ui.add_space(22.0);
    }

    /// Slider rows with their labels aligned in a column.
    pub fn rows(&mut self, ui: &mut Ui, id: &str, add: impl FnOnce(&mut Self, &mut Ui)) {
        egui::Grid::new(id).num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| add(self, ui));
    }

    /// Animated parameters are drawn in the accent colour while they play.
    pub fn color(&self, key: &str, normal: Color32) -> Color32 {
        if self.playing && self.animated.contains(&key) {
            palette::ACCENT
        } else {
            normal
        }
    }

    /// One grid row: a label and a slider. Returns the new value when the
    /// user changed it.
    #[allow(clippy::too_many_arguments)]
    pub fn number(&mut self, ui: &mut Ui, key: &str, label: &str, color: Color32, value: f32, range: RangeInclusive<f32>, unit: Unit) -> Option<f32> {
        ui.label(RichText::new(label).color(self.color(key, color)));
        let mut v = value;
        // Values outside the range (an orbit, a click on the floor) stay as they are until dragged.
        let slider = egui::Slider::new(&mut v, range).clamping(egui::SliderClamping::Edits).trailing_fill(true);
        let slider = match unit {
            Unit::Metres => slider.suffix(" m").fixed_decimals(2),
            Unit::Degrees => slider.suffix("°").fixed_decimals(0),
            Unit::Factor => slider.suffix("×").fixed_decimals(2),
            Unit::Plain => slider.fixed_decimals(2),
            Unit::Index => slider.integer(),
        };
        let r = ui.add(slider);
        ui.end_row();
        self.widgets.push((key.to_string(), r.rect));
        r.changed().then_some(v)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn slider(&mut self, ui: &mut Ui, param: &'static str, label: &str, color: Color32, range: RangeInclusive<f32>, unit: Unit) {
        if let Some(v) = self.number(ui, param, label, color, self.state.p(param), range, unit) {
            self.actions.push(ViewerAction::SetParam(param, v));
        }
    }

    /// A slider over the elements of a record buffer (`samples[i]`).
    pub fn index(&mut self, ui: &mut Ui, label: &str, current: Option<u32>, count: usize, make: fn(u32) -> RecordRef) {
        if count == 0 {
            return;
        }
        let value = current.unwrap_or(0).min(count as u32 - 1) as f32;
        if let Some(v) = self.number(ui, label, label, palette::SELECTED, value, 0.0..=(count - 1) as f32, Unit::Index) {
            self.actions.push(ViewerAction::SetRecord(make(v.round() as u32)));
        }
    }
}
