//! Interactive viewer. It renders through the same Renderer and RGBA target
//! as the recorder, then blits the target to the window.
//!
//! Controls: left-drag orbit, right/middle-drag pan, wheel zoom, click pick,
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
//! touching the camera stops it there. Playing runs it in real time with the narration (FFplay) and
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
use crate::record::{beat_fade, ImageCache};
use crate::renderer::{FrameRequest, Renderer};
use crate::shot::{self, BeatKind, DemoId, PanelMode, ShotState, Timeline};
use crate::ui::{Chrome, DiagramView, DockTab, ViewerAction, ViewerChrome};
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

#[cfg(commercial)]
pub use crate::text::DEFAULT_UI_SCALE;

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

#[derive(Clone, Copy)]
struct Drag {
    button: MouseButton,
    last: Vec2,
    moved: f32,
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
    audio: Option<crate::audio::AudioPlayer>,
    /// The narration's volume; the viewer hands it back when it closes.
    pub narration: Volume,
    images: Option<ImageCache>,
    /// Paused on a diagram beat: its panel's zoom and pan, the beat it is for
    /// (its first frame), the fitted picture's rectangle (egui points), and
    /// the last click (a second one soon after resets the view).
    diagram: DiagramView,
    diagram_beat: Option<u64>,
    diagram_fit: Option<egui::Rect>,
    last_click: Option<Instant>,
    cursor: Vec2,
    drag: Option<Drag>,
    pub ui_has_pointer: bool,
    egui_events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    tab: DockTab,
    /// The dock's slider rectangles from the last frame.
    pub dock_widgets: Vec<(String, egui::Rect)>,
    pub last_content: Option<FrameContent>,
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
}

impl App {
    fn new(options: ViewerOptions) -> Result<App> {
        let timeline = options.timeline.as_ref().map(|p| Timeline::load(p)).transpose()?;
        crate::i18n::set(options.locale.or(timeline.as_ref().map(|t| t.locale)).unwrap_or_default());
        let images = options.timeline.as_ref().map(|p| ImageCache::new(p.parent().unwrap_or(std::path::Path::new("."))));
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
        Ok(App {
            options,
            surface: None,
            renderer: None,
            pending: None,
            state,
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
            audio,
            narration,
            images,
            diagram: DiagramView::default(),
            diagram_beat: None,
            diagram_fit: None,
            last_click: None,
            cursor: Vec2::ZERO,
            drag: None,
            ui_has_pointer: false,
            egui_events: Vec::new(),
            modifiers: egui::Modifiers::default(),
            tab: DockTab::default(),
            dock_widgets: Vec::new(),
            last_content: None,
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
        })
    }

    pub fn renderer(&self) -> &Renderer {
        self.renderer.as_ref().expect("renderer")
    }

    /// Whether the timeline is playing (and owns the shot).
    pub fn timeline_playing(&self) -> bool {
        self.timeline_mode && self.playing && self.timeline.is_some()
    }

    /// Paused on a diagram beat of the timeline: the viewport is the diagram's panel.
    pub fn paused_diagram(&self) -> bool {
        match (&self.timeline, self.timeline_mode && !self.playing) {
            (Some(t), true) => {
                let (_, beat) = t.beat_at(self.timeline_frame);
                beat.kind == BeatKind::Mermaid && beat.image.is_some()
            }
            _ => false,
        }
    }

    /// A pointer position in window pixels, in egui points.
    fn points(&self, p: Vec2) -> egui::Vec2 {
        let scale = self.renderer().layout().scale;
        egui::vec2(p.x / scale, p.y / scale)
    }

    /// The state that is rendered: the playing timeline's frame, or the
    /// interactive state (a paused timeline's shot, or a demonstration with
    /// its animation applied while it plays).
    pub fn render_state(&self) -> ShotState {
        if self.timeline_playing() {
            if let Some(t) = &self.timeline {
                return t.state_at(self.timeline_frame);
            }
        }
        let mut s = self.state.clone();
        if self.playing && !self.timeline_mode {
            for (k, v) in investigation::current().animation(s.demo, self.demo_time) {
                s.set(k, v);
            }
            s.camera = shot::camera_drift(s.camera, self.drift_from, self.demo_time, drift(&s));
        }
        s
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
        self.state = self.default_state(demo);
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
        from + (now.saturating_duration_since(at).as_secs_f64() * self.fps()) as u64
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
        let started = self.audio.as_mut().is_some_and(|a| a.play_from(seconds));
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
            ViewerAction::Reset => {
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
            _ => {}
        }
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
            KeyCode::Escape => {
                if let Some(el) = event_loop {
                    self.exit = ViewerExit::Back;
                    el.exit();
                }
            }
            _ => {}
        }
    }

    fn viewport_pixel(&self, window_px: Vec2) -> Option<Vec2> {
        self.renderer().frame_layout().target_to_viewport_pixel(window_px.x, window_px.y)
    }

    pub fn viewport_height(&self) -> f32 {
        self.renderer().frame_layout().viewport_size_px()[1] as f32
    }

    pub fn pointer_down(&mut self, button: MouseButton, at: Vec2) {
        if self.ui_has_pointer || self.viewport_pixel(at).is_none() {
            return;
        }
        self.drag = Some(Drag { button, last: at, moved: 0.0 });
    }

    pub fn pointer_move(&mut self, at: Vec2) {
        self.cursor = at;
        let Some(mut drag) = self.drag else { return };
        let delta = at - drag.last;
        drag.moved += delta.length();
        drag.last = at;
        self.drag = Some(drag);
        if self.timeline_playing() {
            return;
        }
        if self.paused_diagram() {
            if let Some(fit) = self.diagram_fit {
                let d = self.points(delta);
                let size = fit.size() * self.diagram.zoom;
                self.diagram.center -= egui::vec2(d.x / size.x, d.y / size.y);
                self.diagram.center = self.diagram.center.clamp(egui::Vec2::ZERO, egui::vec2(1.0, 1.0));
            }
            return;
        }
        self.end_intro();
        let height = self.viewport_height();
        match drag.button {
            MouseButton::Left => self.state.camera.orbit(delta),
            MouseButton::Right | MouseButton::Middle => self.state.camera.pan(delta, height),
            _ => {}
        }
    }

    pub fn pointer_up(&mut self, button: MouseButton, at: Vec2) {
        let Some(drag) = self.drag.take() else { return };
        if drag.button == button && button == MouseButton::Left && drag.moved < 4.0 {
            if self.paused_diagram() {
                // A double click on the diagram fits it to its panel again.
                let now = Instant::now();
                if self.last_click.is_some_and(|t| now.duration_since(t).as_secs_f32() < 0.4) {
                    self.diagram = DiagramView::default();
                }
                self.last_click = Some(now);
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
        if self.paused_diagram() && !self.ui_has_pointer {
            // Zoom about the point under the pointer, which stays where it is.
            if let Some(fit) = self.diagram_fit {
                let p = self.points(self.cursor);
                let offset = egui::pos2(p.x, p.y) - fit.center();
                let before = fit.size() * self.diagram.zoom;
                let under = self.diagram.center + egui::vec2(offset.x / before.x, offset.y / before.y);
                self.diagram.zoom = (self.diagram.zoom * 1.15f32.powf(steps)).clamp(1.0, DiagramView::MAX_ZOOM);
                let after = fit.size() * self.diagram.zoom;
                self.diagram.center = (under - egui::vec2(offset.x / after.x, offset.y / after.y)).clamp(egui::Vec2::ZERO, egui::vec2(1.0, 1.0));
                if self.diagram.zoom <= 1.0 {
                    self.diagram = DiagramView::default();
                }
            }
            return;
        }
        if !self.timeline_playing() && !self.ui_has_pointer {
            self.end_intro();
            self.state.camera.zoom(steps);
        }
    }

    fn click(&mut self, at: Vec2) {
        let Some(pixel) = self.viewport_pixel(at) else { return };
        let Some(content) = &self.last_content else { return };
        let shown = self.render_state();
        let Some(renderer) = &self.renderer else { return };
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
                        // The end: pause on the last frame.
                        self.stop();
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
            diagram: None,
        };
        // The frame a screenshot is taken of shows no popup.
        chrome.popups.hidden = self.screenshot;
        // Paused on a diagram beat, the diagram stays, in its panel, zoomed and panned by the pointer.
        let paused_diagram = self.paused_diagram();
        let (kind, image, sources, chapter, fade) = match (&self.timeline, self.timeline_playing() || paused_diagram) {
            (Some(t), true) => {
                let (_, beat) = t.beat_at(self.timeline_frame);
                let image = match (&beat.kind, &beat.image, self.images.as_mut(), self.renderer.as_ref()) {
                    (BeatKind::Mermaid, Some(p), Some(cache), Some(r)) => Some(cache.get(&r.egui_ctx, p)?),
                    _ => None,
                };
                if paused_diagram && self.diagram_beat != Some(beat.start_frame) {
                    self.diagram = DiagramView::default();
                    self.diagram_beat = Some(beat.start_frame);
                }
                let fade = if paused_diagram { 0.0 } else { beat_fade(t, self.timeline_frame) };
                (beat.kind, image, beat.sources.clone(), beat.chapter, fade)
            }
            _ => (BeatKind::Scene, None, Vec::new(), 0, 0.0),
        };
        if paused_diagram {
            chrome.diagram = Some(self.diagram);
            self.diagram_fit = image.map(|(_, size)| crate::ui::fit_rect(self.renderer().layout().content, size));
        } else {
            self.diagram_fit = None;
        }
        let raw_input = egui::RawInput { time: Some(self.frames as f64 / 60.0), events: std::mem::take(&mut self.egui_events), ..Default::default() };
        let renderer = self.renderer.as_mut().expect("renderer");
        let result = renderer.render_frame(
            FrameRequest { state: &state, beat_kind: kind, image, beat_sources: &sources, chapter_number: chapter, fade, time: self.demo_time, raw_input, chrome: Chrome::Viewer(&mut chrome), stamp: None },
            readback,
        );
        self.ui_has_pointer = chrome.ui_has_pointer;
        self.bar_hover = chrome.hover.take();
        self.dock_widgets = std::mem::take(&mut chrome.widgets);
        self.popups = std::mem::take(&mut chrome.popups);
        for action in self.popups.take_actions() {
            popup::perform(action);
        }
        if result.content.is_some() {
            self.last_content = result.content;
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
/// Returns how the viewer was left, and the narration's volume it was left with.
pub fn run_on(event_loop: &mut EventLoop<()>, options: ViewerOptions) -> Result<(ViewerExit, Volume)> {
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
    Ok((app.exit, app.narration))
}


/// The viewer offscreen, as a PNG (`home --snapshot --page viewer`): the
/// current investigation's first demonstration with its dock, or, with
/// `options.timeline`, the timeline playing at `options.start_frame` with its
/// bar; `demo` (a key) chooses the demonstration. No window opens and no
/// sound plays.
pub fn snapshot(options: ViewerOptions, demo: Option<&str>, png: &std::path::Path) -> Result<()> {
    snapshot_page(options, demo, false, png)
}

/// `snapshot`, or with `diagram`, the timeline paused on its first diagram beat
/// with the diagram panel zoomed in (`home --snapshot … --page viewer-diagram`).
pub fn snapshot_page(options: ViewerOptions, demo: Option<&str>, diagram: bool, png: &std::path::Path) -> Result<()> {
    use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows, write_png};
    let size = [options.width.max(64), options.height.max(64)];
    let playing = options.timeline.is_some() && !diagram;
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
    if playing {
        app.playing = true;
        // The clock holds the frame: it starts counting an hour from now.
        app.timeline_anchor = (Instant::now() + std::time::Duration::from_secs(3600), app.timeline_frame);
    } else if diagram {
        let beat = app.timeline.as_ref().and_then(|t| t.beats.iter().find(|b| b.kind == BeatKind::Mermaid && b.image.is_some())).ok_or_else(|| anyhow::anyhow!("the timeline has no diagram beat"))?;
        app.timeline_frame = beat.start_frame + beat.frames / 2;
        app.timeline_mode = true;
        app.playing = false;
        app.render(None)?;
        app.diagram = DiagramView { zoom: 2.5, center: egui::vec2(0.35, 0.5) };
    } else {
        app.state = app.default_state(app.state.demo);
    }
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    // egui sizes its areas on the first passes, and fades them in over its clock.
    for _ in 0..3 {
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    let padded = padded_row_bytes(size[0]) as u64;
    let rgba = unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &buffer, 0, padded * size[1] as u64), size[0], size[1]);
    write_png(png, size[0], size[1], &rgba)?;
    eprintln!("viewer: {} ({}x{}, {})", png.display(), size[0], size[1], if playing { "the timeline playing" } else if diagram { "paused on a diagram" } else { "the first demonstration" });
    Ok(())
}
