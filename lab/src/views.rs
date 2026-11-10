//! Image kinds: GPU images a package's shaders define, which a story creates
//! instances of and drives (`{inset "floor"} {imageFrom "probeMap"} {imageProbe
//! {' 7.0 7.0 8.0}}`, src/shot/). The shader defines what is computed; the story says how
//! many, with what parameters, when, and where. Every frame the renderer runs
//! each live instance's compute entry point into the instance's own texture
//! and shows it live in an inset (src/renderer/, packages/render_lab/src/inset.rs).
//!
//! A kind is a storage texture its module tags, and the uniform of its
//! parameters (its members tagged as a shader's values are, without `label`
//! or `text`):
//!
//! ```wgsl
//! /// The followed probe's two octahedral maps, side by side.
//! /// {@ view=probeMap entry=sdfgiProbeMapView size=240,120 text=demos-probe_maps_gpu}
//! @group(3) @binding(0) var gProbeMapOut: texture_storage_2d<rgba8unorm, write>;
//! struct ProbeMapParams {
//!     /// {@ key=probe default=7,7,8 range=0,16}
//!     probe: vec3<f32>,
//!     /// {@ key=cascade default=0 range=0,3}
//!     cascade: u32,
//! }
//! /// {@ view_params=probeMap}
//! @group(3) @binding(1) var<uniform> gProbeMap: ProbeMapParams;
//!
//! @compute @workgroup_size(8, 8)
//! fn sdfgiProbeMapView(@builtin(global_invocation_id) id: vec3<u32>) { … }
//! ```
//!
//! The entry point runs with the scene's frame (group 0) and the
//! investigation's scene shading (group 1, its data) bound as its other
//! passes are, over one invocation per texel of `size`. It writes display
//! colours (sRGB-encoded), shown as they are, as a CPU picture's pixels are. A kind's key is
//! unique in its package; `text=` (a key of the package's text) is its title.
//!
//! A kind may also read the investigation's pass chain (`reads=diffuse,normal`:
//! its targets, `depth`, `hdr`, its buffers, or its depth views by name). The
//! module then binds those at group 2 exactly as a chain pass does
//! (`gpu::chain_binding`, src/investigation.rs `ShadingPass`), read-only, and
//! the renderer runs the kind after the chain, so it shows this frame's data.
//! A kind without `reads` runs before the scene, with group 2 unbound.
//!
//! A kind that pictures the screen takes the viewport's shape
//! (`aspect=viewport`): `size` is then its landscape size, whose longer side
//! its texture keeps, the other side following the viewport's aspect; the
//! texture is made again when that changes (`ViewKind::texels`).
//! Naga reads a declaration's `///` lines only when no `//` comment comes
//! between them and the declaration before it, so a file's `//` header hides
//! the tags of its first declaration: write it as part of the `///` block.

use crate::resources;
use crate::shader_data::{Storage, number, parse_tags, wgsl_value_type};
use crate::shaders;
use anyhow::{Result, anyhow, bail};
use naga::{ImageClass, ImageDimension, ShaderStage, StorageFormat, TypeInner};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The binding group of a kind's output and parameters.
pub const VIEW_GROUP: u32 = 3;

#[derive(Debug)]
pub struct ViewKind {
    /// Its key in a story (`from=<key>`).
    pub key: String,
    pub package: String,
    /// The shader module that defines it, and its compute entry point.
    pub module: String,
    pub entry: String,
    /// The entry point's `@workgroup_size`.
    pub workgroup: [u32; 3],
    /// The pass-chain targets, buffers, or depth views it reads at group 2
    /// (`reads=`), in order; empty for none.
    pub reads: Vec<String>,
    /// Its texture's size in texels (with `viewport_aspect`, at a landscape
    /// viewport of the same shape; `texels`).
    pub size: [u32; 2],
    /// Its texture takes the viewport's shape (`aspect=viewport`).
    pub viewport_aspect: bool,
    /// Its title: a key of its package's text.
    pub text: Option<String>,
    pub doc: String,
    /// Its parameters, with their uniform's size in bytes (none: no uniform).
    pub params: Vec<ViewParam>,
    pub params_size: u32,
}

#[derive(Debug)]
pub struct ViewParam {
    /// Its key in a story (`image.<key>=…`).
    pub key: String,
    pub offset: u32,
    pub storage: Storage,
    pub components: usize,
    pub default: Vec<f32>,
    pub range: Option<(f32, f32)>,
    pub doc: String,
}

impl ViewKind {
    /// Its texture's size in texels at a viewport of `viewport` pixels: its
    /// `size`, or with `viewport_aspect` that size's longer side along the
    /// viewport's longer one and the other side in the viewport's aspect.
    pub fn texels(&self, viewport: [u32; 2]) -> [u32; 2] {
        if !self.viewport_aspect || viewport[0] == 0 || viewport[1] == 0 {
            return self.size;
        }
        let long = self.size[0].max(self.size[1]) as f32;
        let aspect = viewport[0] as f32 / viewport[1] as f32;
        let side = |s: f32| (s.round() as u32).clamp(1, 2048);
        if aspect >= 1.0 {
            [side(long), side(long / aspect)]
        } else {
            [side(long * aspect), side(long)]
        }
    }
    pub fn param(&self, key: &str) -> Option<&ViewParam> {
        self.params.iter().find(|p| p.key == key)
    }

    /// Why `value` cannot be parameter `key`: one it does not take, or the
    /// wrong number of components.
    pub fn check(&self, key: &str, value: &[f32]) -> Result<(), String> {
        let param = self.param(key).ok_or_else(|| {
            format!(
                "the image kind `{}` takes no parameter `{key}` (its parameters: {})",
                self.key,
                self.params
                    .iter()
                    .map(|p| p.key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        if value.len() != param.components {
            return Err(format!(
                "the image kind `{}`'s parameter `{key}` is {} number(s), not {}",
                self.key,
                param.components,
                value.len()
            ));
        }
        Ok(())
    }

    /// Its parameters' uniform: each parameter's value in `values`, or its
    /// default, clamped to its range.
    pub fn pack(&self, values: &crate::utility::inset::ImageParams) -> Vec<u8> {
        let mut bytes = vec![0u8; self.params_size.max(16) as usize];
        for p in &self.params {
            let value = values.get(&p.key).unwrap_or(&p.default);
            for (k, &v) in value.iter().enumerate().take(p.components) {
                let v = p.range.map_or(v, |(lo, hi)| v.clamp(lo, hi));
                let word = match p.storage {
                    Storage::F32 => v.to_bits(),
                    Storage::U32 => v.round().max(0.0) as u32,
                    Storage::I32 => v.round() as i32 as u32,
                };
                let at = (p.offset as usize) + 4 * k;
                bytes[at..at + 4].copy_from_slice(&word.to_le_bytes());
            }
        }
        bytes
    }
}

/// The image kinds `package`'s shaders define, read once.
pub fn kinds(package: &str) -> Result<&'static [ViewKind]> {
    static KINDS: Mutex<BTreeMap<String, &'static [ViewKind]>> = Mutex::new(BTreeMap::new());
    if let Some(k) = KINDS.lock().unwrap().get(package) {
        return Ok(k);
    }
    let mut out: Vec<ViewKind> = Vec::new();
    for shader in resources::shaders().iter().filter(|s| {
        s.id.strip_prefix(package)
            .is_some_and(|r| r.starts_with('.'))
    }) {
        for kind in read(&shader.id, package)? {
            if let Some(other) = out.iter().find(|k| k.key == kind.key) {
                bail!(
                    "{}: the image kind `{}` is defined twice ({} and {})",
                    package,
                    kind.key,
                    other.module,
                    kind.module
                );
            }
            out.push(kind);
        }
    }
    let leaked: &'static [ViewKind] = Box::leak(out.into_boxed_slice());
    KINDS.lock().unwrap().insert(package.to_string(), leaked);
    Ok(leaked)
}

/// `package`'s image kind `key`.
pub fn kind(package: &str, key: &str) -> Result<&'static ViewKind, String> {
    let kinds = kinds(package).map_err(|e| format!("{e:#}"))?;
    kinds.iter().find(|k| k.key == key).ok_or_else(|| {
        let known: Vec<&str> = kinds.iter().map(|k| k.key.as_str()).collect();
        format!(
            "no image kind `{key}` (the package's: {})",
            crate::story_refs::names_or_none(&known)
        )
    })
}

/// The kinds module `id` defines.
fn read(id: &str, package: &str) -> Result<Vec<ViewKind>> {
    let r = shaders::reflect(id)?;
    let module = &r.module;
    let Some(docs) = module.doc_comments.as_deref() else {
        return Ok(Vec::new());
    };
    let mut kinds = Vec::new();
    let mut params_of: BTreeMap<String, (naga::Handle<naga::GlobalVariable>, String)> =
        BTreeMap::new();
    for (handle, lines) in &docs.global_variables {
        let (tags, doc) = parse_tags(lines)?;
        let var = &module.global_variables[*handle];
        let name = var.name.clone().unwrap_or_default();
        let at = r.locate(&lines[0]);
        let arg = |tag: &str| tags.iter().find(|t| t.name == tag).map(|t| t.args.clone());
        let binding = var.binding.as_ref().map(|b| (b.group, b.binding));
        if let Some(key) = arg("view_params") {
            // A kind's parameters (`view_params=`) are bound at binding 1 of the view group, and join their kind below.
            let [key] = key.as_slice() else {
                bail!("{id}: `{name}`: `view_params=` names one kind{at}")
            };
            if binding != Some((VIEW_GROUP, 1)) {
                bail!(
                    "{id}: `{name}`, the parameters of `{key}`, is bound at @group({VIEW_GROUP}) @binding(1){at}"
                );
            }
            params_of.insert(key.clone(), (*handle, at));
            continue;
        }
        let Some(key) = arg("view") else { continue };
        let [key] = key.as_slice() else {
            bail!("{id}: `{name}`: `view=` names one kind{at}")
        };
        if let Some(t) = tags.iter().find(|t| {
            !["view", "entry", "size", "text", "reads", "aspect"].contains(&t.name.as_str())
        }) {
            bail!(
                "{id}: `{name}`: unknown attribute `{}` of an image kind (view, entry, size, text, reads, aspect){at}",
                t.name
            );
        }
        if binding != Some((VIEW_GROUP, 0)) {
            bail!(
                "{id}: `{name}`, the image kind `{key}`, is bound at @group({VIEW_GROUP}) @binding(0){at}"
            );
        }
        let (entry, workgroup, size, viewport_aspect) =
            kind_shape(module, var, &arg, (id, &name, key, &at))?;
        let text = arg("text").and_then(|t| t.first().cloned());
        let reads = arg("reads").unwrap_or_default();
        kinds.push(ViewKind {
            key: key.clone(),
            package: package.to_string(),
            module: id.to_string(),
            entry,
            workgroup,
            size,
            viewport_aspect,
            text,
            reads,
            doc,
            params: Vec::new(),
            params_size: 0,
        });
    }
    for (key, (handle, at)) in params_of {
        let Some(kind) = kinds.iter_mut().find(|k| k.key == key) else {
            bail!("{id}: parameters of `{key}`, which is not an image kind of the module{at}")
        };
        let var = &module.global_variables[handle];
        let global = var.name.clone().unwrap_or_default();
        let TypeInner::Struct { members, span } = &module.types[var.ty].inner else {
            bail!("{id}: `{global}` is not a struct{at}")
        };
        kind.params_size = *span;
        for (index, m) in members.iter().enumerate() {
            let member = m.name.clone().unwrap_or_default();
            let lines = docs
                .struct_members
                .get(&(var.ty, index))
                .cloned()
                .unwrap_or_default();
            let context = format!("{id}: `{global}.{member}`{at}");
            kind.params.push(read_param(module, &lines, m, &context)?);
        }
    }
    Ok(kinds)
}

/// An image kind's output (a texture_storage_2d<rgba8unorm, write>), its compute entry point and that point's
/// workgroup, its size in texels, and whether its texture takes the viewport's shape.
fn kind_shape(
    module: &naga::Module,
    var: &naga::GlobalVariable,
    arg: &dyn Fn(&str) -> Option<Vec<String>>,
    (id, name, key, at): (&str, &str, &str, &str),
) -> Result<(String, [u32; 3], [u32; 2], bool)> {
    let storage = matches!(
        module.types[var.ty].inner,
        TypeInner::Image {
            dim: ImageDimension::D2,
            arrayed: false,
            class: ImageClass::Storage {
                format: StorageFormat::Rgba8Unorm,
                ..
            }
        }
    );
    if !storage {
        bail!(
            "{id}: `{name}`, the image kind `{key}`, is a texture_storage_2d<rgba8unorm, write>{at}"
        );
    }
    let entry = arg("entry")
        .and_then(|e| e.first().cloned())
        .ok_or_else(|| {
            anyhow!("{id}: the image kind `{key}` names its compute entry point (`entry=`){at}")
        })?;
    let point = module
        .entry_points
        .iter()
        .find(|e| e.name == entry && e.stage == ShaderStage::Compute)
        .ok_or_else(|| {
            anyhow!("{id}: the image kind `{key}`: no compute entry point `{entry}`{at}")
        })?;
    let size = match arg("size").as_deref() {
        Some([w, h]) => [number(w)? as u32, number(h)? as u32],
        _ => {
            bail!("{id}: the image kind `{key}` names its size in texels (`size=width,height`){at}")
        }
    };
    if size[0] == 0 || size[1] == 0 || size[0] > 2048 || size[1] > 2048 {
        bail!(
            "{id}: the image kind `{key}` is {}x{} texels (1 to 2048 a side){at}",
            size[0],
            size[1]
        );
    }
    let viewport_aspect = match arg("aspect").as_deref() {
        None => false,
        Some([a]) if a == "viewport" => true,
        Some(_) => bail!(
            "{id}: the image kind `{key}`: `aspect=` takes `viewport` (its texture takes the viewport's shape){at}"
        ),
    };
    Ok((entry, point.workgroup_size, size, viewport_aspect))
}

/// An image kind's parameter (a member of its `view_params` struct): its key (the member's name unless
/// `key=` gives one), its type, its default, and its range.
fn read_param(
    module: &naga::Module,
    lines: &[String],
    m: &naga::StructMember,
    context: &str,
) -> Result<ViewParam> {
    let member = m.name.clone().unwrap_or_default();
    let (tags, doc) = parse_tags(lines)?;
    if let Some(t) = tags
        .iter()
        .find(|t| !["key", "default", "range"].contains(&t.name.as_str()))
    {
        bail!(
            "{context}: unknown attribute `{}` of an image kind's parameter (key, default, range)",
            t.name
        );
    }
    let (storage, components) = wgsl_value_type(&module.types[m.ty].inner)
        .ok_or_else(|| anyhow!("{context}: a parameter is an f32, u32, i32, or vecN<f32>"))?;
    let key = tags
        .iter()
        .find(|t| t.name == "key")
        .and_then(|t| t.args.first().cloned())
        .unwrap_or(member);
    let default: Vec<f32> = match tags.iter().find(|t| t.name == "default") {
        Some(t) if t.args.len() == components => {
            t.args.iter().map(|a| number(a)).collect::<Result<_>>()?
        }
        Some(t) => bail!(
            "{context}: `default=` has {} number(s), the parameter {components}",
            t.args.len()
        ),
        None => bail!("{context}: a parameter has a `default=`"),
    };
    let range = match tags.iter().find(|t| t.name == "range") {
        Some(t) if t.args.len() == 2 => Some((number(&t.args[0])?, number(&t.args[1])?)),
        Some(_) => bail!("{context}: `range=low,high`"),
        None => None,
    };
    Ok(ViewParam {
        key,
        offset: m.offset,
        storage,
        components,
        default,
        range,
        doc,
    })
}
