//! Rust mirrors of the WGSL records in packages/rlstd/shaders/types.wgsl, their field
//! metadata, and the comparison against the layouts naga computes for them.
//!
//! Matrices are stored column-major: `[[f32; 4]; 4]` is four columns, which is
//! exactly glam's `Mat4::to_cols_array_2d()` and WGSL's `mat4x4<f32>`. A
//! `mat3x3<f32>` is three 16-byte columns, each with 4 bytes of padding.

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ObjectData {
    pub local_to_world: [[f32; 4]; 4],
    pub world_to_local: [[f32; 4]; 4],
    pub local_center: [f32; 3],
    pub bounding_radius: f32,
    pub local_extent: [f32; 3],
    pub object_index: u32,
    pub world_center: [f32; 3],
    pub shape_kind: u32,
    pub world_extent: [f32; 3],
    pub _pad0: f32,
    pub albedo_roughness: [f32; 4],
    pub emissive: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct VoxelSample {
    pub local_position: [f32; 3],
    pub signed_distance: f32,
    pub voxel: [u32; 3],
    pub encoded: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct BrickEntry {
    pub virtual_brick: [u32; 3],
    pub mip_level: u32,
    pub physical_brick: [u32; 3],
    pub physical_index: u32,
    pub distance_scale_bias: [f32; 2],
    pub voxel_size: f32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CardData {
    pub center: [f32; 3],
    pub object_index: u32,
    /// Columns tangent, bitangent, normal; `w` of each column is padding.
    pub tangent_frame: [[f32; 4]; 3],
    pub half_extents: [f32; 3],
    pub page_index: u32,
    pub size_in_pages: [u32; 2],
    pub res_level: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct PageEntry {
    pub physical_page: [u32; 2],
    pub card_index: u32,
    pub mip_level: u32,
    pub card_uv_rect: [f32; 4],
    pub flags: u32,
    pub _pad0: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct RayDebugData {
    pub origin: [f32; 3],
    pub roughness: f32,
    pub incident_dir: [f32; 3],
    pub hit_distance: f32,
    pub hit_position: [f32; 3],
    pub hit_object: u32,
    pub normal: [f32; 3],
    /// EnvBRDFApprox(F0, roughness, N·V): the specular weight of the composite.
    pub env_brdf: f32,
    pub view_dir: [f32; 3],
    /// GGX alpha = roughness²: the width of the VNDF lobe the rays sample.
    pub ggx_alpha: f32,
    pub reflected_dir: [f32; 3],
    pub reflected_hit_distance: f32,
    pub reflected_hit_position: [f32; 3],
    pub reflected_hit_object: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CameraData {
    pub world_to_view: [[f32; 4]; 4],
    pub view_to_clip: [[f32; 4]; 4],
    pub world_to_clip: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub vertical_fov: f32,
    pub viewport_size: [f32; 2],
    pub near_plane: f32,
    pub far_plane: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ProbeRecord {
    pub position: [f32; 3],
    pub first_sample: u32,
    pub normal: [f32; 3],
    pub sample_count: u32,
    pub screen_pixel: [u32; 2],
    pub history_weight: f32,
    pub frames_accumulated: u32,
    pub irradiance: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct RadianceSample {
    pub direction: [f32; 3],
    pub hit_distance: f32,
    pub radiance: [f32; 3],
    pub hit_object: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct FrameUniforms {
    pub camera: CameraData,
    pub light_center_radiance: [f32; 4],
    pub light_axis_u: [f32; 4],
    pub light_axis_v: [f32; 4],
    pub light_color: [f32; 4],
    pub ambient_color: [f32; 4],
    pub floor_params: [f32; 4],
    pub overlay_params: [f32; 4],
    pub cut_plane: [f32; 4],
    pub deform_params: [f32; 4],
    pub sdf_volume: [f32; 4],
    pub atlas_display: [f32; 4],
    pub counts: [u32; 4],
    pub selection: [u32; 4],
    pub selection2: [u32; 4],
    pub counts2: [u32; 4],
    pub cache_view: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MeshNode {
    pub bounds_min: [f32; 3],
    pub first_or_left: u32,
    pub bounds_max: [f32; 3],
    pub triangle_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MeshTriangle {
    pub p0: [f32; 3],
    pub _pad0: f32,
    pub p1: [f32; 3],
    pub _pad1: f32,
    pub p2: [f32; 3],
    pub _pad2: f32,
    pub n0: [f32; 3],
    pub _pad3: f32,
    pub n1: [f32; 3],
    pub _pad4: f32,
    pub n2: [f32; 3],
    pub _pad5: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FieldKind {
    /// `n` 32-bit floats.
    F32(u8),
    /// `n` 32-bit unsigned integers.
    U32(u8),
    /// mat3x3<f32> stored as three padded columns.
    Mat3,
    /// mat4x4<f32> stored as four columns.
    Mat4,
    /// A nested record (only FrameUniforms.camera).
    Nested,
}

impl FieldKind {
    /// Number of scalar words the GPU dump kernels write for this field.
    pub fn dump_words(self) -> usize {
        match self {
            FieldKind::F32(n) | FieldKind::U32(n) => n as usize,
            FieldKind::Mat3 => 9,
            FieldKind::Mat4 => 16,
            FieldKind::Nested => 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RustField {
    pub name: &'static str,
    pub offset: usize,
    pub size: usize,
    pub kind: FieldKind,
}

pub trait Record: Pod {
    /// The struct name in types.wgsl.
    const NAME: &'static str;
    fn fields() -> Vec<RustField>;
}

macro_rules! record {
    ($ty:ident { $($wgsl:literal => $field:ident : $kind:expr),* $(,)? }) => {
        impl Record for $ty {
            const NAME: &'static str = stringify!($ty);
            fn fields() -> Vec<RustField> {
                let probe = <$ty as Zeroable>::zeroed();
                vec![$(RustField {
                    name: $wgsl,
                    offset: std::mem::offset_of!($ty, $field),
                    size: std::mem::size_of_val(&probe.$field),
                    kind: $kind,
                }),*]
            }
        }
    };
}

use FieldKind::{F32, Mat3, Mat4, Nested, U32};

record!(ObjectData {
    "localToWorld" => local_to_world: Mat4,
    "worldToLocal" => world_to_local: Mat4,
    "localCenter" => local_center: F32(3),
    "boundingRadius" => bounding_radius: F32(1),
    "localExtent" => local_extent: F32(3),
    "objectIndex" => object_index: U32(1),
    "worldCenter" => world_center: F32(3),
    "shapeKind" => shape_kind: U32(1),
    "worldExtent" => world_extent: F32(3),
    "albedoRoughness" => albedo_roughness: F32(4),
    "emissive" => emissive: F32(4),
});
record!(VoxelSample {
    "localPosition" => local_position: F32(3),
    "signedDistance" => signed_distance: F32(1),
    "voxel" => voxel: U32(3),
    "encoded" => encoded: U32(1),
});
record!(BrickEntry {
    "virtualBrick" => virtual_brick: U32(3),
    "mipLevel" => mip_level: U32(1),
    "physicalBrick" => physical_brick: U32(3),
    "physicalIndex" => physical_index: U32(1),
    "distanceScaleBias" => distance_scale_bias: F32(2),
    "voxelSize" => voxel_size: F32(1),
    "flags" => flags: U32(1),
});
record!(CardData {
    "center" => center: F32(3),
    "objectIndex" => object_index: U32(1),
    "tangentFrame" => tangent_frame: Mat3,
    "halfExtents" => half_extents: F32(3),
    "pageIndex" => page_index: U32(1),
    "sizeInPages" => size_in_pages: U32(2),
    "resLevel" => res_level: U32(1),
    "flags" => flags: U32(1),
});
record!(PageEntry {
    "physicalPage" => physical_page: U32(2),
    "cardIndex" => card_index: U32(1),
    "mipLevel" => mip_level: U32(1),
    "cardUVRect" => card_uv_rect: F32(4),
    "flags" => flags: U32(1),
});
record!(RayDebugData {
    "origin" => origin: F32(3),
    "roughness" => roughness: F32(1),
    "incidentDir" => incident_dir: F32(3),
    "hitDistance" => hit_distance: F32(1),
    "hitPosition" => hit_position: F32(3),
    "hitObject" => hit_object: U32(1),
    "normal" => normal: F32(3),
    "envBRDF" => env_brdf: F32(1),
    "viewDir" => view_dir: F32(3),
    "ggxAlpha" => ggx_alpha: F32(1),
    "reflectedDir" => reflected_dir: F32(3),
    "reflectedHitDistance" => reflected_hit_distance: F32(1),
    "reflectedHitPosition" => reflected_hit_position: F32(3),
    "reflectedHitObject" => reflected_hit_object: U32(1),
});
record!(CameraData {
    "worldToView" => world_to_view: Mat4,
    "viewToClip" => view_to_clip: Mat4,
    "worldToClip" => world_to_clip: Mat4,
    "position" => position: F32(3),
    "verticalFov" => vertical_fov: F32(1),
    "viewportSize" => viewport_size: F32(2),
    "nearPlane" => near_plane: F32(1),
    "farPlane" => far_plane: F32(1),
});
record!(ProbeRecord {
    "position" => position: F32(3),
    "firstSample" => first_sample: U32(1),
    "normal" => normal: F32(3),
    "sampleCount" => sample_count: U32(1),
    "screenPixel" => screen_pixel: U32(2),
    "historyWeight" => history_weight: F32(1),
    "framesAccumulated" => frames_accumulated: U32(1),
    "irradiance" => irradiance: F32(4),
});
record!(RadianceSample {
    "direction" => direction: F32(3),
    "hitDistance" => hit_distance: F32(1),
    "radiance" => radiance: F32(3),
    "hitObject" => hit_object: U32(1),
});
record!(MeshNode {
    "boundsMin" => bounds_min: F32(3),
    "firstOrLeft" => first_or_left: U32(1),
    "boundsMax" => bounds_max: F32(3),
    "triangleCount" => triangle_count: U32(1),
});
record!(MeshTriangle {
    "p0" => p0: F32(3),
    "p1" => p1: F32(3),
    "p2" => p2: F32(3),
    "n0" => n0: F32(3),
    "n1" => n1: F32(3),
    "n2" => n2: F32(3),
});
record!(FrameUniforms {
    "camera" => camera: Nested,
    "lightCenterRadiance" => light_center_radiance: F32(4),
    "lightAxisU" => light_axis_u: F32(4),
    "lightAxisV" => light_axis_v: F32(4),
    "lightColor" => light_color: F32(4),
    "ambientColor" => ambient_color: F32(4),
    "floorParams" => floor_params: F32(4),
    "overlayParams" => overlay_params: F32(4),
    "cutPlane" => cut_plane: F32(4),
    "deformParams" => deform_params: F32(4),
    "sdfVolume" => sdf_volume: F32(4),
    "atlasDisplay" => atlas_display: F32(4),
    "counts" => counts: U32(4),
    "selection" => selection: U32(4),
    "selection2" => selection2: U32(4),
    "counts2" => counts2: U32(4),
    "cacheView" => cache_view: F32(4),
});

/// Words a dump kernel in verify.wgsl writes for one record:
/// every field in declaration order, matrices as m[row][column].
pub fn expected_dump_words<T: Record>(record: &T) -> Vec<u32> {
    let bytes = bytemuck::bytes_of(record);
    let word = |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let mut out = Vec::new();
    for field in T::fields() {
        match field.kind {
            FieldKind::F32(n) | FieldKind::U32(n) => {
                for i in 0..n as usize {
                    out.push(word(field.offset + 4 * i));
                }
            }
            // Column-major storage: element (row r, column c) lives in column c.
            FieldKind::Mat3 => {
                for r in 0..3 {
                    for c in 0..3 {
                        out.push(word(field.offset + 16 * c + 4 * r));
                    }
                }
            }
            FieldKind::Mat4 => {
                for r in 0..4 {
                    for c in 0..4 {
                        out.push(word(field.offset + 16 * c + 4 * r));
                    }
                }
            }
            FieldKind::Nested => {}
        }
    }
    out
}

pub fn dump_words_per_record<T: Record>() -> usize {
    T::fields().iter().map(|f| f.kind.dump_words()).sum()
}

// ---------------------------------------------------------------------------
// Struct layouts naga computes for types.wgsl at run time (shaders::layout_json).

#[derive(Clone, Debug, Serialize)]
pub struct ShaderField {
    pub name: String,
    pub offset: usize,
    pub size: usize,
    pub alignment: usize,
    /// WGSL spelling of the type, e.g. `mat3x3<f32>` or `vec2<u32>`.
    pub type_name: String,
    /// For matrices: (columns, rows).
    pub matrix: Option<(u32, u32)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ShaderStruct {
    pub name: String,
    /// Size including trailing padding; the array stride in a storage buffer.
    pub size: usize,
    pub alignment: usize,
    pub fields: Vec<ShaderField>,
}

impl ShaderStruct {
    pub fn field(&self, name: &str) -> Option<&ShaderField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Bytes not covered by any field's data: (offset, size, description).
    /// A mat3x3<f32> stores each 12-byte column in 16 bytes.
    pub fn padding(&self) -> Vec<(usize, usize, String)> {
        let mut pads = Vec::new();
        let mut cursor = 0usize;
        for f in &self.fields {
            if f.offset > cursor {
                pads.push((cursor, f.offset - cursor, format!("before {}", f.name)));
            }
            if let Some((columns, 3)) = f.matrix {
                for c in 0..columns as usize {
                    pads.push((f.offset + 16 * c + 12, 4, format!("{} column {c}", f.name)));
                }
            }
            cursor = cursor.max(f.offset + f.size);
        }
        if self.size > cursor {
            pads.push((cursor, self.size - cursor, "tail".to_string()));
        }
        pads
    }
}

pub fn parse_layouts(json: &str) -> anyhow::Result<BTreeMap<String, ShaderStruct>> {
    let root: Value = serde_json::from_str(json)?;
    let mut out = BTreeMap::new();
    for (name, s) in root["structs"].as_object().into_iter().flatten() {
        let fields = s["members"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| ShaderField {
                name: m["name"].as_str().unwrap_or("?").to_string(),
                offset: m["offset"].as_u64().unwrap_or(0) as usize,
                size: m["size"].as_u64().unwrap_or(0) as usize,
                alignment: m["alignment"].as_u64().unwrap_or(0) as usize,
                type_name: m["type"].as_str().unwrap_or("?").to_string(),
                matrix: m["matrix"].as_array().map(|a| (a[0].as_u64().unwrap_or(0) as u32, a[1].as_u64().unwrap_or(0) as u32)),
            })
            .collect();
        out.insert(
            name.clone(),
            ShaderStruct {
                name: name.clone(),
                size: s["size"].as_u64().unwrap_or(0) as usize,
                alignment: s["alignment"].as_u64().unwrap_or(0) as usize,
                fields,
            },
        );
    }
    Ok(out)
}

#[derive(Clone, Debug, Serialize)]
pub struct FieldCheck {
    pub name: String,
    pub wgsl_type: String,
    pub wgsl_offset: usize,
    pub rust_offset: Option<usize>,
    pub wgsl_size: usize,
    pub rust_size: Option<usize>,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct LayoutCheck {
    pub record: String,
    pub wgsl_size: usize,
    pub wgsl_alignment: usize,
    pub rust_size: usize,
    pub padding_bytes: usize,
    pub fields: Vec<FieldCheck>,
    pub ok: bool,
}

pub fn check_record<T: Record>(layouts: &BTreeMap<String, ShaderStruct>) -> LayoutCheck {
    let rust = T::fields();
    let Some(wgsl) = layouts.get(T::NAME) else {
        return LayoutCheck {
            record: T::NAME.to_string(),
            wgsl_size: 0,
            wgsl_alignment: 0,
            rust_size: std::mem::size_of::<T>(),
            padding_bytes: 0,
            fields: vec![],
            ok: false,
        };
    };
    let mut ok = wgsl.size == std::mem::size_of::<T>() && wgsl.fields.len() == rust.len();
    let fields = wgsl
        .fields
        .iter()
        .map(|f| {
            let r = rust.iter().find(|r| r.name == f.name);
            let field_ok = r.is_some_and(|r| r.offset == f.offset && r.size == f.size);
            ok &= field_ok;
            FieldCheck {
                name: f.name.clone(),
                wgsl_type: f.type_name.clone(),
                wgsl_offset: f.offset,
                rust_offset: r.map(|r| r.offset),
                wgsl_size: f.size,
                rust_size: r.map(|r| r.size),
                ok: field_ok,
            }
        })
        .collect();
    LayoutCheck {
        record: T::NAME.to_string(),
        wgsl_size: wgsl.size,
        wgsl_alignment: wgsl.alignment,
        rust_size: std::mem::size_of::<T>(),
        padding_bytes: wgsl.padding().iter().map(|p| p.1).sum(),
        fields,
        ok,
    }
}

pub fn check_all(layouts: &BTreeMap<String, ShaderStruct>) -> Vec<LayoutCheck> {
    vec![
        check_record::<ObjectData>(layouts),
        check_record::<VoxelSample>(layouts),
        check_record::<BrickEntry>(layouts),
        check_record::<CardData>(layouts),
        check_record::<PageEntry>(layouts),
        check_record::<RayDebugData>(layouts),
        check_record::<CameraData>(layouts),
        check_record::<ProbeRecord>(layouts),
        check_record::<RadianceSample>(layouts),
        check_record::<FrameUniforms>(layouts),
        check_record::<MeshNode>(layouts),
        check_record::<MeshTriangle>(layouts),
    ]
}

// ---------------------------------------------------------------------------
// Declarations extracted from types.wgsl for the struct panel.

#[derive(Clone, Debug)]
pub struct Declaration {
    /// (line, field name, declaration text without comments).
    pub fields: Vec<(usize, String, String)>,
}

pub fn parse_declarations(source: &str) -> BTreeMap<String, Declaration> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some(rest) = line.strip_prefix("struct ") {
            let name = rest.trim_end_matches('{').trim().to_string();
            let mut fields = Vec::new();
            i += 1;
            while i < lines.len() && lines[i].trim() != "}" {
                let code = lines[i].split("//").next().unwrap_or("").trim();
                if let Some((field, _)) = code.split_once(':') {
                    fields.push((i + 1, field.trim().to_string(), code.to_string()));
                }
                i += 1;
            }
            out.insert(name, Declaration { fields });
        }
        i += 1;
    }
    out
}
