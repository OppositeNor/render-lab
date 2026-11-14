//! The Experiments tab (src/experiment.rs): every value of the investigation in the dock, and the experiment's
//! tools in the panel right of the viewport (the changed values, the A/B comparison with a reference, the
//! measurement, the performance, the experiment files), under a status of the session. The homepage's
//! Experiment button opens it; the changed values carry from one demonstration to the next while the tab's
//! carry is on, as it starts in a viewer opened so.

mod changes;
mod compare;
mod dock;
mod pages;
mod performance;
mod presets;
mod saving;
mod status;
mod wipe;

use super::{
    DemoChange, Feature, FeatureLayout, FeatureState, FrameAsk, FrameInfo, Host, Page, PanelUi,
    Snapshot,
};
use crate::controls::{Dock, SLIDER_WIDTH};
use crate::experiment::{Compare, Preset, Session};
use crate::i18n::tr;
use crate::investigation;
use crate::renderer::{CompareRequest, Renderer, ViewKind};
use crate::shot::ShotState;
use crate::ui::DockTab;
use crate::values::{self, Entry};
use crate::view::ViewId;
use anyhow::Result;
use changes::{AbRow, ab_group, ab_rows, changed_group};
use compare::{compare_group, measurement_grid};
use dock::values;
use egui::{Ui, vec2};
use performance::performance_group;
use presets::presets_group;
use saving::{Pending, Saved};
use status::status_strip;

/// The tab.
pub const TAB: DockTab = DockTab("experiments");

/// The frames a reference that starts rendering is shown as warming up: a second at 60 frames a second, while
/// its history builds up.
const WARM_FRAMES: u32 = 60;

pub struct ExperimentsFeature;

impl Feature for ExperimentsFeature {
    fn id(&self) -> &'static str {
        TAB.0
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-experiments").to_string())
    }

    fn order(&self) -> i32 {
        10
    }

    fn layout(&self) -> FeatureLayout {
        FeatureLayout {
            panel: true,
            yields: true,
            ..Default::default()
        }
    }

    fn home(&self) -> Option<String> {
        Some(tr("home-experiment_with_the_algorithm").to_string())
    }

    /// `viewer-experiments[:<demo>]` (the tab), `viewer-compare[:<demo>]` (a reference set, an engine setting
    /// moved, a wipe, a measurement), `viewer-compare-stale[:<demo>]` (then a value changed),
    /// `viewer-compare-drag[:<demo>]` (its wipe dragged), `viewer-compare-paused[:<demo>]` (A paused, resumed),
    /// `viewer-experiments-overwrite[:<demo>]` (saved twice, Home), `viewer-experiments-session[:<demo>]` (the
    /// carry off, then on).
    fn pages(&self) -> &'static [Page] {
        &[
            Page {
                name: "viewer-experiments",
                help: "[:<demo>] (the Experiment button's viewer: the Experiments tab)",
            },
            Page {
                name: "viewer-compare",
                help: "[:<demo>] (that viewer with a reference set and compared, a wipe)",
            },
            Page {
                name: "viewer-compare-drag",
                help: "[:<demo>] (that comparison's wipe line dragged through the viewer's input)",
            },
            Page {
                name: "viewer-compare-paused",
                help: "[:<demo>] (that comparison's reference paused, then resumed: warming up)",
            },
            Page {
                name: "viewer-compare-stale",
                help: "[:<demo>] (that comparison measured, then a value changed: the measurement stale)",
            },
            Page {
                name: "viewer-experiments-overwrite",
                help: "[:<demo>] (an experiment saved twice under one name: the Replace question, and Home's)",
            },
            Page {
                name: "viewer-experiments-session",
                help: "[:<demo>] (the tab with its carry off, as Explore opens it: a value changed across two \
                       demonstrations, then with the carry on)",
            },
        ]
    }

    fn snapshot_demo<'a>(&self, _page: &str, args: Option<&'a str>) -> Option<&'a str> {
        args
    }

    fn state(&self) -> Box<dyn crate::feature::FeatureState> {
        Box::new(Experiments {
            session: Session {
                presets: crate::experiment::presets(),
                ..Default::default()
            },
            ..Default::default()
        })
    }
}

/// What the tab's tools ask of the experiment.
#[derive(Clone, Debug, PartialEq)]
enum Asked {
    /// The live state becomes the reference the experiment compares against.
    SetReference,
    ClearReference,
    /// The reference and the live state trade places.
    SwapReference,
    SetCompare(Compare),
    /// The wipe's line, as a share of the scene area's width.
    SetSplit(f32),
    /// The difference view's gain.
    SetGain(f32),
    /// Measure the live image against the reference's (src/metrics.rs).
    Measure,
    /// Save the experiment under this name among the user's (asking first when that replaces a file).
    SavePreset(String),
    /// Save it under this name, replacing the user's file of that name.
    ReplacePreset(String),
    /// Load one of the experiments the tab lists (asking first when that discards unsaved changes).
    LoadPreset(usize),
    /// Load it, discarding the unsaved changes.
    LoadPresetDiscarding(usize),
    /// Neither replace nor discard: the question withdrawn.
    CancelPending,
    /// Carry the changed values to the next demonstration, or not.
    SetCarry(bool),
    /// Stop rendering the reference, or render it again (its history then starts over).
    PauseReference(bool),
}

/// What the tools show of the experiment.
struct ExperimentView {
    /// A reference is set.
    reference: bool,
    compare: Compare,
    split: f32,
    gain: f32,
    measurement: Option<crate::metrics::Measurement>,
    message: Option<String>,
    presets: Vec<Preset>,
    /// GPU time per pass (src/timing.rs): name, mean and 95th percentile in milliseconds.
    timings: Vec<(String, f64, f64)>,
    /// The viewer's frame time: mean and 95th percentile in milliseconds.
    frame_ms: (f64, f64),
    /// Seconds the scene data has been rebuilding (`Investigation::scene_data`).
    rebuilding: Option<f32>,
    /// The values the reference and the live state set differently.
    ab: Vec<AbRow>,
    /// The dock edits the reference (its view is focused), not the live state.
    editing_reference: bool,
    /// The changed values carry over to the next demonstration.
    carry: bool,
    /// The measurement's states or size have changed since it was taken.
    stale: bool,
    /// The reference is not rendered; or it renders again and its history is still building up.
    paused: bool,
    warming: bool,
    /// The session holds changes no experiment file has, and the name it was last saved or loaded as.
    unsaved: bool,
    saved: Option<String>,
    /// A question the tools ask before replacing a file or discarding unsaved changes.
    pending: Option<Pending>,
}

/// What a measurement measured: the live state, the reference's, and the views' size in pixels.
struct Measured {
    live: ShotState,
    reference: ShotState,
    size: [u32; 2],
}

impl Measured {
    /// The measurement no longer shows `live` against `reference` at `size`: a scene's field of either
    /// differs (its demonstration, cameras, frame size, values; not what the panel or a story shows over it),
    /// or the size does.
    fn stale(
        &self,
        live: &ShotState,
        reference: Option<&ShotState>,
        size: Option<[u32; 2]>,
    ) -> bool {
        let scene = |a: &ShotState, b: &ShotState| {
            a.demo == b.demo
                && a.camera == b.camera
                && a.free_camera == b.free_camera
                && a.frame == b.frame
                && a.values == b.values
        };
        !scene(&self.live, live)
            || !reference.is_some_and(|r| r.values == self.reference.values)
            || size != Some(self.size)
    }
}

/// The viewer's experiment, what the renderer last measured, and what the tools asked.
#[derive(Default)]
struct Experiments {
    session: Session,
    timings: Vec<(String, f64, f64)>,
    frame_ms: (f64, f64),
    rebuilding: Option<f32>,
    asked: Vec<Asked>,
    /// A snapshot page's report line, and what a page keeps between its steps (a file's first contents).
    report: Vec<String>,
    report_page: String,
    /// The drag page's split and camera before its drag.
    page_drag: Option<(f32, crate::camera::OrbitCamera)>,
    /// The viewer opened on this tab (the homepage's Experiment button).
    opened_here: bool,
    /// What the session's measurement measured.
    measured: Option<Measured>,
    /// The live view's size at the last frame.
    viewport: Option<[u32; 2]>,
    /// The reference is not rendered (no comparison asked of the frame).
    paused: bool,
    /// How many frames in a row the reference has rendered.
    reference_frames: u32,
    /// The tab has been shown, or the viewer opened on it: from then on, its changes can go unsaved.
    used: bool,
    /// The experiment as last saved or loaded, and whether the session differs from it (`track_unsaved`).
    saved: Option<Saved>,
    unsaved: bool,
    /// The current demonstration's defaults, kept while it stays.
    defaults: Option<ShotState>,
    pending: Option<Pending>,
}

impl Experiments {
    /// What the tools show, `input` the frame's UI input (the live state, the focused view's).
    fn view(&self, input: &crate::ui::UiInput) -> ExperimentView {
        let s = &self.session;
        let reference = s.reference_state(input.state);
        ExperimentView {
            reference: s.reference.is_some(),
            compare: s.compare,
            split: s.split,
            gain: s.gain,
            measurement: s.measurement,
            message: s.message.clone(),
            presets: s.presets.clone(),
            timings: self.timings.clone(),
            frame_ms: self.frame_ms,
            rebuilding: self.rebuilding,
            ab: reference
                .as_ref()
                .map(|r| ab_rows(r, input.state))
                .unwrap_or_default(),
            editing_reference: reference.is_some() && input.focus == reference.as_ref(),
            carry: self.carries(),
            stale: self.stale(input.state),
            paused: self.paused,
            warming: s.reference.is_some() && !self.paused && self.reference_frames < WARM_FRAMES,
            unsaved: self.unsaved,
            saved: self.saved.as_ref().map(|s| s.name.clone()),
            pending: self.pending.clone(),
        }
    }

    /// The session has a measurement, and it no longer shows the live state against the reference.
    fn stale(&self, live: &ShotState) -> bool {
        self.session.measurement.is_some()
            && self
                .measured
                .as_ref()
                .is_none_or(|m| m.stale(live, self.session.reference.as_ref(), self.viewport))
    }

    /// The changed values carry over to the next demonstration: as the tab's carry is set, else in a viewer
    /// opened on the tab.
    fn carries(&self) -> bool {
        self.session.carry.unwrap_or(self.opened_here)
    }

    fn set_reference(&mut self, host: &mut dyn Host) {
        let s = &mut self.session;
        s.reference = Some(host.state().clone());
        if s.compare == Compare::Off {
            s.compare = Compare::Wipe;
        }
        s.measurement = None;
        s.message = Some(tr("experiments-reference_set").to_string());
    }

    /// Measure the live view against the reference's, as the last frame drew them (src/metrics.rs).
    fn measure(&mut self, host: &mut dyn Host) {
        let live = host.state().clone();
        let Some(renderer) = host.renderer() else {
            return;
        };
        let views = renderer
            .read_view(ViewKind::Reference)
            .zip(renderer.read_view(ViewKind::Live));
        match views {
            Some(((ref_display, ref_linear), (display, linear)))
                if ref_display.width == display.width && ref_display.height == display.height =>
            {
                // The reference and the live view, at the same size, are measured against each other.
                self.session.measurement = Some(crate::metrics::measure(
                    &ref_display,
                    &display,
                    Some((&ref_linear, &linear)),
                ));
                self.measured = self.session.reference.clone().map(|reference| Measured {
                    live,
                    reference,
                    size: [display.width as u32, display.height as u32],
                });
                self.session.message = None;
            }
            _ => self.session.message = Some(tr("experiments-measure_needs_reference").to_string()),
        }
    }

    fn apply(&mut self, asked: Asked, host: &mut dyn Host) {
        let interactive = host.interactive();
        let s = &mut self.session;
        match asked {
            Asked::SetReference => self.set_reference(host),
            Asked::ClearReference => {
                s.reference = None;
                s.compare = Compare::Off;
                s.measurement = None;
            }
            Asked::SwapReference if interactive => {
                if let Some(reference) = s.reference.take() {
                    // The reference goes on screen, and the live state becomes the reference; the camera stays put.
                    let camera = host.state().camera;
                    let live = std::mem::replace(
                        host.state_mut(),
                        ShotState {
                            camera,
                            ..reference
                        },
                    );
                    s.reference = Some(live);
                    s.measurement = None;
                }
            }
            Asked::SetCompare(mode) => s.compare = mode,
            Asked::SetSplit(split) => s.split = split.clamp(0.0, 1.0),
            Asked::SetGain(gain) => s.gain = gain.clamp(1.0, 64.0),
            Asked::Measure => self.measure(host),
            Asked::SavePreset(name) => self.save(name, false, host),
            Asked::ReplacePreset(name) => self.save(name, true, host),
            Asked::CancelPending => self.pending = None,
            Asked::SetCarry(carry) => s.carry = Some(carry),
            Asked::PauseReference(paused) => self.paused = paused,
            Asked::LoadPreset(i) if interactive => self.load_preset(i, false, host),
            Asked::LoadPresetDiscarding(i) if interactive => self.load_preset(i, true, host),
            _ => {}
        }
    }

    /// The tab is in use: shown, or the viewer opened on it (not on a timeline).
    fn experimenting(frame: &FrameInfo) -> bool {
        !frame.timeline_mode && (frame.opened == Some(TAB) || frame.tab == TAB)
    }
}

impl FeatureState for Experiments {
    fn selected(&mut self, _host: &mut dyn Host) {
        self.session.presets = crate::experiment::presets();
        self.used = true;
    }

    fn update(&mut self, host: &mut dyn Host) {
        for asked in std::mem::take(&mut self.asked) {
            self.apply(asked, host);
        }
        self.track_unsaved(host);
    }

    fn unsaved(&self) -> Option<String> {
        self.unsaved
            .then(|| tr("experiments-discard_unsaved").to_string())
    }

    fn marked(&self) -> bool {
        self.unsaved
    }

    /// The wipe's line, grabbed in the viewport (src/feature/experiments/wipe.rs).
    fn overlay(&mut self, panel: &mut super::StoryPanel) {
        self.wipe(panel);
    }

    fn dock(&mut self, dock: &mut Dock, ui: &mut Ui) -> bool {
        values(dock, ui);
        true
    }

    fn panel(&mut self, panel: &mut PanelUi) {
        let rect = panel.input.layout.panel;
        crate::paint::panel_card(panel.ink.painter, rect, 12);
        let inner = rect.shrink2(vec2(16.0, 12.0));
        let mut asked = Vec::new();
        let view = self.view(panel.input);
        let dock = &mut *panel.dock;
        egui::Area::new(egui::Id::unique("viewer-value-panel"))
            .fixed_pos(inner.min)
            .constrain(false)
            .fade_in(false)
            .show(panel.ctx, |ui| {
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
                egui::ScrollArea::vertical()
                    .id_salt("viewer-value-panel-scroll")
                    .min_scrolled_height(inner.height())
                    .max_height(inner.height())
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            status_strip(ui, &view, &mut asked);
                            tools(dock, ui, &view, &mut asked);
                        });
                    });
            });
        self.asked.extend(asked);
    }

    /// The reference drawn while the live state is (not on a timeline, not paused), and timings while the tab
    /// is in use.
    fn frame_request(&self, state: &ShotState, frame: &FrameInfo) -> FrameAsk {
        let s = &self.session;
        let compare = s
            .reference_state(state)
            .filter(|_| !frame.timeline_mode && !self.paused)
            .map(|reference| CompareRequest {
                reference,
                mode: s.compare,
                split: s.split,
                gain: s.gain,
            });
        FrameAsk {
            compare,
            timing: Experiments::experimenting(frame),
        }
    }

    fn frame_readout(&mut self, renderer: &Renderer, frame: &FrameInfo) {
        self.timings = renderer
            .timing
            .as_ref()
            .map(|t| t.summary())
            .unwrap_or_default();
        self.frame_ms = frame.frame_ms;
        self.rebuilding = renderer.rebuilding();
        self.opened_here = frame.opened == Some(TAB);
        self.used |= self.opened_here;
        self.viewport = renderer.viewport_size();
        // Each frame the reference renders counts; a frame without it starts its history over.
        if self.session.reference.is_some() && !self.paused && !frame.timeline_mode {
            self.reference_frames = self.reference_frames.saturating_add(1);
        } else {
            self.reference_frames = 0;
        }
    }

    fn view_state(&self, view: ViewId, live: &ShotState) -> Option<ShotState> {
        (view == ViewId::REFERENCE)
            .then(|| self.session.reference_state(live))
            .flatten()
    }

    /// The reference focused: the slider sets the reference's value (its camera is the live one's).
    fn set_value(&mut self, view: ViewId, key: &'static str, value: f32) -> bool {
        let Some(reference) = self
            .session
            .reference
            .as_mut()
            .filter(|_| view == ViewId::REFERENCE)
        else {
            return false;
        };
        reference.set(key, value);
        for (k, v) in investigation::current().linked_params(key, value) {
            reference.set(k, v);
        }
        true
    }

    /// While the carry is on, the values changed so far carry over to the next demonstration; so do the
    /// reference's: its changes from the old demonstration's defaults, over the new one's.
    fn demo_selected(&mut self, change: &DemoChange, host: &mut dyn Host) {
        self.opened_here = change.opened == Some(TAB);
        if !self.carries() {
            return;
        }
        let carried = crate::experiment::changes(change.old, change.old_defaults);
        let reference = self
            .session
            .reference
            .as_ref()
            .map(|r| crate::experiment::changes(r, change.old_defaults));
        if let Err(e) = crate::experiment::apply(host.state_mut(), &carried) {
            self.session.message = Some(format!("{e:#}"));
        }
        if let Some(changes) = reference {
            let mut r = change.defaults.clone();
            match crate::experiment::apply(&mut r, &changes) {
                Ok(()) => self.session.reference = Some(r),
                Err(e) => self.session.message = Some(format!("{e:#}")),
            }
            self.session.measurement = None;
        }
    }

    /// A snapshot page's steps (src/feature/experiments/pages.rs).
    fn snapshot(
        &mut self,
        page: &str,
        _args: Option<&str>,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        self.page_step(page, step, host)
    }
}

/// The panel: the experiment's tools (the changed values, the A/B comparison, the performance, the experiment
/// files). Its values are in the dock (`values`). What its buttons ask goes to `asked`.
fn tools(dock: &mut Dock, ui: &mut Ui, view: &ExperimentView, asked: &mut Vec<Asked>) {
    let defaults = investigation::current().default_state(dock.state.demo, dock.input.statics);
    dock.defaults = Some(defaults);
    let all = values::list();
    let changed: Vec<(&'static Entry, &'static str)> = all
        .iter()
        .flat_map(|e| e.channels().into_iter().map(move |c| (*e, c)))
        .filter(|(_, c)| dock.changed(c))
        .collect();

    if view.reference {
        dock.group(
            ui,
            tr("experiments-ab"),
            Some(tr("experiments-ab_hint")),
            |_, ui| ab_group(ui, &view.ab),
        );
    }

    dock.group(
        ui,
        tr("experiments-experiment"),
        Some(tr("experiments-experiment_hint")),
        |d, ui| {
            changed_group(d, ui, &changed);
        },
    );

    dock.group(
        ui,
        tr("experiments-compare"),
        Some(tr("experiments-compare_hint")),
        |d, ui| {
            compare_group(d, ui, view, asked);
            measurement_grid(ui, view);
        },
    );

    dock.group(
        ui,
        tr("experiments-performance"),
        Some(tr("experiments-performance_hint")),
        |d, ui| {
            performance_group(d, ui, view);
        },
    );

    dock.group(
        ui,
        tr("experiments-presets"),
        Some(tr("experiments-presets_hint")),
        |_, ui| {
            presets_group(ui, view, asked);
        },
    );
}
