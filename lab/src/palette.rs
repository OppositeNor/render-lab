//! Shared colours. Overlays, labels, and struct-panel swatches use the same
//! constants, so an arrow and the field that produced it always match.
//! Values are sRGB.

use egui::Color32;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn to_f32(c: Color32, alpha: f32) -> [f32; 4] {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, alpha]
}

// Interface: the theme's colours (src/theme.rs), read with these functions.
// Until a theme is set, the lab's own (`Theme::CLASSIC`), which the
// recorder keeps, so recorded frames never depend on the user's settings.
static CURRENT: RwLock<Theme> = RwLock::new(Theme::CLASSIC);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The interface's colours. Every colour a theme file may set is a field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub background: Color32,
    pub panel: Color32,
    pub panel_raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub highlight_row: Color32,
    pub padding: Color32,
    /// A widget under the pointer.
    pub hover: Color32,
    /// A selected item (a list's row, a selectable button).
    pub selection: Color32,
    /// A struct's byte cells: even and odd fields.
    pub field_even: Color32,
    pub field_odd: Color32,
    /// A bar's and a slider's rail.
    pub track: Color32,
    /// The timeline's hover card.
    pub card: Color32,
    /// The homepage's smoked glass, for panels full of text.
    pub smoke: Color32,
    /// The homepage's popup, and what dims the page under it.
    pub popup: Color32,
    pub dim: Color32,
    /// The shadow under captions.
    pub shadow: Color32,
    pub code_keyword: Color32,
    pub code_name: Color32,
    pub code_number: Color32,
    pub code_string: Color32,
}

impl Theme {
    /// The lab's own colours (the original video's dark theme).
    pub const CLASSIC: Theme = Theme {
        background: rgb(0x07121f),
        panel: rgb(0x0b1b2d),
        panel_raised: rgb(0x102538),
        border: rgb(0x274155),
        text: rgb(0xeef6ff),
        text_dim: rgb(0x9bb5cc),
        text_faint: rgb(0x6f8aa2),
        accent: rgb(0x64dfd1),
        highlight_row: Color32::from_rgba_premultiplied(30, 70, 80, 200),
        padding: rgb(0x3a4a5a),
        hover: rgb(0x1c3a52),
        selection: rgb(0x145a59),
        field_even: rgb(0x3d5f7d),
        field_odd: rgb(0x2b4760),
        track: rgb(0x263e51),
        card: Color32::from_rgba_unmultiplied_const(7, 18, 31, 240),
        smoke: Color32::from_rgba_unmultiplied_const(6, 14, 24, 120),
        popup: Color32::from_rgba_unmultiplied_const(10, 22, 36, 235),
        dim: Color32::from_rgba_unmultiplied_const(2, 6, 12, 150),
        shadow: rgb(0x020810),
        code_keyword: rgb(0xc792ea),
        code_name: rgb(0x82aaff),
        code_number: rgb(0xf78c6c),
        code_string: rgb(0xc3e88d),
    };
}

/// The theme in use.
pub fn theme() -> Theme {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

/// Use `theme` from the next frame on.
pub fn set_theme(theme: Theme) {
    let mut current = CURRENT.write().unwrap_or_else(|e| e.into_inner());
    if *current != theme {
        *current = theme;
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

/// Counts the theme's changes, so a user of it knows when to restyle.
pub fn theme_generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// The theme's accent, where a colour must be a constant (a value's `color`
/// in a `#[value]` declaration); `themed` turns it into the theme's.
pub const THEME_ACCENT: Color32 = Theme::CLASSIC.accent;

/// `color` as the theme shows it: `THEME_ACCENT` is the theme's accent;
/// any other colour is itself.
pub fn themed(color: Color32) -> Color32 {
    if color == THEME_ACCENT { accent() } else { color }
}

pub fn background() -> Color32 {
    theme().background
}
pub fn panel() -> Color32 {
    theme().panel
}
pub fn panel_raised() -> Color32 {
    theme().panel_raised
}
pub fn border() -> Color32 {
    theme().border
}
pub fn text() -> Color32 {
    theme().text
}
pub fn text_dim() -> Color32 {
    theme().text_dim
}
pub fn text_faint() -> Color32 {
    theme().text_faint
}
pub fn accent() -> Color32 {
    theme().accent
}
pub fn highlight_row() -> Color32 {
    theme().highlight_row
}
pub fn padding() -> Color32 {
    theme().padding
}
pub fn code_keyword() -> Color32 {
    theme().code_keyword
}
pub fn code_name() -> Color32 {
    theme().code_name
}
pub fn code_number() -> Color32 {
    theme().code_number
}
pub fn code_string() -> Color32 {
    theme().code_string
}

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
        ("CameraData", _) => vec![accent()],
        _ => vec![accent()],
    }
}
