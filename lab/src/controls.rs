//! The interactive viewer's dock, the Controls window (src/ui/windows.rs): the tabs (the viewer's
//! features', src/feature/), the "Reset parameters" and "Free camera" buttons, and the shown tab's
//! groups: the sliders for what the current demonstration shows, its pseudo-code, its engine source.
//! Every control sends a `ViewerAction`; the viewer applies it to its
//! `ShotState`, so the sliders always show the state that is rendered. The recorder never draws the dock.
//!
//! `Dock` is the kit an investigation builds its Sliders tab from
//! (`Investigation::dock`): groups, slider rows, record indices, the camera,
//! the record browser, and checkboxes for overlay parameters.

use crate::camera::OrbitCamera;
use crate::i18n::{args, tr, tr_with};
use crate::investigation;
use crate::palette;
use crate::shot::{PanelMode, RecordRef, ShotState};
use crate::ui::{self, DockTab, Ink, UiInput, ViewerAction, ViewerChrome};
use crate::values::{self, Entry, Reflect, Type, Value};
use egui::{pos2, vec2, Color32, FontId, Rect, RichText, Stroke, Ui};
use std::ops::RangeInclusive;

/// Width of a slider's rail (the value box follows it).
pub const SLIDER_WIDTH: f32 = 120.0;
pub const AXES: [(&str, Color32); 3] = [("X", palette::AXIS_X), ("Y", palette::AXIS_Y), ("Z", palette::AXIS_Z)];

pub use crate::values::Unit;

/// The records the Layouts demonstration can show: struct, element count,
/// and the record reference of element `i`.
type RecordKind = (&'static str, usize, fn(u32) -> RecordRef);

pub struct Dock<'a> {
    pub state: &'a ShotState,
    pub input: &'a UiInput<'a>,
    playing: bool,
    /// Parameters the demonstration's animation drives.
    animated: Vec<String>,
    pub actions: Vec<ViewerAction>,
    /// Named widget rectangles, for the autotests.
    pub widgets: Vec<(String, Rect)>,
    /// The demonstration's defaults, when a tab's rows mark each changed value
    /// and offer to reset it (the Experiments tab's).
    pub(crate) defaults: Option<ShotState>,
    /// The dock's tabs (the viewer's features', src/feature/), with their labels.
    tabs: Vec<(DockTab, String)>,
}

/// The Controls window (src/ui/windows.rs): the tabs, with the buttons that act on the demonstration at
/// the row's end, then the tab's groups, scrolled once they overflow the window.
pub fn draw_dock(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let Some(window) = input.layout.window(ui::windows::WindowId::Controls).filter(|w| !w.collapsed) else { return };
    let inner = window.body.shrink2(vec2(16.0, 10.0));
    let mut dock = Dock::new(input, viewer);
    let tab = viewer.tab;
    let features = &mut viewer.features;
    let side = features.feature(tab.0).is_some_and(|f| f.layout().side);
    let mut groups_top = inner.min.y;
    // Not constrained to the screen: a column taller than the window would move the whole
    // window's content up (its top clipped); the clip rect cuts its overflow at the bottom instead.
    egui::Area::new(egui::Id::unique("viewer-dock")).fixed_pos(inner.min).constrain(false).fade_in(false).show(ctx, |ui| {
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
        ui.horizontal(|ui| {
            dock.tab_row(ui, tab);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| dock.buttons(ui));
        });
        let rule = ui.cursor().min.y + 2.0;
        ui.painter().hline(inner.x_range(), rule, Stroke::new(1.0, palette::border()));
        ui.add_space(12.0);
        groups_top = ui.cursor().min.y;
        if !side {
            // Each direction scrolls once the groups overflow it (as of last frame), so a
            // tall group's lower rows stay reachable; with one direction, the wheel drives it.
            let room = vec2(inner.max.x - ui.cursor().min.x, inner.max.y - ui.cursor().min.y);
            let overflow_id = egui::Id::unique("viewer-dock-overflow");
            let overflow = ctx.data(|d| d.get_temp::<[bool; 2]>(overflow_id)).unwrap_or([true, false]);
            let output = egui::ScrollArea::new(overflow).id_salt("viewer-dock-scroll").max_width(room.x).max_height(room.y).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // A feature's tab (src/feature/) has its own dock, or the investigation's.
                    if !features.state_mut(tab.0).is_some_and(|state| state.dock(&mut dock, ui)) {
                        investigation::current().dock(&mut dock, ui);
                    }
                });
            });
            ctx.data_mut(|d| d.insert_temp(overflow_id, [output.content_size.x > room.x + 0.5, output.content_size.y > room.y + 0.5]));
        }
    });
    if side && let Some(state) = viewer.features.state_mut(tab.0) {
        state.side(ink, Rect::from_min_max(pos2(window.body.min.x, groups_top - 10.0), window.body.max), input);
    }
    viewer.actions.extend(dock.actions);
    viewer.widgets = dock.widgets;
}

impl<'a> Dock<'a> {
    pub(crate) fn new(input: &'a UiInput<'a>, viewer: &ViewerChrome) -> Dock<'a> {
        Dock {
            // The focused view's (src/view.rs): its camera and values.
            state: input.focus.unwrap_or(input.state),
            input,
            playing: viewer.playing,
            animated: {
                // The investigation's animation's keys, and its script animation's (src/script/runtime.rs).
                let mut keys: Vec<String> = investigation::current().animation(input.state.demo, 0.0).into_iter().map(|p| p.0.to_string()).collect();
                keys.extend(crate::script::runtime::animate(input.state.demo, 0.0, &mut input.state.clone()).unwrap_or_default());
                keys
            },
            actions: Vec::new(),
            widgets: Vec::new(),
            defaults: None,
            tabs: viewer.features.tabs(viewer.timeline_mode),
        }
    }
}

impl Dock<'_> {
    /// The tabs, one selectable label each.
    fn tab_row(&mut self, ui: &mut Ui, tab: DockTab) {
        for (value, label) in std::mem::take(&mut self.tabs) {
            if ui.selectable_label(tab == value, RichText::new(label).strong()).clicked() {
                self.actions.push(ViewerAction::SelectTab(value));
            }
        }
    }

    /// What acts on the demonstration, right to left: the free camera, and its values back to their defaults.
    fn buttons(&mut self, ui: &mut Ui) {
        let free = ui.add(egui::Button::new(tr("controls-free_camera")).selected(self.state.free_camera.on));
        self.widgets.push(("free-camera".into(), free.rect));
        if free.on_hover_text(tr("controls-free_camera_hint")).clicked() {
            self.actions.push(ViewerAction::ToggleFreeCamera);
        }
        let reset = ui.add(egui::Button::new(tr("controls-reset_parameters")));
        if reset.on_hover_text(tr("controls-reset_parameters_hint")).clicked() {
            self.actions.push(ViewerAction::ResetParameters);
        }
    }

    /// Any element of any record buffer, its struct panel mode, and the highlighted field.
    pub fn layouts(&mut self, ui: &mut Ui) {
        let statics = self.input.statics;
        let kinds: [RecordKind; 2] = [
            ("ObjectData", statics.scene.objects.len(), |i| RecordRef::Object { index: i }),
            ("CameraData", 1, |_| RecordRef::Camera),
        ];
        let (kind, index) = match self.state.record {
            RecordRef::Object { index } => (0, index),
            RecordRef::Camera => (1, 0),
            RecordRef::None => (0, 0),
        };
        let (name, count, make) = kinds[kind];
        self.group(ui, tr("controls-record"), Some(tr("controls-record_hint")), |d, ui| {
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
        self.group(ui, tr("controls-struct_panel"), Some(tr("controls-struct_panel_hint")), |d, ui| {
            for (mode, label) in [(PanelMode::Declaration, tr("controls-declaration")), (PanelMode::Values, tr("controls-values")), (PanelMode::Memory, tr("controls-memory"))] {
                if ui.selectable_label(d.state.panel == mode, label).clicked() {
                    d.actions.push(ViewerAction::SetPanel(mode));
                }
            }
        });
        let fields: Vec<String> = statics.layouts.get(name).map(|s| s.fields.iter().map(|f| f.name.clone()).collect()).unwrap_or_default();
        let none = tr("controls-none");
        self.group(ui, tr("controls-highlight"), Some(tr("controls-highlight_hint")), |d, ui| {
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

    /// The camera's values labelled `view` (src/camera.rs).
    pub fn camera(&mut self, ui: &mut Ui) {
        let entries = OrbitCamera::schema().iter().filter(|e| e.has_label("view")).collect();
        self.entries(ui, tr("controls-camera"), Some(tr("controls-camera_hint")), "camera", entries);
    }

    /// A group of the values labelled `label` (src/values.rs), one row each
    /// with the range, unit, and name its declaration gives: a vector's
    /// components in rows of their own, a toggle as a checkbox.
    pub fn values(&mut self, ui: &mut Ui, title: &str, hint: Option<&str>, label: &str) {
        self.entries(ui, title, hint, label, values::labelled(label));
    }

    /// A group of these values' rows (`id` names its grid).
    fn entries(&mut self, ui: &mut Ui, title: &str, hint: Option<&str>, id: &str, entries: Vec<&'static Entry>) {
        if entries.is_empty() {
            return;
        }
        self.group(ui, title, hint, |d, ui| {
            d.rows(ui, id, |d, ui| {
                for entry in entries {
                    d.value_rows(ui, entry);
                }
            })
        });
    }

    /// The rows of the value `key` (src/values.rs), inside `rows`: for a
    /// group that mixes values with sliders of its own.
    pub fn value(&mut self, ui: &mut Ui, key: &str) {
        let entry = values::entry(key).unwrap_or_else(|| panic!("no value `{key}`"));
        self.value_rows(ui, entry);
    }

    /// The options of the choice `key` (src/values.rs), without its name: a
    /// combo box for more than three, else one selectable label each.
    pub fn choice(&mut self, ui: &mut Ui, key: &str) {
        ui.horizontal(|ui| {
            self.choice_options(ui, key);
            self.help_mark(ui, key);
        });
    }

    /// `choice` without its "?" (a value row has one after its name).
    fn choice_options(&mut self, ui: &mut Ui, key: &str) {
        let entry = values::entry(key).unwrap_or_else(|| panic!("no value `{key}`"));
        let choices = entry.choices.unwrap_or_else(|| panic!("`{key}` is not a choice"));
        let count = choices.options.len();
        let current = (self.state.p(entry.key).round().max(0.0) as usize).min(count - 1);
        let mut picked = None;
        if count > 3 {
            egui::ComboBox::from_id_salt(entry.key).selected_text(choices.option_text(current)).width(250.0).show_ui(ui, |ui| {
                for i in 0..count {
                    if ui.selectable_label(i == current, choices.option_text(i)).clicked() {
                        picked = Some(i);
                    }
                }
            });
        } else {
            for i in 0..count {
                if ui.selectable_label(i == current, choices.option_text(i)).clicked() {
                    picked = Some(i);
                }
            }
        }
        if let Some(i) = picked {
            self.actions.push(ViewerAction::SetParam(entry.key, i as f32));
        }
    }

    /// One value's rows.
    pub(crate) fn value_rows(&mut self, ui: &mut Ui, entry: &'static Entry) {
        if entry.choices.is_some() {
            let color = self.color(entry.key, entry.color.map_or(palette::text(), palette::themed));
            let label = ui.horizontal(|ui| {
                let label = ui.label(RichText::new(entry.label_text()).color(color));
                self.help_mark(ui, entry.key);
                label
            }).inner;
            self.describe(label, entry);
            ui.horizontal(|ui| self.choice_options(ui, entry.key));
            self.reset_cell(ui, entry.key);
            ui.end_row();
            return;
        }
        let camera = OrbitCamera::schema().iter().any(|c| std::ptr::eq(c, entry));
        let channels = entry.channels();
        for (i, channel) in channels.iter().copied().enumerate() {
            let range = entry.range_of(i).unwrap_or((0.0, 1.0));
            let current = if camera { self.state.camera.get(channel).expect("a camera value").f32() } else { self.state.p(channel) };
            if entry.ty == Type::Bool || entry.toggle {
                let color = self.color(channel, entry.color.map_or(palette::text(), palette::themed));
                let label = ui.horizontal(|ui| {
                    let label = ui.label(RichText::new(entry.label_text()).color(color));
                    self.help_mark(ui, channel);
                    label
                }).inner;
                self.describe(label, entry);
                let mut on = current > 0.5;
                if ui.checkbox(&mut on, "").changed() {
                    self.actions.push(ViewerAction::SetParam(channel, if on { 1.0 } else { 0.0 }));
                }
                self.reset_cell(ui, channel);
                ui.end_row();
                continue;
            }
            let (label, color) = if channels.len() > 1 { (["x", "y", "z", "w"][i], AXES.get(i).map_or(palette::text(), |a| a.1)) } else { (entry.label_text(), entry.color.map_or(palette::text(), palette::themed)) };
            // An angle shows wrapped into its range (a yaw of 400° as 40°).
            let shown = if entry.wrap { (current - range.0).rem_euclid(range.1 - range.0) + range.0 } else { current };
            if let Some(v) = self.number(ui, channel, SliderRow { label, color, range: range.0..=range.1, unit: entry.unit }, shown) {
                if camera {
                    let mut c = self.state.camera;
                    c.set(channel, Value::F32(v));
                    self.actions.push(ViewerAction::SetCamera(c));
                } else {
                    self.actions.push(ViewerAction::SetParam(channel, v));
                }
            }
        }
    }

    /// Checkboxes for overlay parameters (0 hidden, 1 shown), four per column.
    pub fn show(&mut self, ui: &mut Ui, toggles: &[(&'static str, &str)]) {
        self.group(ui, tr("controls-show"), Some(tr("controls-show_hint")), |d, ui| {
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
        let color = self.color(param, palette::text());
        let r = ui.horizontal(|ui| {
            let r = ui.checkbox(&mut on, RichText::new(label).color(color));
            self.help_mark(ui, param);
            r
        }).inner;
        self.widgets.push((param.to_string(), r.rect));
        if r.changed() {
            self.actions.push(ViewerAction::SetParam(param, if on { 1.0 } else { 0.0 }));
        }
    }

    pub fn group(&mut self, ui: &mut Ui, title: &str, hint: Option<&str>, add: impl FnOnce(&mut Self, &mut Ui)) {
        ui.vertical(|ui| {
            // A group that explains itself has a "?" after its title, as each value has.
            ui.horizontal(|ui| {
                let heading = ui.label(RichText::new(title).font(FontId::new(24.0, ui::bold())).color(palette::accent()));
                if let Some(hint) = hint.filter(|h| !h.is_empty()) {
                    heading.on_hover_text(hint);
                    hint_mark(ui, hint);
                }
            });
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
        if self.playing && self.animated.iter().any(|k| k == key) {
            palette::accent()
        } else {
            normal
        }
    }

    /// One grid row: a label and a slider, `key`'s, at `value`. Returns the
    /// new value when the user changed it.
    pub fn number(&mut self, ui: &mut Ui, key: &str, row: SliderRow, value: f32) -> Option<f32> {
        let SliderRow { label, color, range, unit } = row;
        let color = self.color(key, color);
        let name = ui.horizontal(|ui| {
            let name = ui.label(RichText::new(label).color(color));
            self.help_mark(ui, key);
            name
        }).inner;
        if let Some(entry) = self.defaults.as_ref().and(values::entry(key)) {
            self.describe(name, entry);
        }
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
        let r = ui.scope(|ui| {
            crate::text::slider_row(ui);
            ui.add(slider)
        }).inner;
        self.reset_cell(ui, key);
        ui.end_row();
        self.widgets.push((key.to_string(), r.rect));
        r.changed().then_some(v)
    }

    /// A grid row with a slider for the parameter `param`.
    pub fn slider(&mut self, ui: &mut Ui, param: &'static str, row: SliderRow) {
        if let Some(v) = self.number(ui, param, row, self.state.p(param)) {
            self.actions.push(ViewerAction::SetParam(param, v));
        }
    }

    /// A slider over the elements of a record buffer (`samples[i]`).
    pub fn index(&mut self, ui: &mut Ui, label: &str, current: Option<u32>, count: usize, make: fn(u32) -> RecordRef) {
        if count == 0 {
            return;
        }
        let value = current.unwrap_or(0).min(count as u32 - 1) as f32;
        if let Some(v) = self.number(ui, label, SliderRow { label, color: palette::SELECTED, range: 0.0..=(count - 1) as f32, unit: Unit::Index }, value) {
            self.actions.push(ViewerAction::SetRecord(make(v.round() as u32)));
        }
    }
}

/// How a slider row looks: its label and colour, and the slider's range
/// and unit.
pub struct SliderRow<'a> {
    pub label: &'a str,
    pub color: Color32,
    pub range: RangeInclusive<f32>,
    pub unit: Unit,
}

/// A "?" that shows `text` when hovered.
fn hint_mark(ui: &mut Ui, text: &str) {
    let mark = RichText::new(" ? ").monospace().color(palette::text_dim()).background_color(palette::background());
    ui.add(egui::Label::new(mark).sense(egui::Sense::hover())).on_hover_text(text);
}

/// A number for a readout: three significant decimals, "∞" for an infinite one.
pub(crate) fn readout(x: f64) -> String {
    if x.is_infinite() {
        "∞".into()
    } else if x.is_nan() {
        "–".into()
    } else if x != 0.0 && x.abs() < 0.001 {
        format!("{x:.2e}")
    } else {
        format!("{x:.4}")
    }
}

impl Dock<'_> {
    /// Whether channel `key` differs from the demonstration's default (`defaults`).
    pub(crate) fn changed(&self, key: &str) -> bool {
        let Some(defaults) = &self.defaults else { return false };
        match (self.state.try_get(key), defaults.try_get(key)) {
            (Some(a), Some(b)) => a.components().iter().zip(b.components()).any(|(x, y)| (x - y).abs() > 1e-5),
            _ => false,
        }
    }

    /// Set channel `key` back to its default (`defaults`), as its slider (the camera's) would.
    pub(crate) fn reset(&mut self, key: &'static str) {
        let Some(value) = self.defaults.as_ref().and_then(|d| d.try_get(key)) else { return };
        let action = if OrbitCamera::schema().iter().any(|e| e.key == key || e.components.contains(&key)) {
            let mut camera = self.state.camera;
            camera.set(key, value);
            ViewerAction::SetCamera(camera)
        } else {
            ViewerAction::SetParam(key, value.f32())
        };
        self.actions.push(action);
    }

    /// With `defaults`, a row's last cell: a reset button for a changed value (`reset`).
    fn reset_cell(&mut self, ui: &mut Ui, key: &str) {
        let Some(defaults) = &self.defaults else { return };
        let channel = values::entry(key).and_then(|e| e.channels().into_iter().chain([e.key]).find(|c| *c == key));
        match channel {
            Some(channel) if self.changed(key) => {
                let default = defaults.try_get(key).map(|v| v.components().iter().map(|n| readout(*n as f64)).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                let reset = ui.add(egui::Button::new(RichText::new("↺").color(palette::accent())).small()).on_hover_text(tr_with("experiments-reset_value", args!(value = default)));
                if reset.clicked() {
                    self.reset(channel);
                }
            }
            _ => {
                ui.label("");
            }
        }
    }

    /// A "?" after a value's name: hovering it shows what the value does
    /// (its help text, else its doc), and the engine property it mirrors.
    fn help_mark(&self, ui: &mut Ui, key: &str) {
        let Some(entry) = values::entry(key) else { return };
        let text = entry.help_text();
        if text.is_empty() {
            return;
        }
        let mut hint = text.to_string();
        if let Some(engine) = entry.engine {
            hint.push_str(&format!("\n{}", tr_with("experiments-engine_property", args!(name = engine))));
        }
        hint_mark(ui, &hint);
    }

    /// With `defaults`, a value's name explains it on hover too: its key, what it does, and the engine property it mirrors.
    fn describe(&self, label: egui::Response, entry: &Entry) {
        if self.defaults.is_none() {
            return;
        }
        let mut text = format!("{}\n{}", entry.key, entry.help_text());
        if let Some(engine) = entry.engine {
            text.push_str(&format!("\n{}", tr_with("experiments-engine_property", args!(name = engine))));
        }
        label.on_hover_text(text);
    }
}
