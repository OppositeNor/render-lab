//! Scripts: Lamlet a package runs while the viewer plays and while a video records (todo/lamlet-live-logic.md).
//! What the investigation's native code computes stays native (its passes, its CPU mirrors, a ray through the
//! scene); a script says what to do with it: where an overlay's arrows go, what a click sets, how a value moves.
//!
//! A package's scripts are its resources `script/<name>` (`<package>.script/<name>`), each a Lamlet document,
//! which the package's other scripts import as `<package>_<name>`. A script imports:
//!
//! - render_lab's runtime library, `render_lab_runtime` (packages/render_lab/language/runtime.lam): the hooks'
//!   types, vectors, shapes, easings;
//! - the investigation's values package, `<package>_values`, written here from its values (`values_source`): a
//!   reader (`vectorsShown`) and a setter (`setVectorsShown`) per key, a choice as a variant;
//! - its natives, `<package>_natives` (`Investigation::script_natives`), and the lab's, `rl` and `rl_runtime`.
//!
//! The lab finds the hooks by their types (runtime.lam): overlays (`Overlay`), click handlers (`ClickHandler`),
//! actions (`{Action Args}`), animations (`Animation`). A hook keeps nothing between calls: it is a function of
//! what the lab passes and of what it reads, and it returns a description (shapes, or values to set) that the lab
//! checks whole and then applies. While a hook runs, natives read the call's context: the shot (`state`), the
//! frame it draws (`frame`, `frame_data`), and the scene's statics (`statics`).
//!
//! Lamlet's engine, documents, and instances are not `Send`, and every hook runs on the thread that draws or
//! records, so each thread has its own runtime (a `thread_local!`), checked once per investigation.

use super::{natives, Package};
use crate::frame::{DrawMark, FrameContent, Label, Statics, FLAG_ARROW, FLAG_DASHED};
use crate::investigation::{self, Investigation};
use crate::shot::ShotState;
use crate::values::{self, Type, Value as LabValue};
use egui::Color32;
use glam::Vec3;
use lamlet::embed::{Binding, Data, Document, Engine, Instance, IntoLamlet, LamletType, SourceFile};
use lamlet::types::TypeDesc;
use lamlet::{Limits, Value};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The resource that holds the runtime library.
pub const LIBRARY: &str = "render_lab.script/runtime";
/// The runtime library's Lamlet package.
pub const LIBRARY_PACKAGE: &str = "render_lab_runtime";
/// The lab's runtime natives' package (and their ids' prefix).
pub const NATIVES: &str = "rl_runtime";

// ---------------------------------------------------------------------------------------------------------
// The call's context

#[derive(Clone, Copy)]
struct Context {
    state: *const ShotState,
    frame: Option<*const FrameContent>,
    statics: Option<*const Statics>,
}

thread_local! {
    static CONTEXT: Cell<Option<Context>> = const { Cell::new(None) };
}

/// Runs `f` (a hook's call) with the context its natives read.
fn with_context<R>(state: &ShotState, frame: Option<&FrameContent>, statics: Option<&Statics>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Context>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CONTEXT.with(|c| c.set(self.0));
        }
    }
    let context = Context { state, frame: frame.map(|f| f as *const FrameContent), statics: statics.map(|s| s as *const Statics) };
    let _restore = Restore(CONTEXT.with(|c| c.replace(Some(context))));
    f()
}

fn context() -> Result<Context, String> {
    CONTEXT.with(Cell::get).ok_or_else(|| "this native reads the call's context: it runs only inside a hook".to_string())
}

/// The shot the running hook reads: for an overlay, the state it draws; for a click, the state on screen; for an
/// action or an animation, the state so far.
pub fn state<R>(f: impl FnOnce(&ShotState) -> R) -> Result<R, String> {
    let c = context()?;
    // SAFETY: `with_context` stored references that outlive its call, and natives run only inside that call; the
    // reference given to `f` cannot escape it.
    Ok(f(unsafe { &*c.state }))
}

/// The frame the running hook draws or was clicked on (an overlay's, a click's).
pub fn frame<R>(f: impl FnOnce(&FrameContent) -> R) -> Result<R, String> {
    let c = context()?;
    let frame = c.frame.ok_or_else(|| "this native reads the frame: it runs only in an overlay or a click handler".to_string())?;
    // SAFETY: as in `state`.
    Ok(f(unsafe { &*frame }))
}

/// The scene's statics, where the running hook has them (an overlay, a click).
pub fn statics<R>(f: impl FnOnce(&Statics) -> R) -> Result<R, String> {
    let c = context()?;
    let statics = c.statics.ok_or_else(|| "this native reads the scene: it runs only in an overlay or a click handler".to_string())?;
    // SAFETY: as in `state`.
    Ok(f(unsafe { &*statics }))
}

/// What the investigation's `build_frame` left for its natives in this frame (`FrameContent::script`), as `T`.
pub fn frame_data<T: Any + Send + Sync>() -> Result<Arc<T>, String> {
    let data = frame(|f| f.script.clone())?.ok_or_else(|| "the frame has no data for the scripts (`FrameContent::script`)".to_string())?;
    data.downcast::<T>().map_err(|_| format!("the frame's data for the scripts is not a {}", std::any::type_name::<T>()))
}

// ---------------------------------------------------------------------------------------------------------
// Vectors and records across the boundary

/// A record type of Float fields, sorted by label as Lamlet keeps them.
fn floats(labels: &[&str]) -> TypeDesc {
    let mut fields: Vec<(Rc<str>, TypeDesc)> = labels.iter().map(|l| (Rc::from(*l), TypeDesc::Float)).collect();
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    TypeDesc::Record(fields)
}

fn float_field(v: &Value, label: &str) -> Result<f32, String> {
    match v.field(label) {
        Some(Value::Float(x)) => Ok(*x as f32),
        Some(other) => Err(format!("field `{label}` is a {}, not a Float", other.type_name())),
        None => Err(format!("no field `{label}`")),
    }
}

macro_rules! lamlet_vector {
    ($name:ident, $glam:ty, $($c:ident)+) => {
        /// A vector as a Lamlet record of Floats (runtime.lam's `Vec2`, `Vec3`, `Vec4`), for natives' signatures.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name(pub $glam);

        impl LamletType for $name {
            fn lamlet_type() -> TypeDesc {
                floats(&[$(stringify!($c)),+])
            }
        }

        impl IntoLamlet for $name {
            fn into_lamlet(self) -> Result<Value, String> {
                Ok(Value::record(vec![$((Rc::from(stringify!($c)), (self.0.$c as f64).into_lamlet()?)),+]))
            }
        }

        impl lamlet::embed::FromLamlet for $name {
            fn from_lamlet(v: &Value) -> Result<Self, String> {
                Ok($name(<$glam>::new($(float_field(v, stringify!($c))?),+)))
            }
        }
    };
}

lamlet_vector!(V2, glam::Vec2, x y);
lamlet_vector!(V3, glam::Vec3, x y z);
lamlet_vector!(V4, glam::Vec4, x y z w);

/// A Rust struct that crosses to Lamlet as a record, for a package's natives' signatures: each field with its
/// Lamlet label (`lab::lamlet_record! { pub struct Hit { pub point: V3 = "point", pub light_on: bool = "lightOn" } }`).
#[macro_export]
macro_rules! lamlet_record {
    ($(#[$meta:meta])* $vis:vis struct $name:ident { $($(#[$fmeta:meta])* $fvis:vis $field:ident : $ty:ty = $label:literal),* $(,)? }) => {
        $(#[$meta])* $vis struct $name { $($(#[$fmeta])* $fvis $field: $ty),* }

        impl $crate::script::lamlet::embed::LamletType for $name {
            fn lamlet_type() -> $crate::script::lamlet::types::TypeDesc {
                let mut fields: Vec<(std::rc::Rc<str>, $crate::script::lamlet::types::TypeDesc)> =
                    vec![$((std::rc::Rc::from($label), <$ty as $crate::script::lamlet::embed::LamletType>::lamlet_type())),*];
                fields.sort_by(|a, b| a.0.cmp(&b.0));
                $crate::script::lamlet::types::TypeDesc::Record(fields)
            }
        }

        impl $crate::script::lamlet::embed::IntoLamlet for $name {
            fn into_lamlet(self) -> Result<$crate::script::lamlet::Value, String> {
                Ok($crate::script::lamlet::Value::record(vec![$((std::rc::Rc::from($label), $crate::script::lamlet::embed::IntoLamlet::into_lamlet(self.$field)?)),*]))
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------------------
// The lab's runtime natives

/// The value `key` of the running hook's shot.
fn value_of(key: &str) -> Result<LabValue, String> {
    state(|s| s.try_get(key))?.ok_or_else(|| format!("the shot has no value `{key}`"))
}

/// Registers the lab's runtime natives (`rl_runtime`): the values package's readers call them.
fn register(engine: &mut Engine) -> Result<(), lamlet::embed::Error> {
    let r = engine.natives_mut();
    r.function("rl_runtime/value-float", |_, (key,): (String,)| Ok(value_of(&key)?.f32() as f64))
        .doc("Fn([String], Float). The shot's value `key` as a number (the values package's readers call it).")
        .register()?;
    r.function("rl_runtime/value-int", |_, (key,): (String,)| {
        Ok(match value_of(&key)? {
            LabValue::I32(n) => n as i64,
            LabValue::U32(n) => n as i64,
            other => other.f32().round() as i64,
        })
    })
    .doc("Fn([String], Int). The shot's whole-number value `key`.")
    .register()?;
    r.function("rl_runtime/value-bool", |_, (key,): (String,)| Ok(value_of(&key)?.f32() > 0.5))
        .doc("Fn([String], Bool). The shot's value `key`, on above 0.5.")
        .register()?;
    r.function("rl_runtime/value-vec2", |_, (key,): (String,)| match value_of(&key)? {
        LabValue::Vec2(v) => Ok(V2(glam::Vec2::from(v))),
        other => Err(format!("`{key}` is a {}, not a vec2", other.ty().name())),
    })
    .doc("Fn([String], Vec2). The shot's vec2 value `key`.")
    .register()?;
    r.function("rl_runtime/value-vec3", |_, (key,): (String,)| match value_of(&key)? {
        LabValue::Vec3(v) => Ok(V3(Vec3::from(v))),
        other => Err(format!("`{key}` is a {}, not a vec3", other.ty().name())),
    })
    .doc("Fn([String], Vec3). The shot's vec3 value `key`.")
    .register()?;
    r.function("rl_runtime/value-vec4", |_, (key,): (String,)| match value_of(&key)? {
        LabValue::Vec4(v) => Ok(V4(glam::Vec4::from(v))),
        other => Err(format!("`{key}` is a {}, not a vec4", other.ty().name())),
    })
    .doc("Fn([String], Vec4). The shot's vec4 value `key`.")
    .register()?;
    r.function("rl_runtime/value-choice", |_, (key,): (String,)| {
        let choices = values::entry(&key).and_then(|e| e.choices).ok_or_else(|| format!("`{key}` is not a choice"))?;
        let index = value_of(&key)?.u32() as usize;
        choices.options.get(index).map(|o| o.name.to_string()).ok_or_else(|| format!("`{key}` is {index}, past its options ({})", choices.names()))
    })
    .doc("Fn([String], String). The name of the option the shot's choice `key` holds.")
    .register()?;
    r.function("rl_runtime/text", |_, (key,): (String,)| {
        let package = investigation::current().package().unwrap_or("render_lab");
        crate::i18n::try_package_text(package, &key).or_else(|| crate::i18n::try_package_text("render_lab", &key)).map(str::to_string).ok_or_else(|| format!("no text `{key}` in {package}'s text table or the lab's"))
    })
    .doc("Fn([String], String). The investigation's package's text `key` in the viewer's language, or the lab's.")
    .register()?;
    r.function("rl_runtime/demo", |_, ()| state(|s| s.demo.key().to_string()))
        .doc("Fn([], String). The key of the demonstration the shot shows.")
        .register()?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------
// The values package

/// The names a Lamlet source declares at its top level (`$[# name …]`, `$[# {name …} …]`, `$[Native name …]`,
/// `$[Macro !name …]`), not private ones: what a script that imports it with `Import*` gets.
fn declared(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for head in ["$[# {", "$[# ", "$[Native ", "$[Macro !"] {
        for (i, _) in source.match_indices(head) {
            let name: String = source[i + head.len()..].chars().take_while(|c| !c.is_whitespace() && !"{}[]".contains(*c)).collect();
            if !name.is_empty() && !name.starts_with('_') {
                out.insert(name);
            }
        }
    }
    out
}

/// `name` with its first letter upper-cased (`setVectorsShown` from `vectorsShown`).
fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn quoted(s: &str) -> String {
    format!("{s:?}")
}

/// The values package (`<package>_values`) of the current investigation: a reader and a setter per value and
/// per vector component, named after its key (`vectorsShown`, `setVectorsShown`; `camera.yaw` is `cameraYaw`),
/// and a variant type per choice. A key whose name `reserved` holds (std's, the runtime library's, the
/// natives') is left out; the second list says which.
pub fn values_source(package: &str, reserved: &BTreeSet<String>) -> (String, Vec<String>) {
    let mut out = format!(
        "The values package of `{package}`, written by the lab from the investigation's values (lab/src/script/runtime.rs):\n\
         a reader and a setter per key. A reader is the value in the shot the running hook reads; a setter makes the\n\
         `Write` that sets it, which the lab applies after the hook returns.\n\n\
         $[Import std]$[ImportAs {LIBRARY_PACKAGE} lib]$[ImportAs {NATIVES} rt]\n\n\
         $[# {{_write key value}} {{: {{. key key value value}} lib::Write}}]\n"
    );
    let mut skipped = Vec::new();
    let mut names = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut emit = |key: &str, ty: Type, choices: Option<&'static values::Choices>, out: &mut String| {
        let Ok(name) = lamlet_present::vocab::camel(&[key]) else {
            skipped.push(key.to_string());
            return;
        };
        let setter = format!("set{}", capitalized(&name));
        if reserved.contains(&name) || reserved.contains(&setter) || !names.insert(name.clone()) {
            skipped.push(key.to_string());
            return;
        }
        let k = quoted(key);
        // A choice whose options are all Lamlet labels is a variant; its type is named after the enum.
        if let Some(c) = choices.filter(|c| c.options.iter().all(|o| lamlet_present::vocab::camel(&[o.name]).is_ok_and(|n| n == o.name))) {
            let t = c.name;
            if types.insert(t) {
                let fields: Vec<String> = c.options.iter().map(|o| format!("{{std::Field {} std::Unit}}", quoted(o.name))).collect();
                out.push_str(&format!("\nThe options of `{t}`.\n$[# {t} [% {{std::Variant {{' {}}}}}]]\n", fields.join(" ")));
                let mut of = format!("{{std::error {{std::++ \"no option of {t} is named \" s}}}}");
                for o in c.options.iter().rev() {
                    of = format!("{{std::caseBool {{std::= s {}}} {{^ {{}} {{: {{#{} std::unit}} {t}}}}} {{^ {{}} {of}}}}}", quoted(o.name), o.name);
                }
                out.push_str(&format!("$[# {{_of{t} s}} {of}]\n"));
                let handlers: Vec<String> = c.options.iter().map(|o| format!("{} {{^ {{u}} {}}}", o.name, quoted(o.name))).collect();
                out.push_str(&format!("$[# {{_name{t} o}} {{std::match {{: o {t}}} {{. {}}}}}]\n", handlers.join(" ")));
            }
            out.push_str(&format!("\nThe shot's `{key}`.\n$[# {{{name}}} {{_of{t} {{rt::valueChoice {k}}}}}]\n"));
            out.push_str(&format!("Sets `{key}`.\n$[# {{{setter} o}} {{_write {k} {{std::toData {{_name{t} o}}}}}}]\n"));
            return;
        }
        let (reader, ty) = match ty {
            Type::F32 => ("valueFloat", "std::Float"),
            Type::I32 | Type::U32 if choices.is_none() => ("valueInt", "std::Int"),
            Type::I32 | Type::U32 => {
                // A choice whose options are not all labels: its option's name.
                out.push_str(&format!("\nThe shot's `{key}`, its option's name.\n$[# {{{name}}} {{rt::valueChoice {k}}}]\n"));
                out.push_str(&format!("Sets `{key}` to the option named `o`.\n$[# {{{setter} o}} {{_write {k} {{std::toData {{: o std::String}}}}}}]\n"));
                return;
            }
            Type::Bool => ("valueBool", "std::Bool"),
            Type::Vec2 => ("valueVec2", "lib::Vec2"),
            Type::Vec3 => ("valueVec3", "lib::Vec3"),
            Type::Vec4 => ("valueVec4", "lib::Vec4"),
        };
        out.push_str(&format!("\nThe shot's `{key}`.\n$[# {{{name}}} {{rt::{reader} {k}}}]\n"));
        out.push_str(&format!("Sets `{key}`.\n$[# {{{setter} x}} {{_write {k} {{std::toData {{: x {ty}}}}}}}]\n"));
    };
    for e in values::list() {
        emit(e.key, e.ty, e.choices, &mut out);
        for c in e.components {
            emit(c, Type::F32, None, &mut out);
        }
    }
    (out, skipped)
}

// ---------------------------------------------------------------------------------------------------------
// The runtime

/// A hook's binding in its module.
struct Hook {
    module: usize,
    binding: Binding,
}

/// A checked script.
struct Module {
    name: String,
    instance: Instance,
    _document: Document,
}

/// One investigation's scripts, checked, with their hooks.
pub struct Runtime {
    /// The engine its documents were checked with, kept for as long as they run.
    engine: Engine,
    package: String,
    modules: Vec<Module>,
    overlays: Vec<Hook>,
    readouts: Vec<Hook>,
    clicks: BTreeMap<String, Hook>,
    actions: BTreeMap<String, (Hook, Vec<ScriptArg>)>,
    animations: BTreeMap<String, Hook>,
    /// Each overlay's last shapes that drew, by view: what the viewer keeps showing while it fails.
    last: BTreeMap<(usize, Option<&'static str>), Vec<Shape>>,
    /// The failing hooks and their messages, shown until the next call of the hook succeeds.
    errors: BTreeMap<String, String>,
    /// The messages already printed.
    reported: BTreeSet<String>,
}

/// The budgets of one call: a broken script fails the call instead of freezing a frame.
fn limits() -> Limits {
    Limits { max_instructions: 2_000_000, max_frames: 10_000, max_stack: 1_000_000, max_alloc_bytes: 64 << 20, max_native_calls: 100_000, ..Limits::default() }
}

/// The packages a script may import (besides `std`): the lab's storyboard package and runtime library, the
/// values package, and the package's scripts as `<package>_<name>`.
pub struct Sources {
    pub packages: Vec<Package>,
    /// The package's scripts: resource key, Lamlet package name.
    pub scripts: Vec<(String, String)>,
    /// Keys the values package leaves out (their names are taken).
    pub skipped: Vec<String>,
}

fn resource_text(key: &str) -> Result<(String, String), String> {
    let r = crate::resources::find(key).ok_or_else(|| format!("no resource {key} (render-lab cook)"))?;
    let text = std::str::from_utf8(r.bytes()).map_err(|e| format!("{key}: {e}"))?;
    Ok((r.path().display().to_string(), text.to_string()))
}

/// The engine for `investigation`'s scripts, with every package they may import.
pub fn engine(investigation: &'static dyn Investigation) -> Result<(Engine, Sources), String> {
    let package = investigation.package().unwrap_or("render_lab");
    let mut engine = Engine::new();
    // A `Native` declaration with no registered implementation fails the check, not the first call.
    engine.options_mut().require_linked = true;
    natives::register(&mut engine, Some(investigation)).map_err(|e| e.render())?;
    register(&mut engine).map_err(|e| e.render())?;
    register_frame_natives(&mut engine).map_err(|e| e.render())?;
    investigation.script_natives(engine.natives_mut()).map_err(|e| format!("{}'s natives: {}", investigation.path(), e.render()))?;
    let runtime_stubs = engine.natives_mut().declarations(NATIVES).map_err(|e| e.render())?;
    let package_stubs = engine.natives_mut().declarations(package).map_err(|e| e.render())?;
    let (library_file, library) = resource_text(LIBRARY)?;
    let mut reserved = declared(lamlet::package::STD_SOURCE);
    reserved.extend(declared(&library));
    reserved.extend(declared(&package_stubs));
    let (values, skipped) = values_source(package, &reserved);
    let mut packages = vec![
        Package::render_lab()?,
        Package { name: LIBRARY_PACKAGE.into(), file: library_file, source: library },
        Package { name: NATIVES.into(), file: format!("<{NATIVES} natives>"), source: runtime_stubs },
        Package { name: format!("{package}_natives"), file: format!("<{package} natives>"), source: package_stubs },
        Package { name: format!("{package}_values"), file: format!("<{package} values>"), source: values },
    ];
    let prefix = format!("{package}.script/");
    let mut scripts = Vec::new();
    for r in crate::resources::all().iter().filter(|r| r.id().starts_with(&prefix)) {
        let name = format!("{package}_{}", r.id()[prefix.len()..].replace('-', "_"));
        let (file, source) = resource_text(r.id())?;
        packages.push(Package { name: name.clone(), file, source });
        scripts.push((r.id().to_string(), name));
    }
    engine.set_resolver(super::resolver(&packages)?);
    Ok((engine, Sources { packages, scripts, skipped }))
}

/// A Type a runtime library binding holds (`Shape`, `Write`, `Click`).
fn library_type(library: &Document, name: &str) -> Result<TypeDesc, String> {
    match library.binding(name).map_err(|e| e.render())?.value {
        Value::Type(t) => Ok((*t).clone()),
        other => Err(format!("{LIBRARY}: `{name}` is a {}, not a Type", other.type_name())),
    }
}

impl Runtime {
    /// `investigation`'s scripts checked, or None when its package has none.
    fn load(investigation: &'static dyn Investigation) -> Result<Option<Runtime>, String> {
        let package = investigation.package().unwrap_or("render_lab");
        let prefix = format!("{package}.script/");
        if package == "render_lab" || !crate::resources::all().iter().any(|r| r.id().starts_with(&prefix)) {
            return Ok(None);
        }
        let (engine, sources) = engine(investigation)?;
        let library = sources.packages.iter().find(|p| p.name == LIBRARY_PACKAGE).expect("the runtime library");
        let library = engine.check(SourceFile::new(library.file.clone(), library.source.clone())).map_err(|e| e.render())?;
        let shape = library_type(&library, "Shape")?;
        let write = library_type(&library, "Write")?;
        let click = library_type(&library, "Click")?;
        let rows = TypeDesc::List(Rc::new(library_type(&library, "Row")?));
        let shapes = TypeDesc::List(Rc::new(shape));
        let writes = TypeDesc::List(Rc::new(write));
        let mut runtime = Runtime {
            package: package.to_string(),
            modules: Vec::new(),
            overlays: Vec::new(),
            readouts: Vec::new(),
            clicks: BTreeMap::new(),
            actions: BTreeMap::new(),
            animations: BTreeMap::new(),
            last: BTreeMap::new(),
            errors: BTreeMap::new(),
            reported: BTreeSet::new(),
            engine,
        };
        for (key, name) in &sources.scripts {
            let source = sources.packages.iter().find(|p| &p.name == name).expect("a script's source");
            let document = runtime.engine.check(SourceFile::new(source.file.clone(), source.source.clone())).map_err(|e| e.render())?;
            let mut instance = runtime.engine.instance(&document).map_err(|e| e.render())?;
            instance.set_limits(limits());
            let module = runtime.modules.len();
            for binding in document.bindings() {
                if binding.name.starts_with('_') {
                    continue;
                }
                let Some(TypeDesc::Func(params, ret)) = &binding.ty else { continue };
                let named = binding.name.clone();
                let taken = |m: &BTreeMap<String, Hook>| m.contains_key(&named);
                match params.as_slice() {
                    [TypeDesc::Float] if **ret == shapes => runtime.overlays.push(Hook { module, binding }),
                    [TypeDesc::Float] if **ret == rows => runtime.readouts.push(Hook { module, binding }),
                    [p] if **ret == writes && *p == click => {
                        if taken(&runtime.clicks) {
                            return Err(format!("{key}: a second click handler `{named}`"));
                        }
                        runtime.clicks.insert(named, Hook { module, binding });
                    }
                    [TypeDesc::Float] if **ret == writes => {
                        if taken(&runtime.animations) {
                            return Err(format!("{key}: a second animation `{named}`"));
                        }
                        runtime.animations.insert(named, Hook { module, binding });
                    }
                    [TypeDesc::Float, TypeDesc::Record(fields)] if **ret == writes => {
                        if runtime.actions.contains_key(&named) {
                            return Err(format!("{key}: a second action `{named}`"));
                        }
                        let args = fields.iter().map(|(l, t)| ScriptArg::of(l, t)).collect::<Result<Vec<_>, _>>().map_err(|e| format!("{key}: the action `{named}`: {e}"))?;
                        runtime.actions.insert(named, (Hook { module, binding }, args));
                    }
                    _ => {}
                }
            }
            runtime.modules.push(Module { name: key.clone(), instance, _document: document });
        }
        // A name is the scripts' or the code's, not both.
        let code = investigation.actions();
        if let Some(c) = code.clicks.iter().find(|c| runtime.clicks.contains_key(c.key)) {
            return Err(format!("the click handler `{}` is both a script's and the investigation's code's (`#[click]`)", c.key));
        }
        if let Some(a) = code.specs.iter().find(|a| runtime.actions.contains_key(a.key)) {
            return Err(format!("the action `{}` is both a script's and the investigation's code's (`#[action]`)", a.key));
        }
        // A demonstration's animation is one of the scripts'.
        if let Some(demos) = investigation.demo_specs() {
            for spec in &demos.specs {
                if let Some(name) = spec.animation.filter(|n| !runtime.animations.contains_key(*n)) {
                    let known: Vec<&str> = runtime.animations.keys().map(String::as_str).collect();
                    return Err(format!("demonstration `{}`: no animation `{name}` in the scripts (theirs: {})", spec.id.key(), if known.is_empty() { "none".into() } else { known.join(", ") }));
                }
            }
        }
        Ok(Some(runtime))
    }

    /// The hook `kind` names, if the scripts have it.
    fn hook(&self, kind: &Kind) -> Option<&Hook> {
        match kind {
            Kind::Overlay(i) => self.overlays.get(*i),
            Kind::Readout(i) => self.readouts.get(*i),
            Kind::Click(name) => self.clicks.get(name),
            Kind::Action(name) => self.actions.get(name).map(|a| &a.0),
            Kind::Animation(name) => self.animations.get(name),
        }
    }

    /// Calls the hook `kind` with `args` in the context given.
    fn call(&mut self, kind: &Kind, args: Vec<Value>, state: &ShotState, frame: Option<&FrameContent>, statics: Option<&Statics>) -> Result<Data, String> {
        let h = self.hook(kind).ok_or_else(|| format!("no {kind}"))?;
        let (module, binding) = (h.module, h.binding.clone());
        let instance = &mut self.modules[module].instance;
        let value = with_context(state, frame, statics, || instance.call(&binding, args)).map_err(|e| e.render())?;
        Data::from_value(&value).map_err(|e| format!("`{}` returned {e}", binding.name))
    }

    /// Calls a hook that returns writes and applies them to `target` (all or none): whether it set anything; None
    /// when the scripts have no such hook. A failure is reported (`failed`) and sets nothing.
    fn run(&mut self, kind: Kind, args: Vec<Value>, state: &ShotState, frame: Option<&FrameContent>, statics: Option<&Statics>, target: &mut ShotState) -> Option<bool> {
        self.hook(&kind)?;
        let name = kind.to_string();
        let applied = self.call(&kind, args, state, frame, statics).and_then(|d| writes(&d)).map(|w| apply(&w, target));
        match applied {
            Ok(set) => {
                self.succeeded(&name);
                Some(set)
            }
            Err(e) => {
                self.failed(&name, e);
                Some(false)
            }
        }
    }

    /// A hook failed: in a recording or a selftest, the run fails; in the viewer, the message is printed once and
    /// shown until the hook succeeds again.
    fn failed(&mut self, hook: &str, message: String) {
        if STRICT.load(Ordering::Relaxed) {
            panic!("script {hook} ({}): {message}", self.package);
        }
        if self.reported.insert(message.clone()) {
            eprintln!("script {hook} ({}): {message}", self.package);
        }
        self.errors.insert(hook.to_string(), message);
    }

    fn succeeded(&mut self, hook: &str) {
        self.errors.remove(hook);
    }
}

/// A hook, by kind and name.
enum Kind {
    Overlay(usize),
    Readout(usize),
    Click(String),
    Action(String),
    Animation(String),
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kind::Overlay(i) => write!(f, "overlay {i}"),
            Kind::Readout(i) => write!(f, "readout {i}"),
            Kind::Click(n) => write!(f, "click handler `{n}`"),
            Kind::Action(n) => write!(f, "action `{n}`"),
            Kind::Animation(n) => write!(f, "animation `{n}`"),
        }
    }
}

/// Whether a failing script fails the run (recordings, selftests) rather than showing its error.
static STRICT: AtomicBool = AtomicBool::new(false);

/// Makes a failing script fail the run: a recording or a selftest must not show a frame a script did not draw.
pub fn set_strict(strict: bool) {
    STRICT.store(strict, Ordering::Relaxed);
}

thread_local! {
    /// Each investigation's runtime on this thread, by its path: None when it has no scripts, an error when they
    /// did not check.
    static RUNTIMES: RefCell<BTreeMap<String, Result<Option<Runtime>, String>>> = const { RefCell::new(BTreeMap::new()) };
}

/// Runs `f` with the current investigation's runtime, loading it the first time; None when it has no scripts
/// (or, in the viewer, when they do not check: the message is printed once and shown on the frame).
fn with_runtime<R>(f: impl FnOnce(&mut Runtime) -> R) -> Result<Option<R>, String> {
    let investigation = investigation::current();
    // An investigation without a package of its own has no scripts.
    if investigation.package().is_none_or(|p| p == "render_lab") {
        return Ok(None);
    }
    RUNTIMES.with(|all| {
        let mut all = all.borrow_mut();
        let entry = all.entry(investigation.path()).or_insert_with(|| {
            let loaded = Runtime::load(investigation);
            if let Err(e) = &loaded {
                if STRICT.load(Ordering::Relaxed) {
                    panic!("the scripts of {}: {e}", investigation.path());
                }
                eprintln!("the scripts of {}: {e}", investigation.path());
            }
            loaded
        });
        match entry {
            Ok(Some(runtime)) => Ok(Some(f(runtime))),
            Ok(None) => Ok(None),
            Err(e) => Err(e.clone()),
        }
    })
}

/// Checks the current investigation's scripts now (a selftest, `render-lab script check`): the error when they do
/// not check, and the hooks they have.
pub fn check() -> Result<Vec<String>, String> {
    let investigation = investigation::current();
    let runtime = Runtime::load(investigation)?;
    let Some(r) = runtime else { return Ok(Vec::new()) };
    let mut hooks: Vec<String> = r.overlays.iter().map(|h| format!("overlay {} ({})", h.binding.name, r.modules[h.module].name)).collect();
    hooks.extend(r.readouts.iter().map(|h| format!("readout {} ({})", h.binding.name, r.modules[h.module].name)));
    hooks.extend(r.clicks.iter().map(|(n, h)| format!("click {n} ({})", r.modules[h.module].name)));
    hooks.extend(r.actions.iter().map(|(n, (h, args))| format!("action {n}({}) ({})", args.iter().map(ScriptArg::describe).collect::<Vec<_>>().join(", "), r.modules[h.module].name)));
    hooks.extend(r.animations.iter().map(|(n, h)| format!("animation {n} ({})", r.modules[h.module].name)));
    Ok(hooks)
}

// ---------------------------------------------------------------------------------------------------------
// Overlays

/// A shape an overlay drew, decoded before any is drawn.
#[derive(Clone, Debug)]
enum Shape {
    Segment { from: Vec3, to: Vec3, color: Color32, alpha: f32, width: f32, flags: u32 },
    Point { at: Vec3, radius: f32, color: Color32, alpha: f32 },
    Label { at: Vec3, text: String, color: Color32 },
}

fn field<'a>(d: &'a Data, label: &str) -> Result<&'a Data, String> {
    match d {
        Data::Record(fields) => fields.get(label).ok_or_else(|| format!("no field `{label}`")),
        _ => Err(format!("not a record (reading `{label}`)")),
    }
}

fn number(d: &Data, label: &str) -> Result<f32, String> {
    match field(d, label)? {
        Data::Float(x) => Ok(*x as f32),
        other => Err(format!("`{label}` is {other:?}, not a Float")),
    }
}

fn flag(d: &Data, label: &str) -> Result<bool, String> {
    match field(d, label)? {
        Data::Bool(b) => Ok(*b),
        other => Err(format!("`{label}` is {other:?}, not a Bool")),
    }
}

fn text<'a>(d: &'a Data, label: &str) -> Result<&'a str, String> {
    match field(d, label)? {
        Data::String(s) => Ok(s),
        other => Err(format!("`{label}` is {other:?}, not a String")),
    }
}

fn point(d: &Data, label: &str) -> Result<Vec3, String> {
    let p = field(d, label)?;
    Ok(Vec3::new(number(p, "x")?, number(p, "y")?, number(p, "z")?))
}

/// `#rrggbb`, or a colour of the lab's theme by name: `text`, `textDim`, `textFaint`, `accent`.
fn color(d: &Data) -> Result<Color32, String> {
    let s = text(d, "color")?;
    use crate::palette;
    match s {
        "text" => return Ok(palette::text()),
        "textDim" => return Ok(palette::text_dim()),
        "textFaint" => return Ok(palette::text_faint()),
        "accent" => return Ok(palette::accent()),
        _ => {}
    }
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6).and_then(|h| u32::from_str_radix(h, 16).ok()).ok_or_else(|| format!("the colour {s:?} is not #rrggbb"))?;
    Ok(crate::palette::rgb(hex))
}

/// The overlay's shapes, each checked; a label's text key looked up in `package`'s text, then the lab's.
fn shapes(list: &Data, package: &str) -> Result<Vec<Shape>, String> {
    let Data::List(items) = list else { return Err("an overlay returns a list".into()) };
    items
        .iter()
        .map(|item| {
            let Data::Variant(tag, d) = item else { return Err(format!("{item:?} is not a Shape")) };
            Ok(match tag.as_str() {
                "segment" => {
                    let flags = if flag(d, "arrow")? { FLAG_ARROW } else { 0 } | if flag(d, "dashed")? { FLAG_DASHED } else { 0 };
                    Shape::Segment { from: point(d, "from")?, to: point(d, "to")?, color: color(d)?, alpha: number(d, "alpha")?, width: number(d, "width")?, flags }
                }
                "point" => Shape::Point { at: point(d, "at")?, radius: number(d, "radius")?, color: color(d)?, alpha: number(d, "alpha")? },
                "label" => Shape::Label { at: point(d, "at")?, text: text(d, "text")?.to_string(), color: color(d)? },
                "labelText" => {
                    let key = text(d, "text")?;
                    let text = crate::i18n::try_package_text(package, key).or_else(|| crate::i18n::try_package_text("render_lab", key)).ok_or_else(|| format!("no text `{key}` in {package}'s text table or the lab's"))?;
                    Shape::Label { at: point(d, "at")?, text: text.to_string(), color: color(d)? }
                }
                other => return Err(format!("no shape `{other}`")),
            })
        })
        .collect()
}

/// A readout hook's rows, each checked.
fn readout_rows(list: &Data) -> Result<Vec<(String, String, Color32)>, String> {
    let Data::List(items) = list else { return Err("a readout returns a list of Rows".into()) };
    items.iter().map(|r| Ok((text(r, "name")?.to_string(), text(r, "value")?.to_string(), color(r)?))).collect()
}

/// Draws `shapes` at `at` in the frame's lists (`FrameContent::overlay_at`), or after everything; returns where they
/// end, so the next overlay follows them.
fn draw(shapes: &[Shape], content: &mut FrameContent, at: Option<DrawMark>) -> Option<DrawMark> {
    let mut drawn = FrameContent::new(Vec::new(), content.uniforms);
    for s in shapes {
        match s {
            Shape::Segment { from, to, color, alpha, width, flags } => drawn.draw.segment(*from, *to, *color, *alpha, *width, *flags),
            Shape::Point { at, radius, color, alpha } => drawn.draw.point(*at, *radius, *color, *alpha),
            Shape::Label { at, text, color } => drawn.labels.push(Label { anchor: *at, text: text.clone(), color: *color }),
        }
    }
    let Some(at) = at else {
        content.draw.segments.extend(drawn.draw.segments);
        content.draw.points.extend(drawn.draw.points);
        content.labels.extend(drawn.labels);
        return None;
    };
    let end = DrawMark { segments: at.segments + drawn.draw.segments.len(), points: at.points + drawn.draw.points.len(), labels: at.labels + drawn.labels.len() };
    content.draw.segments.splice(at.segments..at.segments, drawn.draw.segments);
    content.draw.points.splice(at.points..at.points, drawn.draw.points);
    content.labels.splice(at.labels..at.labels, drawn.labels);
    Some(end)
}

/// Draws the current investigation's overlays over `content` (after its `build_frame`) and adds its readouts' rows,
/// at `time` seconds, and shows the message of a failing hook. Nothing without scripts.
pub fn overlay(state: &ShotState, statics: &Statics, content: &mut FrameContent, time: f32) {
    let view = crate::view::name();
    let mut at = content.overlay_at.filter(|m| m.segments <= content.draw.segments.len() && m.points <= content.draw.points.len() && m.labels <= content.labels.len());
    let result = with_runtime(|rt| {
        for i in 0..rt.overlays.len() {
            let name = Kind::Overlay(i).to_string();
            let drawn = rt.call(&Kind::Overlay(i), vec![Value::Float(time as f64)], state, Some(content), Some(statics)).and_then(|d| shapes(&d, &rt.package));
            match drawn {
                Ok(s) => {
                    rt.succeeded(&name);
                    rt.last.insert((i, view), s);
                }
                Err(e) => rt.failed(&name, e),
            }
            if let Some(s) = rt.last.get(&(i, view)) {
                at = draw(s, content, at);
            }
        }
        for i in 0..rt.readouts.len() {
            let name = Kind::Readout(i).to_string();
            let rows = rt.call(&Kind::Readout(i), vec![Value::Float(time as f64)], state, Some(content), Some(statics)).and_then(|d| readout_rows(&d));
            match rows {
                Ok(rows) => {
                    rt.succeeded(&name);
                    content.readout.get_or_insert_with(Default::default).rows.extend(rows);
                }
                Err(e) => rt.failed(&name, e),
            }
        }
        rt.errors.values().cloned().collect::<Vec<_>>()
    });
    let errors = match result {
        Ok(errors) => errors.unwrap_or_default(),
        Err(e) => vec![e],
    };
    for e in errors {
        content.notes.push((crate::i18n::tr_with("script.error", crate::args!(message = e.lines().next().unwrap_or_default())), crate::palette::MISMATCH));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Writes

/// A hook's writes, each checked against the key's declaration (its type, a whole number's range, a choice's
/// options) before any is applied: the key and its value, or a component's number.
fn writes(list: &Data) -> Result<Vec<(String, Written)>, String> {
    let Data::List(items) = list else { return Err("a hook returns a list of Writes".into()) };
    items
        .iter()
        .map(|w| {
            let key = text(w, "key")?.to_string();
            let value = field(w, "value")?;
            let entry = values::entry(&key).ok_or_else(|| format!("the shot has no value `{key}`"))?;
            let written = if entry.key != key {
                // A vector's component.
                Written::Number(match value {
                    Data::Float(x) => *x as f32,
                    other => return Err(format!("`{key}` is a number, not {other:?}")),
                })
            } else {
                Written::Value(typed(&key, entry, value)?)
            };
            Ok((key, written))
        })
        .collect()
}

/// A value a hook writes.
enum Written {
    Value(LabValue),
    /// A vector's component, set by itself.
    Number(f32),
}

/// `value` as `entry`'s type.
fn typed(key: &str, entry: &values::Entry, value: &Data) -> Result<LabValue, String> {
    let wrong = || format!("`{key}` is a {}, not {value:?}", entry.ty.name());
    let whole = |n: i64, lo: i64, hi: i64| if (lo..=hi).contains(&n) { Ok(n) } else { Err(format!("`{key}` is a {}: {n} is out of its range", entry.ty.name())) };
    if let Some(c) = entry.choices {
        let index = match value {
            Data::String(name) => c.index(name).ok_or_else(|| format!("`{key}` has no option `{name}` (its options: {})", c.names()))? as i64,
            _ => return Err(wrong()),
        };
        return Ok(match entry.ty {
            Type::I32 => LabValue::I32(index as i32),
            _ => LabValue::U32(index as u32),
        });
    }
    let vector = |n: usize| -> Result<Vec<f32>, String> { ["x", "y", "z", "w"][..n].iter().map(|c| number(value, c)).collect() };
    Ok(match (entry.ty, value) {
        (Type::F32, Data::Float(x)) => LabValue::F32(*x as f32),
        (Type::I32, Data::Int(n)) => LabValue::I32(whole(*n, i32::MIN as i64, i32::MAX as i64)? as i32),
        (Type::U32, Data::Int(n)) => LabValue::U32(whole(*n, 0, u32::MAX as i64)? as u32),
        (Type::Bool, Data::Bool(b)) => LabValue::Bool(*b),
        (Type::Vec2, Data::Record(_)) => LabValue::Vec2(vector(2)?.try_into().expect("two numbers")),
        (Type::Vec3, Data::Record(_)) => LabValue::Vec3(vector(3)?.try_into().expect("three numbers")),
        (Type::Vec4, Data::Record(_)) => LabValue::Vec4(vector(4)?.try_into().expect("four numbers")),
        _ => return Err(wrong()),
    })
}

/// Applies checked writes, in order: whether there were any.
fn apply(writes: &[(String, Written)], state: &mut ShotState) -> bool {
    for (key, w) in writes {
        match w {
            Written::Value(v) => state.set_value(key, *v),
            Written::Number(x) => state.set(key, *x),
        }
    }
    !writes.is_empty()
}

// ---------------------------------------------------------------------------------------------------------
// Clicks

crate::lamlet_record! {
    /// Where a ray from the camera hits the scene (`rl_runtime/trace`).
    pub struct Hit {
        pub point: V3 = "point",
        /// The surface's normal there, as the object gives it (it may face away from the ray).
        pub normal: V3 = "normal",
        /// The ray's direction.
        pub dir: V3 = "dir",
        /// The object's index in the scene.
        pub object: i64 = "object",
        /// The object's name (`sphere`).
        pub name: String = "name",
    }
}

/// The ray from the frame's camera through `uv` (0..1 across and down) into its objects, skipping the objects named
/// in `skip` (a name the scene lacks skips nothing: the investigation's scenes differ).
fn trace(u: f64, v: f64, skip: &[String]) -> Result<Option<Hit>, String> {
    let names: Vec<&'static str> = statics(|s| s.scene.objects.iter().map(|o| o.name).collect())?;
    frame(|content| {
        let camera = &content.uniforms.camera;
        let (origin, dir) = crate::camera::Projector::new(camera).ray(glam::Vec2::new(u as f32, v as f32) * glam::Vec2::from(camera.viewport_size));
        let mut best: Option<(f32, Vec3, usize)> = None;
        for (i, o) in content.objects.iter().enumerate() {
            if names.get(i).is_some_and(|n| skip.iter().any(|s| s == n)) {
                continue;
            }
            if let Some((t, n)) = crate::scene::intersect_object(o, origin, dir, best.map_or(100.0, |b| b.0)) {
                best = Some((t, n, i));
            }
        }
        best.map(|(t, n, i)| Hit { point: V3(origin + dir * t), normal: V3(n), dir: V3(dir), object: i as i64, name: names.get(i).copied().unwrap_or_default().to_string() })
    })
}

/// Registers the natives that read the frame (`rl_runtime/trace`, `rl_runtime/viewport`).
fn register_frame_natives(engine: &mut Engine) -> Result<(), lamlet::embed::Error> {
    engine
        .natives_mut()
        .function("rl_runtime/trace", |_, (u, v, skip): (f64, f64, Vec<String>)| trace(u, v, &skip))
        .doc("Fn([Float, Float, List(String)], Option(Hit)). Where the ray from the frame's camera through (u, v) (0 to 1 across and down) first hits the scene, past the objects named in `skip` (names the scene lacks are ignored).")
        .register()?;
    engine
        .natives_mut()
        .function("rl_runtime/viewport", |_, ()| frame(|content| V2(glam::Vec2::from(content.uniforms.camera.viewport_size))))
        .doc("Fn([], Vec2). The frame's viewport in pixels: a click at (u, v) is at pixel (u x, v y).")
        .register()?;
    Ok(())
}

/// The scripts' click handlers, by name (a story gives one to an image: `imageClick`).
pub fn click_handlers() -> Vec<String> {
    with_runtime(|rt| rt.clicks.keys().cloned().collect()).ok().flatten().unwrap_or_default()
}

/// Runs the scripts' click handler `key` on `frame` (a click at `frame.uv` on the picture of `frame.content`):
/// whether it set anything; None when the scripts have no such handler. The handler named `scene` takes a click
/// on the scene itself, before `Investigation::click`.
pub fn click(key: &str, frame: &mut crate::actions::ClickFrame) -> Option<bool> {
    let click = lamlet::embed::record().field("u", frame.uv.x as f64).field("v", frame.uv.y as f64).build().ok()?;
    let (shown, content, statics) = (frame.shown, frame.content, frame.statics);
    with_runtime(|rt| rt.run(Kind::Click(key.to_string()), vec![click], shown, Some(content), Some(statics), frame.state)).ok().flatten().flatten()
}

// ---------------------------------------------------------------------------------------------------------
// Actions

/// An argument of a script action: a field of its `Args` record. Its type gives its kind as a story writes it:
/// `Float` a number, `Int` a whole number, `Bool` true or false, `String` a name, `Vec2`/`Vec3` numbers;
/// `Option(T)` an optional one.
#[derive(Clone, Debug)]
pub struct ScriptArg {
    pub key: String,
    pub kind: crate::actions::ArgKind,
    pub required: bool,
    ty: TypeDesc,
}

impl ScriptArg {
    fn of(key: &str, ty: &TypeDesc) -> Result<ScriptArg, String> {
        use crate::actions::ArgKind;
        let (inner, required) = match ty {
            TypeDesc::Variant(alts) if alts.len() == 2 && &*alts[0].0 == "none" && alts[0].1 == TypeDesc::Unit && &*alts[1].0 == "some" => (&alts[1].1, false),
            other => (other, true),
        };
        let kind = match inner {
            TypeDesc::Float => ArgKind::Number,
            TypeDesc::Int => ArgKind::Integer,
            TypeDesc::Bool => ArgKind::Toggle,
            TypeDesc::String => ArgKind::Name,
            TypeDesc::Record(fields) if *inner == floats(&["x", "y"]) || *inner == floats(&["x", "y", "z"]) => ArgKind::Vector(fields.len()),
            other => return Err(format!("the argument `{key}` is a {other}; an argument is a Float, an Int, a Bool, a String, a Vec2 or Vec3, or an Option of one")),
        };
        Ok(ScriptArg { key: key.to_string(), kind, required, ty: inner.clone() })
    }

    pub fn describe(&self) -> String {
        format!("{}: {}{}", self.key, self.kind.describe(), if self.required { "" } else { " (optional)" })
    }

    /// The story's value for it (JSON), as Lamlet.
    fn value(&self, given: Option<&serde_json::Value>) -> Result<Value, String> {
        let Some(v) = given.filter(|v| !v.is_null()) else {
            return if self.required { Err("missing".into()) } else { Ok(Value::variant("none", Value::Unit)) };
        };
        // A number at the lab's precision (f32), as the investigation's code reads it.
        let number = |v: &serde_json::Value| v.as_f64().map(|n| n as f32 as f64).ok_or_else(|| format!("{v} is not a number"));
        let value = match &self.ty {
            TypeDesc::Float => Value::Float(number(v)?),
            TypeDesc::Int => {
                let n = number(v)?;
                if n.fract() != 0.0 || n.abs() > 9.0e15 {
                    return Err(format!("{v} is not a whole number"));
                }
                Value::Int(n as i64)
            }
            TypeDesc::Bool => Value::Bool(v.as_bool().ok_or_else(|| format!("{v} is not true or false"))?),
            TypeDesc::String => Value::Str(Rc::from(v.as_str().ok_or_else(|| format!("{v} is not a name"))?)),
            TypeDesc::Record(fields) => {
                let list = v.as_array().filter(|a| a.len() == fields.len()).ok_or_else(|| format!("{v} is not {} numbers", fields.len()))?;
                let labels = ["x", "y", "z"];
                Value::record(list.iter().zip(labels).map(|(n, l)| Ok((Rc::from(l), Value::Float(number(n)?)))).collect::<Result<Vec<_>, String>>()?)
            }
            other => return Err(format!("an argument of type {other}")),
        };
        Ok(if self.required { value } else { Value::variant("some", value) })
    }
}

/// A script action: its name and arguments.
#[derive(Clone, Debug)]
pub struct ScriptAction {
    pub name: String,
    pub args: Vec<ScriptArg>,
}

/// The scripts' actions.
pub fn actions() -> Vec<ScriptAction> {
    with_runtime(|rt| rt.actions.iter().map(|(n, (_, args))| ScriptAction { name: n.clone(), args: args.clone() }).collect()).ok().flatten().unwrap_or_default()
}

/// The argument record a story's call gives the action `args` takes.
fn arguments(name: &str, args: &[ScriptArg], given: &BTreeMap<String, serde_json::Value>) -> Result<Value, String> {
    if let Some(extra) = given.keys().find(|k| !args.iter().any(|a| &a.key == *k)) {
        return Err(format!("the action `{name}` takes no argument `{extra}`"));
    }
    let fields = args.iter().map(|a| Ok((Rc::from(a.key.as_str()), a.value(given.get(&a.key)).map_err(|e| format!("the action `{name}`'s argument `{}` ({}): {e}", a.key, a.kind.describe()))?)));
    Ok(Value::record(fields.collect::<Result<Vec<_>, String>>()?))
}

/// Why a story's call of a script action cannot run (an argument it does not take, one missing or of the wrong
/// kind); None when the scripts have no such action.
pub fn check_action(call: &crate::actions::ActionCall) -> Option<Result<(), String>> {
    with_runtime(|rt| rt.actions.get(&call.name).map(|(_, args)| arguments(&call.name, args, &call.args).map(|_| ()))).ok().flatten().flatten()
}

/// Runs the script action `action` on `state` (the state so far in `Timeline::state_at`): None when the scripts have
/// no such action. Its writes apply all or none.
pub fn run_action(action: &crate::actions::RunningAction, state: &mut ShotState) -> Option<()> {
    let snapshot = state.clone();
    with_runtime(|rt| {
        let (_, args) = rt.actions.get(&action.key)?;
        let kind = Kind::Action(action.key.clone());
        match arguments(&action.key, args, &action.args) {
            Ok(record) => {
                rt.run(kind, vec![Value::Float(action.elapsed as f64), record], &snapshot, None, None, state);
            }
            Err(e) => rt.failed(&kind.to_string(), e),
        }
        Some(())
    })
    .ok()
    .flatten()
    .flatten()
}

// ---------------------------------------------------------------------------------------------------------
// Animations

/// The script animation of `demo` (its `animation`), if it has one.
fn animation_of(demo: crate::shot::DemoId) -> Option<&'static str> {
    investigation::current().demo_specs()?.specs.iter().find(|s| s.id == demo)?.animation
}

/// Applies `demo`'s script animation at `t` seconds to `state` (after the investigation's own animation): the keys
/// it set, or None when the demonstration has no script animation. Its writes apply all or none.
pub fn animate(demo: crate::shot::DemoId, t: f32, state: &mut ShotState) -> Option<Vec<String>> {
    let name = animation_of(demo)?;
    let snapshot = state.clone();
    with_runtime(|rt| {
        let kind = Kind::Animation(name.to_string());
        let written = rt.call(&kind, vec![Value::Float(t as f64)], &snapshot, None, None).and_then(|d| writes(&d));
        match written {
            Ok(w) => {
                rt.succeeded(&kind.to_string());
                apply(&w, state);
                w.into_iter().map(|(k, _)| k).collect()
            }
            Err(e) => {
                rt.failed(&kind.to_string(), e);
                Vec::new()
            }
        }
    })
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine with the runtime library (from its source: the built resource is not there in a unit test) and the
    /// lab's runtime natives' stubs.
    fn engine() -> Engine {
        let mut engine = Engine::new();
        register(&mut engine).unwrap();
        register_frame_natives(&mut engine).unwrap();
        let stubs = engine.natives_mut().declarations(NATIVES).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../packages/render_lab/language/runtime.lam");
        let library = Package::read(LIBRARY_PACKAGE, &path).unwrap();
        let packages = [library, Package { name: NATIVES.into(), file: "<stubs>".into(), source: stubs }];
        engine.set_resolver(super::super::resolver(&packages).unwrap());
        engine
    }

    /// Each expression region's value of a document that imports std and the runtime library.
    fn values(regions: &[&str]) -> Vec<Data> {
        let engine = engine();
        let body: String = regions.iter().map(|r| format!("${{{r}}}\n")).collect();
        let document = engine.check(SourceFile::new("test.lam", format!("$[Import* std]$[Import* {LIBRARY_PACKAGE}]\n{body}"))).unwrap_or_else(|e| panic!("{}", e.render()));
        let mut instance = engine.instance(&document).unwrap();
        instance.evaluate().unwrap_or_else(|e| panic!("{}", e.render())).iter().map(|(_, v)| Data::from_value(v).unwrap()).collect()
    }

    #[test]
    fn the_library_computes_what_it_says() {
        let v = values(&[
            "length {sweep {vec3 0.0 1.0 0.0} {vec3 1.0 0.0 0.0} 90}",
            "steps 0.7 2.8",
            "seq {' {span 1.0 {^ {x} x}} {span 2.0 {^ {x} {+ 10.0 x}}}} 2.5",
            "seq {' {span 1.0 {^ {x} x}} {span 2.0 {^ {x} {+ 10.0 x}}}} 9.0",
            "pingPong 2.0 3.0",
            "looping 2.0 5.0",
            "hex2 31",
            "hex2 5",
            "vectorText {vec3 1.0 -0.25 0.12345} 3",
            "easeInOut 0.25",
            "length {grownArrows {vec3 0.0 0.0 0.0} {' {named \"N\" {vec3 0.0 1.0 0.0} \"#ffffff\"} {named \"L\" {vec3 1.0 0.0 0.0} \"#ffffff\"}} 0.5 1.0 4.0 1.5}",
            "length {lobeLine {vec3 0.0 0.0 0.0} {' {vec3 0.0 1.0 0.0} {vec3 1.0 0.0 0.0}} {' 1.0 2.0} \"#ffffff\" 0.8 1.0 3.0 none}",
            "inPlane {vec3 0.0 1.0 0.0} {vec3 0.0 1.0 0.0}",
        ]);
        assert_eq!(v[0], Data::Int(91));
        // 2.8 / 0.7 is 3.9999999999999996: the fourth step counts from its frame.
        assert_eq!(v[1], Data::Int(4));
        assert_eq!(v[2], Data::Float(11.5));
        assert_eq!(v[3], Data::Float(12.0));
        assert_eq!(v[4], Data::Float(0.5));
        assert_eq!(v[5], Data::Float(0.5));
        assert_eq!(v[6], Data::String("1F".into()));
        assert_eq!(v[7], Data::String("05".into()));
        assert_eq!(v[8], Data::String("(1.000, -0.250, 0.123)".into()));
        assert_eq!(v[9], Data::Float(crate::animation::Ease::InOut.apply(0.25) as f64));
        // N whole with its label; L half grown, its label not yet out.
        assert_eq!(v[10], Data::Int(3));
        // Two values through the point: three segments.
        assert_eq!(v[11], Data::Int(3));
        // Along the normal, any perpendicular unit vector.
        assert!(matches!(&v[12], Data::Record(f) if f["y"] == Data::Float(0.0)));
    }

    #[test]
    fn shapes_are_checked_before_use() {
        let v = values(&["' {arrow {vec3 0.0 0.0 0.0} {vec3 1.0 0.0 0.0} \"#ff0000\" 1.0 4.0} {point {vec3 0.0 1.0 0.0} 8.0 \"accent\" 0.5}"]);
        let s = shapes(&v[0], "render_lab").unwrap();
        assert!(matches!(&s[0], Shape::Segment { flags, width, color, .. } if *flags == FLAG_ARROW && *width == 4.0 && *color == crate::palette::rgb(0xff0000)));
        assert!(matches!(&s[1], Shape::Point { alpha, .. } if *alpha == 0.5));
        let bad = values(&["' {segment {vec3 0.0 0.0 0.0} {vec3 1.0 0.0 0.0} \"red\" 1.0 4.0}"]);
        assert!(shapes(&bad[0], "render_lab").unwrap_err().contains("#rrggbb"));
    }

    #[test]
    fn action_arguments_take_their_kinds_from_their_types() {
        use crate::actions::ArgKind;
        let int = TypeDesc::Variant(vec![(Rc::from("none"), TypeDesc::Unit), (Rc::from("some"), TypeDesc::Int)]);
        let from = ScriptArg::of("from", &int).unwrap();
        assert!(!from.required && from.kind == ArgKind::Integer);
        let at = ScriptArg::of("at", &floats(&["x", "y", "z"])).unwrap();
        assert!(at.required && at.kind == ArgKind::Vector(3));
        assert!(ScriptArg::of("list", &TypeDesc::List(Rc::new(TypeDesc::Int))).is_err());
        // A story's numbers: a whole number for an Int, the lab's precision for a Float, none for an option left out.
        assert!(matches!(from.value(Some(&serde_json::json!(3.0))).unwrap(), Value::Variant(v) if v.0.as_ref() == "some" && matches!(v.1, Value::Int(3))));
        assert!(from.value(Some(&serde_json::json!(2.5))).is_err());
        assert!(matches!(from.value(None).unwrap(), Value::Variant(v) if v.0.as_ref() == "none"));
        let seconds = ScriptArg::of("seconds", &TypeDesc::Float).unwrap();
        assert!(matches!(seconds.value(Some(&serde_json::json!(0.7))).unwrap(), Value::Float(x) if x == 0.7f32 as f64));
        assert!(seconds.value(None).is_err());
        let args = [seconds, from];
        let given: BTreeMap<String, serde_json::Value> = [("secs".to_string(), serde_json::json!(1.0))].into();
        assert!(arguments("marchCone", &args, &given).unwrap_err().contains("no argument `secs`"));
    }
}
