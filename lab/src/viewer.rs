//! Interactive viewer. It renders through the same Renderer and RGBA target
//! as the recorder, then blits the target to the window.
//!
//! Controls: left-drag orbit (on a clickable inset: pick, and keep picking
//! under the cursor while dragging), right/middle-drag pan, wheel zoom, click pick,
//! 1–9, 0, −, = the investigation's demonstrations in order, Space
//! play/pause, Right or . step (on a timeline, Left and Right skip 10 s
//! back and forward), R reset, T switch between the timeline and
//! the free demonstrations (with a timeline), F11 a screenshot, Esc or the
//! header's Home button back to the homepage. The dock's sliders
//! (`controls.rs`) edit the demonstration's parameters.
//!
//! The homepage opens the viewer on the demonstrations, or on a timeline,
//! paused at its start or at a chosen chapter. Either way the camera first
//! moves into the shot, one of three intros at random (`animation::Intro`);
//! touching the camera stops it there. Playing runs it in real time (times the speed the dock and the bar set,
//! 0.25x to 4x, the narration's pitch kept) with the narration (FFplay) and
//! captions; the footer's bar seeks. Pausing hands the current shot to the
//! user: the camera, the dock's sliders, and picking work on a copy of the
//! timeline's state, and playing again returns to the timeline where it
//! paused. The narration's volume is set in the dock and at the bar's end,
//! playing or not; a playing narration restarts where it is to take it.
//!
//! `viewer-autotest --report REPORT` drives the same input handlers with the
//! investigation's scripted sequence (`ViewerTest`, including a real window
//! resize) and writes a JSON report.
//!
//! The viewer runs on an event loop it is given (`run_on`), so the homepage
//! can open it and get the window back when it closes.

use crate::animation::{Intro, IntroMove};
use crate::camera::Projector;
use crate::frame::FrameContent;
use crate::gpu::{self, Gpu};
use crate::investigation;
use crate::loading::{self, Loading};
use crate::record::beat_fade;
use crate::renderer::{FrameRequest, Renderer};
use crate::shot::{self, SceneType, DemoId, PanelMode, ShotState, Timeline};
use crate::ui::{Chrome, DockTab, ViewerAction, ViewerChrome};
use crate::values::Reflect;
use crate::volume::Volume;
use anyhow::Result;
use glam::Vec2;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// How long FFplay takes from starting to sounding (measured on this lab's
/// machine: 0.11–0.17 s of overhead for one second of audio, start and end together).
const AUDIO_START_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

/// How far Left and Right skip on a timeline, and how long the viewport says so.
const SKIP_SECONDS: i64 = 10;
const SKIP_NOTICE: std::time::Duration = std::time::Duration::from_millis(900);

pub struct ViewerOptions {
    pub timeline: Option<PathBuf>,
    /// The narration track played with the timeline.
    pub audio: Option<PathBuf>,
    pub manifest_dir: PathBuf,
    /// Size of the viewer's text and panels relative to the video's.
    pub ui_scale: f32,
    pub autotest: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub locale: Option<crate::i18n::Locale>,
    /// Open the timeline at this frame (a chapter chosen on the homepage).
    pub start_frame: u64,
    /// Open borderless fullscreen (the Config page's setting).
    pub fullscreen: bool,
    /// The narration's volume (the Config page's setting); the dock changes it.
    pub narration: Volume,
    /// Opened by the homepage's Experiment button: the Experiments tab, the
    /// changed values kept from one demonstration to the next (src/experiment.rs).
    pub experiments: bool,
    /// The layout's dragged sizes (the settings'), and the settings file a
    /// drag writes them to (None: kept for this run only).
    pub layout: crate::config::ViewerLayout,
    pub settings: Option<PathBuf>,
    /// The timeline is a part of a series with a part after it, to play next:
    /// at its end the viewer leaves with `ViewerExit::Next`.
    pub next: bool,
    /// Play the timeline as soon as its scene is ready (the part after one that ended).
    pub autoplay: bool,
}

use crate::home::fullscreen_mode;
use crate::popup::{self, Popup, Popups};

/// How the viewer was left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerExit {
    /// Esc: back to the homepage.
    Back,
    /// The window was closed (or the autotest finished).
    Closed,
    /// A part of a series played to its end: on to the next part.
    Next,
}

/// A scripted test of the viewer, driven once per redraw (`viewer-autotest`).
pub trait ViewerTest {
    /// Drive the viewer for one redraw; true when the test is finished.
    fn tick(&mut self, app: &mut App) -> bool;
    /// The window reported a new size.
    fn resized(&mut self, width: u32, height: u32);
    /// Write the JSON report; true if every check passed.
    fn finish(&self) -> Result<bool>;
    fn report(&self) -> &std::path::Path;
}

pub struct Surface {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    blit: wgpu::RenderPipeline,
    blit_group: wgpu::BindGroup,
}

#[derive(Clone)]
struct Drag {
    button: MouseButton,
    last: Vec2,
    moved: f32,
    /// A press on an inset's picture with a click handler: the handler and
    /// the picture (canvas points); the drag moves the picked point.
    inset: Option<(String, egui::Rect)>,
}

/// The window while its scene loads (src/loading.rs): the surface, and
/// the device the loading page draws with.
struct Pending {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    loading: Loading,
    screen: loading::Screen,
}

pub struct App {
    options: ViewerOptions,
    pub surface: Option<Surface>,
    pub renderer: Option<Renderer>,
    pending: Option<Pending>,
    pub state: ShotState,
    /// The free camera as the user set it (toggled, or moved while on), over
    /// whatever the state or a playing timeline says, until Reset or a
    /// change of mode.
    free_view: Option<crate::camera::FreeCamera>,
    pub demo_time: f32,
    /// Demo time at which the camera sway of the playing animation started.
    drift_from: f32,
    pub playing: bool,
    pub timeline: Option<Timeline>,
    pub timeline_frame: u64,
    /// While the timeline plays: the instant it was (or will be) at a frame.
    /// Its position is read from the wall clock against this anchor, so a slow
    /// or skipped redraw drops frames instead of falling behind the narration.
    timeline_anchor: (Instant, u64),
    pub timeline_mode: bool,
    /// The last skip: the seconds skipped by presses in a row, and when the last one was.
    skip: Option<(i64, Instant)>,
    /// The last live reload of the shaders: what it says, whether it failed,
    /// and when (a success shows for a few seconds, a failure until fixed).
    shader_reload: Option<(String, bool, Instant)>,
    audio: Option<crate::audio::AudioPlayer>,
    /// The narration's volume; the viewer hands it back when it closes.
    pub narration: Volume,
    /// The timeline's speed, the narration's too (`ui::SPEEDS`; 1 by default).
    pub speed: f32,
    /// Paused on a diagram beat: a first view of it (a snapshot's: src/diagram.rs
    /// `Paused`), which the UI's view (zoom, pan, boxes moved) starts from.
    diagram_preset: Option<(f32, egui::Vec2)>,
    cursor: Vec2,
    drag: Option<Drag>,
    pub ui_has_pointer: bool,
    egui_events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    tab: DockTab,
    /// The dock's slider rectangles from the last frame.
    pub dock_widgets: Vec<(String, egui::Rect)>,
    pub last_content: Option<FrameContent>,
    /// The insets of the last frame drawn, for clicks (`UiReport::insets`).
    last_insets: Vec<crate::ui::InsetArea>,
    /// Every beat of the timeline, for the bar's hover preview.
    bar_sections: Vec<crate::ui::BarSection>,
    /// The bar's hover preview drawn last frame (for the autotest).
    pub bar_hover: Option<String>,
    last_tick: Instant,
    frames: u64,
    autotest: Option<Box<dyn ViewerTest>>,
    pub exit: ViewerExit,
    /// The Home button was pressed: the event loop exits after this frame.
    pub leaving: bool,
    /// F11: save the next presented frame (`readback::screenshot`).
    screenshot: bool,
    /// Messages over the viewer (src/popup.rs), such as where a screenshot went.
    popups: Popups,
    error: Option<anyhow::Error>,
    /// The camera's move into the first shot, and when it started (src/animation.rs).
    intro: Option<(IntroMove, Instant)>,
    /// The experiment: its reference, comparison, and measurement (src/experiment.rs).
    pub experiment: crate::experiment::Session,
    /// Recent frame times in milliseconds (the Experiments tab's readout).
    frame_times: std::collections::VecDeque<f64>,
    /// The layout's dragged sizes; the viewer hands them back when it closes.
    pub layout: crate::config::ViewerLayout,
    /// The window's cursor as last set (egui's request, src/renderer.rs `cursor`).
    cursor_icon: egui::CursorIcon,
    /// A topic the knowledge panel selects and centres once (a snapshot's).
    knowledge_focus: Option<String>,
    /// The knowledge graph is shown full screen (Esc closes it).
    knowledge_overlay: bool,
    /// The frame graph's DAG is shown full screen (Esc closes it).
    dag_overlay: bool,
    /// The full-screen DAG shares the screen with the selected pass's WGSL.
    graph_source: bool,
}

impl App {
    fn new(options: ViewerOptions) -> Result<App> {
        let timeline = options.timeline.as_ref().map(|p| Timeline::load(p)).transpose()?;
        crate::i18n::set(options.locale.or(timeline.as_ref().map(|t| t.locale)).unwrap_or_default());
        let autotest = match options.autotest.clone() {
            Some(report) => Some(investigation::current().viewer_test(report).ok_or_else(|| anyhow::anyhow!("{} has no viewer autotest", investigation::current().title()))?),
            None => None,
        };
        let mut audio = options.audio.clone().and_then(|a| crate::audio::AudioPlayer::new(a, &options.manifest_dir));
        if let Some(a) = audio.as_mut() {
            a.set_gain(options.narration.gain());
        }
        let narration = options.narration;
        // A timeline opens on its first frame, paused; the autotest starts in the free demonstrations.
        let start_in_timeline = timeline.is_some() && autotest.is_none();
        let no_autotest = autotest.is_none();
        let start_frame = timeline.as_ref().map_or(0, |t| options.start_frame.min(t.total_frames.saturating_sub(1)));
        let state = match (&timeline, start_in_timeline) {
            (Some(t), true) => t.state_at(start_frame),
            _ => ShotState::for_demo(investigation::current().demos()[0]),
        };
        let camera = state.camera;
        let scene = investigation::current().demo_scene(state.demo);
        let tab = if options.experiments { DockTab::Experiments } else { DockTab::default() };
        let layout = options.layout;
        Ok(App {
            options,
            surface: None,
            renderer: None,
            pending: None,
            state,
            free_view: None,
            demo_time: 0.0,
            drift_from: 0.0,
            playing: false,
            bar_sections: timeline.as_ref().map(bar_sections).unwrap_or_default(),
            bar_hover: None,
            timeline,
            timeline_frame: start_frame,
            timeline_anchor: (Instant::now(), start_frame),
            timeline_mode: start_in_timeline,
            skip: None,
            shader_reload: None,
            audio,
            narration,
            speed: 1.0,
            diagram_preset: None,
            cursor: Vec2::ZERO,
            drag: None,
            ui_has_pointer: false,
            egui_events: Vec::new(),
            modifiers: egui::Modifiers::default(),
            tab,
            dock_widgets: Vec::new(),
            last_content: None,
            last_insets: Vec::new(),
            last_tick: Instant::now(),
            frames: 0,
            autotest,
            exit: ViewerExit::Closed,
            leaving: false,
            screenshot: false,
            popups: Popups::default(),
            error: None,
            // The autotest checks the camera, so it opens straight on the shot.
            intro: no_autotest.then(|| (Intro::random().fit(camera, |c| scene.camera_clear(c.eye())), Instant::now())),
            experiment: crate::experiment::Session { presets: crate::experiment::presets(), ..Default::default() },
            frame_times: std::collections::VecDeque::new(),
            layout,
            cursor_icon: egui::CursorIcon::Default,
            knowledge_focus: None,
            knowledge_overlay: false,
            dag_overlay: false,
            graph_source: false,
        })
    }

    pub fn renderer(&self) -> &Renderer {
        self.renderer.as_ref().expect("renderer")
    }

    /// Whether the timeline is playing (and owns the shot).
    pub fn timeline_playing(&self) -> bool {
        self.timeline_mode && self.playing && self.timeline.is_some()
    }

    /// Paused on a 2D scene of the timeline: the viewport keeps showing its
    /// page (a diagram or a sources card), not the 3D state behind it.
    pub fn paused_page(&self) -> bool {
        match (&self.timeline, self.timeline_mode && !self.playing) {
            (Some(t), true) => t.beat_at(self.timeline_frame).1.kind == SceneType::TwoD,
            _ => false,
        }
    }

    /// Paused on a 2D scene that shows a diagram: the viewport is the diagram's panel, zoomed and panned.
    pub fn paused_diagram(&self) -> bool {
        self.paused_page() && self.timeline.as_ref().is_some_and(|t| t.beat_at(self.timeline_frame).1.shows_diagram())
    }

    /// The state that is rendered: the playing timeline's frame, or the
    /// interactive state (a paused timeline's shot, or a demonstration with
    /// its animation applied while it plays).
    pub fn render_state(&self) -> ShotState {
        let mut s = match &self.timeline {
            Some(t) if self.timeline_playing() => t.state_at(self.timeline_frame),
            _ => {
                let mut s = self.state.clone();
                if self.playing && !self.timeline_mode {
                    for (k, v) in investigation::current().animation(s.demo, self.demo_time) {
                        s.set(k, v);
                    }
                    s.camera = shot::camera_drift(s.camera, self.drift_from, self.demo_time, drift(&s));
                }
                s
            }
        };
        if let Some(free) = self.free_view {
            s.free_camera = free;
        }
        s
    }

    /// The free camera the user moves: theirs, else the shown one, now theirs.
    fn free_view(&mut self) -> &mut crate::camera::FreeCamera {
        let shown = self.render_state().free_camera;
        self.free_view.get_or_insert(shown)
    }

    /// Whether the viewport shows the free camera's view.
    fn free_camera_on(&self) -> bool {
        self.render_state().free_camera.on
    }

    pub fn default_state(&mut self, demo: impl Into<DemoId>) -> ShotState {
        let demo = demo.into();
        self.renderer.as_mut().map(|r| r.default_state(demo)).unwrap_or_else(|| ShotState::for_demo(demo))
    }

    /// Stop the intro where it is, keeping its camera, so the user's camera input carries on from what they see.
    fn end_intro(&mut self) {
        if let Some((intro, started)) = self.intro.take() {
            self.state.camera = intro.camera(self.state.camera, started.elapsed().as_secs_f32());
        }
    }

    /// The rendered state with the intro's camera on its way into the shot.
    fn with_intro(&mut self, mut state: ShotState, now: Instant) -> ShotState {
        if let Some((intro, started)) = self.intro {
            let t = (now - started).as_secs_f32();
            state.camera = intro.camera(state.camera, t);
            if t >= Intro::SECONDS {
                self.intro = None;
            }
        }
        state
    }

    pub fn select_demo(&mut self, demo: impl Into<DemoId>) {
        let demo = demo.into();
        self.intro = None;
        self.stop_audio();
        // Experimenting, the values changed so far carry over to the next demonstration.
        // So does the reference's: its changes from the old demonstration's defaults, over the new one's.
        let (carried, reference) = if self.options.experiments {
            let defaults = self.default_state(self.state.demo);
            (crate::experiment::changes(&self.state, &defaults), self.experiment.reference.as_ref().map(|r| crate::experiment::changes(r, &defaults)))
        } else {
            (Default::default(), None)
        };
        self.state = self.default_state(demo);
        if let Err(e) = crate::experiment::apply(&mut self.state, &carried) {
            self.experiment.message = Some(format!("{e:#}"));
        }
        if let Some(changes) = reference {
            let mut r = self.default_state(demo);
            match crate::experiment::apply(&mut r, &changes) {
                Ok(()) => self.experiment.reference = Some(r),
                Err(e) => self.experiment.message = Some(format!("{e:#}")),
            }
            self.experiment.measurement = None;
        }
        self.demo_time = 0.0;
        self.drift_from = 0.0;
        self.playing = false;
        self.timeline_mode = false;
        self.show_tab();
    }

    /// The Code tab shows the demonstration's first listing in the panel;
    /// the other tabs show its struct panel.
    fn show_tab(&mut self) {
        if self.tab == DockTab::Captions && !self.timeline_mode {
            self.tab = DockTab::Parameters;
        }
        let defaults = self.default_state(self.state.demo);
        // The Prerequisites tab shows the knowledge graph in the panel.
        if self.tab == DockTab::Prerequisites {
            self.state.panel = PanelMode::Knowledge;
        } else if self.state.panel == PanelMode::Knowledge {
            self.state.panel = defaults.panel;
        }
        // The Frame graph tab shows the frame graph (its DAG, unless a view of it shows).
        let graph = matches!(self.state.panel, PanelMode::Graph | PanelMode::Dag | PanelMode::Resources);
        if self.tab == DockTab::Graph {
            if !graph {
                self.state.panel = PanelMode::Dag;
            }
        } else if graph {
            self.state.panel = defaults.panel;
            self.dag_overlay = false;
        }
        if self.tab == DockTab::Code {
            if self.state.panel != PanelMode::Code {
                self.state.panel = PanelMode::Code;
                self.state.code = investigation::current().listing_for_demo(self.state.demo).map(|l| l.name.to_string());
                self.state.highlight.clear();
            }
        } else if self.state.panel == PanelMode::Code {
            self.state.panel = defaults.panel;
            self.state.highlight = defaults.highlight;
            self.state.code = None;
        }
    }

    /// The chapters the Prerequisites tab is about: on a timeline, the
    /// current one; else those whose demonstrations include this one.
    fn knowledge_chapters(&self) -> Vec<u32> {
        match (&self.timeline, self.timeline_mode) {
            (Some(t), true) => vec![t.chapter_at(self.timeline_frame).number],
            _ => crate::knowledge::current().map(|g| g.chapters_of_demo(self.state.demo.key())).unwrap_or_default(),
        }
    }

    /// Whether the demonstration's animation drives `param`.
    fn animated(&self, param: &str) -> bool {
        investigation::current().animation(self.state.demo, self.demo_time).iter().any(|p| p.0 == param)
    }

    /// Keep the animation's current values and the swayed camera, so that
    /// the sliders and the view hold the pose it stopped at.
    fn bake_animation(&mut self) {
        for (k, v) in investigation::current().animation(self.state.demo, self.demo_time) {
            self.state.set(k, v);
        }
        self.state.camera = shot::camera_drift(self.state.camera, self.drift_from, self.demo_time, drift(&self.state));
        self.drift_from = self.demo_time;
    }

    fn play(&mut self) {
        self.playing = true;
        // The camera sways from where it is now.
        self.drift_from = self.demo_time;
        if self.timeline_mode {
            // The timeline takes the shot back from where it paused, with its narration.
            self.start_audio();
        }
    }

    fn stop(&mut self) {
        if self.playing && !self.timeline_mode {
            self.bake_animation();
        }
        if self.playing && self.timeline_mode {
            self.stop_audio();
            self.take_timeline_shot();
            // Paused, the dock shows the caption, to read again.
            self.tab = DockTab::Captions;
            self.show_tab();
        }
        self.playing = false;
    }

    /// A paused timeline hands its shot to the user: the interactive state
    /// starts as the timeline's state at the current frame.
    fn take_timeline_shot(&mut self) {
        if let Some(t) = &self.timeline {
            self.state = t.state_at(self.timeline_frame);
            self.demo_time = 0.0;
            self.drift_from = 0.0;
        }
    }

    /// The playing timeline's frame at `now`, from the wall clock since its anchor.
    pub fn anchored_frame(&self, now: Instant) -> u64 {
        let (at, from) = self.timeline_anchor;
        from + (now.saturating_duration_since(at).as_secs_f64() * self.fps() * self.speed as f64) as u64
    }

    /// The instant the playing timeline's clock counts from.
    pub fn timeline_anchor(&self) -> Instant {
        self.timeline_anchor.0
    }

    fn fps(&self) -> f64 {
        self.timeline.as_ref().map_or(30.0, |t| t.fps as f64)
    }

    /// Start the narration at the current frame, and anchor the timeline's
    /// clock there. FFplay takes about a tenth of a second to start sounding,
    /// so the picture holds this frame that long.
    fn start_audio(&mut self) {
        let seconds = self.timeline_frame as f64 / self.fps();
        let speed = self.speed;
        let started = self.audio.as_mut().is_some_and(|a| {
            a.set_speed(speed);
            a.play_from(seconds)
        });
        let delay = if started { AUDIO_START_DELAY } else { std::time::Duration::ZERO };
        self.timeline_anchor = (Instant::now() + delay, self.timeline_frame);
    }

    pub fn stop_audio(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            a.stop();
        }
    }

    /// Move the timeline to `frame`: playing, the narration follows; paused,
    /// the new frame's shot is handed over.
    fn seek(&mut self, frame: u64) {
        let Some(total) = self.timeline.as_ref().map(|t| t.total_frames) else { return };
        self.timeline_frame = frame.min(total.saturating_sub(1));
        if self.timeline_playing() {
            self.start_audio();
        } else if self.timeline_mode {
            self.take_timeline_shot();
        }
    }

    /// Skip the timeline `seconds` back or forward; presses in the same
    /// direction while the notice shows add up in it.
    fn skip(&mut self, seconds: i64) {
        let Some(fps) = self.timeline.as_ref().map(|t| t.fps as i64) else { return };
        self.seek((self.timeline_frame as i64 + seconds * fps).max(0) as u64);
        let now = Instant::now();
        let total = match self.skip {
            Some((s, at)) if now - at < SKIP_NOTICE && s.signum() == seconds.signum() => s + seconds,
            _ => seconds,
        };
        self.skip = Some((total, now));
    }

    pub fn apply(&mut self, action: ViewerAction) {
        let interactive = !self.timeline_playing();
        match action {
            ViewerAction::SelectDemo(d) => self.select_demo(d),
            ViewerAction::TogglePlay => {
                if self.playing {
                    self.stop();
                } else {
                    self.play();
                }
            }
            ViewerAction::Step => {
                self.stop();
                if self.timeline_mode {
                    self.seek(self.timeline_frame + 1);
                } else {
                    self.drift_from = self.demo_time;
                    self.demo_time += 1.0 / 30.0;
                    self.bake_animation();
                }
            }
            ViewerAction::ToggleFreeCamera => {
                let shown = self.render_state();
                let mut free = shown.free_camera;
                free.on = !free.on;
                // Turned on, it starts beside the main camera, seeing it and its view.
                if free.on {
                    free.place(&crate::camera::FreeCamera::beside(&shown.camera));
                }
                self.free_view = Some(free);
            }
            ViewerAction::Reset => {
                self.free_view = None;
                if self.timeline_mode {
                    self.seek(0);
                } else {
                    self.state = self.default_state(self.state.demo);
                    self.demo_time = 0.0;
                    self.drift_from = 0.0;
                    self.playing = false;
                }
            }
            ViewerAction::ToggleTimeline => {
                if self.timeline.is_some() {
                    self.stop_audio();
                    self.playing = false;
                    self.timeline_mode = !self.timeline_mode;
                    self.free_view = None;
                    if self.timeline_mode {
                        self.take_timeline_shot();
                    }
                    self.show_tab();
                }
            }
            ViewerAction::Seek(fraction) => {
                let total = self.timeline.as_ref().map_or(0, |t| t.total_frames);
                self.seek((fraction.clamp(0.0, 1.0) as f64 * total as f64) as u64);
            }
            ViewerAction::SetParam(name, value) if interactive => {
                // Taking hold of an animated value stops the animation where it is.
                if self.playing && self.animated(name) {
                    self.stop();
                }
                self.state.set(name, value);
                for (k, v) in investigation::current().linked_params(name, value) {
                    self.state.set(k, v);
                }
            }
            ViewerAction::SetCamera(camera) if interactive => {
                // The camera sliders show the swayed view; hold it before changing it.
                self.end_intro();
                self.stop();
                self.state.camera = camera;
            }
            ViewerAction::SetRecord(record) if interactive => {
                if std::mem::discriminant(&record) != std::mem::discriminant(&self.state.record) {
                    self.state.highlight = shot::default_highlight(record);
                }
                self.state.record = record;
            }
            ViewerAction::SetHighlight(fields) if interactive => self.state.highlight = fields,
            ViewerAction::SetPanel(mode) if interactive => self.state.panel = mode,
            ViewerAction::Inspect(key) if interactive => inspect(&mut self.state, key),
            ViewerAction::ResetParameters if interactive => {
                // The camera stays unless the dock shows it (the overview's camera sliders).
                self.stop();
                let demo = self.state.demo;
                let defaults = self.default_state(demo);
                let camera = if investigation::current().resets_camera(demo) { defaults.camera } else { self.state.camera };
                self.state = ShotState { camera, ..defaults };
                self.demo_time = 0.0;
                self.drift_from = 0.0;
                self.playing = false;
            }
            ViewerAction::SelectTab(tab) => {
                self.tab = tab;
                if tab == DockTab::Experiments {
                    self.experiment.presets = crate::experiment::presets();
                }
                self.show_tab();
            }
            ViewerAction::SetCode(name) if interactive => {
                self.state.code = Some(name.to_string());
                self.state.highlight.clear();
            }
            ViewerAction::Home => {
                self.stop_audio();
                self.exit = ViewerExit::Back;
                self.leaving = true;
            }
            ViewerAction::ResetParam(key) if interactive => {
                let defaults = self.default_state(self.state.demo);
                if let Some(value) = defaults.try_get(key) {
                    if crate::camera::OrbitCamera::schema().iter().any(|e| e.key == key || e.components.contains(&key)) {
                        self.end_intro();
                        self.state.camera.set(key, value);
                    } else {
                        self.state.set(key, value.f32());
                        for (k, v) in investigation::current().linked_params(key, value.f32()) {
                            self.state.set(k, v);
                        }
                    }
                }
            }
            ViewerAction::SetReference => {
                self.experiment.reference = Some(self.state.clone());
                if self.experiment.compare == crate::experiment::Compare::Off {
                    self.experiment.compare = crate::experiment::Compare::Wipe;
                }
                self.experiment.measurement = None;
                self.experiment.message = Some(crate::i18n::tr("experiments.reference_set").to_string());
            }
            ViewerAction::ClearReference => {
                self.experiment.reference = None;
                self.experiment.compare = crate::experiment::Compare::Off;
                self.experiment.measurement = None;
            }
            ViewerAction::SwapReference if interactive => {
                if let Some(reference) = self.experiment.reference.take() {
                    let camera = self.state.camera;
                    let live = std::mem::replace(&mut self.state, ShotState { camera, ..reference });
                    self.experiment.reference = Some(live);
                    self.experiment.measurement = None;
                }
            }
            ViewerAction::SetCompare(mode) => self.experiment.compare = mode,
            ViewerAction::SetSplit(split) => self.experiment.split = split.clamp(0.0, 1.0),
            ViewerAction::SetGain(gain) => self.experiment.gain = gain.clamp(1.0, 64.0),
            ViewerAction::Measure => self.measure(),
            ViewerAction::SavePreset(name) => {
                let defaults = self.default_state(self.state.demo);
                let experiment = crate::experiment::Experiment::capture(&name, &self.state, &defaults, self.experiment.reference.as_ref());
                self.experiment.message = Some(match crate::experiment::save_user(&experiment) {
                    Ok(path) => crate::i18n::tr_with("experiments.saved", crate::i18n::args!(path = path.display().to_string())),
                    Err(e) => format!("{e:#}"),
                });
                self.experiment.presets = crate::experiment::presets();
            }
            ViewerAction::LoadPreset(i) if interactive => {
                if let Some(preset) = self.experiment.presets.get(i).cloned() {
                    self.experiment.message = Some(match self.load_experiment(&preset) {
                        Ok(()) => crate::i18n::tr_with("experiments.loaded", crate::i18n::args!(name = preset.name.clone())),
                        Err(e) => format!("{e:#}"),
                    });
                }
            }
            ViewerAction::ResizeLayout(part, size) => {
                let size = size.map(|s| s.round().max(1.0) as u32);
                match part {
                    crate::ui::LayoutPart::Panel => self.layout.panel_width = size,
                    crate::ui::LayoutPart::CodePanel => self.layout.code_panel_width = size,
                    crate::ui::LayoutPart::Dock => self.layout.dock_height = size,
                }
            }
            ViewerAction::SaveLayout => self.save_layout(),
            ViewerAction::SetKnowledgeOverlay(on) => self.knowledge_overlay = on,
            ViewerAction::SetDagOverlay(on) => self.dag_overlay = on,
            ViewerAction::ShowSource(on) => self.graph_source = on,
            ViewerAction::ResetLayout => {
                self.layout = Default::default();
                self.save_layout();
            }
            ViewerAction::SetNarration(volume, finished) => {
                self.narration = volume;
                if let Some(a) = self.audio.as_mut() {
                    a.set_gain(volume.gain());
                }
                // FFplay takes a volume when it starts: a playing narration restarts where it is.
                if finished && self.timeline_playing() {
                    self.start_audio();
                }
            }
            ViewerAction::SetSpeed(speed) => {
                // Playing, the timeline goes on from where it is at the new speed, the narration with it.
                if self.timeline_playing() {
                    let total = self.timeline.as_ref().map_or(0, |t| t.total_frames);
                    self.timeline_frame = self.anchored_frame(Instant::now()).min(total.saturating_sub(1));
                    self.speed = speed;
                    self.start_audio();
                } else {
                    self.speed = speed;
                }
            }
            _ => {}
        }
    }

    /// Write the layout's sizes to the settings file, the other settings as it has them.
    fn save_layout(&mut self) {
        // What the user sees: the sizes as the layout kept them (clamped to the window).
        if let Some(r) = self.renderer.as_ref() {
            let l = r.frame_layout();
            let dock = (l.captions.max.y - l.captions.min.y).round() as u32;
            let panel = (l.panel.max.x - l.panel.min.x).round() as u32;
            if self.layout.dock_height.is_some() {
                self.layout.dock_height = Some(dock);
            }
            let code = matches!(self.state.panel, PanelMode::Code | PanelMode::Knowledge | PanelMode::Dag | PanelMode::Resources);
            let width = if code { &mut self.layout.code_panel_width } else { &mut self.layout.panel_width };
            if width.is_some() && l.viewport.max.x < l.panel.min.x {
                *width = Some(panel);
            }
        }
        let Some(path) = self.options.settings.clone() else { return };
        if let Err(e) = crate::config::Config::save_layout(&path, self.layout) {
            self.popups.push(Popup::new(crate::i18n::tr("ui.layout_not_saved"), format!("{e:#}")).closing_after(8.0));
        }
    }

    /// Measure the live view against the reference's, as the last frame drew them (src/metrics.rs).
    fn measure(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else { return };
        let views = renderer.read_view(crate::renderer::ViewKind::Reference).zip(renderer.read_view(crate::renderer::ViewKind::Live));
        match views {
            Some(((ref_display, ref_linear), (display, linear))) if ref_display.width == display.width && ref_display.height == display.height => {
                self.experiment.measurement = Some(crate::metrics::measure(&ref_display, &display, Some((&ref_linear, &linear))));
                self.experiment.message = None;
            }
            _ => self.experiment.message = Some(crate::i18n::tr("experiments.measure_needs_reference").to_string()),
        }
    }

    /// Open an experiment file: its demonstration, camera, and base values,
    /// and its reference (with a comparison) when it has one.
    fn load_experiment(&mut self, preset: &crate::experiment::Preset) -> Result<()> {
        let experiment = preset.load()?;
        experiment.check()?;
        self.stop();
        self.intro = None;
        let demo = experiment.start_demo()?;
        let mut state = self.default_state(demo);
        experiment.apply(&mut state, &experiment.base)?;
        self.experiment.reference = match experiment.reference_values() {
            Some(values) => {
                let mut reference = state.clone();
                experiment.apply(&mut reference, &values)?;
                Some(reference)
            }
            None => None,
        };
        if self.experiment.reference.is_some() && self.experiment.compare == crate::experiment::Compare::Off {
            self.experiment.compare = crate::experiment::Compare::Wipe;
        }
        self.experiment.measurement = None;
        self.state = state;
        Ok(())
    }

    pub fn key(&mut self, code: KeyCode, event_loop: Option<&ActiveEventLoop>) {
        const DEMO_KEYS: [KeyCode; 12] = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
            KeyCode::Digit0,
            KeyCode::Minus,
            KeyCode::Equal,
        ];
        // The full-screen DAG and knowledge graph take the keys; Esc closes them.
        if self.dag_overlay {
            if code == KeyCode::Escape {
                self.dag_overlay = false;
            }
            return;
        }
        if self.knowledge_overlay {
            if code == KeyCode::Escape {
                self.knowledge_overlay = false;
            }
            return;
        }
        // The keys choose among the shown scene's demonstrations.
        let investigation = investigation::current();
        let demos = investigation.scene_demos(investigation.demo_scene(self.state.demo));
        if let Some(d) = DEMO_KEYS.iter().position(|k| *k == code).and_then(|i| demos.get(i)) {
            self.apply(ViewerAction::SelectDemo(*d));
            return;
        }
        match code {
            KeyCode::Space => self.apply(ViewerAction::TogglePlay),
            KeyCode::ArrowLeft if self.timeline_mode => self.skip(-SKIP_SECONDS),
            KeyCode::ArrowRight if self.timeline_mode => self.skip(SKIP_SECONDS),
            KeyCode::ArrowRight | KeyCode::Period => self.apply(ViewerAction::Step),
            KeyCode::KeyR => self.apply(ViewerAction::Reset),
            KeyCode::KeyT => self.apply(ViewerAction::ToggleTimeline),
            KeyCode::KeyC => self.apply(ViewerAction::ToggleFreeCamera),
            KeyCode::Escape => {
                if let Some(el) = event_loop {
                    self.exit = ViewerExit::Back;
                    el.exit();
                }
            }
            _ => {}
        }
    }

    /// The pixel of the scene under a window pixel: in the main view, or in a
    /// view beside it at the same place of the main one (the views share
    /// one size, and usually the camera), so a drag or a click in any view
    /// moves or picks through the main view's camera.
    fn scene_pixel(&self, window_px: Vec2) -> Option<Vec2> {
        let renderer = self.renderer();
        let layout = renderer.frame_layout();
        layout.target_to_viewport_pixel(window_px.x, window_px.y).or_else(|| {
            renderer.frame_panes().iter().find_map(|rect| crate::ui::Layout { viewport: *rect, ..layout }.target_to_viewport_pixel(window_px.x, window_px.y))
        })
    }

    pub fn viewport_height(&self) -> f32 {
        self.renderer().frame_layout().viewport_size_px()[1] as f32
    }

    pub fn pointer_down(&mut self, button: MouseButton, at: Vec2) {
        if self.ui_has_pointer || self.scene_pixel(at).is_none() {
            return;
        }
        // A left press on a picture with a click handler picks there at once,
        // and dragging keeps picking under the cursor.
        let point = self.to_egui(at);
        let inset = (button == MouseButton::Left && !self.timeline_playing())
            .then(|| self.last_insets.iter().rev().find(|i| i.frame.contains(point)))
            .flatten()
            .and_then(|i| i.click.clone().filter(|_| i.picture.contains(point)).map(|h| (h, i.picture)));
        if let Some((handler, picture)) = &inset {
            self.run_inset_click(handler, *picture, point);
        }
        self.drag = Some(Drag { button, last: at, moved: 0.0, inset });
    }

    /// Run an inset's click handler at `point` (canvas points), clamped to its picture.
    fn run_inset_click(&mut self, handler: &str, picture: egui::Rect, point: egui::Pos2) {
        let Some(content) = &self.last_content else { return };
        let shown = self.render_state();
        let Some(renderer) = &self.renderer else { return };
        let uv = ((point - picture.min) / picture.size()).clamp(egui::Vec2::ZERO, egui::vec2(1.0, 1.0));
        let mut frame = crate::actions::ClickFrame { shown: &shown, state: &mut self.state, uv: Vec2::new(uv.x, uv.y), content, statics: &renderer.statics };
        if let Err(e) = (investigation::current().actions().click)(handler, &mut frame) {
            eprintln!("click on an image: {e}");
        }
    }

    pub fn pointer_move(&mut self, at: Vec2) {
        self.cursor = at;
        let Some(mut drag) = self.drag.clone() else { return };
        let delta = at - drag.last;
        drag.moved += delta.length();
        drag.last = at;
        self.drag = Some(drag.clone());
        // A playing timeline holds its camera; the free camera stays the viewer's to move.
        if self.timeline_playing() && !self.free_camera_on() {
            return;
        }
        if let Some((handler, picture)) = &drag.inset {
            let point = self.to_egui(at);
            self.run_inset_click(handler, *picture, point);
            return;
        }
        // A paused diagram takes the pointer itself (src/diagram.rs): the camera keeps its place.
        if self.paused_diagram() {
            return;
        }
        let height = self.viewport_height();
        // With the free camera on, the pointer moves it; the main camera keeps its place.
        if self.free_camera_on() {
            let free = self.free_view();
            let mut orbit = free.orbit();
            match drag.button {
                MouseButton::Left => orbit.orbit(delta),
                MouseButton::Right | MouseButton::Middle => orbit.pan(delta, height),
                _ => {}
            }
            free.place(&orbit);
            return;
        }
        self.end_intro();
        // A picture the investigation turns itself takes the drag; the camera keeps its place.
        if let (MouseButton::Left, Some(values)) = (drag.button, investigation::current().pointer_values(&self.state)) {
            let turn = |state: &mut ShotState, key: &str, by: f32| {
                if let Some(v) = state.try_get(key) {
                    state.set(key, v.f32() + by);
                }
            };
            turn(&mut self.state, values.yaw, -delta.x * 0.3);
            turn(&mut self.state, values.pitch, -delta.y * 0.3);
            return;
        }
        match drag.button {
            MouseButton::Left => self.state.camera.orbit(delta),
            MouseButton::Right | MouseButton::Middle => self.state.camera.pan(delta, height),
            _ => {}
        }
    }

    pub fn pointer_up(&mut self, button: MouseButton, at: Vec2) {
        let Some(drag) = self.drag.take() else { return };
        // A press on a picture already picked, and its drag kept picking.
        if drag.inset.is_some() {
            return;
        }
        if drag.button == button && button == MouseButton::Left && drag.moved < 4.0 {
            // A paused diagram takes its clicks itself (src/diagram.rs).
            if self.paused_diagram() {
                return;
            }
            self.click(at);
        }
    }

    fn to_egui(&self, p: Vec2) -> egui::Pos2 {
        let scale = self.renderer().layout().scale;
        egui::pos2(p.x / scale, p.y / scale)
    }

    /// A cursor move in window pixels, for egui and the 3D view.
    pub fn cursor_moved(&mut self, p: Vec2) {
        self.egui_events.push(egui::Event::PointerMoved(self.to_egui(p)));
        self.pointer_move(p);
    }

    pub fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if let Some(b) = match button {
            MouseButton::Left => Some(egui::PointerButton::Primary),
            MouseButton::Right => Some(egui::PointerButton::Secondary),
            MouseButton::Middle => Some(egui::PointerButton::Middle),
            _ => None,
        } {
            self.egui_events.push(egui::Event::PointerButton { pos: self.to_egui(self.cursor), button: b, pressed, modifiers: self.modifiers });
        }
        if pressed {
            self.pointer_down(button, self.cursor);
        } else {
            self.pointer_up(button, self.cursor);
        }
    }

    /// Keys for a number field being typed into (a slider's value).
    fn egui_key(&mut self, event: &winit::event::KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        let key = match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Some(egui::Key::Enter),
            PhysicalKey::Code(KeyCode::Escape) => Some(egui::Key::Escape),
            PhysicalKey::Code(KeyCode::Backspace) => Some(egui::Key::Backspace),
            PhysicalKey::Code(KeyCode::Delete) => Some(egui::Key::Delete),
            PhysicalKey::Code(KeyCode::ArrowLeft) => Some(egui::Key::ArrowLeft),
            PhysicalKey::Code(KeyCode::ArrowRight) => Some(egui::Key::ArrowRight),
            PhysicalKey::Code(KeyCode::Home) => Some(egui::Key::Home),
            PhysicalKey::Code(KeyCode::End) => Some(egui::Key::End),
            PhysicalKey::Code(KeyCode::Tab) => Some(egui::Key::Tab),
            PhysicalKey::Code(KeyCode::KeyA) if self.modifiers.command => Some(egui::Key::A),
            _ => None,
        };
        if let Some(key) = key {
            self.egui_events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: event.repeat, modifiers: self.modifiers });
        }
        if let (true, Some(text)) = (pressed, &event.text) {
            if !text.chars().any(char::is_control) {
                self.egui_events.push(egui::Event::Text(text.to_string()));
            }
        }
    }

    pub fn wheel(&mut self, steps: f32) {
        // A paused diagram zooms itself (src/diagram.rs).
        if self.paused_diagram() {
            return;
        }
        if self.free_camera_on() && !self.ui_has_pointer {
            let free = self.free_view();
            let mut orbit = free.orbit();
            orbit.zoom(steps);
            free.place(&orbit);
        } else if !self.timeline_playing() && !self.ui_has_pointer {
            self.end_intro();
            match investigation::current().pointer_values(&self.state).and_then(|v| v.zoom) {
                Some(key) => {
                    if let Some(v) = self.state.try_get(key) {
                        self.state.set(key, (v.f32() + steps * 0.1).clamp(0.0, 1.0));
                    }
                }
                None => self.state.camera.zoom(steps),
            }
        }
    }

    fn click(&mut self, at: Vec2) {
        let Some(pixel) = self.scene_pixel(at) else { return };
        let Some(content) = &self.last_content else { return };
        let shown = self.render_state();
        let Some(renderer) = &self.renderer else { return };
        // An inset covers the scene behind it; a click on the picture of one
        // with a click handler (`PictureInset::click`) runs the handler.
        let point = self.to_egui(at);
        if self.last_insets.iter().rev().any(|i| i.frame.contains(point)) {
            return;
        }
        let projector = Projector::new(&content.uniforms.camera);
        investigation::current().click(&shown, &mut self.state, content, &renderer.statics, &projector, pixel);
    }

    fn status(&self) -> String {
        use crate::i18n::{args, tr, tr_with};
        let mode = if let (true, Some(tl)) = (self.timeline_mode, &self.timeline) {
            let clock = |frame: u64| {
                let s = frame / tl.fps as u64;
                format!("{}:{:02}", s / 60, s % 60)
            };
            let chapter = tl.chapter_at(self.timeline_frame);
            let what = if self.playing {
                tr("viewer.playing")
            } else {
                tr("viewer.paused_hint")
            };
            let audio = if self.audio.is_some() { "" } else { tr("viewer.no_narration_track") };
            format!("{} / {} · {} · {what}{audio}", clock(self.timeline_frame), clock(tl.total_frames), tr_with("viewer.chapter", args!(number = chapter.number)))
        } else {
            format!("{} · t = {:.2} s", if self.playing { tr("viewer.playing") } else { tr("viewer.paused") }, self.demo_time)
        };
        let hint = investigation::current().demo_hint(self.state.demo).unwrap_or_else(|| tr("viewer.drag_hint"));
        format!("{mode} · {hint}")
    }

    pub fn configure(&mut self, size: PhysicalSize<u32>) {
        let (Some(surface), Some(renderer)) = (self.surface.as_mut(), self.renderer.as_mut()) else { return };
        surface.config.width = size.width.max(1);
        surface.config.height = size.height.max(1);
        surface.surface.configure(&renderer.gpu.device, &surface.config);
        renderer.resize(size.width.max(64), size.height.max(64));
        surface.blit_group = blit_group(renderer);
    }

    /// Advance the clocks and render the frame with its chrome into the
    /// renderer's target (and `readback`), applying what its controls did.
    fn render(&mut self, readback: Option<&wgpu::Buffer>) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last_tick).as_secs_f32().min(0.1);
        self.last_tick = now;
        // Live reload: about twice a second, the checkout's WGSL files changed since are read
        // again and every pipeline made from them (a broken edit keeps the last good ones).
        if self.frames % 30 == 0 && self.renderer.is_some() {
            let changed = crate::resources::reload_changed();
            if !changed.is_empty() {
                use crate::i18n::{args, tr_with};
                let files: Vec<String> = changed.iter().map(|id| crate::resources::get(id).source().rsplit('/').next().unwrap_or_default().to_string()).collect();
                self.shader_reload = Some(match self.renderer.as_mut().expect("renderer").reload_shaders() {
                    Ok(()) => (tr_with("graph.reloaded", args!(files = files.join(", "))), false, now),
                    Err(e) => (tr_with("graph.reload_failed", args!(error = e)), true, now),
                });
            }
        }
        if self.playing {
            if self.timeline_mode {
                if let Some(t) = &self.timeline {
                    // The wall clock since the anchor, so the narration stays in
                    // step; the autotest advances one frame per redraw.
                    let frame = if self.autotest.is_some() {
                        self.timeline_frame + 1
                    } else {
                        self.anchored_frame(now)
                    };
                    self.timeline_frame = frame.min(t.total_frames - 1);
                    if self.timeline_frame + 1 >= t.total_frames {
                        // The end: pause on the last frame, or on to the next part.
                        self.stop();
                        if self.options.next && self.autotest.is_none() {
                            self.exit = ViewerExit::Next;
                            self.leaving = true;
                        }
                    }
                }
            } else {
                self.demo_time += dt;
            }
        }
        if let Some(t) = &self.timeline {
            self.timeline_frame = self.timeline_frame.min(t.total_frames - 1);
        }
        let state = self.render_state();
        let state = self.with_intro(state, now);
        // Playing, the caption on screen; paused, the one at or last before
        // the frame, and the one before that.
        let captions = self.timeline.as_ref().filter(|_| self.timeline_mode).map_or((None, None), |t| {
            let (_, beat) = t.beat_at(self.timeline_frame);
            let frame = self.timeline_frame;
            if self.playing {
                (None, beat.captions.iter().find(|c| c.start <= frame && frame < c.end).map(|c| c.text.clone()))
            } else {
                let at = beat.captions.iter().rposition(|c| c.start <= frame);
                (at.and_then(|i| i.checked_sub(1)).map(|i| beat.captions[i].text.clone()), at.map(|i| beat.captions[i].text.clone()))
            }
        });
        let mut chrome = ViewerChrome {
            playing: self.playing,
            timeline: self.timeline.as_ref().map(|t| (self.timeline_frame, t.total_frames)),
            timeline_mode: self.timeline_mode,
            chapters: self.timeline.as_ref().map(|t| t.chapters.iter().map(|c| c.start_frame as f32 / t.total_frames.max(1) as f32).collect()).unwrap_or_default(),
            sections: self.bar_sections.clone(),
            fps: self.timeline.as_ref().map_or(30, |t| t.fps),
            hover: None,
            caption: captions.1,
            previous_caption: captions.0,
            skip: self.skip.filter(|_| self.timeline_mode).and_then(|(s, at)| {
                let left = 1.0 - (now - at).as_secs_f32() / SKIP_NOTICE.as_secs_f32();
                (left > 0.0).then(|| (s, (left * 3.0).min(1.0)))
            }),
            status: self.status(),
            tab: self.tab,
            actions: Vec::new(),
            ui_has_pointer: false,
            widgets: Vec::new(),
            popups: std::mem::take(&mut self.popups),
            narration: self.narration,
            speed: self.speed,
            diagram: None,
            experiment: crate::ui::ExperimentView::default(),
            knowledge: crate::ui::KnowledgeChrome { chapters: self.knowledge_chapters(), focus: self.knowledge_focus.clone(), overlay: self.knowledge_overlay },
            dag_overlay: self.dag_overlay && matches!(self.state.panel, PanelMode::Dag | PanelMode::Resources),
            graph_source: self.graph_source,
            shader_reload: self.shader_reload.as_ref().filter(|(_, failed, at)| *failed || at.elapsed().as_secs_f32() < 4.0).map(|(t, f, _)| (t.clone(), *f)),
        };
        // The experiment: the reference drawn while the live state is (not on a timeline), and timings while its tab shows.
        let experimenting = !self.timeline_mode && (self.options.experiments || self.tab == DockTab::Experiments);
        self.frame_times.push_back(dt as f64 * 1000.0);
        if self.frame_times.len() > 120 {
            self.frame_times.pop_front();
        }
        if let Some(renderer) = self.renderer.as_mut() {
            let compare = self.experiment.reference_state(&state).filter(|_| !self.timeline_mode).map(|reference| crate::renderer::CompareRequest { reference, mode: self.experiment.compare, split: self.experiment.split, gain: self.experiment.gain });
            renderer.set_compare(compare);
            // The performance panel measures as it shows, here or on a timeline.
            renderer.set_timing(experimenting || matches!(state.panel, PanelMode::Performance | PanelMode::Graph | PanelMode::Dag | PanelMode::Resources));
            let times: Vec<f64> = self.frame_times.iter().copied().collect();
            let (mean, _, p95) = crate::metrics::summary(&times);
            chrome.experiment = crate::ui::ExperimentView {
                reference: self.experiment.reference.is_some(),
                compare: self.experiment.compare,
                split: self.experiment.split,
                gain: self.experiment.gain,
                measurement: self.experiment.measurement,
                message: self.experiment.message.clone(),
                presets: self.experiment.presets.clone(),
                timings: renderer.timing.as_ref().map(|t| t.summary()).unwrap_or_default(),
                frame_ms: (mean, p95),
                rebuilding: renderer.rebuilding(),
            };
        }
        // The frame a screenshot is taken of shows no popup.
        chrome.popups.hidden = self.screenshot;
        // Paused on a diagram beat, the diagram stays, in its panel, zoomed and panned by the pointer.
        let paused_diagram = self.paused_diagram();
        let (kind, diagram, sources, chapter, fade, notices) = match (&self.timeline, self.timeline_playing() || self.paused_page()) {
            (Some(t), true) => {
                let (_, beat) = t.beat_at(self.timeline_frame);
                // Playing, a diagram moves from the beat before (or builds up); paused, it is settled.
                let shown = if self.timeline_playing() { t.diagram_at(self.timeline_frame) } else { beat.beat_diagram() };
                let diagram = shown.map(|d| (d.graph.clone(), d.focus.to_vec(), d.scene.cloned(), (d.transition.from.map(<[String]>::to_vec), d.transition.seconds, d.transition.built)));
                let fade = if self.paused_page() { 0.0 } else { beat_fade(t, self.timeline_frame) };
                (beat.kind, diagram, beat.sources.clone(), beat.chapter, fade, beat.notices)
            }
            _ => (SceneType::ThreeD, None, Vec::new(), 0, 0.0, false),
        };
        if paused_diagram {
            // The view is set up afresh for another beat, or another first view.
            let beat = self.timeline.as_ref().map_or(0, |t| t.beat_at(self.timeline_frame).1.start_frame);
            let preset = self.diagram_preset.map_or(0, |(z, c)| (z.to_bits() as u64) << 32 ^ (c.x.to_bits() as u64) << 16 ^ c.y.to_bits() as u64);
            chrome.diagram = Some(crate::ui::DiagramPage { key: beat ^ preset.rotate_left(7), preset: self.diagram_preset });
        }
        let raw_input = egui::RawInput { time: Some(self.frames as f64 / 60.0), events: std::mem::take(&mut self.egui_events), ..Default::default() };
        let renderer = self.renderer.as_mut().expect("renderer");
        renderer.user_layout = self.layout;
        let result = renderer.render_frame(
            FrameRequest { state: &state, scene_type: kind, diagram: diagram.as_ref().map(|(graph, focus, scene, (from, seconds, built))| crate::diagram::BeatDiagram { graph, focus, scene: scene.as_ref(), transition: crate::diagram::Transition { from: from.as_deref(), seconds: *seconds, built: *built } }), beat_sources: &sources, notices, chapter_number: chapter, fade, time: self.demo_time, raw_input, chrome: Chrome::Viewer(&mut chrome), stamp: None, history: true },
            readback,
        );
        self.ui_has_pointer = chrome.ui_has_pointer;
        self.bar_hover = chrome.hover.take();
        let cursor = self.renderer().cursor;
        if cursor != self.cursor_icon {
            self.cursor_icon = cursor;
            if let Some(surface) = &self.surface {
                surface.window.set_cursor(window_cursor(cursor));
            }
        }
        self.dock_widgets = std::mem::take(&mut chrome.widgets);
        self.popups = std::mem::take(&mut chrome.popups);
        for action in self.popups.take_actions() {
            popup::perform(action);
        }
        if result.content.is_some() {
            self.last_content = result.content;
            self.last_insets = result.report.insets;
        }
        for action in chrome.actions {
            self.apply(action);
        }
        Ok(())
    }

    fn redraw(&mut self) -> Result<()> {
        self.render(None)?;
        // Present: blit the RGBA frame to the swapchain.
        let renderer = self.renderer.as_ref().unwrap();
        let surface = self.surface.as_ref().unwrap();
        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let size = surface.window.inner_size();
                self.configure(size);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = renderer.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&surface.blit);
            pass.set_bind_group(0, renderer.frame_bind_group_ref(), &[]);
            pass.set_bind_group(1, &surface.blit_group, &[]);
            pass.draw(0..3, 0..1);
        }
        renderer.gpu.queue.submit([encoder.finish()]);
        if std::mem::take(&mut self.screenshot) {
            let saved = crate::readback::screenshot(&renderer.gpu.device, &renderer.gpu.queue, &frame.texture, &self.options.manifest_dir);
            self.popups.push(Popup::screenshot(&saved));
        }
        surface.window.pre_present_notify();
        renderer.gpu.queue.present(frame);
        self.frames += 1;
        Ok(())
    }
}

/// The window's cursor for one egui asks for.
fn window_cursor(icon: egui::CursorIcon) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as W;
    match icon {
        egui::CursorIcon::ResizeHorizontal | egui::CursorIcon::ResizeColumn => W::EwResize,
        egui::CursorIcon::ResizeVertical | egui::CursorIcon::ResizeRow => W::NsResize,
        egui::CursorIcon::PointingHand => W::Pointer,
        egui::CursorIcon::Text => W::Text,
        egui::CursorIcon::Grab => W::Grab,
        egui::CursorIcon::Grabbing => W::Grabbing,
        _ => W::Default,
    }
}

/// Every beat of a timeline with its chapter, for the bar's hover preview.
fn bar_sections(t: &Timeline) -> Vec<crate::ui::BarSection> {
    let total = t.total_frames.max(1) as f32;
    t.beats
        .iter()
        .map(|b| crate::ui::BarSection {
            start: b.start_frame as f32 / total,
            chapter: b.chapter,
            chapter_title: t.chapters.iter().find(|c| c.number == b.chapter).map(|c| c.title.clone()).unwrap_or_default(),
            headline: b.headline.clone(),
        })
        .collect()
}

/// The camera sway of a state (0 when the investigation has no `cameraDrift`).
fn drift(state: &ShotState) -> f32 {
    state.try_get("cameraDrift").map_or(0.0, |v| v.f32())
}

fn blit_group(renderer: &Renderer) -> wgpu::BindGroup {
    renderer.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blit"),
        layout: &renderer.pipelines.texture_pair_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&renderer.frame_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&renderer.frame_view) },
        ],
    })
}

impl App {
    /// Create the window and the GPU, and start loading the scene (once per run).
    fn open(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() || self.pending.is_some() || self.error.is_some() {
            return;
        }
        let title = format!("Render Lab · {}", investigation::current().title());
        let attributes = Window::default_attributes().with_title(title).with_inner_size(PhysicalSize::new(self.options.width, self.options.height)).with_fullscreen(fullscreen_mode(self.options.fullscreen));
        let result = (|| -> Result<()> {
            let window = Arc::new(event_loop.create_window(attributes)?);
            let instance = gpu::instance(Some(Box::new(event_loop.owned_display_handle())));
            let surface = instance.create_surface(window.clone())?;
            let gpu = Gpu::new(instance, Some(&surface))?;
            let size = window.inner_size();
            let caps = surface.get_capabilities(&gpu.adapter);
            // Prefer a UNORM swapchain: the frame already holds sRGB-encoded bytes.
            let format = caps.formats.iter().copied().find(|f| !f.is_srgb() && matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm)).unwrap_or(caps.formats[0]);
            let mut config = surface.get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1)).ok_or_else(|| anyhow::anyhow!("surface unsupported"))?;
            config.format = format;
            config.present_mode = wgpu::PresentMode::Fifo;
            // So F11 can copy the frame it presents.
            if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
                config.usage |= wgpu::TextureUsages::COPY_SRC;
            }
            surface.configure(&gpu.device, &config);
            eprintln!("GPU: {} · swapchain {:?}", gpu.info.name, format);
            if let Some(a) = &self.audio {
                eprintln!("Narration: {}", a.track().display());
            }
            let scene = investigation::current().demo_scene(self.state.demo);
            let loading = Loading::start(investigation::current(), gpu.clone(), scene, [size.width, size.height]);
            let screen = loading::Screen::new(&gpu.device, format);
            self.pending = Some(Pending { window, surface, config, gpu, loading, screen });
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    /// Draws the loading page, and opens the scene once its renderer is ready.
    fn loading_frame(&mut self) -> Result<()> {
        let pending = self.pending.as_mut().expect("loading");
        if let Some(renderer) = pending.loading.poll() {
            let mut renderer = renderer?;
            let Pending { window, surface, config, .. } = self.pending.take().unwrap();
            renderer.ui_scale = self.options.ui_scale;
            let blit = renderer.pipelines.blit(&renderer.gpu.device, config.format);
            let blit_group = blit_group(&renderer);
            self.surface = Some(Surface { window, surface, config, blit, blit_group });
            self.renderer = Some(renderer);
            // The window may have changed size while it loaded; the clocks start now.
            let size = self.surface.as_ref().unwrap().window.inner_size();
            self.configure(size);
            let now = Instant::now();
            self.last_tick = now;
            self.timeline_anchor = (now, self.timeline_frame);
            if let Some((_, started)) = self.intro.as_mut() {
                *started = now;
            }
            if self.options.autoplay && self.timeline_mode {
                self.play();
            }
            return Ok(());
        }
        let frame = match pending.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                pending.surface.configure(&pending.gpu.device, &pending.config);
                return Ok(());
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let title = investigation::current().title();
        pending.screen.draw(&pending.gpu, &view, [pending.config.width, pending.config.height], self.options.ui_scale, title, &pending.loading.progress());
        pending.window.pre_present_notify();
        pending.gpu.queue.present(frame);
        Ok(())
    }

    /// The window's events while its scene loads.
    fn loading_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.exit = ViewerExit::Closed;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let pending = self.pending.as_mut().unwrap();
                pending.config.width = size.width.max(1);
                pending.config.height = size.height.max(1);
                pending.surface.configure(&pending.gpu.device, &pending.config);
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.loading_frame() {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        // A run on a reused event loop starts here; `resumed` may not follow.
        if cause == StartCause::Init {
            self.open(event_loop);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.open(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.pending.is_some() {
            self.loading_event(event_loop, event);
            return;
        }
        if self.renderer.is_none() {
            return;
        }
        // The autotest drives the input itself; a real pointer over the window would move its sliders.
        let user_input = matches!(event, WindowEvent::CursorMoved { .. } | WindowEvent::CursorLeft { .. } | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::KeyboardInput { .. });
        if self.autotest.is_some() && user_input {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.exit = ViewerExit::Closed;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.configure(size);
                if let Some(t) = self.autotest.as_mut() {
                    t.resized(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor_moved(Vec2::new(position.x as f32, position.y as f32)),
            WindowEvent::CursorLeft { .. } => self.egui_events.push(egui::Event::PointerGone),
            WindowEvent::MouseInput { state, button, .. } => self.mouse_button(button, state == ElementState::Pressed),
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self.renderer().layout().scale;
                let (unit, scroll, steps) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (egui::MouseWheelUnit::Line, egui::vec2(x, y), y),
                    MouseScrollDelta::PixelDelta(PhysicalPosition { x, y }) => (egui::MouseWheelUnit::Point, egui::vec2(x as f32, y as f32) / scale, y as f32 / 40.0),
                };
                self.egui_events.push(egui::Event::MouseWheel { unit, delta: scroll, phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown, modifiers: self.modifiers });
                self.wheel(steps);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let m = modifiers.state();
                self.modifiers = egui::Modifiers { alt: m.alt_key(), ctrl: m.control_key(), shift: m.shift_key(), mac_cmd: false, command: m.control_key() };
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat && event.physical_key == PhysicalKey::Code(KeyCode::F11) => {
                self.screenshot = true;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if self.renderer().egui_ctx.egui_wants_keyboard_input() {
                    // A slider's value is being typed: the keys belong to it.
                    self.egui_key(&event);
                } else if event.state == ElementState::Pressed && !event.repeat {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        self.key(code, Some(event_loop));
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.redraw() {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if self.autotest.is_some() {
                    let mut test = self.autotest.take().unwrap();
                    let done = test.tick(self);
                    self.autotest = Some(test);
                    if done {
                        event_loop.exit();
                    }
                }
                if self.leaving {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.surface {
            s.window.request_redraw();
        }
        if let Some(p) = &self.pending {
            p.window.request_redraw();
        }
    }
}

/// Open the viewer in its own event loop (`render-lab view`).
pub fn run(options: ViewerOptions) -> Result<()> {
    let mut event_loop = EventLoop::new()?;
    run_on(&mut event_loop, options).map(|_| ())
}

/// Run the viewer on an event loop that may be reused afterwards (the
/// homepage's); its window closes when it returns.
/// Returns how the viewer was left, and the narration's volume and the layout it was left with.
pub fn run_on(event_loop: &mut EventLoop<()>, options: ViewerOptions) -> Result<(ViewerExit, Volume, crate::config::ViewerLayout)> {
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(options)?;
    event_loop.run_app_on_demand(&mut app)?;
    app.stop_audio();
    if let Some(e) = app.error.take() {
        return Err(e);
    }
    if let Some(test) = app.autotest.take() {
        if !test.finish()? {
            anyhow::bail!("viewer autotest failed; see {}", test.report().display());
        }
    }
    Ok((app.exit, app.narration, app.layout))
}


/// The viewer offscreen, as a PNG (`home --snapshot --page viewer`): the
/// current investigation's first demonstration with its dock, or, with
/// `options.timeline`, the timeline playing at `options.start_frame` with its
/// bar; `demo` (a key) chooses the demonstration. No window opens and no
/// sound plays.
pub fn snapshot(options: ViewerOptions, demo: Option<&str>, png: &std::path::Path) -> Result<()> {
    snapshot_page(options, demo, None, png)
}

/// A 2D scene a snapshot pauses the timeline on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PausedPage {
    /// The first 2D scene with a diagram, its panel zoomed in (`--page viewer-diagram`).
    Diagram,
    /// The first sources card (`--page viewer-sources`).
    Sources,
}

/// The viewer without a window, on `demo` (its key) or the first
/// demonstration, its scene loaded: for a snapshot.
fn offscreen_app(options: ViewerOptions, demo: Option<&str>) -> Result<(App, Gpu, [u32; 2])> {
    let size = [options.width.max(64), options.height.max(64)];
    let mut app = App::new(ViewerOptions { audio: None, ..options })?;
    app.intro = None;
    if let Some(key) = demo {
        app.state.demo = *investigation::current().demos().iter().find(|d| d.key() == key).ok_or_else(|| anyhow::anyhow!("no demonstration `{key}`"))?;
    }
    let gpu = Gpu::new(gpu::instance(None), None)?;
    let scene = investigation::current().demo_scene(app.state.demo);
    let mut renderer = Loading::start(investigation::current(), gpu.clone(), scene, size).wait()?;
    renderer.ui_scale = app.options.ui_scale;
    app.renderer = Some(renderer);
    Ok((app, gpu, size))
}

/// The viewer offscreen in experiments mode (`home --snapshot … --page
/// viewer-experiments` or `viewer-compare`): the Experiments tab; with
/// `compare`, the demonstration's state set as the reference, then one engine
/// setting (else any ranged value) moved to its other end, compared with a
/// wipe and measured.
pub fn snapshot_experiments(options: ViewerOptions, demo: Option<&str>, compare: bool, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { experiments: true, ..options }, demo)?;
    app.state = app.default_state(app.state.demo);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    let mut changed = None;
    if compare {
        app.apply(ViewerAction::SetReference);
        let entries = crate::values::list();
        let camera = crate::camera::OrbitCamera::schema();
        let ranged = |e: &&crate::values::Entry| e.range.is_some() && e.components.is_empty() && e.choices.is_none() && !camera.iter().any(|c| std::ptr::eq(c, *e));
        let entry = entries.iter().find(|e| e.engine.is_some() && ranged(e)).or_else(|| entries.iter().find(|e| ranged(e) && !e.labels.is_empty()));
        if let Some(e) = entry {
            let (lo, hi) = e.range.expect("ranged");
            let now = app.state.p(e.key);
            let to = if (now - lo).abs() > (now - hi).abs() { lo } else { hi };
            app.apply(ViewerAction::SetParam(e.key, to));
            changed = Some(format!("{} {now} → {to}", e.key));
        }
        app.apply(ViewerAction::SetCompare(crate::experiment::Compare::Wipe));
        // A setting baked into the scene data: its build waited for, so the snapshot shows it.
        let state = app.state.clone();
        app.renderer.as_mut().expect("renderer").prepare_scene_data(&state);
    }
    for i in 0..18 {
        if compare && i == 15 {
            app.apply(ViewerAction::Measure);
        }
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    let measured = app.experiment.measurement.map(|m| format!("; PSNR {:.2} dB, SSIM {:.4}, FLIP {:.4}", m.psnr, m.ssim, m.flip)).unwrap_or_default();
    eprintln!("viewer experiments: {} ({}x{}){}{measured}", png.display(), size[0], size[1], changed.map(|c| format!("; reference compared with {c}")).unwrap_or_default());
    Ok(())
}

/// The viewer offscreen with the performance panel (`home --snapshot X.png
/// --page viewer-performance[:<demo>]`): measured over 60 frames, then the
/// most expensive pass called out, as a story's `highlight` would.
pub fn snapshot_performance(options: ViewerOptions, demo: Option<&str>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    app.state.panel = PanelMode::Performance;
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    for _ in 0..60 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let passes = app.renderer().timing.as_ref().map(|t| t.summary()).unwrap_or_default();
    let top = passes.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|p| p.0.clone());
    if let Some(name) = &top {
        app.state.highlight = vec![name.clone()];
    }
    for _ in 0..4 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    let list: Vec<String> = passes.iter().map(|(n, mean, _)| format!("{n} {mean:.2} ms")).collect();
    eprintln!("viewer performance: {} ({}x{}); {}; called out: {}", png.display(), size[0], size[1], if list.is_empty() { "no GPU timestamps".into() } else { list.join(", ") }, top.unwrap_or_else(|| "none".into()));
    Ok(())
}

/// The viewer offscreen with the free camera on (`home --snapshot … --page
/// viewer-free[:<demo>]`): the scene from beside the main camera, which is
/// drawn with its view cone, and the main camera's own view in a corner.
pub fn snapshot_free(options: ViewerOptions, demo: Option<&str>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    app.apply(ViewerAction::ToggleFreeCamera);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    for _ in 0..8 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    let free = app.render_state().free_camera;
    eprintln!("viewer free camera: {} ({}x{}); {:?}, free camera yaw {:.0} pitch {:.0} distance {:.1}, main view {:?}", png.display(), size[0], size[1], app.state.demo, free.yaw, free.pitch, free.distance, app.renderer().last_main_view);
    Ok(())
}

/// The viewer offscreen on its Code tab (`home --snapshot … --page
/// viewer-code:<listing>[@<tag>]`), showing `listing` (its name) on its
/// demonstration, with the lines of `tag` lit (default: its first tag), in
/// the listing style in use (`--listing-style`).
pub fn snapshot_code(options: ViewerOptions, listing: &str, tag: Option<&str>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let current = investigation::current();
    let found = current.listing(listing).ok_or_else(|| {
        let names: Vec<&str> = current.listings().iter().map(|l| l.name).collect();
        anyhow::anyhow!("no listing `{listing}`; known: {}", names.join(", "))
    })?;
    let (mut app, gpu, size) = offscreen_app(options, found.demo.as_deref())?;
    app.state = app.default_state(app.state.demo);
    app.tab = DockTab::Code;
    app.show_tab();
    app.state.code = Some(found.name.to_string());
    let tag = tag.map(str::to_string).or_else(|| found.lines.iter().flat_map(|l| l.tags.iter()).next().cloned());
    app.state.highlight = tag.iter().cloned().collect();
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    for _ in 0..16 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    eprintln!("viewer code: {} ({}x{}), listing {listing} as {}, lit: {}", png.display(), size[0], size[1], crate::pseudocode::style().key(), tag.unwrap_or_else(|| "nothing".into()));
    Ok(())
}

/// The viewer offscreen on its Prerequisites tab (`home --snapshot … --page
/// viewer-prerequisites[:<demo>]`), the knowledge graph in the panel fitted to
/// the demonstration's chapters; with `focus` (`viewer-knowledge[:<demo>]`), the
/// panel zoomed onto the first topic of the audience's level, selected; with
/// `overlay` (`viewer-knowledge-graph[:<demo>]`), the graph full screen.
pub fn snapshot_prerequisites(options: ViewerOptions, demo: Option<&str>, focus: bool, overlay: bool, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    let graph = crate::knowledge::current().ok_or_else(|| anyhow::anyhow!("{} has no knowledge graph (its package's knowledge.toml)", investigation::current().title()))?;
    app.tab = DockTab::Prerequisites;
    app.show_tab();
    app.knowledge_overlay = overlay;
    if focus {
        let chapters = app.knowledge_chapters();
        let context = graph.context(&chapters);
        let p = graph.audience();
        app.knowledge_focus = p.level.iter().find(|&&i| context[i]).or(p.level.first()).map(|&i| graph.nodes[i].id.clone());
    }
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    for _ in 0..16 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    let chapters = app.knowledge_chapters();
    eprintln!("viewer prerequisites: {} ({}x{}); demonstration {}, chapters {:?}{}", png.display(), size[0], size[1], app.state.demo.key(), chapters, app.knowledge_focus.as_ref().map(|f| format!(", focused on {f}")).unwrap_or_default());
    Ok(())
}

/// The viewer offscreen with its edges dragged by the pointer (`home
/// --snapshot X.png --page viewer-layout`): the panel's edge 200 points left,
/// the dock's edge 120 points up, as a user drags them. The layout is written
/// to X.toml (a settings file), not to the user's settings.
pub fn snapshot_layout(options: ViewerOptions, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let settings = png.with_extension("toml");
    let _ = std::fs::remove_file(&settings);
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { settings: Some(settings.clone()), layout: Default::default(), ..options }, None)?;
    app.state = app.default_state(app.state.demo);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    for _ in 0..3 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let before = app.renderer().frame_layout();
    let press = |app: &mut App, at: egui::Pos2, pressed: bool| app.egui_events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() });
    // A drag from `from` to `to` in steps, a frame each, as egui sees a pointer.
    let drag = |app: &mut App, from: egui::Pos2, to: egui::Pos2| -> Result<()> {
        app.egui_events.push(egui::Event::PointerMoved(from));
        app.render(Some(&buffer))?;
        press(app, from, true);
        app.render(Some(&buffer))?;
        for k in 1..=6 {
            app.egui_events.push(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)));
            app.render(Some(&buffer))?;
        }
        press(app, to, false);
        app.render(Some(&buffer))?;
        Ok(())
    };
    let panel_edge = egui::pos2((before.viewport.max.x + before.panel.min.x) / 2.0, before.content.center().y);
    if before.viewport.max.x + 1.0 < before.panel.min.x {
        drag(&mut app, panel_edge, panel_edge - egui::vec2(200.0, 0.0))?;
    }
    let dock_edge = egui::pos2(before.captions.center().x, (before.content.max.y + before.captions.min.y) / 2.0);
    drag(&mut app, dock_edge, dock_edge - egui::vec2(0.0, 120.0))?;
    for _ in 0..16 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let after = app.renderer().frame_layout();
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    let width = |l: &crate::ui::Layout| l.panel.max.x - l.panel.min.x;
    let dock = |l: &crate::ui::Layout| l.captions.max.y - l.captions.min.y;
    let saved = crate::config::Config::load(&settings).map(|c| format!("{:?}", c.layout)).unwrap_or_else(|e| format!("{e:#}"));
    eprintln!("viewer layout: {} ({}x{}); panel {:.0} → {:.0} points, dock {:.0} → {:.0} points; saved to {}: {saved}", png.display(), size[0], size[1], width(&before), width(&after), dock(&before), dock(&after), settings.display());
    Ok(())
}

/// `snapshot`, or with `paused`, the timeline paused on its first 2D scene of
/// that page (`home --snapshot … --page viewer-diagram` or `viewer-sources`).
pub fn snapshot_page(options: ViewerOptions, demo: Option<&str>, paused: Option<PausedPage>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let playing = options.timeline.is_some() && paused.is_none();
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    if playing {
        app.playing = true;
        // The clock holds the frame: it starts counting an hour from now.
        app.timeline_anchor = (Instant::now() + std::time::Duration::from_secs(3600), app.timeline_frame);
    } else if let Some(page) = paused {
        let wanted = |b: &crate::shot::TimelineBeat| b.kind == SceneType::TwoD && b.shows_diagram() == (page == PausedPage::Diagram);
        let beat = app.timeline.as_ref().and_then(|t| t.beats.iter().find(|b| wanted(b))).ok_or_else(|| anyhow::anyhow!("the timeline has no {page:?} 2D scene"))?;
        app.timeline_frame = beat.start_frame + beat.frames / 2;
        app.timeline_mode = true;
        app.playing = false;
        app.render(None)?;
        if page == PausedPage::Diagram {
            app.diagram_preset = Some((2.5, egui::vec2(0.35, 0.5)));
        }
    } else {
        app.state = app.default_state(app.state.demo);
    }
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    // egui sizes its areas on the first passes, and fades them in over its
    // clock; a still view's history (FrameRequest::history) fills over 16 frames.
    for _ in 0..16 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    eprintln!("viewer: {} ({}x{}, {})", png.display(), size[0], size[1], match (playing, paused) { (true, _) => "the timeline playing", (_, Some(PausedPage::Diagram)) => "paused on a diagram", (_, Some(PausedPage::Sources)) => "paused on a sources card", _ => "the first demonstration" });
    Ok(())
}

/// The graph panel's inspector (`ViewerAction::Inspect`): the resource
/// `<node>.<resource>` in the top-right inset (the image `graph`), its node
/// called out; none hides it.
pub fn inspect(state: &mut ShotState, key: Option<String>) {
    use crate::utility::inset::{Corner, ImageSource, StoryImage};
    state.images.retain(|i| i.name != INSPECTOR);
    state.highlight.clear();
    let Some(key) = key else { return };
    if let Some((node, _)) = key.rsplit_once('.') {
        state.highlight = vec![node.to_string()];
    }
    state.images.push(StoryImage { name: INSPECTOR.to_string(), source: ImageSource::Graph { key, show: None, params: Default::default() }, title: None, corner: Corner::TopRight, alpha: 1.0, click: None, mark: [0.0; 4], mark_alpha: 0.0, mark_text: None });
}

/// The name of the graph panel's inspector image.
pub const INSPECTOR: &str = "graph";

/// How a graph snapshot shows the frame graph: the grid in the panel, the
/// DAG in the panel, or the DAG full screen; the DAG's called-out nodes.
#[derive(Clone, Debug, Default)]
pub struct GraphSnapshot {
    pub dag: bool,
    /// The resources' cards instead of the DAG.
    pub resources: bool,
    /// The selected pass's WGSL beside the full-screen DAG (`viewer-source:<node>`).
    pub source: bool,
    pub full_screen: bool,
    pub highlight: Vec<String>,
}

/// The viewer offscreen with the frame graph in the panel (`home --snapshot …
/// --page viewer-graph[:<demo>]`, or `viewer-inspect:<node>.<resource>[/<show>]`;
/// `viewer-dag[:<demo>][@<node>,…]` and `viewer-dag-full[:<demo>][@<node>,…]`: the DAG):
/// the inspector showing `inspect`, or the first resource the most expensive
/// pass writes.
pub fn snapshot_graph(options: ViewerOptions, demo: Option<&str>, inspect_key: Option<&str>, how: GraphSnapshot, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    app.state.panel = if how.resources { PanelMode::Resources } else if how.dag { PanelMode::Dag } else { PanelMode::Graph };
    app.dag_overlay = how.full_screen;
    app.graph_source = how.source;
    app.tab = DockTab::Graph;
    let graph = crate::graph::FrameGraph::current(&[]);
    if let Some(unknown) = how.highlight.iter().find(|h| graph.node(h).is_none()) {
        anyhow::bail!("no node `{unknown}` (nodes: {})", graph.nodes.iter().map(|n| n.name).collect::<Vec<_>>().join(", "));
    }
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    for _ in 0..30 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    // `?name=v,v&…`: the inspector's parameters (`graph::INSPECT_PARAMS`).
    let (inspect_key, query) = match inspect_key.and_then(|k| k.split_once('?')) {
        Some((k, q)) => (Some(k), q),
        None => (inspect_key, ""),
    };
    let params = crate::graph::parse_params(query).map_err(|e| anyhow::anyhow!(e))?;
    let (key, show) = match inspect_key {
        Some(k) => match k.split_once('/') {
            Some((key, show)) => (key.to_string(), Some(show.to_string())),
            None => (k.to_string(), None),
        },
        None => {
            // The most expensive live pass that writes something of its own.
            let passes = app.renderer().timing.as_ref().map(|t| t.summary()).unwrap_or_default();
            let cost = |n: &crate::graph::Node| passes.iter().find(|p| p.0 == n.timing).map_or(0.0, |p| p.1);
            let node = graph.order.iter().map(|&i| &graph.nodes[i]).filter(|n| !n.writes.is_empty() && n.writes[0] != "frame").max_by(|a, b| cost(a).total_cmp(&cost(b))).ok_or_else(|| anyhow::anyhow!("no node writes anything"))?;
            (format!("{}.{}", node.name, node.writes[0]), None)
        }
    };
    graph.inspected(&key).map_err(|e| anyhow::anyhow!(e))?;
    if let Some(show) = &show {
        crate::graph::Show::from_key(show).map_err(|e| anyhow::anyhow!(e))?;
    }
    inspect(&mut app.state, Some(key.clone()));
    // The page's call-outs over the inspector's.
    if !how.highlight.is_empty() {
        app.state.highlight = how.highlight.clone();
    }
    if let Some(crate::utility::inset::ImageSource::Graph { show: s, params: p, .. }) = app.state.images.iter_mut().find(|i| i.name == INSPECTOR).map(|i| &mut i.source) {
        *s = show;
        *p = params;
    }
    for _ in 0..4 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    eprintln!("viewer graph: {} ({}x{}); {} nodes, {} resources; inspecting {key}", png.display(), size[0], size[1], graph.nodes.len(), graph.resources.len());
    Ok(())
}

