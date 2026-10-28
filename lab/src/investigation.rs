//! An investigation: one engine feature the lab explains (each is the code of
//! a package, packages/<name>/src, built as the library the lab loads at run
//! time; src/investigations.rs). The lab's renderer, recorder, viewer,
//! dock, and timeline player are shared; an investigation supplies what is
//! specific to its feature: its demonstrations and their parameters, the
//! frame content each shot shows, the dock's sliders, picking, its source
//! index and pseudo-code, its timelines, its verification suite, and, if it
//! shades the scene its own way, the WGSL that does it (`SceneShading`).
//!
//! One investigation is current at a time (`set`, `current`), the way the
//! locale is (src/i18n.rs): the homepage or the command line chooses it
//! before anything is loaded or rendered. `scoped` makes another current on
//! one thread for a while (the homepage's backdrop).

use crate::camera::Projector;
use crate::controls::Dock;
use crate::demo::{Demos, Tab};
use crate::frame::{FrameContent, Statics};
use crate::pseudocode::Listing;
use crate::resources;
use crate::scene::SceneId;
use crate::shot::{DemoId, ShotState};
use crate::sources::SourceIndex;
#[cfg(commercial)]
use crate::surface_cache::Layer;
use crate::viewer::ViewerTest;
use glam::Vec2;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::RwLock;

/// A game engine the lab investigates; its features are investigations.
pub struct Engine {
    /// Short key, used in `engine/feature` paths.
    pub id: &'static str,
    pub name: &'static str,
    /// One or two sentences for the homepage.
    pub summary: &'static str,
}

/// A narrated timeline of an investigation, playable in the viewer: a demo
/// of its package (src/packages.rs).
pub struct TimelineInfo {
    pub id: String,
    pub title: String,
    /// The edition's name on the homepage's picker (`English`, `中文`).
    pub edition: String,
    /// The timeline JSON in its built package. Images its beats name are
    /// relative to its directory.
    pub path: String,
    /// The narration track in its built package (it may not be built).
    pub narration: String,
}

/// Words the shared chrome uses for the engine.
pub struct Wording {
    /// The engine in running text: "≈ <engine>: <structure>" in the struct panel.
    pub engine_short: &'static str,
    /// The header's tag before the engine's version and commit.
    pub engine_tag: &'static str,
    /// The checkout directory source paths are relative to, with a slash.
    pub checkout_dir: &'static str,
}

/// An investigation's own shading of the scene pass, in place of the lab's
/// (`sceneFS` in packages/render_lab/shaders/scene.wgsl). `module` is a shader its
/// package declares (src/shaders.rs), composed after the lab's
/// `render_lab.shader/scene` (the
/// shared prelude and scene.wgsl, so it may use `SceneVaryings`,
/// `SceneOutput`, and everything in common.wgsl), and it declares bind group 1:
///
/// - binding 0: `var<uniform> gShading: array<vec4<f32>, SHADING_UNIFORMS>`,
///   the frame's `FrameContent::shading`;
/// - binding 1: `var<storage, read> gShadingData: array<vec4<f32>>`, `data()`,
///   computed once per renderer;
/// - binding 2: `var<storage, read> gShadingFrame: array<vec4<f32>>`, the
///   frame's `FrameContent::shading_frame`.
///
/// Its compute entry points in `verify` take the verification buffers as
/// group 2 (`Renderer::run_shading_verify`).
///
/// With no `outputs`, the scene pass writes the lab's HDR target. With
/// `outputs`, it is the first pass of a chain: the fragment shader writes
/// those `targets` (in order), which stay multisampled, then each of `passes`
/// runs as a compute pass, and the last writes `"hdr"`, which the frame
/// shows. A pass binds group 2 as described on `ShadingPass`.
///
/// With a `prepass` fragment entry, the chain starts with a depth pre-pass
/// (that entry may only write `@builtin(sample_mask)`), then runs `early`
/// compute passes, then the scene pass tests against that depth (LessEqual,
/// no depth writes). The scene pass's fragment shader may read `scene_reads`
/// from group 2, bound as for a pass.
pub struct SceneShading {
    /// The full key of its shader module (`[[shader]]` in its package).
    pub module: &'static str,
    /// The fragment entry point of the scene pass.
    pub fragment: &'static str,
    pub data: fn() -> Vec<[f32; 4]>,
    pub verify: &'static [&'static str],
    /// The chain's colour targets, besides `"hdr"` and `"depth"`.
    pub targets: &'static [ShadingTarget],
    /// The scene pass's colour outputs, by target name; empty for `"hdr"` alone.
    pub outputs: &'static [&'static str],
    pub passes: &'static [ShadingPass],
    pub prepass: Option<&'static str>,
    pub early: &'static [ShadingPass],
    /// The chain's storage buffers, besides its targets.
    pub buffers: &'static [ShadingBuffer],
    pub scene_reads: &'static [&'static str],
}

/// A storage buffer of an investigation's pass chain: `words_per_tile` u32
/// for each `tile` x `tile` block of the viewport.
pub struct ShadingBuffer {
    pub name: &'static str,
    pub words_per_tile: u32,
    pub tile: u32,
}

/// A colour target of an investigation's pass chain.
pub struct ShadingTarget {
    pub name: &'static str,
    pub format: wgpu::TextureFormat,
    /// The viewport's size divided by this (1 full resolution, 2 half).
    pub divisor: u32,
}

/// A compute pass of an investigation's pass chain. Group 2 binds each
/// target at a fixed place, so one module holds every pass: target `i` of
/// `SceneShading::targets` at binding `2 i` when read (`texture_2d<f32>`, or
/// `texture_multisampled_2d<f32>` for the scene pass's outputs) and
/// `2 i + 1` when written (a write-only storage texture). With `N` targets,
/// `"depth"` (the scene pass's multisampled depth,
/// `texture_depth_multisampled_2d`) is binding `2 N`, and `"hdr"` (the lab's
/// HDR target, `rgba16float`) binding `2 N + 1`; buffer `k` of
/// `SceneShading::buffers` is binding `2 N + 2 + k`
/// (`var<storage, read_write> array<u32>`, whether read or written). The
/// dispatch covers the first written target, or the viewport when that is a
/// buffer, in workgroups of `workgroup`.
pub struct ShadingPass {
    pub entry: &'static str,
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
    pub workgroup: [u32; 2],
}

pub struct SelftestOptions {
    pub report: PathBuf,
    /// Also check labels and text for every scene beat of this timeline.
    pub timeline: Option<PathBuf>,
    pub ffmpeg: PathBuf,
    pub work_dir: PathBuf,
}

pub trait Investigation: Sync {
    fn engine(&self) -> &'static Engine;
    /// Short key, used in `engine/feature` paths.
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    /// A paragraph for the homepage.
    fn summary(&self) -> &'static str;
    fn wording(&self) -> &'static Wording;
    /// The footer line, in the current locale.
    fn footer(&self) -> &'static str;
    /// Its narrated timelines: its package's demos.
    fn timelines(&self) -> &'static [TimelineInfo] {
        self.package().map(resources::timelines).unwrap_or(&[])
    }
    fn sources(&self) -> &'static SourceIndex;
    fn listings(&self) -> &'static [Listing];
    /// The package of its content and code (packages/<name>, src/packages.rs), if it
    /// has one.
    fn package(&self) -> Option<&'static str> {
        None
    }

    fn listing(&self, name: &str) -> Option<&'static Listing> {
        self.listings().iter().find(|l| l.name == name)
    }

    /// The listing the viewer's Code tab opens for a demonstration.
    fn listing_for_demo(&self, demo: DemoId) -> Option<&'static Listing> {
        self.listings().iter().find(|l| l.demo.as_deref() == Some(demo.key()))
    }

    /// Its demonstrations as data (src/demo.rs), when it describes them
    /// in files; the methods below then read them. An investigation without
    /// them implements `demos`, `demo_title`, `demo_tab`, `param_default`,
    /// and `shot_state` itself.
    fn demo_specs(&self) -> Option<&'static Demos> {
        None
    }
    /// The text of a key its data names, in the current locale (its own table).
    fn text(&self, key: &'static str) -> &'static str {
        crate::i18n::tr_key(key)
    }

    /// Demonstrations in the order of the viewer's keys (1–9, 0, −, =).
    fn demos(&self) -> &'static [DemoId] {
        specs(self).ids()
    }
    fn demo_title(&self, demo: DemoId) -> &'static str {
        self.text(specs(self).spec(demo).title)
    }
    /// The demonstration's short name on the viewer's tab row.
    fn demo_tab(&self, demo: DemoId) -> &'static str {
        match specs(self).spec(demo).tab {
            Tab::Text(key) => self.text(key),
            Tab::Plain(name) => name,
        }
    }
    /// The lab scene a demonstration shows (src/scene.rs); the renderer
    /// switches to it for the demonstration's frames.
    fn demo_scene(&self, demo: DemoId) -> SceneId {
        self.demo_specs().map_or(SceneId::STUDY_ROOM, |d| d.spec(demo).scene)
    }
    /// The viewer's status-line hint, if the demonstration has its own.
    fn demo_hint(&self, demo: DemoId) -> Option<&'static str> {
        self.demo_specs().and_then(|d| d.spec(demo).hint).map(|key| self.text(key))
    }
    /// Whether "Reset parameters" also resets the camera (its dock shows the camera).
    fn resets_camera(&self, demo: DemoId) -> bool {
        self.demo_specs().is_some_and(|d| d.spec(demo).resets_camera)
    }

    /// The default of every shot parameter; None for an unknown name. The lab
    /// reads `cameraDrift` (the camera sway) when an investigation has it.
    fn param_default(&self, name: &str) -> Option<f32> {
        specs(self).param_default(name)
    }
    /// The demonstration's starting state.
    fn shot_state(&self, demo: DemoId) -> ShotState {
        specs(self).shot_state(demo)
    }
    /// The starting state, completed from the prepared data.
    fn default_state(&self, demo: DemoId, _statics: &Statics) -> ShotState {
        self.shot_state(demo)
    }
    /// The scene behind the homepage `t` seconds after it opened, drawn
    /// full-screen under frosted glass; None for a plain background.
    fn backdrop(&self, _t: f32) -> Option<ShotState> {
        None
    }
    /// The viewer's Play animation: parameter values at `t` seconds.
    fn animation(&self, _demo: DemoId, _t: f32) -> Vec<(&'static str, f32)> {
        Vec::new()
    }
    /// Parameters the viewer sets together with `name`.
    fn linked_params(&self, _name: &str, _value: f32) -> Vec<(&'static str, f32)> {
        Vec::new()
    }

    /// Everything one shot shows. A pure function of its arguments, so the
    /// recorder stays deterministic.
    fn build_frame(&self, state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent;
    /// A setting of this shot that the prepared data is built with; the
    /// renderer rebuilds that data when it changes. None keeps it as built.
    #[cfg(commercial)]
    fn prepared_setting(&self, _state: &ShotState) -> Option<f32> {
        None
    }
    /// A status line under the Surface Cache inset's title.
    #[cfg(commercial)]
    fn cache_status(&self, _state: &ShotState, _pages: usize, _layer: Layer) -> Option<String> {
        None
    }
    /// The scene pass's shading, when the investigation brings its own.
    fn scene_shading(&self) -> Option<&'static SceneShading> {
        None
    }
    /// The dock's Sliders tab for the current demonstration.
    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui);
    /// A click in the viewport at `pixel` on the frame of `shown` (the state on
    /// screen); changes `state` and returns true if it selected something.
    fn click(&self, shown: &ShotState, state: &mut ShotState, content: &FrameContent, statics: &Statics, projector: &Projector, pixel: Vec2) -> bool;

    /// The scripted test of the viewer's controls (`viewer-autotest`).
    fn viewer_test(&self, report: PathBuf) -> Option<Box<dyn ViewerTest>> {
        let _ = report;
        None
    }
    /// The verification suite (`selftest`); Ok(false) when a check failed.
    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool>;
}

/// An investigation's demonstration files, for the methods that read them.
fn specs<I: Investigation + ?Sized>(investigation: &I) -> &'static Demos {
    investigation.demo_specs().unwrap_or_else(|| panic!("{} has no demonstration files: it implements demos, demo_title, demo_tab, param_default, and shot_state", investigation.title()))
}

impl dyn Investigation {
    /// `engine/feature`, as the command line names it.
    pub fn path(&self) -> String {
        format!("{}/{}", self.engine().id, self.id())
    }

    /// The scenes its demonstrations show, in the order they first appear:
    /// the viewer's scene tabs.
    pub fn scenes(&self) -> Vec<SceneId> {
        let mut scenes = Vec::new();
        for demo in self.demos() {
            let scene = self.demo_scene(*demo);
            if !scenes.contains(&scene) {
                scenes.push(scene);
            }
        }
        scenes
    }

    /// Its demonstrations of `scene`, in order.
    pub fn scene_demos(&self, scene: SceneId) -> Vec<DemoId> {
        self.demos().iter().copied().filter(|d| self.demo_scene(*d) == scene).collect()
    }

    /// The scene of its first demonstration, which the renderer prepares first.
    pub fn first_scene(&self) -> SceneId {
        self.demo_scene(self.demos()[0])
    }
}

static CURRENT: RwLock<Option<&'static dyn Investigation>> = RwLock::new(None);

thread_local! {
    /// The investigation `scoped` makes current on this thread.
    static SCOPED: Cell<Option<&'static dyn Investigation>> = const { Cell::new(None) };
}

pub fn set(investigation: &'static dyn Investigation) {
    *CURRENT.write().unwrap() = Some(investigation);
}

/// The investigation chosen on the homepage or the command line, or the one
/// `scoped` made current on this thread.
pub fn current() -> &'static dyn Investigation {
    SCOPED.get().unwrap_or_else(|| CURRENT.read().unwrap().expect("no investigation selected"))
}

/// Runs `f` with `investigation` current on this thread alone: a scene of
/// one investigation (the homepage's backdrop) prepared or drawn while
/// another is chosen.
pub fn scoped<R>(investigation: &'static dyn Investigation, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<&'static dyn Investigation>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED.set(self.0);
        }
    }
    let _restore = Restore(SCOPED.replace(Some(investigation)));
    f()
}
