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
use crate::view::ViewId;
use crate::ui::{Chrome, DockTab, ViewerAction, ViewerChrome};
use crate::values::Reflect;
use crate::volume::Volume;
use anyhow::Result;
use glam::Vec2;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

mod playback;
mod actions;
mod input;
mod frame;
mod events;
mod snapshot;
use frame::*;
pub use snapshot::*;

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

use crate::window::fullscreen_mode;
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
    /// The view pressed on (src/view.rs): the drag moves its camera, the click goes to it.
    view: ViewId,
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
    /// The main view's last content when it was not the one on screen (the free camera on).
    last_main_content: Option<FrameContent>,
    /// The view the dock and the keyboard act on (src/view.rs): a click on a view focuses it.
    pub focus: ViewId,
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
    /// The window's cursor as last set (egui's request, src/renderer/ `cursor`).
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
            last_main_content: None,
            focus: ViewId::MAIN,
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
                    crate::script::runtime::animate(s.demo, self.demo_time, &mut s);
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
    if let Some(test) = app.autotest.take() && !test.finish()? {
        anyhow::bail!("viewer autotest failed; see {}", test.report().display());
    }
    Ok((app.exit, app.narration, app.layout))
}
