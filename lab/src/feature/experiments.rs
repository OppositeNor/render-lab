//! The Experiments tab (src/experiment.rs): every value of the investigation in the dock, and the experiment's
//! tools in the panel right of the viewport (the changed values, the A/B comparison with a reference, the
//! measurement, the performance, the experiment files). The homepage's Experiment button opens it; a viewer
//! opened so carries the changed values from one demonstration to the next.

mod changes;
mod compare;
mod dock;
mod performance;
mod presets;

use super::{
    DemoChange, Feature, FeatureLayout, FeatureState, FrameAsk, FrameInfo, Host, Page, PanelUi,
    Snapshot,
};
use crate::controls::{Dock, SLIDER_WIDTH};
use crate::experiment::{Compare, Preset, Session};
use crate::i18n::{args, tr, tr_with};
use crate::investigation;
use crate::renderer::{CompareRequest, Renderer, ViewKind};
use crate::shot::ShotState;
use crate::ui::DockTab;
use crate::values::{self, Entry, Reflect};
use crate::view::ViewId;
use anyhow::Result;
use changes::changed_group;
use compare::{compare_group, measurement_grid};
use dock::values;
use egui::{Ui, vec2};
use performance::performance_group;
use presets::presets_group;

/// The tab.
pub const TAB: DockTab = DockTab("experiments");

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
    /// moved, a wipe, a measurement).
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
    /// Save the experiment under this name among the user's.
    SavePreset(String),
    /// Load one of the experiments the tab lists.
    LoadPreset(usize),
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
}

/// The viewer's experiment, what the renderer last measured, and what the tools asked.
#[derive(Default)]
struct Experiments {
    session: Session,
    timings: Vec<(String, f64, f64)>,
    frame_ms: (f64, f64),
    rebuilding: Option<f32>,
    asked: Vec<Asked>,
    /// A snapshot page's report line.
    report: Vec<String>,
}

impl Experiments {
    fn view(&self) -> ExperimentView {
        let s = &self.session;
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
        }
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
                self.session.message = None;
            }
            _ => self.session.message = Some(tr("experiments-measure_needs_reference").to_string()),
        }
    }

    /// Open an experiment file: its demonstration, camera, and base values,
    /// and its reference (with a comparison) when it has one.
    fn load(&mut self, preset: &Preset, host: &mut dyn Host) -> Result<()> {
        let experiment = preset.load()?;
        experiment.check()?;
        let demo = experiment.start_demo()?;
        let mut state = host.default_state(demo);
        experiment.apply(&mut state, &experiment.base)?;
        self.session.reference = match experiment.reference_values() {
            Some(values) => {
                let mut reference = state.clone();
                experiment.apply(&mut reference, &values)?;
                Some(reference)
            }
            None => None,
        };
        if self.session.reference.is_some() && self.session.compare == Compare::Off {
            self.session.compare = Compare::Wipe;
        }
        self.session.measurement = None;
        host.set_state(state);
        Ok(())
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
            Asked::SavePreset(name) => {
                let defaults = host.default_state(host.state().demo);
                let experiment = crate::experiment::Experiment::capture(
                    &name,
                    host.state(),
                    &defaults,
                    s.reference.as_ref(),
                );
                s.message = Some(match crate::experiment::save_user(&experiment) {
                    Ok(path) => tr_with(
                        "experiments-saved",
                        args!(path = path.display().to_string()),
                    ),
                    Err(e) => format!("{e:#}"),
                });
                s.presets = crate::experiment::presets();
            }
            Asked::LoadPreset(i) if interactive => {
                if let Some(preset) = s.presets.get(i).cloned() {
                    let message = match self.load(&preset, host) {
                        Ok(()) => tr_with("experiments-loaded", args!(name = preset.name.clone())),
                        Err(e) => format!("{e:#}"),
                    };
                    self.session.message = Some(message);
                }
            }
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
    }

    fn update(&mut self, host: &mut dyn Host) {
        for asked in std::mem::take(&mut self.asked) {
            self.apply(asked, host);
        }
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
        let view = self.view();
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
                        ui.vertical(|ui| tools(dock, ui, &view, &mut asked));
                    });
            });
        self.asked.extend(asked);
    }

    /// The reference drawn while the live state is (not on a timeline), and timings while the tab is in use.
    fn frame_request(&self, state: &ShotState, frame: &FrameInfo) -> FrameAsk {
        let s = &self.session;
        let compare = s
            .reference_state(state)
            .filter(|_| !frame.timeline_mode)
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

    /// The values changed so far carry over to the next demonstration; so do the reference's: its changes from
    /// the old demonstration's defaults, over the new one's.
    fn demo_selected(&mut self, change: &DemoChange, host: &mut dyn Host) {
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

    /// The demonstration at its defaults, rendered over eighteen frames; `viewer-compare` first sets it as the
    /// reference, moves one engine setting (else any ranged value) to its other end, compares them with a
    /// wipe, and measures them before the fifteenth.
    fn snapshot(
        &mut self,
        page: &str,
        _args: Option<&str>,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        let compare = page == "viewer-compare";
        if step == 0 {
            let defaults = host.default_state(host.state().demo);
            *host.state_mut() = defaults;
            if compare {
                // The comparison page: the defaults as the reference, then one ranged value moved to its far end.
                self.set_reference(host);
                let entries = values::list();
                let camera = crate::camera::OrbitCamera::schema();
                let ranged = |e: &&Entry| {
                    e.range.is_some()
                        && e.components.is_empty()
                        && e.choices.is_none()
                        && !camera.iter().any(|c| std::ptr::eq(c, *e))
                };
                let entry = entries
                    .iter()
                    .find(|e| e.engine.is_some() && ranged(e))
                    .or_else(|| entries.iter().find(|e| ranged(e) && !e.labels.is_empty()));
                if let Some(e) = entry {
                    // The value goes to whichever end of its range is farther from where it is.
                    let (lo, hi) = e.range.expect("ranged");
                    let now = host.state().p(e.key);
                    let to = if (now - lo).abs() > (now - hi).abs() {
                        lo
                    } else {
                        hi
                    };
                    host.set_value(e.key, to);
                    self.report
                        .push(format!("reference compared with {} {now} → {to}", e.key));
                }
                self.session.compare = Compare::Wipe;
                // A setting baked into the scene data: its build waited for, so the snapshot shows it.
                let state = host.state().clone();
                if let Some(renderer) = host.renderer() {
                    renderer.prepare_scene_data(&state);
                }
            }
        }
        if step == 18 {
            if let Some(m) = self.session.measurement {
                self.report.push(format!(
                    "PSNR {:.2} dB, SSIM {:.4}, FLIP {:.4}",
                    m.psnr, m.ssim, m.flip
                ));
            }
            return Ok(Snapshot::Done(self.report.join("; ")));
        }
        if compare && step == 15 {
            self.measure(host);
        }
        Ok(Snapshot::Settle)
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
