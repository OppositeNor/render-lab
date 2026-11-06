//! How the inspector shows a resource: its visualizers and their parameters,
//! the inspector's entry points, and the resources a program pass uses.

use super::*;

/// A view of a resource as text, `<show>[?<param>=<value>[,<value>]&…]`
/// (`SceneShading::shows`, the inspector's page): its visualizer, if named,
/// and its parameters, checked.
pub fn parse_view(spec: &str) -> Result<(Option<Show>, crate::utility::inset::ImageParams), String> {
    let (show, query) = spec.split_once('?').unwrap_or((spec, ""));
    let show = if show.is_empty() { None } else { Some(Show::from_key(show)?) };
    Ok((show, parse_params(query)?))
}

/// The inspector's parameters as text, `<param>=<value>[,<value>]&…`, checked.
pub fn parse_params(query: &str) -> Result<crate::utility::inset::ImageParams, String> {
    let mut params = std::collections::BTreeMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (name, value) = pair.split_once('=').ok_or_else(|| format!("`{pair}`: expected name=value"))?;
        let value: Vec<f32> = value.split(',').map(|v| v.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|e| format!("`{pair}`: {e}"))?;
        check_inspect_param(name, &value)?;
        params.insert(name.to_string(), value);
    }
    Ok(params)
}

/// How the inspector draws a resource (`imageShow=`; packages/render_lab/shaders/inspect.wgsl).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Show {
    /// HDR colour: `exposure` stops, then sRGB.
    Color,
    /// Stored values as they are (albedo, masks); `channel` picks one.
    Unorm,
    /// A normal: `encoding` 0 signed xyz, 1 unsigned xyz, 2 octahedral xy.
    Normal,
    /// Reversed depth on a logarithmic ramp over `range` (near bright).
    Depth,
    /// One channel (`channel`) on a ramp over `range`.
    Ramp,
    /// One channel, diverging around 0 over `range`.
    Signed,
    /// A buffer's word (`word` of each tile's) as a count on a ramp over `range`; 0 black.
    Count,
    /// A buffer's word read as a float, on a ramp over `range`.
    Float,
    /// Distinct colours per value (ids, stencil references); 0 black.
    Hash,
}

impl Show {
    pub const ALL: [Show; 9] = [Show::Color, Show::Unorm, Show::Normal, Show::Depth, Show::Ramp, Show::Signed, Show::Count, Show::Float, Show::Hash];

    pub fn key(self) -> &'static str {
        match self {
            Show::Color => "color",
            Show::Unorm => "unorm",
            Show::Normal => "normal",
            Show::Depth => "depth",
            Show::Ramp => "ramp",
            Show::Signed => "signed",
            Show::Count => "count",
            Show::Float => "float",
            Show::Hash => "hash",
        }
    }

    pub fn from_key(key: &str) -> Result<Show, String> {
        Show::ALL.into_iter().find(|s| s.key() == key).ok_or_else(|| format!("imageShow `{key}`: one of {}", Show::ALL.map(Show::key).join(", ")))
    }

    /// Its index in the shader (`InspectParams::mode`).
    pub fn mode(self) -> u32 {
        Show::ALL.iter().position(|s| *s == self).expect("a show") as u32
    }

    /// The range it maps to 0..1 unless `image.range` says.
    pub fn range(self) -> [f32; 2] {
        match self {
            Show::Depth => [1e-3, 1.0],
            Show::Signed => [-1.0, 1.0],
            Show::Count => [0.0, 64.0],
            _ => [0.0, 1.0],
        }
    }
}

/// The inspector's parameters (`image.<key>=` on a graph image): key,
/// components, default (`range`: the show's), and range.
pub const INSPECT_PARAMS: &[(&str, usize, f32, (f32, f32))] = &[
    ("exposure", 1, 0.0, (-16.0, 16.0)),
    ("channel", 1, 0.0, (0.0, 4.0)),
    ("encoding", 1, 0.0, (0.0, 3.0)),
    ("average", 1, 0.0, (0.0, 1.0)),
    ("range", 2, 0.0, (-1e9, 1e9)),
    ("slice", 1, 0.5, (0.0, 1.0)),
    ("level", 1, 0.0, (0.0, 64.0)),
    ("axis", 1, 1.0, (0.0, 2.0)),
    ("word", 1, 0.0, (0.0, 1e6)),
    // An integer image's bit field shown: its first bit and its width (0: the whole value).
    ("bits", 2, 0.0, (0.0, 32.0)),
    // Which image of a binding array (`ProgramImage::count`).
    ("index", 1, 0.0, (0.0, 64.0)),
];

/// Checks a graph image's parameter.
pub fn check_inspect_param(key: &str, value: &[f32]) -> Result<(), String> {
    let Some(&(_, components, _, (lo, hi))) = INSPECT_PARAMS.iter().find(|p| p.0 == key) else {
        return Err(format!("image.{key}: a graph image takes {}", INSPECT_PARAMS.iter().map(|p| p.0).collect::<Vec<_>>().join(", ")));
    };
    if value.len() != components {
        return Err(format!("image.{key} takes {components} number(s), not {}", value.len()));
    }
    if let Some(v) = value.iter().find(|v| !(lo..=hi).contains(*v)) {
        return Err(format!("image.{key} = {v}: outside {lo}..{hi}"));
    }
    Ok(())
}

/// The inspector's entry point for a resource of this shape (inspect.wgsl), and its source's binding.
pub fn inspect_entry(shape: &Shape) -> (&'static str, u32) {
    match shape {
        Shape::Texture { format: wgpu::TextureFormat::Depth32Float, samples, .. } if *samples > 1 => ("inspectDepthMsCS", 4),
        Shape::Texture { samples, .. } if *samples > 1 => ("inspectTextureMsCS", 3),
        Shape::Texture { .. } => ("inspectTextureCS", 2),
        Shape::Buffer { .. } | Shape::UpdateBuffer => ("inspectBufferCS", 6),
        Shape::DepthArray { .. } => ("inspectDepthArrayCS", 5),
        Shape::Volume { .. } => ("inspectVolumeCS", 7),
        Shape::Bytes { .. } => ("inspectBufferCS", 6),
        Shape::Image { kind, format, .. } => {
            use crate::investigation::ImageKind;
            let int = image_class(*format);
            match (kind, int) {
                (ImageKind::D2, 0) => ("inspectTextureCS", 2),
                (ImageKind::D3, 0) => ("inspectVolumeCS", 7),
                (ImageKind::D3, 1) => ("inspectVolumeUintCS", 8),
                (ImageKind::D3, _) => ("inspectVolumeSintCS", 9),
                (ImageKind::D2Array, 0) => ("inspectArrayCS", 10),
                (ImageKind::D2Array, 1) | (ImageKind::D2, 1) => ("inspectArrayUintCS", 11),
                (ImageKind::D2Array, _) | (ImageKind::D2, _) => ("inspectArraySintCS", 12),
            }
        }
    }
}

/// A format's class for the inspector: 0 float (unorm, snorm, float), 1 unsigned integer, 2 signed integer.
pub fn image_class(format: wgpu::TextureFormat) -> u32 {
    match format.sample_type(None, None) {
        Some(wgpu::TextureSampleType::Uint) => 1,
        Some(wgpu::TextureSampleType::Sint) => 2,
        _ => 0,
    }
}

/// The resources a program pass's uses read and write (samplers are neither).
pub fn program_reads(uses: &[crate::investigation::ProgramUse]) -> Vec<&'static str> {
    program_resources(uses, crate::investigation::Access::reads)
}

pub fn program_writes(uses: &[crate::investigation::ProgramUse]) -> Vec<&'static str> {
    program_resources(uses, crate::investigation::Access::writes)
}

/// The resources of the uses whose access `keep`s, each once, in order.
pub(super) fn program_resources(uses: &[crate::investigation::ProgramUse], keep: impl Fn(crate::investigation::Access) -> bool) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for u in uses.iter().filter(|u| keep(u.access)) {
        if !out.contains(&u.resource) {
            out.push(u.resource);
        }
    }
    out
}

/// How a resource shows unless `imageShow` says.
pub fn default_show(shape: &Shape) -> Show {
    match shape {
        Shape::Texture { format, .. } => match format {
            wgpu::TextureFormat::Depth32Float => Show::Depth,
            wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float | wgpu::TextureFormat::Rg11b10Ufloat => Show::Color,
            wgpu::TextureFormat::R8Unorm | wgpu::TextureFormat::R16Float | wgpu::TextureFormat::R32Float => Show::Ramp,
            _ => Show::Unorm,
        },
        // One word a pixel holds a value (a depth, a factor), read as f32; more, counts and indices.
        Shape::Buffer { words_per_tile: 1, .. } => Show::Float,
        Shape::Buffer { .. } | Shape::UpdateBuffer => Show::Count,
        Shape::DepthArray { .. } => Show::Depth,
        Shape::Volume { .. } => Show::Color,
        Shape::Bytes { .. } => Show::Count,
        Shape::Image { format, .. } => match image_class(*format) {
            0 => default_show(&Shape::Texture { format: *format, divisor: 1, samples: 1 }),
            1 => Show::Hash,
            _ => Show::Signed,
        },
    }
}
