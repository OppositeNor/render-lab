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
// the @location attributes in packages/rlstd/shaders/overlay.wgsl.

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

pub const FLAG_ARROW: u32 = 1;
pub const FLAG_DASHED: u32 = 2;
// Segment and box kinds (overlay.wgsl): a segment as given, an object's
// local axis, and an object's oriented or world-aligned bounds.
pub const SEG_EXPLICIT: u32 = 0;
pub const SEG_OBJECT_AXIS: u32 = 1;
pub const BOX_OBJECT_OBB: u32 = 0;
pub const BOX_OBJECT_AABB: u32 = 1;

#[derive(Default)]
pub struct DrawLists {
    pub segments: Vec<SegmentInstance>,
    pub boxes: Vec<BoxInstance>,
    pub points: Vec<PointInstance>,
    /// Objects drawn as translucent shells.
    pub ghosts: Vec<u32>,
}

impl DrawLists {
    pub fn segment(&mut self, p0: Vec3, p1: Vec3, color: Color32, alpha: f32, width: f32, flags: u32) {
        if alpha <= 0.0 {
            return;
        }
        self.segments.push(SegmentInstance { p0: p0.to_array(), width, p1: p1.to_array(), flags, color: to_f32(color, alpha), rid: [SEG_EXPLICIT, 0, 0, 0] });
    }

    pub fn record_segment(&mut self, kind: u32, index: u32, sub: u32, length: f32, color: [f32; 4], width: f32, flags: u32) {
        if color[3] == 0.0 {
            return;
        }
        self.segments.push(SegmentInstance { p0: [length, 0.0, 0.0], width, p1: [0.0; 3], flags, color, rid: [kind, index, sub, 0] });
    }

    pub fn record_box(&mut self, kind: u32, index: u32, color: Color32, alpha: f32, width: f32, flags: u32) {
        if alpha <= 0.0 {
            return;
        }
        self.boxes.push(BoxInstance { rid: [kind, index, 0, flags], color: to_f32(color, alpha), params: [width, 0.0, 0.0, 0.0] });
    }

    pub fn point(&mut self, p: Vec3, radius: f32, color: Color32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        self.points.push(PointInstance { position: p.to_array(), radius, color: to_f32(color, alpha), rid: [0; 4] });
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
    /// A formula drawn above the rows: a string holding a formula tag,
    /// `{@ formula="LaTeX" display=yes colors="#rrggbb,…"}` (src/rich_text.rs).
    pub formula: Option<String>,
    /// The term lit in it, and how lit it is (0..1), in its tag's colour for that term.
    pub formula_lit: Option<(usize, f32)>,
    pub rows: Vec<(String, String, Color32)>,
}

/// Vectors of `FrameContent::shading` (an investigation's `SceneShading` reads them).
pub const SHADING_UNIFORMS: usize = 32;
/// The most vectors `FrameContent::shading_frame` may hold.
pub const SHADING_FRAME_CAPACITY: usize = 4096;

pub struct FrameContent {
    pub objects: Vec<ObjectData>,
    pub uniforms: FrameUniforms,
    pub draw: DrawLists,
    pub labels: Vec<Label>,
    pub panel_record: Option<PanelRecord>,
    /// Without a panel record, what the struct panel shows instead.
    pub readout: Option<Readout>,
    pub notes: Vec<(String, Color32)>,
    /// Pictures in the viewport's corners (packages/rlstd/src/inset.rs).
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
}

impl FrameContent {
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
