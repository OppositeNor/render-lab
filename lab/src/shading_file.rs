//! An investigation's scene shading as data: its package's `shading`
//! resource (`shading.toml`), read into a `SceneShading`
//! (src/investigation.rs) once per run. What computes stays code: the file
//! names the functions its investigation registers
//! (`Investigation::shading_hooks`), and the lab looks them up by name.
//!
//! The frame is its `[[node]]` list, in order (`SceneShading::nodes`): the
//! frame graph declares the nodes so, and the renderer runs them in that
//! order where their reads and writes allow. Each node has a `kind`:
//!
//! - `update`: an update pass (they come first), `entry`, `reads`, `writes`,
//!   `workgroup` (x);
//! - `depth-views`: the depth views (shadow maps), `name`, `size`,
//!   `vertices`, `bias` (`[constant, slope]`);
//! - `prepass`: the depth pre-pass, `entry` (its fragment entry), `writes`
//!   (the targets it writes besides the depth);
//! - `compute`: a compute pass of the chain, `entry`, `reads`, `writes`,
//!   `workgroup` (`[x, y]`);
//! - `program`: a pass of the program, `name`, `compute` and `workgroup`
//!   (`[x, y, z]`) or `vertex` and `fragment`, `instances` (1), `uses`; a run
//!   of consecutive program nodes is where the plan's commands for them are
//!   recorded;
//! - `scene`: the scene pass (exactly one), `entry` (its fragment entry),
//!   `outputs` (the targets it writes; none: `hdr`), `reads`.
//!
//! The rest is what the nodes use, in kebab case of `SceneShading`'s fields:
//!
//! - `module` (required); `data` (a `Data` hook; none: no data), `verify`,
//!   `history` (lists, empty when left out);
//! - `[[target]]` `name`, `format`, `divisor` (1); `[[buffer]]` `name`,
//!   `words-per-tile`, `tile`; `[[volume]]` `name`, `format`, `size`, `mips` (1);
//! - `[resolution]` `targets`, `divisor` (a `Divisor` hook);
//! - `[update]` `plan` (an `UpdatePlan` hook); `[[update.buffer]]` `name`,
//!   `words` (a `Words` hook);
//! - `[shows]`: a resource's default view, `name = "<show>[?<param>=…]"`;
//! - `[program]` `plan` (a `Program` hook), `warmup` (0), `record-history`
//!   (false), `scene-uses`;
//!   `[[program.image]]` `name`, `kind` (`2d`, `2d-array`, `3d`), `size`,
//!   `format`, `count` (1), `mips` (1; `"all"`), `viewport` (0);
//!   `[[program.buffer]]` `name`, `bytes`, `indirect` (false), `count` (1).
//!
//! A use is an inline table: `binding`, `resource`, `access` (`sampled`,
//! `load`, `read`, `write`, `read-write`, `atomic`, `uniform`, `sampler`),
//! and which elements it binds: element 0 when left out, `which = "all"` or
//! `"instance"`, `fixed = k`, `pick = [..]`, or `mip = k`.
//!
//! Formats are WebGPU's names (`rgba16float`, `rgba8unorm-srgb`). An unknown
//! key, a missing field, or a hook the investigation does not register is an
//! error that names the file; whether the order works (each node once, the
//! update passes first, the pre-pass before the scene pass, reads after
//! writes) is the frame graph's to check (src/graph/).

use crate::investigation::{
    Access, DepthViews, FrameNode, ImageKind, PassKind, ProgramBuffer, ProgramImage, ProgramPass, ProgramUse, SceneShading, ShadingBuffer, ShadingHook, ShadingPass,
    ShadingProgram, ShadingResolution, ShadingTarget, ShadingUpdate, ShadingVolume, UpdateBuffer, UpdatePass, Which,
};
use std::sync::RwLock;
use toml_edit::{Item, TableLike};

type Hooks = [(&'static str, ShadingHook)];

/// The scene shading of `package`, read from its `shading` resource the first
/// time it is asked for and kept for the run (the renderer tells shadings
/// apart by address); None when the package has none. A file that does not
/// read is a panic naming it, as a missing resource is.
pub fn of(package: &'static str, hooks: &'static Hooks) -> Option<&'static SceneShading> {
    static CACHE: RwLock<Vec<(&'static str, Option<&'static SceneShading>)>> = RwLock::new(Vec::new());
    if let Some((_, shading)) = CACHE.read().unwrap().iter().find(|(p, _)| *p == package) {
        return *shading;
    }
    let mut cache = CACHE.write().unwrap();
    if let Some((_, shading)) = cache.iter().find(|(p, _)| *p == package) {
        return *shading;
    }
    let shading = crate::resources::find(&format!("{package}.shading")).map(|resource| {
        let shading = parse(resource.text(), hooks).unwrap_or_else(|e| panic!("shading: {}: {e}", resource.source()));
        &*Box::leak(Box::new(shading))
    });
    cache.push((package, shading));
    shading
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn leak_slice<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// Fails on a key of `t` not in `fields`.
fn known(t: &dyn TableLike, what: &str, fields: &[&str]) -> Result<(), String> {
    match t.iter().find(|(k, _)| !fields.contains(k)) {
        Some((key, _)) => Err(format!("{what} has no field `{key}` (it has {})", fields.join(", "))),
        None => Ok(()),
    }
}

fn string(t: &dyn TableLike, field: &str, what: &str) -> Result<Option<&'static str>, String> {
    t.get(field).map(|i| i.as_str().map(leak).ok_or_else(|| format!("{what}: `{field}` is a string"))).transpose()
}

fn required(t: &dyn TableLike, field: &str, what: &str) -> Result<&'static str, String> {
    string(t, field, what)?.ok_or_else(|| format!("{what} needs `{field}`"))
}

fn strings(t: &dyn TableLike, field: &str, what: &str) -> Result<&'static [&'static str], String> {
    let Some(item) = t.get(field) else { return Ok(&[]) };
    let fail = || format!("{what}: `{field}` is a list of strings");
    let array = item.as_array().ok_or_else(fail)?;
    Ok(leak_slice(array.iter().map(|v| v.as_str().map(leak).ok_or_else(fail)).collect::<Result<Vec<_>, _>>()?))
}

fn integers(item: &Item, field: &str, what: &str) -> Result<Vec<i64>, String> {
    let fail = || format!("{what}: `{field}` is a list of integers");
    item.as_array().ok_or_else(fail)?.iter().map(|v| v.as_integer().ok_or_else(fail)).collect()
}

fn int(t: &dyn TableLike, field: &str, what: &str) -> Result<Option<i64>, String> {
    t.get(field).map(|i| i.as_integer().ok_or_else(|| format!("{what}: `{field}` is an integer"))).transpose()
}

fn u32_or(t: &dyn TableLike, field: &str, what: &str, default: Option<u32>) -> Result<u32, String> {
    match int(t, field, what)? {
        Some(v) => u32::try_from(v).map_err(|_| format!("{what}: `{field}` is not a u32")),
        None => default.ok_or_else(|| format!("{what} needs `{field}`")),
    }
}

fn u32s<const N: usize>(t: &dyn TableLike, field: &str, what: &str) -> Result<[u32; N], String> {
    let item = t.get(field).ok_or_else(|| format!("{what} needs `{field}`"))?;
    let values = integers(item, field, what)?;
    let values: Vec<u32> = values.into_iter().map(u32::try_from).collect::<Result<_, _>>().map_err(|_| format!("{what}: `{field}` holds u32s"))?;
    values.try_into().map_err(|_| format!("{what}: `{field}` has {N} numbers"))
}

fn tables<'a>(t: &'a dyn TableLike, field: &str) -> Result<Vec<&'a toml_edit::Table>, String> {
    match t.get(field) {
        None => Ok(Vec::new()),
        Some(item) => Ok(item.as_array_of_tables().ok_or_else(|| format!("`{field}` is a list of `[[{field}]]` tables"))?.iter().collect()),
    }
}

fn table<'a>(t: &'a dyn TableLike, field: &str) -> Result<Option<&'a dyn TableLike>, String> {
    t.get(field).map(|i| i.as_table_like().ok_or_else(|| format!("`{field}` is a table"))).transpose()
}

fn hook(hooks: &Hooks, name: &str, what: &str) -> Result<ShadingHook, String> {
    hooks.iter().find(|(n, _)| *n == name).map(|(_, h)| *h).ok_or_else(|| {
        let names: Vec<&str> = hooks.iter().map(|(n, _)| *n).collect();
        format!("{what}: no hook `{name}` (its investigation registers {})", crate::story_refs::names_or_none(&names))
    })
}

/// WebGPU's name of each format a shading may declare.
const FORMATS: &[(&str, wgpu::TextureFormat)] = &[
    ("r8unorm", wgpu::TextureFormat::R8Unorm),
    ("r8uint", wgpu::TextureFormat::R8Uint),
    ("r16uint", wgpu::TextureFormat::R16Uint),
    ("r16sint", wgpu::TextureFormat::R16Sint),
    ("r16float", wgpu::TextureFormat::R16Float),
    ("r32float", wgpu::TextureFormat::R32Float),
    ("r32uint", wgpu::TextureFormat::R32Uint),
    ("r32sint", wgpu::TextureFormat::R32Sint),
    ("rg8unorm", wgpu::TextureFormat::Rg8Unorm),
    ("rg8uint", wgpu::TextureFormat::Rg8Uint),
    ("rg16float", wgpu::TextureFormat::Rg16Float),
    ("rg32float", wgpu::TextureFormat::Rg32Float),
    ("rg32uint", wgpu::TextureFormat::Rg32Uint),
    ("rgba8unorm", wgpu::TextureFormat::Rgba8Unorm),
    ("rgba8unorm-srgb", wgpu::TextureFormat::Rgba8UnormSrgb),
    ("rgba8uint", wgpu::TextureFormat::Rgba8Uint),
    ("bgra8unorm", wgpu::TextureFormat::Bgra8Unorm),
    ("rgb10a2unorm", wgpu::TextureFormat::Rgb10a2Unorm),
    ("rg11b10ufloat", wgpu::TextureFormat::Rg11b10Ufloat),
    ("rgb9e5ufloat", wgpu::TextureFormat::Rgb9e5Ufloat),
    ("rgba16uint", wgpu::TextureFormat::Rgba16Uint),
    ("rgba16sint", wgpu::TextureFormat::Rgba16Sint),
    ("rgba16float", wgpu::TextureFormat::Rgba16Float),
    ("rgba32float", wgpu::TextureFormat::Rgba32Float),
    ("rgba32uint", wgpu::TextureFormat::Rgba32Uint),
    ("rgba32sint", wgpu::TextureFormat::Rgba32Sint),
    ("depth32float", wgpu::TextureFormat::Depth32Float),
];

fn format(t: &dyn TableLike, what: &str) -> Result<wgpu::TextureFormat, String> {
    let name = required(t, "format", what)?;
    FORMATS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f).ok_or_else(|| format!("{what}: format `{name}` is not one of {}", FORMATS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")))
}

const ACCESSES: &[(&str, Access)] = &[
    ("sampled", Access::Sampled),
    ("load", Access::Load),
    ("read", Access::Read),
    ("write", Access::Write),
    ("read-write", Access::ReadWrite),
    ("atomic", Access::Atomic),
    ("uniform", Access::Uniform),
    ("sampler", Access::Sampler),
];
const KINDS: &[(&str, ImageKind)] = &[("2d", ImageKind::D2), ("2d-array", ImageKind::D2Array), ("3d", ImageKind::D3)];

fn named<T: Copy>(table: &[(&str, T)], name: &str, field: &str, what: &str) -> Result<T, String> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v).ok_or_else(|| format!("{what}: `{field}` `{name}` is not one of {}", table.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")))
}

fn program_use(t: &dyn TableLike, what: &str) -> Result<ProgramUse, String> {
    known(t, what, &["binding", "resource", "access", "which", "fixed", "pick", "mip"])?;
    let resource = required(t, "resource", what)?;
    let what = &format!("{what} of `{resource}`");
    let mut choices = Vec::new();
    if let Some(which) = string(t, "which", what)? {
        choices.push(named(&[("all", Which::All), ("instance", Which::Instance)], which, "which", what)?);
    }
    if t.contains_key("fixed") {
        choices.push(Which::Fixed(u32_or(t, "fixed", what, None)?));
    }
    if let Some(item) = t.get("pick") {
        let pick = integers(item, "pick", what)?.into_iter().map(u32::try_from).collect::<Result<Vec<_>, _>>().map_err(|_| format!("{what}: `pick` holds u32s"))?;
        choices.push(Which::Pick(leak_slice(pick)));
    }
    if let Some(mip) = int(t, "mip", what)? {
        choices.push(Which::Mip(i32::try_from(mip).map_err(|_| format!("{what}: `mip` is not an i32"))?));
    }
    if choices.len() > 1 {
        return Err(format!("{what}: one of `which`, `fixed`, `pick`, `mip`"));
    }
    Ok(ProgramUse {
        binding: u32_or(t, "binding", what, None)?,
        resource,
        access: named(ACCESSES, required(t, "access", what)?, "access", what)?,
        which: choices.pop().unwrap_or(Which::First),
    })
}

fn uses(t: &dyn TableLike, field: &str, what: &str) -> Result<&'static [ProgramUse], String> {
    let Some(item) = t.get(field) else { return Ok(&[]) };
    let fail = || format!("{what}: `{field}` is a list of inline tables");
    let array = item.as_array().ok_or_else(fail)?;
    let uses = array.iter().map(|v| program_use(v.as_inline_table().ok_or_else(fail)?, &format!("{what}: a use"))).collect::<Result<Vec<_>, _>>()?;
    Ok(leak_slice(uses))
}

fn program_pass(t: &dyn TableLike, what: &str) -> Result<ProgramPass, String> {
    known(t, what, &["kind", "name", "compute", "workgroup", "vertex", "fragment", "instances", "uses"])?;
    let name = required(t, "name", what)?;
    let what = &format!("program pass `{name}`");
    let kind = match (string(t, "compute", what)?, string(t, "vertex", what)?, string(t, "fragment", what)?) {
        (Some(entry), None, None) => PassKind::Compute { entry, workgroup: u32s(t, "workgroup", what)? },
        (None, Some(vertex), Some(fragment)) if !t.contains_key("workgroup") => PassKind::Raster { vertex, fragment },
        _ => return Err(format!("{what}: `compute` and `workgroup`, or `vertex` and `fragment`")),
    };
    Ok(ProgramPass { name, kind, uses: uses(t, "uses", what)?, instances: u32_or(t, "instances", what, Some(1))? })
}

fn program(t: &dyn TableLike, hooks: &Hooks, passes: Vec<ProgramPass>) -> Result<ShadingProgram, String> {
    known(t, "[program]", &["plan", "warmup", "record-history", "scene-uses", "image", "buffer"])?;
    let plan_name = required(t, "plan", "[program]")?;
    let ShadingHook::Program { init, plan } = hook(hooks, plan_name, "[program] plan")? else { return Err(format!("[program]: hook `{plan_name}` is not a Program")) };
    let mut images = Vec::new();
    for i in tables(t, "image")? {
        known(i, "[[program.image]]", &["name", "kind", "size", "format", "count", "mips", "viewport"])?;
        let name = required(i, "name", "[[program.image]]")?;
        let what = &format!("image `{name}`");
        let mips = match i.get("mips") {
            Some(m) if m.as_str() == Some("all") => u32::MAX,
            _ => u32_or(i, "mips", what, Some(1))?,
        };
        images.push(ProgramImage {
            name,
            kind: named(KINDS, required(i, "kind", what)?, "kind", what)?,
            size: u32s(i, "size", what)?,
            format: format(i, what)?,
            count: u32_or(i, "count", what, Some(1))?,
            mips,
            viewport: u32_or(i, "viewport", what, Some(0))?,
        });
    }
    let mut buffers = Vec::new();
    for b in tables(t, "buffer")? {
        known(b, "[[program.buffer]]", &["name", "bytes", "indirect", "count"])?;
        let name = required(b, "name", "[[program.buffer]]")?;
        let what = &format!("buffer `{name}`");
        let bytes = int(b, "bytes", what)?.and_then(|v| u64::try_from(v).ok()).ok_or_else(|| format!("{what} needs `bytes`, a positive integer"))?;
        let indirect = b.get("indirect").map(|v| v.as_bool().ok_or_else(|| format!("{what}: `indirect` is true or false"))).transpose()?.unwrap_or(false);
        buffers.push(ProgramBuffer { name, bytes, indirect, count: u32_or(b, "count", what, Some(1))? });
    }
    Ok(ShadingProgram {
        images: leak_slice(images),
        buffers: leak_slice(buffers),
        passes: leak_slice(passes),
        scene_uses: uses(t, "scene-uses", "[program]")?,
        init,
        plan,
        warmup: u32_or(t, "warmup", "[program]", Some(0))?,
        record_history: t.get("record-history").map(|v| v.as_bool().ok_or_else(|| "[program]: `record-history` is true or false".to_string())).transpose()?.unwrap_or(false),
    })
}

/// What the `[[node]]` list declares, gathered by kind.
#[derive(Default)]
struct Nodes {
    order: Vec<FrameNode>,
    updates: Vec<UpdatePass>,
    depth_views: Option<&'static DepthViews>,
    prepass: Option<(&'static str, &'static [&'static str])>,
    passes: Vec<ShadingPass>,
    programs: Vec<ProgramPass>,
    scene: Option<(&'static str, &'static [&'static str], &'static [&'static str])>,
}

fn nodes(t: &dyn TableLike) -> Result<Nodes, String> {
    let mut n = Nodes::default();
    for (k, x) in tables(t, "node")?.into_iter().enumerate() {
        let what = &format!("node {}", k + 1);
        let kind = required(x, "kind", what)?;
        match kind {
            "update" => {
                known(x, what, &["kind", "entry", "reads", "writes", "workgroup"])?;
                let entry = required(x, "entry", what)?;
                let what = &format!("update pass `{entry}`");
                n.order.push(FrameNode::Update(n.updates.len()));
                n.updates.push(UpdatePass { entry, reads: strings(x, "reads", what)?, writes: strings(x, "writes", what)?, workgroup: u32_or(x, "workgroup", what, None)? });
            }
            "depth-views" => {
                known(x, what, &["kind", "name", "size", "vertices", "bias"])?;
                if n.depth_views.is_some() {
                    return Err(format!("{what}: a second depth-views node"));
                }
                let bias = x.get("bias").ok_or_else(|| format!("{what} needs `bias`"))?;
                let bias = bias.as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("{what}: `bias` is [constant, slope]"))?;
                let constant = bias.get(0).and_then(|v| v.as_integer()).and_then(|v| i32::try_from(v).ok()).ok_or_else(|| format!("{what}: the bias's constant is an integer"))?;
                let slope = bias.get(1).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).ok_or_else(|| format!("{what}: the bias's slope is a number"))? as f32;
                n.depth_views = Some(Box::leak(Box::new(DepthViews { name: required(x, "name", what)?, size: u32_or(x, "size", what, None)?, vertices: strings(x, "vertices", what)?, bias: (constant, slope) })));
                n.order.push(FrameNode::DepthViews);
            }
            "prepass" => {
                known(x, what, &["kind", "entry", "writes"])?;
                if n.prepass.is_some() {
                    return Err(format!("{what}: a second prepass node"));
                }
                n.prepass = Some((required(x, "entry", what)?, strings(x, "writes", what)?));
                n.order.push(FrameNode::Prepass);
            }
            "compute" => {
                known(x, what, &["kind", "entry", "reads", "writes", "workgroup"])?;
                let entry = required(x, "entry", what)?;
                let what = &format!("compute pass `{entry}`");
                let pass = ShadingPass { entry, reads: strings(x, "reads", what)?, writes: strings(x, "writes", what)?, workgroup: u32s(x, "workgroup", what)? };
                n.order.push(FrameNode::Compute(n.passes.len()));
                n.passes.push(pass);
            }
            "program" => {
                n.order.push(FrameNode::Program(n.programs.len()));
                n.programs.push(program_pass(x, what)?);
            }
            "scene" => {
                known(x, what, &["kind", "entry", "outputs", "reads"])?;
                if n.scene.is_some() {
                    return Err(format!("{what}: a second scene node"));
                }
                n.scene = Some((required(x, "entry", what)?, strings(x, "outputs", what)?, strings(x, "reads", what)?));
                n.order.push(FrameNode::Scene);
            }
            other => return Err(format!("{what}: kind `{other}` is not one of update, depth-views, prepass, compute, program, scene")),
        }
    }
    Ok(n)
}

/// Reads a shading file, its hooks looked up in `hooks`.
pub fn parse(text: &str, hooks: &Hooks) -> Result<SceneShading, String> {
    let document = toml_edit::Document::parse(text.to_string()).map_err(|e| format!("parse: {e}"))?;
    let t: &dyn TableLike = document.as_table();
    known(t, "a shading file", &["module", "data", "verify", "history", "target", "buffer", "volume", "resolution", "update", "shows", "program", "node"])?;
    let data = match string(t, "data", "the file")? {
        None => Vec::new as fn() -> Vec<[f32; 4]>,
        Some(name) => match hook(hooks, name, "data")? {
            ShadingHook::Data(f) => f,
            _ => return Err(format!("data: hook `{name}` is not a Data")),
        },
    };
    let mut targets = Vec::new();
    for x in tables(t, "target")? {
        known(x, "[[target]]", &["name", "format", "divisor"])?;
        let name = required(x, "name", "[[target]]")?;
        let what = &format!("target `{name}`");
        targets.push(ShadingTarget { name, format: format(x, what)?, divisor: u32_or(x, "divisor", what, Some(1))? });
    }
    let mut buffers = Vec::new();
    for x in tables(t, "buffer")? {
        known(x, "[[buffer]]", &["name", "words-per-tile", "tile"])?;
        let name = required(x, "name", "[[buffer]]")?;
        let what = &format!("buffer `{name}`");
        buffers.push(ShadingBuffer { name, words_per_tile: u32_or(x, "words-per-tile", what, None)?, tile: u32_or(x, "tile", what, None)? });
    }
    let mut volumes = Vec::new();
    for x in tables(t, "volume")? {
        known(x, "[[volume]]", &["name", "format", "size", "mips"])?;
        let name = required(x, "name", "[[volume]]")?;
        let what = &format!("volume `{name}`");
        volumes.push(ShadingVolume { name, format: format(x, what)?, size: u32s(x, "size", what)?, mips: u32_or(x, "mips", what, Some(1))? });
    }
    let n = nodes(t)?;
    let (fragment, outputs, scene_reads) = n.scene.ok_or("the frame needs a `scene` node")?;
    let resolution = table(t, "resolution")?
        .map(|x| -> Result<&'static ShadingResolution, String> {
            let what = "[resolution]";
            known(x, what, &["targets", "divisor"])?;
            let name = required(x, "divisor", what)?;
            let ShadingHook::Divisor(divisor) = hook(hooks, name, "[resolution] divisor")? else { return Err(format!("{what}: hook `{name}` is not a Divisor")) };
            Ok(Box::leak(Box::new(ShadingResolution { targets: strings(x, "targets", what)?, divisor })))
        })
        .transpose()?;
    let update = match (table(t, "update")?, n.updates.is_empty()) {
        (None, true) => None,
        (None, false) => return Err("update nodes need an `[update]` table (its plan)".into()),
        (Some(_), true) => return Err("`[update]` without update nodes".into()),
        (Some(x), false) => {
            let what = "[update]";
            known(x, what, &["plan", "buffer"])?;
            let name = required(x, "plan", what)?;
            let ShadingHook::UpdatePlan(plan) = hook(hooks, name, "[update] plan")? else { return Err(format!("{what}: hook `{name}` is not an UpdatePlan")) };
            let mut buffers = Vec::new();
            for b in tables(x, "buffer")? {
                known(b, "[[update.buffer]]", &["name", "words"])?;
                let name = required(b, "name", "[[update.buffer]]")?;
                let words = required(b, "words", &format!("update buffer `{name}`"))?;
                let ShadingHook::Words(words) = hook(hooks, words, &format!("update buffer `{name}`"))? else { return Err(format!("update buffer `{name}`: hook `{words}` is not a Words")) };
                buffers.push(UpdateBuffer { name, words });
            }
            Some(&*Box::leak(Box::new(ShadingUpdate { passes: leak_slice(n.updates), buffers: leak_slice(buffers), plan })))
        }
    };
    let shows = match table(t, "shows")? {
        None => &[][..],
        Some(x) => leak_slice(x.iter().map(|(k, v)| v.as_str().map(|s| (leak(k), leak(s))).ok_or_else(|| format!("[shows]: `{k}` is a string"))).collect::<Result<Vec<_>, _>>()?),
    };
    let program = match (table(t, "program")?, n.programs.is_empty()) {
        (None, true) => None,
        (None, false) => return Err("program nodes need a `[program]` table (its plan)".into()),
        (Some(x), _) => Some(&*Box::leak(Box::new(program(x, hooks, n.programs)?))),
    };
    Ok(SceneShading {
        module: required(t, "module", "the file")?,
        fragment,
        data,
        verify: strings(t, "verify", "the file")?,
        targets: leak_slice(targets),
        outputs,
        passes: leak_slice(n.passes),
        prepass: n.prepass.map(|(entry, _)| entry),
        prepass_outputs: n.prepass.map_or(&[][..], |(_, writes)| writes),
        buffers: leak_slice(buffers),
        scene_reads,
        depth_views: n.depth_views,
        volumes: leak_slice(volumes),
        update,
        resolution,
        history: strings(t, "history", "the file")?,
        shows,
        program,
        nodes: leak_slice(n.order),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investigation::{ProgramCommand, ProgramFrame};

    fn data() -> Vec<[f32; 4]> {
        vec![[1.0; 4]]
    }
    fn init() -> Box<dyn std::any::Any + Send> {
        Box::new(())
    }
    fn plan(_: &mut dyn std::any::Any, _: &ProgramFrame) -> Vec<ProgramCommand> {
        Vec::new()
    }
    static HOOKS: &Hooks = &[("data", ShadingHook::Data(data)), ("plan", ShadingHook::Program { init, plan })];

    const FILE: &str = r#"
module = "p.shader/s"
data = "data"

[[target]]
name = "color"
format = "rgba16float"

[[target]]
name = "half"
format = "r8unorm"
divisor = 2

[[node]]
kind = "program"
name = "fill"
compute = "fillCS"
workgroup = [8, 8, 1]
instances = 2
uses = [
    { binding = 1, resource = "img", access = "write", mip = 0 },
    { binding = 2, resource = "img", access = "load", pick = [1, 0] },
    { binding = 3, resource = "img", access = "sampled", which = "all" },
]

[[node]]
kind = "depth-views"
name = "shadow"
size = 512
vertices = ["shadowVS"]
bias = [-2, -1.5]

[[node]]
kind = "prepass"
entry = "prepassFS"

[[node]]
kind = "compute"
entry = "tilesCS"
reads = ["depth"]
writes = ["half"]
workgroup = [16, 16]

[[node]]
kind = "scene"
entry = "fs"
outputs = ["color"]
reads = ["half"]

[[node]]
kind = "compute"
entry = "blurCS"
reads = ["color"]
writes = ["hdr"]
workgroup = [8, 8]

[shows]
half = "ramp?range=0,1"

[program]
plan = "plan"
warmup = 2
record-history = true
scene-uses = [{ binding = 1, resource = "img", access = "load" }]

[[program.image]]
name = "img"
kind = "2d-array"
size = [4, 4, 3]
format = "rgba16sint"
count = 2
mips = "all"
"#;

    #[test]
    fn reads_the_frame_in_order() {
        let s = parse(FILE, HOOKS).unwrap();
        assert_eq!((s.module, s.fragment, s.outputs, s.scene_reads), ("p.shader/s", "fs", &["color"][..], &["half"][..]));
        assert_eq!((s.data)(), vec![[1.0; 4]]);
        assert_eq!((s.targets[1].format, s.targets[1].divisor), (wgpu::TextureFormat::R8Unorm, 2));
        assert_eq!(s.nodes, &[FrameNode::Program(0), FrameNode::DepthViews, FrameNode::Prepass, FrameNode::Compute(0), FrameNode::Scene, FrameNode::Compute(1)]);
        assert_eq!((s.passes[0].entry, s.passes[1].entry, s.passes[1].workgroup), ("tilesCS", "blurCS", [8, 8]));
        assert_eq!((s.prepass, s.prepass_outputs), (Some("prepassFS"), &[][..]));
        let d = s.depth_views.unwrap();
        assert_eq!((d.name, d.size, d.bias), ("shadow", 512, (-2, -1.5)));
        assert_eq!(s.shows, &[("half", "ramp?range=0,1")]);
        let p = s.program.unwrap();
        assert_eq!((p.warmup, p.record_history), (2, true));
        assert_eq!((p.images[0].kind, p.images[0].count, p.images[0].mips, p.images[0].viewport), (ImageKind::D2Array, 2, u32::MAX, 0));
        assert_eq!((p.passes[0].name, p.passes[0].instances), ("fill", 2));
        assert_eq!(p.passes[0].uses.iter().map(|u| u.which).collect::<Vec<_>>(), vec![Which::Mip(0), Which::Pick(&[1, 0]), Which::All]);
        assert_eq!(p.scene_uses[0].which, Which::First);
        assert_eq!(s.program_runs(), vec![0]);
        assert!(s.update.is_none() && s.resolution.is_none());
    }

    #[test]
    fn refuses_what_it_does_not_know() {
        let fails = |text: &str, wanted: &str| {
            let e = parse(text, HOOKS).err().unwrap_or_else(|| panic!("`{text}` read"));
            assert!(e.contains(wanted), "`{e}` does not say `{wanted}`");
        };
        let scene = "\n[[node]]\nkind = \"scene\"\nentry = \"fs\"\n";
        fails(&format!("module = \"m\"\ncolour = 1{scene}"), "no field `colour`");
        fails(&format!("module = \"m\"\ndata = \"nothing\"{scene}"), "no hook `nothing`");
        fails(&format!("module = \"m\"\ndata = \"plan\"{scene}"), "is not a Data");
        fails(&format!("module = \"m\"\n[[target]]\nname = \"t\"\nformat = \"rgb8\"{scene}"), "format `rgb8`");
        fails(scene, "needs `module`");
        fails("module = \"m\"", "needs a `scene` node");
        fails(&format!("module = \"m\"{scene}{scene}"), "a second scene node");
        fails(&format!("module = \"m\"{scene}[[node]]\nkind = \"raster\""), "kind `raster`");
        fails(&format!("module = \"m\"{scene}[[node]]\nkind = \"program\"\nname = \"p\"\ncompute = \"c\"\nworkgroup = [1, 1, 1]"), "need a `[program]` table");
        fails(
            &format!("module = \"m\"{scene}[[node]]\nkind = \"program\"\nname = \"p\"\ncompute = \"c\"\nworkgroup = [1, 1, 1]\nuses = [{{ binding = 1, resource = \"r\", access = \"load\", fixed = 1, mip = 0 }}]\n[program]\nplan = \"plan\""),
            "one of `which`, `fixed`, `pick`, `mip`",
        );
    }
}
