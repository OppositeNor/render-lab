//! Shader values: the members of a shader's uniform struct that tags in its
//! `///` comments declare, which the lab reflects (naga, src/shaders.rs)
//! into values by key (src/values.rs) and packs into the shader's buffer.
//! A tag is written as a story's are, `{@ name=value …}`, and read by the
//! same parser (src/rich_text.rs); a value of several numbers or names is
//! comma-separated, as in a story (`camera.target=0,1,2`).
//!
//! ```wgsl
//! struct Shading {
//!     /// The light's intensity in candelas.
//!     /// {@ key=lightIntensity default=6 range=0,30 label=light text=controls.intensity}
//!     intensity: f32,
//!     /// {@ derived}
//!     invRadiusSquared: f32,
//! }
//! /// {@ shader_data}
//! @group(1) @binding(0) var<uniform> gShading: Shading;
//! ```
//!
//! On the global: `shader_data`, with `scope=name` to prefix its keys
//! (`name.key`). On every member, one of:
//!
//! - `key=name`: a value. `default=` (one number per component;
//!   `true`/`false` for a toggle) is required; `range=low,high` (or a pair
//!   per component), `step=`, `unit=m|deg|factor|plain|index`, `wrap`,
//!   `label=a,b`, `text=` (a key of the package's text), `toggle` (on or
//!   off, packed as 1 or 0), and, for a vector, `components=x,y,z` (its
//!   components' keys; `name.x`, … by default) are optional. An `index` is a
//!   whole number, clamped to its range when packed. `choice=Name` makes it
//!   a choice of the options of the investigation's `#[derive(Choice)]` enum
//!   `Name` (its `default=` an option's name, its range their indices); the
//!   shader must compose the enum's constants, which the lab writes into
//!   the package's `build/` (`write_choices`). A value is a scalar (`f32`,
//!   `u32`, `i32`) or a `vecN<f32>`.
//! - `derived`: a member the investigation fills from the values
//!   (`Investigation::shader_data`), such as an inverse or a product. It is
//!   not a value.
//!
//! The rest of a comment is the value's `doc`. An unknown attribute, a member
//! with neither, or a value without a default is an error naming its file and
//! line. The package's parameters (src/demo.rs) include its shaders' values,
//! so its demonstrations, timelines, and stories set them by key.

use crate::frame::{FrameContent, Statics, SHADING_UNIFORMS};
use crate::investigation;
use crate::shaders::{self, Reflected};
use crate::shot::ShotState;
use crate::values::{Choices, Entry, Type, Unit, Value};
use anyhow::{anyhow, bail, Context, Result};
use naga::{ScalarKind, TypeInner};

/// How a member is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    F32,
    U32,
    I32,
}

/// One member of the struct: where it is, and the value it holds, if any.
#[derive(Debug)]
pub struct Member {
    /// Its name in the struct (what `Block::set` takes).
    pub name: &'static str,
    pub offset: u32,
    pub size: u32,
    /// Its scalars' type (of every scalar of an array or a matrix too).
    pub storage: Option<Storage>,
    /// The value it holds; None for a derived member.
    pub entry: Option<&'static Entry>,
}

/// A shader's values: its `@shader_data` global and that struct's members.
#[derive(Debug)]
pub struct Schema {
    pub shader: &'static str,
    pub global: &'static str,
    /// Its `@group` and `@binding`.
    pub binding: (u32, u32),
    /// The struct's size in bytes.
    pub size: u32,
    pub members: Vec<Member>,
}

impl Schema {
    /// Its values, in the struct's order.
    pub fn entries(&self) -> impl Iterator<Item = &'static Entry> + '_ {
        self.members.iter().filter_map(|m| m.entry)
    }

    pub fn member(&self, name: &str) -> &Member {
        self.members.iter().find(|m| m.name == name).unwrap_or_else(|| panic!("{} has no member `{name}` in {}", self.global, self.shader))
    }
}

/// One attribute of a tag: `name=a,b,c` (its arguments) or a bare `name`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tag {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
}

/// The tags of a comment's lines (each with its `///`): every `{@ …}` in
/// them, read as a story's tags are (src/rich_text.rs), and the rest of the
/// text, its doc.
pub(crate) fn parse_tags(lines: &[String]) -> Result<(Vec<Tag>, String)> {
    let mut tags = Vec::new();
    let mut doc = Vec::new();
    for line in lines {
        let mut rest = line.trim_start_matches('/').trim();
        let mut text = String::new();
        while let Some(at) = rest.find("{@") {
            text.push_str(&rest[..at]);
            let (attrs, end) = crate::rich_text::parse_tag(rest, at + 2).map_err(|e| anyhow!(e))?;
            for (name, value) in attrs {
                let args = if value.is_empty() { Vec::new() } else { value.split(',').map(|a| a.trim().to_string()).collect() };
                tags.push(Tag { name, args });
            }
            rest = &rest[end..];
        }
        text.push_str(rest);
        if !text.trim().is_empty() {
            doc.push(text.trim().to_string());
        }
    }
    Ok((tags, doc.join(" ")))
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub(crate) fn number(text: &str) -> Result<f32> {
    match text {
        "true" => Ok(1.0),
        "false" => Ok(0.0),
        _ => text.parse::<f32>().map_err(|_| anyhow!("`{text}` is not a number")),
    }
}

/// The scalar kind and component count of a value's WGSL type.
pub(crate) fn wgsl_value_type(inner: &TypeInner) -> Option<(Storage, usize)> {
    let storage = |kind: ScalarKind, width: u8| match (kind, width) {
        (ScalarKind::Float, 4) => Some(Storage::F32),
        (ScalarKind::Uint, 4) => Some(Storage::U32),
        (ScalarKind::Sint, 4) => Some(Storage::I32),
        _ => None,
    };
    match inner {
        TypeInner::Scalar(s) => Some((storage(s.kind, s.width)?, 1)),
        TypeInner::Vector { size, scalar } if scalar.kind == ScalarKind::Float && scalar.width == 4 => Some((Storage::F32, *size as usize)),
        _ => None,
    }
}

/// The scalars' storage of any member (an array's or a matrix's elements').
fn scalar_storage(module: &naga::Module, inner: &TypeInner) -> Option<Storage> {
    let of = |s: naga::Scalar| match s.kind {
        ScalarKind::Float => Some(Storage::F32),
        ScalarKind::Uint => Some(Storage::U32),
        ScalarKind::Sint => Some(Storage::I32),
        _ => None,
    };
    match inner {
        TypeInner::Scalar(s) | TypeInner::Vector { scalar: s, .. } | TypeInner::Matrix { scalar: s, .. } => of(*s),
        TypeInner::Array { base, .. } => scalar_storage(module, &module.types[*base].inner),
        _ => None,
    }
}

/// A member's value, from its tags.
/// The choice `name` of the package `package` (an investigation's `choices`).
fn find_choices(package: &str, name: &str) -> Result<&'static Choices> {
    let known: Vec<&'static Choices> = crate::investigations::all().iter().filter(|i| i.package() == Some(package)).flat_map(|i| i.choices().iter().copied()).collect();
    known.iter().copied().find(|c| c.name == name).ok_or_else(|| anyhow!("no choice `{name}` (the package's investigations declare: {})", known.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")))
}

fn entry(r: &Reflected, scope: Option<&str>, tags: &[Tag], doc: String, storage: Storage, components: usize) -> Result<Entry> {
    let one = |name: &str| -> Result<Option<&Tag>> {
        let found: Vec<&Tag> = tags.iter().filter(|t| t.name == name).collect();
        if found.len() > 1 {
            bail!("`{name}` is given twice");
        }
        Ok(found.first().copied())
    };
    let scoped = |key: &str| match scope {
        Some(s) => format!("{s}.{key}"),
        None => key.to_string(),
    };
    let arg = |tag: &Tag, n: usize| -> Result<()> {
        if tag.args.len() != n {
            bail!("`{}=` takes {n} value{}", tag.name, if n == 1 { "" } else { "s" });
        }
        Ok(())
    };
    let key_tag = one("key")?.unwrap();
    arg(key_tag, 1)?;
    let key = leak(scoped(&key_tag.args[0]));
    let toggle = one("toggle")?.is_some();
    let package = r.id.split('.').next().unwrap_or_default();
    let choices = match one("choice")? {
        Some(t) => {
            arg(t, 1)?;
            if components != 1 || storage == Storage::I32 || toggle {
                bail!("a choice is one u32 or f32");
            }
            let c = find_choices(package, &t.args[0])?;
            // The constants the shader compares against must be the enum's current ones.
            if let Some(source) = c.wgsl_source() {
                if !r.source.contains(&source) {
                    bail!("the shader does not compose {}'s current constants: compose its package's `{}` (a `built = true` resource the lab writes)", c.name, c.wgsl_file());
                }
            }
            Some(c)
        }
        None => None,
    };
    let unit = match one("unit")? {
        _ if choices.is_some() => Unit::Index,
        Some(t) => {
            arg(t, 1)?;
            Unit::parse(&t.args[0]).ok_or_else(|| anyhow!("unknown unit `{}` (m, deg, factor, plain, index)", t.args[0]))?
        }
        None => Unit::Plain,
    };
    let ty = match (components, storage, toggle, unit) {
        _ if choices.is_some() => Type::U32,
        (1, _, true, _) => Type::Bool,
        (1, Storage::F32 | Storage::U32, false, Unit::Index) | (1, Storage::U32, false, _) => Type::U32,
        (1, Storage::I32, false, _) => Type::I32,
        (1, Storage::F32, false, _) => Type::F32,
        (2, Storage::F32, false, _) => Type::Vec2,
        (3, Storage::F32, false, _) => Type::Vec3,
        (4, Storage::F32, false, _) => Type::Vec4,
        _ => bail!("a toggle is one number"),
    };
    let default = one("default")?.ok_or_else(|| anyhow!("a value needs `default=`"))?;
    if default.args.len() != ty.components() {
        bail!("`default=` of a {} has {} number{}", ty.name(), ty.components(), if ty.components() == 1 { "" } else { "s" });
    }
    let numbers: Vec<f32> = match choices {
        // A choice's default is an option's name.
        Some(c) => vec![c.index(&default.args[0]).ok_or_else(|| anyhow!("`default={}`: {} has no such option ({})", default.args[0], c.name, c.names()))? as f32],
        None => default.args.iter().map(|a| number(a)).collect::<Result<_>>()?,
    };
    // One low, high pair, or one per component; a choice's is its options'.
    let (range, component_ranges) = match one("range")? {
        Some(_) if choices.is_some() => bail!("a choice's range is its options"),
        None if choices.is_some() => (choices.map(|c| (0.0, c.options.len() as f32 - 1.0)), Vec::new()),
        Some(t) if t.args.len() == 2 => (Some((number(&t.args[0])?, number(&t.args[1])?)), Vec::new()),
        Some(t) if ty.components() > 1 && t.args.len() == 2 * ty.components() => {
            let n: Vec<f32> = t.args.iter().map(|a| number(a)).collect::<Result<_>>()?;
            (None, n.chunks(2).map(|c| (c[0], c[1])).collect())
        }
        Some(_) => bail!("`range=` takes a low and a high, or one pair per component"),
        None => (None, Vec::new()),
    };
    let step = match one("step")? {
        Some(t) => {
            arg(t, 1)?;
            Some(number(&t.args[0])?)
        }
        None => None,
    };
    let labels: Vec<&'static str> = tags.iter().filter(|t| t.name == "label").flat_map(|t| t.args.iter().map(|a| leak(a.clone()))).collect();
    let text = match one("text")? {
        Some(t) => {
            arg(t, 1)?;
            Some(leak(t.args[0].clone()))
        }
        None => None,
    };
    let components: Vec<&'static str> = match one("components")? {
        Some(t) if ty.components() > 1 => {
            arg(t, ty.components())?;
            t.args.iter().map(|c| leak(scoped(c))).collect()
        }
        Some(_) => bail!("only a vector has `components=`"),
        None if ty.components() > 1 => ["x", "y", "z", "w"][..ty.components()].iter().map(|c| leak(format!("{key}.{c}"))).collect(),
        None => Vec::new(),
    };
    Ok(Entry {
        key,
        ty,
        default: Some(Value::from_components(ty, &numbers)),
        range,
        component_ranges: Box::leak(component_ranges.into_boxed_slice()),
        step,
        unit,
        wrap: one("wrap")?.is_some(),
        labels: Box::leak(labels.into_boxed_slice()),
        text,
        name: None,
        color: None,
        toggle: false,
        choices,
        package: leak(package.to_string()),
        doc: leak(doc),
        components: Box::leak(components.into_boxed_slice()),
    })
}

/// The tags a member may carry.
const MEMBER_TAGS: &[&str] = &["key", "derived", "default", "range", "step", "unit", "wrap", "label", "text", "toggle", "components", "choice"];

fn build(r: &'static Reflected) -> Result<Option<Schema>> {
    let module = &r.module;
    let Some(docs) = module.doc_comments.as_deref() else { return Ok(None) };
    let mut found = None;
    for (handle, lines) in &docs.global_variables {
        let (tags, _) = parse_tags(lines)?;
        if !tags.iter().any(|t| t.name == "shader_data") {
            continue;
        }
        let var = &module.global_variables[*handle];
        let name = var.name.clone().unwrap_or_default();
        let at = r.locate(&lines[0]);
        if let Some(other) = tags.iter().find(|t| t.name != "shader_data" && t.name != "scope") {
            bail!("{}: `{name}`: unknown attribute `{}` of a global's tag (shader_data, scope){at}", r.id, other.name);
        }
        if found.is_some() {
            bail!("{}: more than one `shader_data` global (`{name}`){at}", r.id);
        }
        let scope = match tags.iter().find(|t| t.name == "scope") {
            Some(t) if t.args.len() == 1 => Some(t.args[0].clone()),
            Some(_) => bail!("{}: `scope=` names one scope{at}", r.id),
            None => None,
        };
        found = Some((*handle, scope, at));
    }
    let Some((handle, scope, at)) = found else { return Ok(None) };
    let var = &module.global_variables[handle];
    let global = var.name.clone().unwrap_or_default();
    let binding = var.binding.as_ref().map(|b| (b.group, b.binding)).ok_or_else(|| anyhow!("{}: `{global}` has no binding{at}", r.id))?;
    let ty = &module.types[var.ty];
    let TypeInner::Struct { members, span } = &ty.inner else { bail!("{}: `{global}` is not a struct, so its members cannot carry values{at}", r.id) };
    let mut out = Vec::new();
    for (index, m) in members.iter().enumerate() {
        let name = m.name.clone().unwrap_or_default();
        let lines = docs.struct_members.get(&(var.ty, index)).cloned().unwrap_or_default();
        let at = lines.first().map(|l| r.locate(l)).unwrap_or_else(|| r.locate(&format!("{name}:")));
        let context = || format!("{}: `{global}.{name}`{at}", r.id);
        let (tags, doc) = parse_tags(&lines).with_context(context)?;
        if let Some(t) = tags.iter().find(|t| !MEMBER_TAGS.contains(&t.name.as_str())) {
            return Err(anyhow!("unknown attribute `{}` ({})", t.name, MEMBER_TAGS.join(", "))).with_context(context);
        }
        let inner = &module.types[m.ty].inner;
        let size = inner.size(module.to_ctx());
        let derived = tags.iter().any(|t| t.name == "derived");
        let keyed = tags.iter().any(|t| t.name == "key");
        let entry = match (keyed, derived) {
            (true, true) => return Err(anyhow!("a member has `key=` or `derived`, not both")).with_context(context),
            (false, false) => return Err(anyhow!("a member needs a tag: `{{@ key=… default=…}}` (a value) or `{{@ derived}}` (filled by the investigation)")).with_context(context),
            (false, true) => {
                if let Some(t) = tags.iter().find(|t| t.name != "derived") {
                    return Err(anyhow!("a derived member is not a value, so it takes no `{}`", t.name)).with_context(context);
                }
                None
            }
            (true, false) => {
                let (storage, components) = wgsl_value_type(inner).ok_or_else(|| anyhow!("a value is f32, u32, i32, or vecN<f32>")).with_context(context)?;
                Some(&*Box::leak(Box::new(entry(r, scope.as_deref(), &tags, doc, storage, components).with_context(context)?)))
            }
        };
        out.push(Member { name: leak(name), offset: m.offset, size, storage: scalar_storage(module, inner), entry });
    }
    let mut keys: Vec<&str> = out.iter().filter_map(|m| m.entry).flat_map(|e| std::iter::once(e.key).chain(e.components.iter().copied())).collect();
    keys.sort();
    if let Some(w) = keys.windows(2).find(|w| w[0] == w[1]) {
        bail!("{}: `{global}`: the key `{}` is given twice{at}", r.id, w[0]);
    }
    Ok(Some(Schema { shader: r.id, global: leak(global), binding, size: *span, members: out }))
}

/// The values the shader `id` declares, or None when it has no
/// `@shader_data` global; read once per run.
pub fn schema(id: &str) -> Result<Option<&'static Schema>> {
    static CACHE: std::sync::Mutex<Vec<(&'static str, Option<&'static Schema>)>> = std::sync::Mutex::new(Vec::new());
    if let Some((_, s)) = CACHE.lock().unwrap().iter().find(|(i, _)| *i == id) {
        return Ok(*s);
    }
    let reflected = shaders::reflect(id)?;
    let schema = build(reflected)?.map(|s| &*Box::leak(Box::new(s)));
    let mut cache = CACHE.lock().unwrap();
    if let Some((_, s)) = cache.iter().find(|(i, _)| *i == id) {
        return Ok(*s);
    }
    cache.push((reflected.id, schema));
    Ok(schema)
}

/// Writes the WGSL constants of every choice the investigations declare
/// with a prefix (`Choices::wgsl_source`) to the package resource whose
/// source is its `wgsl_file`: in the built package the lab reads, and in the
/// source package's `build/` (git-ignored), where the next build's shader
/// check finds it. Runs when the investigations load, before any shader is
/// composed; a file already up to date is left alone.
pub fn write_choices(investigations: &[&'static dyn crate::investigation::Investigation]) {
    for investigation in investigations {
        let Some(package) = investigation.package() else { continue };
        for choices in investigation.choices() {
            let Some(text) = choices.wgsl_source() else { continue };
            let file = choices.wgsl_file();
            let resource = crate::resources::all().iter().find(|r| r.id().starts_with(&format!("{package}.")) && r.source().ends_with(&file));
            let Some(resource) = resource else {
                eprintln!("{package}: no resource declares {file}, so its shaders cannot compose {}'s constants (declare it `built = true`)", choices.name);
                continue;
            };
            for path in [resource.path(), crate::resources::root().join(resource.source())] {
                if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
                    continue;
                }
                // Written beside it, then moved, so a reader never sees half a file (nor a linked copy changed).
                let write = || -> std::io::Result<()> {
                    std::fs::create_dir_all(path.parent().expect("a file in a folder"))?;
                    let partial = path.with_extension(format!("partial.{}", std::process::id()));
                    std::fs::write(&partial, &text)?;
                    std::fs::rename(&partial, &path)
                };
                if let Err(e) = write() {
                    eprintln!("{package}: write {}: {e}", path.display());
                }
            }
        }
    }
}

/// The bytes of a `@shader_data` struct for one frame.
pub struct Block {
    pub schema: &'static Schema,
    bytes: Vec<u8>,
}

impl Block {
    pub fn new(schema: &'static Schema) -> Block {
        Block { schema, bytes: vec![0; schema.size as usize] }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn put(&mut self, offset: u32, storage: Storage, number: f32) {
        let bytes = match storage {
            Storage::F32 => number.to_le_bytes(),
            Storage::U32 => (number.round().max(0.0) as u32).to_le_bytes(),
            Storage::I32 => (number.round() as i32).to_le_bytes(),
        };
        self.bytes[offset as usize..offset as usize + 4].copy_from_slice(&bytes);
    }

    /// Every value member from the state's values (a toggle as 1 or 0, an
    /// index as the nearest whole number in its range).
    pub fn fill(&mut self, state: &ShotState) {
        for i in 0..self.schema.members.len() {
            let member = &self.schema.members[i];
            let Some(entry) = member.entry else { continue };
            let value = state.get(entry.key);
            let clamped = match (value, entry.range) {
                (Value::U32(x), Some((low, high))) => Value::U32(x.clamp(low.max(0.0) as u32, high.max(0.0) as u32)),
                (Value::I32(x), Some((low, high))) => Value::I32(x.clamp(low as i32, high as i32)),
                _ => value,
            };
            let (offset, storage) = (member.offset, member.storage.expect("a value member has scalars"));
            for (k, number) in clamped.components().into_iter().enumerate() {
                self.put(offset + 4 * k as u32, storage, number);
            }
        }
    }

    /// Sets member `name` to `value` (one number per scalar).
    pub fn set(&mut self, name: &str, value: impl Into<Value>) {
        self.write(name, &value.into().components());
    }

    /// Writes `numbers` from the start of member `name` (an array's or a
    /// matrix's scalars in memory order, vec3 padding included); panics if
    /// they overrun it.
    pub fn write(&mut self, name: &str, numbers: &[f32]) {
        let member = self.schema.member(name);
        let (offset, size, storage) = (member.offset, member.size, member.storage.unwrap_or_else(|| panic!("`{name}` has no scalars")));
        assert!(numbers.len() as u32 * 4 <= size, "{} numbers overrun `{name}` ({size} bytes) in {}", numbers.len(), self.schema.shader);
        for (k, n) in numbers.iter().enumerate() {
            self.put(offset + 4 * k as u32, storage, *n);
        }
    }
}

/// An investigation's frame (`Investigation::build_frame`), with its scene
/// shading's `@shader_data` uniform packed from the state when its shader
/// declares one (the investigation fills the derived members,
/// `Investigation::shader_data`).
pub fn build_frame(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
    let current = investigation::current();
    let mut content = current.build_frame(state, statics, viewport, pixel_scale, time);
    if let Some(block) = scene_block(state) {
        let floats: &[f32] = bytemuck::cast_slice(block.bytes());
        for (i, f) in floats.iter().enumerate() {
            content.shading[i / 4][i % 4] = *f;
        }
    }
    content
}

/// The current investigation's scene-shading uniform for `state`, when its
/// shader declares one.
pub fn scene_block(state: &ShotState) -> Option<Block> {
    let current = investigation::current();
    let module = current.scene_shading()?.module;
    let schema = schema(module).unwrap_or_else(|e| panic!("{e:#}"))?;
    assert!(schema.binding == (1, 0), "{}: the lab fills `@shader_data` for the scene shading's `@group(1) @binding(0)` uniform; `{}` is at {:?}", schema.shader, schema.global, schema.binding);
    assert!(schema.size as usize <= SHADING_UNIFORMS * 16, "{}: `{}` is {} bytes; the scene shading's uniform holds {}", schema.shader, schema.global, schema.size, SHADING_UNIFORMS * 16);
    let mut block = Block::new(schema);
    block.fill(state);
    current.shader_data(&mut block, state);
    Some(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn tags_and_doc() {
        let (tags, doc) = parse_tags(&lines(&["/// The light's intensity.", r#"/// {@ key=lightIntensity default=6 range=0,30 text="a b" toggle}"#])).unwrap();
        assert_eq!(doc, "The light's intensity.");
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["key", "default", "range", "text", "toggle"]);
        assert_eq!(tags[2].args, ["0", "30"]);
        assert_eq!(tags[3].args, ["a b"]);
        assert!(tags[4].args.is_empty());
        assert!(parse_tags(&lines(&["/// {@ range=0,1"])).is_err());
        // Text around a tag is the doc.
        let (tags, doc) = parse_tags(&lines(&["/// Unused. {@ derived}"])).unwrap();
        assert_eq!((tags.len(), doc.as_str()), (1, "Unused."));
    }

    #[test]
    fn packing() {
        let entry = Box::leak(Box::new(Entry {
            key: "light",
            ty: Type::Vec3,
            default: None,
            range: None,
            component_ranges: &[],
            step: None,
            unit: Unit::Plain,
            wrap: false,
            labels: &[],
            text: None,
            name: None,
            color: None,
            toggle: false,
            choices: None,
            package: "",
            doc: "",
            components: &["lightX", "lightY", "lightZ"],
        }));
        let schema: &'static Schema = Box::leak(Box::new(Schema {
            shader: "test",
            global: "gTest",
            binding: (1, 0),
            size: 32,
            members: vec![
                Member { name: "position", offset: 0, size: 12, storage: Some(Storage::F32), entry: Some(entry) },
                Member { name: "count", offset: 12, size: 4, storage: Some(Storage::U32), entry: None },
                Member { name: "more", offset: 16, size: 16, storage: Some(Storage::F32), entry: None },
            ],
        }));
        let mut block = Block::new(schema);
        block.set("count", 7u32);
        block.write("more", &[1.0, 2.0]);
        let words: Vec<u32> = block.bytes().chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        assert_eq!(words[3], 7);
        assert_eq!(f32::from_bits(words[4]), 1.0);
        assert_eq!(f32::from_bits(words[5]), 2.0);
        assert!(std::panic::catch_unwind(move || Block::new(schema).write("count", &[1.0, 2.0])).is_err());
    }
}
