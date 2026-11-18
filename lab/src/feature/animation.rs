//! The Animation tab: the demonstration's clips (src/clip.rs) as a dopesheet in the panel right of the viewport,
//! each clip a row of its tracks with their keyframes as marks, the chosen clip's playhead across them. Dragging
//! along a clip's rows scrubs it, and the viewport shows the demonstration with the clip at that time; the
//! buttons play the clip from there, or step to its next or last keyframe. The panel reads the clips; it does
//! not change them.

use super::{Feature, FeatureLayout, FeatureState, Host, Page, PanelUi, Snapshot};
use crate::clip::{Clip, Track};
use crate::i18n::{args, tr, tr_with};
use crate::palette;
use crate::shot::ShotState;
use anyhow::{Result, anyhow};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2};
use std::sync::Arc;
use std::time::Instant;

pub struct AnimationFeature;

impl Feature for AnimationFeature {
    fn id(&self) -> &'static str {
        "animation"
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-animation").to_string())
    }

    fn order(&self) -> i32 {
        25
    }

    /// An investigation whose scripts have clips (src/script/runtime/clips.rs).
    fn available(&self, _investigation: &dyn crate::investigation::Investigation) -> bool {
        !crate::script::runtime::clips().is_empty()
    }

    fn layout(&self) -> FeatureLayout {
        FeatureLayout {
            panel: true,
            ..Default::default()
        }
    }

    fn pages(&self) -> &'static [Page] {
        &[Page {
            name: "viewer-animation",
            help: "[:<demo>][@<clip>=<seconds>] (the Animation tab: the demonstration's clips as a dopesheet, that clip scrubbed to that time; none: its first clip at its start)",
        }]
    }

    fn snapshot_demo<'a>(&self, _page: &str, args: Option<&'a str>) -> Option<&'a str> {
        args.and_then(|a| a.split('@').next())
            .filter(|d| !d.is_empty())
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Dopesheet::default())
    }
}

/// What the panel asks of the tab.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Asked {
    /// Choose the clip of this row and put its playhead at these seconds.
    Scrub(usize, f32),
    /// Play the chosen clip from the playhead, or pause it.
    Toggle,
    /// To the chosen clip's next keyframe (1) or its last (-1).
    Step(i32),
}

#[derive(Default)]
struct Dopesheet {
    /// The demonstration's clips, as the panel last read them.
    clips: Vec<Arc<Clip>>,
    /// The chosen clip, and its playhead in seconds.
    chosen: usize,
    t: f32,
    /// Playing the chosen clip: since when, from which time.
    playing: Option<(Instant, f32)>,
    /// The state the clip plays over (the demonstration's when scrubbing began), and the one last shown.
    base: Option<ShotState>,
    shown: Option<ShotState>,
    asked: Option<Asked>,
    report: String,
}

/// Whether a clip runs for ever (a run of no length: an animation hook's own clip), so it has no end.
fn open(clip: &Clip) -> bool {
    clip.tracks
        .iter()
        .any(|t| matches!(t, Track::Run { seconds, .. } if *seconds <= 0.0))
}

/// The seconds a clip's lanes span: its length, or for one that runs for ever, ten seconds or past the playhead.
fn span(clip: &Clip, t: f32) -> f32 {
    if open(clip) {
        10.0f32.max(t * 1.25)
    } else {
        clip.seconds().max(1e-3)
    }
}

impl Dopesheet {
    fn clip(&self) -> Option<&Arc<Clip>> {
        self.clips.get(self.chosen)
    }

    /// The chosen clip at the playhead over the base state: shown in the viewport. A state changed since (a
    /// slider, another demonstration) becomes the new base.
    fn show(&mut self, host: &mut dyn Host) {
        let Some(clip) = self.clip().cloned() else {
            return;
        };
        if self.base.is_none() || self.shown.as_ref() != Some(host.state()) {
            self.base = Some(host.state().clone());
        }
        let mut state = self.base.clone().expect("a base state");
        crate::clip::apply(&clip, self.t, &mut state);
        self.shown = Some(state.clone());
        host.set_state(state);
    }

    /// The chosen clip's keyframe times, in order, its start and end among them.
    fn marks(&self) -> Vec<f32> {
        let Some(clip) = self.clip() else {
            return Vec::new();
        };
        let mut marks: Vec<f32> = clip.tracks.iter().flat_map(|t| t.marks()).collect();
        marks.push(0.0);
        marks.push(clip.seconds());
        marks.sort_by(f32::total_cmp);
        marks.dedup();
        marks
    }
}

impl FeatureState for Dopesheet {
    fn selected(&mut self, host: &mut dyn Host) {
        self.clips = crate::clip::demo_clips(host.state().demo);
    }

    fn update(&mut self, host: &mut dyn Host) {
        // The demonstration's clips, read again when it changes.
        let demo_clips = crate::clip::demo_clips(host.state().demo);
        if demo_clips
            .iter()
            .map(|c| &c.name)
            .ne(self.clips.iter().map(|c| &c.name))
        {
            *self = Dopesheet {
                clips: demo_clips,
                ..Default::default()
            };
        }
        // A clip that runs for ever has no end to stop or step to.
        let length = self.clip().map_or(0.0, |c| {
            if open(c) {
                f32::INFINITY
            } else {
                c.seconds()
            }
        });
        match self.asked.take() {
            Some(Asked::Scrub(row, t)) => {
                self.chosen = row;
                self.t = t;
                self.playing = None;
                self.show(host);
            }
            Some(Asked::Toggle) => {
                // At its end, playing starts the clip again.
                self.playing = match self.playing {
                    Some(_) => None,
                    None if self.t >= length => {
                        self.t = 0.0;
                        Some((Instant::now(), 0.0))
                    }
                    None => Some((Instant::now(), self.t)),
                };
                self.show(host);
            }
            Some(Asked::Step(dir)) => {
                // The next keyframe after the playhead, or the last before it.
                let marks = self.marks();
                let t = self.t;
                self.t = if dir > 0 {
                    marks
                        .into_iter()
                        .find(|m| *m > t + 1e-4)
                        .unwrap_or(if length.is_finite() {
                            length
                        } else {
                            t
                        })
                } else {
                    marks
                        .into_iter()
                        .rev()
                        .find(|m| *m < t - 1e-4)
                        .unwrap_or(0.0)
                };
                self.playing = None;
                self.show(host);
            }
            None => {}
        }
        if let Some((since, from)) = self.playing {
            // Playing, the playhead follows the clock, and stops at the clip's end.
            self.t = (from + since.elapsed().as_secs_f32()).min(length);
            if self.t >= length {
                self.playing = None;
            }
            self.show(host);
        }
    }

    fn panel(&mut self, ui: &mut PanelUi) {
        if let Some(asked) = draw(ui, self) {
            self.asked = Some(asked);
        }
    }

    /// `viewer-animation[:<demo>][@<clip>=<seconds>]`: rendered once (the tab opens), the clip scrubbed to the time,
    /// then settled over eight frames.
    fn snapshot(
        &mut self,
        _page: &str,
        args: Option<&str>,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        match step {
            0 => return Ok(Snapshot::Render),
            2..=8 => return Ok(Snapshot::Settle),
            9.. => return Ok(Snapshot::Done(self.report.clone())),
            _ => {}
        }
        self.selected(host);
        let at = args.and_then(|a| a.split_once('@')).map(|(_, at)| at);
        let (name, t) = match at {
            Some(at) => {
                let (name, t) = at.split_once('=').ok_or_else(|| {
                    anyhow!("an animation page scrubs `<clip>=<seconds>`, not `{at}`")
                })?;
                let t: f32 = t
                    .parse()
                    .map_err(|_| anyhow!("`{t}` is not a number of seconds"))?;
                (Some(name.to_string()), t)
            }
            None => (None, 0.0),
        };
        let row = match &name {
            Some(n) => self
                .clips
                .iter()
                .position(|c| &c.name == n)
                .ok_or_else(|| {
                    let names: Vec<&str> = self.clips.iter().map(|c| c.name.as_str()).collect();
                    anyhow!(
                        "the demonstration has no clip `{n}` (its: {})",
                        crate::story_refs::names_or_none(&names)
                    )
                })?,
            None => 0,
        };
        self.asked = Some(Asked::Scrub(row, t));
        self.update(host);
        self.report = match self.clip() {
            Some(c) => format!(
                "clip {} at {:.2} s of {}, {} track(s): {}",
                c.name,
                self.t,
                if open(c) {
                    "a clip that runs for ever".to_string()
                } else {
                    format!("{:.2} s", c.seconds())
                },
                c.tracks.len(),
                c.tracks
                    .iter()
                    .map(|t| t.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            None => "no clips".into(),
        };
        Ok(Snapshot::Settle)
    }
}

/// The height of a track's row, and of a clip's heading, in points.
const ROW: f32 = 30.0;
const HEADING: f32 = 40.0;

/// The dopesheet in the panel: the controls, then each clip's heading and its tracks' rows. What it asks of
/// the tab, if anything.
fn draw(panel: &mut PanelUi, sheet: &Dopesheet) -> Option<Asked> {
    let rect = panel.input.layout.panel;
    crate::paint::panel_card(panel.ink.painter, rect, 12);
    let inner = rect.shrink2(vec2(20.0, 16.0));
    let mut asked = None;
    let mut widgets = Vec::new();
    egui::Area::new(egui::Id::unique("viewer-animation-panel"))
        .fixed_pos(inner.min)
        .constrain(false)
        .fade_in(false)
        .show(panel.ctx, |ui| {
            ui.set_width(inner.width());
            ui.set_clip_rect(inner);
            ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
            ui.spacing_mut().button_padding = vec2(12.0, 8.0);
            ui.label(
                RichText::new(tr("controls-animation"))
                    .size(30.0)
                    .strong()
                    .color(palette::text()),
            );
            if sheet.clips.is_empty() {
                ui.label(
                    RichText::new(tr("animation-none"))
                        .size(22.0)
                        .color(palette::text_dim()),
                );
                return;
            }
            controls(ui, sheet, &mut asked, &mut widgets);
            egui::ScrollArea::vertical()
                .id_salt("viewer-animation-scroll")
                .max_height(inner.bottom() - ui.cursor().top())
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (row, clip) in sheet.clips.iter().enumerate() {
                        clip_rows(ui, sheet, row, clip, &mut asked, &mut widgets);
                    }
                });
        });
    panel.dock.widgets.extend(widgets);
    asked
}

/// Play or pause, the steps to the last and next keyframe, and where the playhead is.
fn controls(
    ui: &mut egui::Ui,
    sheet: &Dopesheet,
    asked: &mut Option<Asked>,
    widgets: &mut Vec<(String, Rect)>,
) {
    ui.horizontal(|ui| {
        let button = |ui: &mut egui::Ui, label: &'static str| {
            ui.button(RichText::new(label).size(20.0).color(palette::text()))
        };
        let back = button(ui, tr("animation-step_back"));
        let toggle = if sheet.playing.is_some() {
            button(ui, tr("animation-pause"))
        } else {
            button(ui, tr("animation-play"))
        };
        let next = button(ui, tr("animation-step_forward"));
        for (name, r, a) in [
            ("animation-back", &back, Asked::Step(-1)),
            ("animation-toggle", &toggle, Asked::Toggle),
            ("animation-next", &next, Asked::Step(1)),
        ] {
            widgets.push((name.to_string(), r.rect));
            if r.clicked() {
                *asked = Some(a);
            }
        }
        let t = format!("{:.2}", sheet.t);
        let time = match sheet.clip() {
            Some(c) if open(c) => tr_with("animation-time_open", args!(t = t)),
            c => tr_with(
                "animation-time",
                args!(
                    t = t,
                    length = format!("{:.2}", c.map_or(0.0, |c| c.seconds()))
                ),
            ),
        };
        ui.label(RichText::new(time).size(20.0).color(palette::text_dim()));
    });
}

/// A clip's heading and its tracks' rows: each a label and a lane of its marks, the chosen clip's playhead
/// across them. A press or a drag in the lanes scrubs the clip.
fn clip_rows(
    ui: &mut egui::Ui,
    sheet: &Dopesheet,
    row: usize,
    clip: &Clip,
    asked: &mut Option<Asked>,
    widgets: &mut Vec<(String, Rect)>,
) {
    let chosen = row == sheet.chosen;
    let length = span(clip, sheet.t);
    let width = ui.available_width();
    let height = HEADING + ROW * clip.tracks.len() as f32;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    widgets.push((format!("animation-clip:{}", clip.name), rect));
    let painter = ui.painter_at(rect);
    let label_w = (width * 0.34).min(220.0);
    let lane = Rect::from_min_max(pos2(rect.left() + label_w, rect.top()), rect.max)
        .shrink2(vec2(8.0, 0.0));
    let x_of = |t: f32| lane.left() + lane.width() * (t / length).clamp(0.0, 1.0);
    let heading = if open(clip) {
        tr_with("animation-clip_open", args!(name = clip.name.clone()))
    } else {
        tr_with(
            "animation-clip",
            args!(
                name = clip.name.clone(),
                length = format!("{:.2}", clip.seconds())
            ),
        )
    };
    let head_color = if chosen {
        palette::accent()
    } else {
        palette::text()
    };
    painter.text(
        pos2(rect.left(), rect.top() + HEADING * 0.5),
        egui::Align2::LEFT_CENTER,
        heading,
        egui::FontId::proportional(22.0),
        head_color,
    );
    for (i, track) in clip.tracks.iter().enumerate() {
        // Each track: its label, its lane's line over the time it runs, and a diamond at each keyframe.
        let y = rect.top() + HEADING + ROW * (i as f32 + 0.5);
        painter.text(
            pos2(rect.left() + 12.0, y),
            egui::Align2::LEFT_CENTER,
            track.label(),
            egui::FontId::proportional(20.0),
            palette::text_dim(),
        );
        let (a, b) = track.span();
        painter.line_segment(
            [pos2(x_of(0.0), y), pos2(x_of(length), y)],
            Stroke::new(1.0, palette::text_faint()),
        );
        painter.line_segment(
            [pos2(x_of(a), y), pos2(x_of(b), y)],
            Stroke::new(3.0, head_color),
        );
        for m in track.marks() {
            diamond(&painter, pos2(x_of(m), y), 6.0, head_color);
        }
    }
    if chosen {
        // The playhead, across the chosen clip's tracks.
        let x = x_of(sheet.t);
        painter.line_segment(
            [pos2(x, rect.top() + HEADING * 0.8), pos2(x, rect.bottom())],
            Stroke::new(2.0, palette::SELECTED),
        );
    }
    if let Some(p) = response
        .interact_pointer_pos()
        .filter(|_| response.is_pointer_button_down_on() || response.clicked())
    {
        let t = ((p.x - lane.left()) / lane.width()).clamp(0.0, 1.0) * length;
        *asked = Some(Asked::Scrub(row, t));
    }
}

/// A keyframe's mark: a filled diamond of half-size `r` at `at`.
fn diamond(painter: &egui::Painter, at: Pos2, r: f32, color: Color32) {
    let points = vec![
        pos2(at.x, at.y - r),
        pos2(at.x + r, at.y),
        pos2(at.x, at.y + r),
        pos2(at.x - r, at.y),
    ];
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}
