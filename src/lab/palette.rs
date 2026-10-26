//! Shared colours. Overlays, labels, and struct-panel swatches use the same
//! constants, so an arrow and the field that produced it always match.
//! Values are sRGB.

use egui::Color32;

pub const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn to_f32(c: Color32, alpha: f32) -> [f32; 4] {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, alpha]
}

// Interface (the existing dark theme of the original video).
pub const BACKGROUND: Color32 = rgb(0x07121f);
pub const PANEL: Color32 = rgb(0x0b1b2d);
pub const PANEL_RAISED: Color32 = rgb(0x102538);
pub const BORDER: Color32 = rgb(0x274155);
pub const TEXT: Color32 = rgb(0xeef6ff);
pub const TEXT_DIM: Color32 = rgb(0x9bb5cc);
pub const TEXT_FAINT: Color32 = rgb(0x6f8aa2);
pub const ACCENT: Color32 = rgb(0x64dfd1);
pub const HIGHLIGHT_ROW: Color32 = Color32::from_rgba_premultiplied(30, 70, 80, 200);
pub const PADDING: Color32 = rgb(0x3a4a5a);

// Geometry.
pub const AXIS_X: Color32 = rgb(0xff5d5d);
pub const AXIS_Y: Color32 = rgb(0x6be37a);
pub const AXIS_Z: Color32 = rgb(0x5aa0ff);
pub const OBB: Color32 = rgb(0xffd166);
pub const AABB: Color32 = rgb(0x4dd6e8);
pub const POINT: Color32 = rgb(0xffffff);

// Reflection vectors: one colour per RayDebugData field.
pub const ORIGIN: Color32 = rgb(0xc9d6e3);
pub const INCIDENT: Color32 = rgb(0xffa14a);
pub const NORMAL: Color32 = rgb(0x6be3b0);
pub const REFLECTED: Color32 = rgb(0xff6fb5);
pub const VIEW: Color32 = rgb(0x8fb8ff);
pub const HIT: Color32 = rgb(0xffffff);
pub const CONE: Color32 = rgb(0xff6fb5);

// Distance fields.
pub const BRICK_STORED: Color32 = rgb(0x8fd3ff);
pub const BRICK_EMPTY: Color32 = rgb(0x4a6a86);
pub const SELECTED: Color32 = rgb(0xffd166);
pub const ATLAS_SLOT: Color32 = rgb(0x5d7d9c);
pub const INSIDE: Color32 = rgb(0xfa6b2e);
pub const SURFACE: Color32 = rgb(0xf7f2e0);
pub const OUTSIDE: Color32 = rgb(0x338cfa);

// Surface Cache.
pub const CARD: Color32 = rgb(0xb6e3ff);
pub const PAGE: Color32 = rgb(0xffd166);

// Pseudo-code listings.
pub const CODE_KEYWORD: Color32 = rgb(0xc792ea);
pub const CODE_NAME: Color32 = rgb(0x82aaff);
pub const CODE_NUMBER: Color32 = rgb(0xf78c6c);
pub const CODE_STRING: Color32 = rgb(0xc3e88d);

// Probes.
pub const PROBE: Color32 = rgb(0xf4f1de);
pub const PROBE_SELECTED: Color32 = rgb(0xffd166);
pub const HISTORY_REJECT: Color32 = rgb(0xff6b6b);
pub const MISMATCH: Color32 = rgb(0xff6b6b);

// The global distance field, card generation, cache management, and trace paths.
/// One colour per global distance field clipmap, finest first.
pub const CLIPMAPS: [Color32; 4] = [rgb(0x64dfd1), rgb(0x8fb8ff), rgb(0xc792ea), rgb(0xf78c6c)];
pub const PAGE_KEPT: Color32 = rgb(0x5d7d9c);
pub const PAGE_NEW: Color32 = rgb(0xffd166);
pub const SURFEL: Color32 = rgb(0xf4f1de);
pub const SURFEL_REJECTED: Color32 = rgb(0xff6b6b);
pub const LOCKED: Color32 = rgb(0x8fb8ff);
pub const FEEDBACK: Color32 = rgb(0xffd166);
pub const EVICTED: Color32 = rgb(0xff6b6b);
pub const ADAPTIVE: Color32 = rgb(0xc792ea);

/// Swatch colours for a highlighted struct field (several for matrices).
pub fn field_colors(record: &str, field: &str) -> Vec<Color32> {
    match (record, field) {
        ("ObjectData", "localToWorld") | ("ObjectData", "worldToLocal") => vec![AXIS_X, AXIS_Y, AXIS_Z],
        ("ObjectData", "localCenter") | ("ObjectData", "localExtent") => vec![OBB],
        ("ObjectData", "worldCenter") | ("ObjectData", "worldExtent") => vec![AABB],
        ("ObjectData", "boundingRadius") => vec![POINT],
        ("RayDebugData", "origin") => vec![ORIGIN],
        ("RayDebugData", "incidentDir") => vec![INCIDENT],
        ("RayDebugData", "hitPosition") | ("RayDebugData", "hitDistance") => vec![HIT],
        ("RayDebugData", "normal") => vec![NORMAL],
        ("RayDebugData", "viewDir") | ("RayDebugData", "envBRDF") => vec![VIEW],
        ("RayDebugData", "reflectedDir") | ("RayDebugData", "reflectedHitPosition") | ("RayDebugData", "reflectedHitDistance") => vec![REFLECTED],
        ("RayDebugData", "ggxAlpha") | ("RayDebugData", "roughness") => vec![CONE],
        ("CardData", "tangentFrame") => vec![AXIS_X, AXIS_Y, AXIS_Z],
        ("CardData", "halfExtents") | ("CardData", "center") => vec![CARD],
        ("CardData", "pageIndex") | ("CardData", "sizeInPages") => vec![PAGE],
        ("PageEntry", _) => vec![PAGE],
        ("BrickEntry", "virtualBrick") | ("BrickEntry", "physicalBrick") | ("BrickEntry", "physicalIndex") => vec![SELECTED],
        ("BrickEntry", "distanceScaleBias") => vec![INSIDE, SURFACE, OUTSIDE],
        ("VoxelSample", "signedDistance") | ("VoxelSample", "encoded") => vec![INSIDE, SURFACE, OUTSIDE],
        ("VoxelSample", _) => vec![SELECTED],
        ("ProbeRecord", _) => vec![PROBE_SELECTED],
        ("RadianceSample", _) => vec![PROBE],
        ("CameraData", _) => vec![ACCENT],
        _ => vec![ACCENT],
    }
}
