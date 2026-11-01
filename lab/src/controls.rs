//! The interactive viewer's dock: sliders for what the current demonstration
//! shows, "Play animation" and "Reset parameters" buttons, the narration's
//! volume, and tabs with the demonstration's pseudo-code and engine source.
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
use crate::experiment::Compare;
use crate::ui::{self, DockTab, ExperimentView, Ink, UiInput, ViewerAction, ViewerChrome};
use crate::values::{self, Entry, Reflect, Type, Value};
use crate::volume::{self, Volume};
use egui::{pos2, vec2, Color32, CornerRadius, FontId, Rect, RichText, Stroke, StrokeKind, Ui};
use std::ops::RangeInclusive;

/// Width of the dock's button column.
const BUTTONS: f32 = 300.0;
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
    /// A timeline is shown (it adds the Captions tab).
    timeline: bool,
    /// Parameters the demonstration's animation drives.
    animated: Vec<&'static str>,
    /// The narration's volume (src/volume.rs).
    narration: Volume,
    pub actions: Vec<ViewerAction>,
    widgets: Vec<(String, Rect)>,
    /// In the Experiments tab: the demonstration's defaults, so each row
    /// marks a changed value and offers to reset it.
    defaults: Option<ShotState>,
    experiment: ExperimentView,
    /// The Prerequisites tab's chapters (src/knowledge.rs).
    knowledge: crate::ui::KnowledgeChrome,
}

pub fn draw_dock(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let rect = input.layout.captions;
    let painter = ink.painter;
    painter.rect_filled(rect, CornerRadius::same(10), palette::panel());
    painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
    let inner = rect.shrink2(vec2(16.0, 10.0));
    let divider = inner.min.x + BUTTONS + 16.0;
    painter.vline(divider, rect.y_range().shrink(12.0), Stroke::new(1.0, palette::border()));
    let demo = input.state.demo;
    let mut dock = Dock::new(input, viewer);
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
            if !matches!(tab, DockTab::Sources | DockTab::Captions) {
                // Each direction scrolls once the groups overflow it (as of last frame), so a
                // tall group's lower rows stay reachable; with one direction, the wheel drives it.
                let room = vec2(inner.max.x - ui.cursor().min.x, inner.max.y - ui.cursor().min.y);
                let overflow_id = egui::Id::unique("viewer-dock-overflow");
                let overflow = ctx.data(|d| d.get_temp::<[bool; 2]>(overflow_id)).unwrap_or([true, false]);
                let output = egui::ScrollArea::new(overflow).id_salt("viewer-dock-scroll").max_width(room.x).max_height(room.y).show(ui, |ui| {
                    ui.horizontal_top(|ui| match tab {
                        DockTab::Code => dock.code(ui),
                        DockTab::Experiments => dock.experiment_values(ui),
                        DockTab::Prerequisites => dock.prerequisites(ui),
                        _ => investigation::current().dock(&mut dock, ui),
                    });
                });
                ctx.data_mut(|d| d.insert_temp(overflow_id, [output.content_size.x > room.x + 0.5, output.content_size.y > room.y + 0.5]));
            }
        });
    });
    let side = Rect::from_min_max(pos2(divider, rect.min.y), rect.max);
    match tab {
        DockTab::Sources => ui::draw_source_list(ink, side, demo),
        DockTab::Captions => ui::draw_caption_tab(ink, side.shrink2(vec2(16.0, 0.0)), viewer.previous_caption.as_deref(), viewer.caption.as_deref()),
        _ => {}
    }
    viewer.actions.extend(dock.actions);
    viewer.widgets = dock.widgets;
}

/// The Experiments tab's tools in the panel's place, right of the viewport
/// (the dock below holds every value): the changed values, the A/B
/// comparison, the performance, and the experiment files, one column, scrolled.
pub fn draw_experiment_panel(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let rect = input.layout.panel;
    ink.painter.rect_filled(rect, CornerRadius::same(10), palette::panel());
    ink.painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
    let inner = rect.shrink2(vec2(16.0, 12.0));
    let mut dock = Dock::new(input, viewer);
    egui::Area::new(egui::Id::unique("viewer-value-panel")).fixed_pos(inner.min).constrain(false).fade_in(false).show(ctx, |ui| {
        ui.set_width(inner.width());
        ui.set_clip_rect(inner);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let spacing = ui.spacing_mut();
        spacing.button_padding = vec2(10.0, 3.0);
        spacing.item_spacing = vec2(8.0, 4.0);
        spacing.slider_width = SLIDER_WIDTH;
        spacing.icon_width = 22.0;
        spacing.icon_width_inner = 12.0;
        spacing.icon_spacing = 8.0;
        // As the dock scrolls its groups: vertically, within the panel.
        // Scrolled when the tools overflow the panel (many passes, many changed values);
        // its height is the panel's, not what the area had last frame.
        egui::ScrollArea::vertical().id_salt("viewer-value-panel-scroll").min_scrolled_height(inner.height()).max_height(inner.height()).auto_shrink([false, true]).show(ui, |ui| {
            ui.vertical(|ui| dock.experiments(ui));
        });
    });
    viewer.actions.extend(dock.actions);
    viewer.widgets.extend(dock.widgets);
}

impl<'a> Dock<'a> {
    fn new(input: &'a UiInput<'a>, viewer: &ViewerChrome) -> Dock<'a> {
        Dock {
            state: input.state,
            input,
            playing: viewer.playing,
            timeline: viewer.timeline_mode,
            animated: investigation::current().animation(input.state.demo, 0.0).into_iter().map(|p| p.0).collect(),
            narration: viewer.narration,
            actions: Vec::new(),
            widgets: Vec::new(),
            defaults: None,
            experiment: viewer.experiment.clone(),
            knowledge: viewer.knowledge.clone(),
        }
    }
}

impl Dock<'_> {
    fn buttons(&mut self, ui: &mut Ui, tab: DockTab) {
        // The tabs wrap within the column.
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(BUTTONS);
            let captions = self.timeline.then(|| (DockTab::Captions, tr("controls.captions")));
            let prerequisites = crate::knowledge::current().map(|_| (DockTab::Prerequisites, tr("controls.prerequisites")));
            let tabs = [(DockTab::Parameters, tr("controls.sliders")), (DockTab::Experiments, tr("controls.experiments")), (DockTab::Code, tr("controls.code")), (DockTab::Sources, tr("controls.source"))];
            for (value, label) in tabs.into_iter().chain(prerequisites).chain(captions) {
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
        let layout = ui.add(egui::Button::new(tr("controls.reset_layout")).min_size(full));
        self.widgets.push(("reset-layout".into(), layout.rect));
        if layout.on_hover_text(tr("controls.reset_layout_hint")).clicked() {
            self.actions.push(ViewerAction::ResetLayout);
        }
        // The narration's volume, playing or not; the timeline bar has it too.
        let mut narration = self.narration;
        let r = ui
            .horizontal(|ui| {
                ui.label(RichText::new(tr("controls.narration")).color(palette::text_dim())).on_hover_text(tr("controls.narration_hint"));
                volume::slider(ui, &mut narration, 26.0, BUTTONS - 228.0)
            })
            .inner;
        if r.changed || r.committed {
            self.actions.push(ViewerAction::SetNarration(narration, r.committed));
        }
    }

    /// The Prerequisites tab (src/knowledge.rs): for the context's chapters,
    /// the level the investigation's audience starts from, what it should
    /// know (by area, the nearest the level first; a mark on what the
    /// chapters use directly), and what they teach beyond the level. A click
    /// on a topic selects it in the knowledge panel.
    fn prerequisites(&mut self, ui: &mut Ui) {
        let Some(graph) = crate::knowledge::current() else {
            ui.label(RichText::new(tr("knowledge.none")).color(palette::text_dim()));
            return;
        };
        let chapters = crate::ui::knowledge_chapters(graph, &self.knowledge);
        let p = graph.audience();
        let context = graph.context(&chapters);
        let carried = p.carried();
        let ranks = graph.ranks();
        let heading = if chapters.is_empty() { tr("knowledge.for_all").to_string() } else { tr_with("knowledge.for_chapters", args!(chapters = chapters.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "))) };
        // A topic: its title (a click selects it in the graph) and its summary under it.
        let topic = |d: &mut Self, ui: &mut Ui, i: usize, color: Color32| {
            let node = &graph.nodes[i];
            let mark = if carried.contains(&i) { " ↗" } else { "" };
            let r = ui.selectable_label(false, RichText::new(format!("{}{mark}", node.title)).color(color));
            let r = if mark.is_empty() { r } else { r.on_hover_text(tr("prerequisites.carried_hint")) };
            if r.clicked() {
                crate::ui::focus_topic(ui.ctx(), i);
                if d.state.panel != PanelMode::Knowledge {
                    d.actions.push(ViewerAction::SetPanel(PanelMode::Knowledge));
                }
            }
            ui.add(egui::Label::new(RichText::new(&node.summary).size(20.0).color(palette::text_dim())).wrap());
            ui.add_space(6.0);
        };
        // Each group a column of this width, its summaries wrapped in it.
        const COLUMN: f32 = 360.0;
        let level: Vec<usize> = p.level.iter().copied().filter(|&i| context[i]).collect();
        self.group(ui, tr("prerequisites.level"), Some(&format!("{heading}\n\n{}", tr("prerequisites.level_hint"))), |d, ui| {
            ui.set_width(COLUMN);
            let full = ui.button(tr("knowledge.full_screen")).on_hover_text(tr("prerequisites.full_screen_hint"));
            if full.clicked() {
                d.actions.push(ViewerAction::SetKnowledgeOverlay(true));
            }
            ui.add_space(6.0);
            for &i in &level {
                topic(d, ui, i, palette::accent());
            }
        });
        // What the audience should know, by area, the nearest the level first.
        for area in crate::knowledge::AREAS {
            let mut known: Vec<usize> = (0..graph.nodes.len()).filter(|&i| p.is_known(i) && context[i] && graph.nodes[i].area == *area).collect();
            if known.is_empty() {
                continue;
            }
            known.sort_by_key(|&i| std::cmp::Reverse(ranks[i]));
            let title = match *area {
                "gpu" => tr("knowledge.area.gpu"),
                "rt" => tr("knowledge.area.rt"),
                "geo" => tr("knowledge.area.geo"),
                "cg" => tr("knowledge.area.cg"),
                "light" => tr("knowledge.area.light"),
                _ => tr("knowledge.area.gi"),
            };
            let title = tr_with("prerequisites.known", args!(area = title));
            self.group(ui, &title, Some(tr("prerequisites.known_hint")), |d, ui| {
                ui.set_width(COLUMN);
                for &i in &known {
                    topic(d, ui, i, palette::code_string());
                }
            });
        }
        let teach: Vec<usize> = p.teach.iter().copied().filter(|&i| context[i]).collect();
        if !teach.is_empty() {
            self.group(ui, tr("prerequisites.teach"), Some(tr("prerequisites.teach_hint")), |d, ui| {
                ui.set_width(COLUMN);
                for &i in &teach {
                    topic(d, ui, i, palette::code_number());
                }
            });
        }
    }

    /// The Code tab: this demonstration's listings, and one button per step
    /// (tag) that lights its lines in the panel.
    fn code(&mut self, ui: &mut Ui) {
        let demo = self.state.demo;
        let listings: Vec<_> = investigation::current().listings().iter().filter(|l| l.demo.as_deref() == Some(demo.key())).collect();
        if listings.is_empty() {
            ui.label(RichText::new(tr("controls.no_listing")).color(palette::text_dim()));
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
        self.group(ui, tr("controls.record"), Some(tr("controls.record_hint")), |d, ui| {
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
        self.group(ui, tr("controls.struct_panel"), Some(tr("controls.struct_panel_hint")), |d, ui| {
            for (mode, label) in [(PanelMode::Declaration, tr("controls.declaration")), (PanelMode::Values, tr("controls.values")), (PanelMode::Memory, tr("controls.memory"))] {
                if ui.selectable_label(d.state.panel == mode, label).clicked() {
                    d.actions.push(ViewerAction::SetPanel(mode));
                }
            }
        });
        let fields: Vec<String> = statics.layouts.get(name).map(|s| s.fields.iter().map(|f| f.name.clone()).collect()).unwrap_or_default();
        let none = tr("controls.none");
        self.group(ui, tr("controls.highlight"), Some(tr("controls.highlight_hint")), |d, ui| {
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
        self.entries(ui, tr("controls.camera"), Some(tr("controls.camera_hint")), "camera", entries);
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
    fn value_rows(&mut self, ui: &mut Ui, entry: &'static Entry) {
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
            if let Some(v) = self.number(ui, channel, label, color, shown, range.0..=range.1, entry.unit) {
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
        self.group(ui, tr("controls.show"), Some(tr("controls.show_hint")), |d, ui| {
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
        if self.playing && self.animated.contains(&key) {
            palette::accent()
        } else {
            normal
        }
    }

    /// One grid row: a label and a slider. Returns the new value when the
    /// user changed it.
    #[allow(clippy::too_many_arguments)]
    pub fn number(&mut self, ui: &mut Ui, key: &str, label: &str, color: Color32, value: f32, range: RangeInclusive<f32>, unit: Unit) -> Option<f32> {
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

/// A "?" that shows `text` when hovered.
fn hint_mark(ui: &mut Ui, text: &str) {
    let mark = RichText::new(" ? ").monospace().color(palette::text_dim()).background_color(palette::background());
    ui.add(egui::Label::new(mark).sense(egui::Sense::hover())).on_hover_text(text);
}

/// A number for a readout: three significant decimals, "∞" for an infinite one.
fn readout(x: f64) -> String {
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
    /// Whether channel `key` differs from the demonstration's default (in the Experiments tab).
    fn changed(&self, key: &str) -> bool {
        let Some(defaults) = &self.defaults else { return false };
        match (self.state.try_get(key), defaults.try_get(key)) {
            (Some(a), Some(b)) => a.components().iter().zip(b.components()).any(|(x, y)| (x - y).abs() > 1e-5),
            _ => false,
        }
    }

    /// In the Experiments tab, a row's last cell: a reset button for a changed value.
    fn reset_cell(&mut self, ui: &mut Ui, key: &str) {
        let Some(defaults) = &self.defaults else { return };
        let channel = values::entry(key).and_then(|e| e.channels().into_iter().chain([e.key]).find(|c| *c == key));
        match channel {
            Some(channel) if self.changed(key) => {
                let default = defaults.try_get(key).map(|v| v.components().iter().map(|n| readout(*n as f64)).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                let reset = ui.add(egui::Button::new(RichText::new("↺").color(palette::accent())).small()).on_hover_text(tr_with("experiments.reset_value", args!(value = default)));
                if reset.clicked() {
                    self.actions.push(ViewerAction::ResetParam(channel));
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
            hint.push_str(&format!("\n{}", tr_with("experiments.engine_property", args!(name = engine))));
        }
        hint_mark(ui, &hint);
    }

    /// In the Experiments tab, a value's name explains it on hover too: its key, what it does, and the engine property it mirrors.
    fn describe(&self, label: egui::Response, entry: &Entry) {
        if self.defaults.is_none() {
            return;
        }
        let mut text = format!("{}\n{}", entry.key, entry.help_text());
        if let Some(engine) = entry.engine {
            text.push_str(&format!("\n{}", tr_with("experiments.engine_property", args!(name = engine))));
        }
        label.on_hover_text(text);
    }

    /// The Experiments tab's panel: the experiment's tools (the changed values,
    /// the A/B comparison, the performance, the experiment files). Its values
    /// are in the dock (`experiment_values`).
    fn experiments(&mut self, ui: &mut Ui) {
        let defaults = investigation::current().default_state(self.state.demo, self.input.statics);
        self.defaults = Some(defaults);
        let all = values::list();
        let changed: Vec<(&'static Entry, &'static str)> = all.iter().flat_map(|e| e.channels().into_iter().map(move |c| (*e, c))).filter(|(_, c)| self.changed(c)).collect();

        self.group(ui, tr("experiments.experiment"), Some(tr("experiments.experiment_hint")), |d, ui| {
            ui.set_max_width(330.0);
            ui.label(RichText::new(tr_with("experiments.changed", args!(count = changed.len()))).color(palette::text_dim()));
            if changed.is_empty() {
                ui.label(RichText::new(tr("experiments.no_changes")).color(palette::text_faint()));
            }
            for (entry, channel) in changed.iter().take(8) {
                ui.horizontal(|ui| {
                    let name = if entry.components.is_empty() { entry.label_text().to_string() } else { format!("{} {}", entry.label_text(), channel.rsplit('.').next().unwrap_or(channel)) };
                    let value = d.state.try_get(channel).map(|v| readout(v.f32() as f64)).unwrap_or_default();
                    if ui.small_button("↺").clicked() {
                        d.actions.push(ViewerAction::ResetParam(channel));
                    }
                    ui.label(RichText::new(format!("{name} = {value}")).color(palette::text()));
                });
            }
            if changed.len() > 8 {
                ui.label(RichText::new(tr_with("experiments.more", args!(count = changed.len() - 8))).color(palette::text_faint()));
            }
            if ui.add_enabled(!changed.is_empty(), egui::Button::new(tr("experiments.reset_all"))).clicked() {
                d.actions.push(ViewerAction::ResetParameters);
            }
        });

        let view = self.experiment.clone();
        self.group(ui, tr("experiments.compare"), Some(tr("experiments.compare_hint")), |d, ui| {
            ui.set_max_width(360.0);
            ui.horizontal(|ui| {
                if ui.button(tr("experiments.set_reference")).on_hover_text(tr("experiments.set_reference_hint")).clicked() {
                    d.actions.push(ViewerAction::SetReference);
                }
                if ui.add_enabled(view.reference, egui::Button::new(tr("experiments.swap"))).on_hover_text(tr("experiments.swap_hint")).clicked() {
                    d.actions.push(ViewerAction::SwapReference);
                }
                if ui.add_enabled(view.reference, egui::Button::new(tr("experiments.clear"))).clicked() {
                    d.actions.push(ViewerAction::ClearReference);
                }
            });
            ui.add_enabled_ui(view.reference, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for mode in Compare::ALL {
                        let label = match mode {
                            Compare::Off => tr("experiments.compare_off"),
                            Compare::Wipe => tr("experiments.compare_wipe"),
                            Compare::SideBySide => tr("experiments.compare_side"),
                            Compare::Difference => tr("experiments.compare_difference"),
                            Compare::Reference => tr("experiments.compare_reference"),
                        };
                        if ui.selectable_label(view.compare == mode, label).clicked() {
                            d.actions.push(ViewerAction::SetCompare(mode));
                        }
                    }
                });
                d.rows(ui, "experiment-compare", |d, ui| {
                    if view.compare == Compare::Wipe {
                        if let Some(v) = d.number(ui, "experiment.split", tr("experiments.split"), palette::text(), view.split, 0.0..=1.0, Unit::Plain) {
                            d.actions.push(ViewerAction::SetSplit(v));
                        }
                    }
                    if view.compare == Compare::Difference {
                        if let Some(v) = d.number(ui, "experiment.gain", tr("experiments.gain"), palette::text(), view.gain, 1.0..=64.0, Unit::Factor) {
                            d.actions.push(ViewerAction::SetGain(v));
                        }
                    }
                });
                if ui.button(tr("experiments.measure")).on_hover_text(tr("experiments.measure_hint")).clicked() {
                    d.actions.push(ViewerAction::Measure);
                }
            });
            if let Some(m) = view.measurement {
                egui::Grid::new("experiment-measurement").num_columns(2).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                    for (name, value) in [("PSNR (dB)", m.psnr), ("SSIM", m.ssim), ("ꟻLIP", m.flip), ("MSE", m.mse), ("relMSE", m.relmse), ("L̄ A", m.luminance_reference), ("L̄ B", m.luminance)] {
                        ui.label(RichText::new(name).color(palette::text_dim()));
                        ui.label(RichText::new(readout(value)).color(palette::text()));
                        ui.end_row();
                    }
                });
            }
        });

        self.group(ui, tr("experiments.performance"), Some(tr("experiments.performance_hint")), |d, ui| {
            // The performance panel on the right, or back to the demonstration's panel.
            let shown = d.state.panel == PanelMode::Performance;
            let label = if shown { tr("experiments.hide_panel") } else { tr("experiments.show_panel") };
            if ui.selectable_label(shown, label).on_hover_text(tr("experiments.show_panel_hint")).clicked() {
                let back = d.defaults.as_ref().map_or(PanelMode::Values, |s| s.panel);
                d.actions.push(ViewerAction::SetPanel(if shown { back } else { PanelMode::Performance }));
            }
            egui::Grid::new("experiment-timings").num_columns(3).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                ui.label(RichText::new(tr("experiments.frame")).color(palette::text_dim()));
                ui.label(RichText::new(format!("{:.2} ms", view.frame_ms.0)).color(palette::text()));
                ui.label(RichText::new(format!("p95 {:.2}", view.frame_ms.1)).color(palette::text_faint()));
                ui.end_row();
                for (pass, mean, p95) in &view.timings {
                    ui.label(RichText::new(ui::pass_label(pass)).color(palette::text_dim()));
                    ui.label(RichText::new(format!("{mean:.3} ms")).color(palette::text()));
                    ui.label(RichText::new(format!("p95 {p95:.3}")).color(palette::text_faint()));
                    ui.end_row();
                }
            });
            if view.timings.is_empty() {
                ui.label(RichText::new(tr("experiments.no_timestamps")).color(palette::text_faint()));
            }
            if let Some(seconds) = view.rebuilding {
                ui.label(RichText::new(tr_with("experiments.rebuilding", args!(seconds = format!("{seconds:.1}")))).color(palette::accent()));
            }
        });

        self.group(ui, tr("experiments.presets"), Some(tr("experiments.presets_hint")), |d, ui| {
            ui.set_max_width(330.0);
            let name_id = egui::Id::unique("experiments-preset-name");
            let mut name = ui.data(|data| data.get_temp::<String>(name_id)).unwrap_or_default();
            ui.horizontal(|ui| {
                if ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("experiments.preset_name")).desired_width(200.0)).changed() {
                    ui.data_mut(|data| data.insert_temp(name_id, name.clone()));
                }
                if ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tr("experiments.save"))).on_hover_text(tr("experiments.save_hint")).clicked() {
                    d.actions.push(ViewerAction::SavePreset(name.trim().to_string()));
                }
            });
            for (i, preset) in view.presets.iter().enumerate() {
                let shipped = matches!(preset.source, crate::experiment::Source::Shipped(_));
                let label = if shipped { format!("{} · {}", preset.name, tr("experiments.shipped")) } else { preset.name.clone() };
                if ui.button(label).on_hover_text(tr("experiments.load_hint")).clicked() {
                    d.actions.push(ViewerAction::LoadPreset(i));
                }
            }
            if let Some(message) = &view.message {
                ui.add(egui::Label::new(RichText::new(message).color(palette::text_dim())).wrap());
            }
        });

    }

    /// The Experiments tab's dock: every value of the investigation in
    /// groups side by side (the engine's settings by label, the scene's, the
    /// demonstrations' presentation, the camera), a long group in columns of
    /// twelve rows; the first column has the filter, which keeps the matching ones.
    fn experiment_values(&mut self, ui: &mut Ui) {
        let defaults = investigation::current().default_state(self.state.demo, self.input.statics);
        self.defaults = Some(defaults);
        let filter_id = egui::Id::unique("experiments-filter");
        let shown_id = egui::Id::unique("experiments-presentation");
        let mut filter = ui.data(|d| d.get_temp::<String>(filter_id)).unwrap_or_default();
        let mut toggle_presentation = false;
        let shown_before = ui.data(|d| d.get_temp::<bool>(shown_id)).unwrap_or(false);
        self.group(ui, tr("controls.experiments"), Some(tr("experiments.experiment_hint")), |_, ui| {
            ui.set_max_width(260.0);
            if ui.add(egui::TextEdit::singleline(&mut filter).hint_text(tr("experiments.filter")).desired_width(250.0)).changed() {
                ui.data_mut(|data| data.insert_temp(filter_id, filter.clone()));
            }
            let label = if shown_before { tr("experiments.hide_presentation").to_string() } else { tr("experiments.presentation").to_string() };
            if ui.selectable_label(shown_before, label).on_hover_text(tr("experiments.presentation_hint")).clicked() {
                toggle_presentation = true;
            }
        });
        if toggle_presentation {
            ui.data_mut(|d| d.insert_temp(shown_id, !shown_before));
        }
        let all = values::list();
        let camera = OrbitCamera::schema();
        let is_camera = |e: &Entry| camera.iter().any(|c| std::ptr::eq(c, e));
        let needle = filter.trim().to_lowercase();
        let matches = |e: &Entry| needle.is_empty() || [e.key, e.label_text(), e.doc, e.engine.unwrap_or("")].iter().any(|t| t.to_lowercase().contains(&needle));
        let engine: Vec<&'static Entry> = all.iter().copied().filter(|e| e.engine.is_some() && matches(e)).collect();
        let scene: Vec<&'static Entry> = all.iter().copied().filter(|e| e.engine.is_none() && !is_camera(e) && !e.labels.is_empty() && !e.toggle && matches(e)).collect();
        let presentation: Vec<&'static Entry> = all.iter().copied().filter(|e| e.engine.is_none() && !is_camera(e) && (e.labels.is_empty() || e.toggle) && matches(e)).collect();
        if !engine.is_empty() {
            self.group(ui, tr("experiments.engine_settings"), Some(tr("experiments.engine_settings_hint")), |d, ui| {
                let mut labels: Vec<&str> = Vec::new();
                for e in &engine {
                    let label = e.labels.first().copied().unwrap_or("");
                    if !labels.contains(&label) {
                        labels.push(label);
                    }
                }
                ui.horizontal_top(|ui| for label in labels {
                    ui.vertical(|ui| {
                    if !label.is_empty() {
                        // The package's name for the group (its dock's `controls.<label>`), else the label.
                        let package = engine.iter().find(|e| e.labels.first().copied() == Some(label)).map_or("rlstd", |e| e.package);
                        let title = crate::i18n::try_package_text(package, &format!("controls.{label}")).unwrap_or(label);
                        ui.label(RichText::new(title).color(palette::text_faint()));
                    }
                    d.rows(ui, &format!("experiment-engine-{label}"), |d, ui| {
                        for e in engine.iter().filter(|e| e.labels.first().copied().unwrap_or("") == label) {
                            d.value_rows(ui, e);
                        }
                    });
                    });
                    ui.add_space(16.0);
                });
            });
        }
        let column = |d: &mut Self, ui: &mut Ui, title: &str, hint: &str, id: &str, entries: &[&'static Entry]| {
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
        column(self, ui, tr("experiments.scene"), tr("experiments.scene_hint"), "experiment-scene", &scene);
        let shown = shown_before || !needle.is_empty();
        if shown {
            column(self, ui, tr("experiments.presentation"), tr("experiments.presentation_hint"), "experiment-presentation", &presentation);
        }
        let cameras: Vec<&'static Entry> = camera.iter().filter(|e| e.has_label("view") && matches(e)).collect();
        column(self, ui, tr("controls.camera"), tr("controls.camera_hint"), "experiment-camera", &cameras);
    }
}
