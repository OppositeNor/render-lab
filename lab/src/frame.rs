//! What one frame shows, as the renderer consumes it: the GPU records, the
//! frame uniforms, overlay instances, 3D labels, the record for the struct
//! panel, its notes, and the insets; plus `Statics`, the data prepared once
//! per run. An investigation's `build_frame` fills a `FrameContent` from a
//! shot state (src/investigation.rs).

use crate::investigation;
use crate::layout::{self, Declaration, FrameUniforms, ObjectData, ShaderStruct};
use crate::palette::to_f32;
use crate::scene::{self, Meshes, Scene, SceneId};
use crate::shot::{DemoId, ShotState};
use crate::utility::inset::PictureInset;
use bytemuck::{Pod, Zeroable};
use egui::Color32;
use glam::Vec3;
use std::collections::BTreeMap;

// Overlay instance records (vertex buffers, instance step). Layouts match
// the @location attributes in packages/render_lab/shaders/overlay.wgsl.

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct SegmentInstance {
    pub p0: [f32; 3],
    pub width: f32,
    pub p1: [f32; 3],
    pub flags: u32,
    pub color: [f32; 4],
    pub rid: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct BoxInstance {
    pub rid: [u32; 4],
    pub color: [f32; 4],
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct PointInstance {
    pub position: [f32; 3],
    pub radius: f32,
    pub color: [f32; 4],
    pub rid: [u32; 4],
}

/// A solid box: `center`, and its three half-extent vectors `u`, `v`, `w`
/// (any orientation; `.w` of each unused), lit flat with darkened edges.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CuboidInstance {
    pub center: [f32; 3],
    pub pad: f32,
    pub u: [f32; 4],
    pub v: [f32; 4],
    pub w: [f32; 4],
    pub color: [f32; 4],
}

pub const FLAG_ARROW: u32 = 1;
pub const FLAG_DASHED: u32 = 2;
// Segment and box kinds (overlay.wgsl): a segment as given, an object's
// local axis, and an object's oriented or world-aligned bounds.
pub const SEG_EXPLICIT: u32 = 0;
pub const SEG_OBJECT_AXIS: u32 = 1;
pub const BOX_OBJECT_OBB: u32 = 0;
pub const BOX_OBJECT_AABB: u32 = 1;

#[derive(Clone, Default)]
pub struct DrawLists {
    pub segments: Vec<SegmentInstance>,
    pub boxes: Vec<BoxInstance>,
    pub points: Vec<PointInstance>,
    /// Solid boxes (columns of a field, voxels, bars), drawn first and writing
    /// depth, so they hide each other and the lines behind them.
    pub cuboids: Vec<CuboidInstance>,
    /// Objects drawn as translucent shells.
    pub ghosts: Vec<u32>,
    /// Text pinned at a point (a chart's axis names and marks): drawn exactly there, unlike `labels`, which are
    /// placed beside their anchors with a leader line.
    pub texts: Vec<PinnedText>,
}

/// Text drawn at a point of the scene (`DrawLists::text`).
#[derive(Clone, Debug)]
pub struct PinnedText {
    pub at: Vec3,
    pub text: String,
    pub color: Color32,
    /// Which point of the text's box sits at `at`: 0 to 1 across (left to right) and down (top to bottom).
    pub align: [f32; 2],
}

impl DrawLists {
    pub fn segment(&mut self, p0: Vec3, p1: Vec3, color: Color32, alpha: f32, width: f32, flags: u32) {
        if alpha <= 0.0 {
            return;
        }
        self.segments.push(SegmentInstance { p0: p0.to_array(), width, p1: p1.to_array(), flags, color: to_f32(color, alpha), rid: [SEG_EXPLICIT, 0, 0, 0] });
    }

    pub fn point(&mut self, p: Vec3, radius: f32, color: Color32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        self.points.push(PointInstance { position: p.to_array(), radius, color: to_f32(color, alpha), rid: [0; 4] });
    }

    /// Text at `at`, the point `align` of its box (0 to 1 across and down: `[0.5, 1.0]` centres it above `at`).
    pub fn text(&mut self, at: Vec3, text: impl Into<String>, color: Color32, align: [f32; 2]) {
        self.texts.push(PinnedText { at, text: text.into(), color, align });
    }

    /// A solid box at `center` with half-extent vectors `axes` (lit flat, its edges darkened).
    pub fn cuboid(&mut self, center: Vec3, axes: [Vec3; 3], color: Color32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        let [u, v, w] = axes.map(|a| a.extend(0.0).to_array());
        self.cuboids.push(CuboidInstance { center: center.to_array(), pad: 0.0, u, v, w, color: to_f32(color, alpha) });
    }

    pub fn polyline(&mut self, points: &[Vec3], color: Color32, alpha: f32, width: f32) {
        for w in points.windows(2) {
            self.segment(w[0], w[1], color, alpha, width, 0);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Label {
    pub anchor: Vec3,
    pub text: String,
    pub color: Color32,
}

#[derive(Clone, Debug)]
pub struct PanelRecord {
    pub struct_name: &'static str,
    /// e.g. `objects[6] · rotated box`
    pub title: String,
    pub bytes: Vec<u8>,
    /// Buffer and element index the bytes came from (for verification).
    pub buffer: &'static str,
    pub index: usize,
}

/// Values shown in the struct panel's place when a frame has no panel
/// record: a title and subtitle, then one row per value (name, value, colour).
#[derive(Clone, Debug, Default)]
pub struct Readout {
    pub title: String,
    pub subtitle: String,
    /// A formula drawn above the rows: a string holding a formula,
    /// `{ FORMULA("LaTeX", display: "yes", colors: "#rrggbb,…") }` (src/rich_text.rs).
    pub formula: Option<String>,
    /// The term lit in it, and how lit it is (0..1), in its tag's colour for that term.
    pub formula_lit: Option<(usize, f32)>,
    pub rows: Vec<(String, String, Color32)>,
}

/// Vectors of `FrameContent::shading` (an investigation's `SceneShading` reads them).
pub const SHADING_UNIFORMS: usize = 32;
/// The most vectors `FrameContent::shading_frame` may hold.
pub const SHADING_FRAME_CAPACITY: usize = 4096;

#[derive(Clone)]
pub struct FrameContent {
    pub objects: Vec<ObjectData>,
    pub uniforms: FrameUniforms,
    pub draw: DrawLists,
    pub labels: Vec<Label>,
    pub panel_record: Option<PanelRecord>,
    /// Without a panel record, what the struct panel shows instead.
    pub readout: Option<Readout>,
    pub notes: Vec<(String, Color32)>,
    /// Pictures in the viewport's corners (packages/render_lab/src/inset.rs).
    pub insets: Vec<PictureInset>,
    /// The images it publishes for its story (`Investigation::images`), by
    /// key: drawn only while the story shows one (`ShotState::shows_image`).
    pub images: Vec<(String, PictureInset)>,
    /// The points it publishes for its story's arrows and labels
    /// (`Investigation::annotations`), by key.
    pub points: Vec<(String, Vec3)>,
    /// The arrow sets it publishes, by key: computed only while the story
    /// shows one (`ShotState::shows_arrows`).
    pub arrow_sets: Vec<(String, Vec<crate::utility::annotations::Arrow>)>,
    /// The uniforms of the investigation's own scene shading, if it has one.
    pub shading: [[f32; 4]; SHADING_UNIFORMS],
    /// Per-frame data of that shading too large for uniforms (lights, decals,
    /// probes), at most SHADING_FRAME_CAPACITY vectors.
    pub shading_frame: Vec<[f32; 4]>,
    /// The GPU reads it asks for (src/fetch.rs), copied after the frame's passes.
    pub fetches: Vec<crate::fetch::Request>,
    /// What `build_frame` leaves for the package's script natives in this frame (lab/src/script/runtime.rs,
    /// `frame_data`): values it computed that an overlay draws from, so a native need not compute them again.
    pub script: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    /// Where the scripts' overlays go in what `build_frame` drew (`mark`): labels are placed in their order, so an
    /// overlay keeps the place its drawing had. None: after everything.
    pub overlay_at: Option<DrawMark>,
}

/// A place in a frame's draw lists and labels (`FrameContent::mark`).
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawMark {
    pub segments: usize,
    pub points: usize,
    pub labels: usize,
}

impl FrameContent {
    /// Ask for `region` of `source` to be read back after this frame's passes
    /// (src/fetch.rs), its result named `key`, tagged with the frame's inputs;
    /// and return the latest result of `key`, of this frame or one before it.
    pub fn fetch(&mut self, key: &str, source: crate::fetch::Source, region: crate::fetch::Region) -> Option<std::sync::Arc<crate::fetch::Fetched>> {
        self.fetch_tagged(key, source, region, None)
    }

    /// `fetch` with a tag of the investigation's own (what the result depends on), None for the frame's inputs.
    pub fn fetch_tagged(&mut self, key: &str, source: crate::fetch::Source, region: crate::fetch::Region, tag: Option<u64>) -> Option<std::sync::Arc<crate::fetch::Fetched>> {
        self.fetches.push(crate::fetch::Request { key: key.to_string(), source, region, tag });
        crate::fetch::get(key)
    }

    /// Where its draw lists and labels end now: what `overlay_at` takes.
    pub fn mark(&self) -> DrawMark {
        DrawMark { segments: self.draw.segments.len(), points: self.draw.points.len(), labels: self.labels.len() }
    }

    /// A frame of `objects` under `uniforms` with nothing else: no overlays,
    /// labels, panel record, or insets.
    pub fn new(objects: Vec<ObjectData>, uniforms: FrameUniforms) -> FrameContent {
        FrameContent {
            objects,
            uniforms,
            draw: DrawLists::default(),
            labels: Vec::new(),
            panel_record: None,
            readout: None,
            notes: Vec::new(),
            insets: Vec::new(),
            images: Vec::new(),
            points: Vec::new(),
            arrow_sets: Vec::new(),
            fetches: Vec::new(),
            script: None,
            overlay_at: None,
            shading: Default::default(),
            shading_frame: Vec::new(),
        }
    }
}

/// Data prepared once for one scene, which never changes during a run.
pub struct Statics {
    pub scene: Scene,
    pub meshes: Meshes,
    pub layouts: BTreeMap<String, ShaderStruct>,
    pub declarations: BTreeMap<String, Declaration>,
}

impl Statics {
    /// The scene, its raster meshes, and the record layouts.
    pub fn prepare(id: SceneId) -> anyhow::Result<Statics> {
        let scene = Scene::build(id);
        let meshes = scene::build_meshes(&scene);
        let layouts = layout::parse_layouts(&crate::shaders::layout_json()?)?;
        let declarations = layout::parse_declarations(crate::shaders::types_source());
        Ok(Statics {
            scene,
            meshes,
            layouts,
            declarations,
        })
    }

    /// The current investigation's starting state, completed from the
    /// prepared data (of whichever scene these are).
    pub fn default_state(&self, demo: impl Into<DemoId>) -> ShotState {
        investigation::current().default_state(demo.into(), self)
    }
}
