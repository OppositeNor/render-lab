//! Scripts: Lamlet a package runs while the viewer plays and while a video records (the `render-lab-lamlet` skill).
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

use super::{Package, natives};
use crate::frame::{DrawMark, FLAG_ARROW, FLAG_DASHED, FrameContent, Label, Statics};
use crate::investigation::{self, Investigation};
use crate::shot::ShotState;
use crate::values::{self, Type, Value as LabValue};
use egui::Color32;
use glam::Vec3;
use lamlet::embed::{
    Binding, Data, Document, Engine, Instance, IntoLamlet, LamletType, Part, SourceFile,
};
use lamlet::types::TypeDesc;
use lamlet::{Limits, Value};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

mod overlays;
mod writes;
pub use overlays::*;
pub use writes::*;

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
fn with_context<R>(
    state: &ShotState,
    frame: Option<&FrameContent>,
    statics: Option<&Statics>,
    f: impl FnOnce() -> R,
) -> R {
    struct Restore(Option<Context>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CONTEXT.with(|c| c.set(self.0));
        }
    }
    let context = Context {
        state,
        frame: frame.map(|f| f as *const FrameContent),
        statics: statics.map(|s| s as *const Statics),
    };
    let _restore = Restore(CONTEXT.with(|c| c.replace(Some(context))));
    f()
}

fn context() -> Result<Context, String> {
    CONTEXT.with(Cell::get).ok_or_else(|| {
        "this native reads the call's context: it runs only inside a hook".to_string()
    })
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
    let frame = c.frame.ok_or_else(|| {
        "this native reads the frame: it runs only in an overlay or a click handler".to_string()
    })?;
    // SAFETY: as in `state`.
    Ok(f(unsafe { &*frame }))
}

/// The scene's statics, where the running hook has them (an overlay, a click).
pub fn statics<R>(f: impl FnOnce(&Statics) -> R) -> Result<R, String> {
    let c = context()?;
    let statics = c.statics.ok_or_else(|| {
        "this native reads the scene: it runs only in an overlay or a click handler".to_string()
    })?;
    // SAFETY: as in `state`.
    Ok(f(unsafe { &*statics }))
}

/// What the investigation's `build_frame` left for its natives in this frame (`FrameContent::script`), as `T`.
pub fn frame_data<T: Any + Send + Sync>() -> Result<Arc<T>, String> {
    let data = frame(|f| f.script.clone())?.ok_or_else(|| {
        "the frame has no data for the scripts (`FrameContent::script`)".to_string()
    })?;
    data.downcast::<T>().map_err(|_| {
        format!(
            "the frame's data for the scripts is not a {}",
            std::any::type_name::<T>()
        )
    })
}

// ---------------------------------------------------------------------------------------------------------
// Vectors and records across the boundary

/// A record type of Float fields, sorted by label as Lamlet keeps them.
fn floats(labels: &[&str]) -> TypeDesc {
    let mut fields: Vec<(Rc<str>, TypeDesc)> = labels
        .iter()
        .map(|l| (Rc::from(*l), TypeDesc::Float))
        .collect();
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    TypeDesc::Record(fields)
}

fn float_field(v: &Value, label: &str) -> Result<f32, String> {
    match v.field(label) {
        Some(Value::Float(x)) => Ok(*x as f32),
        Some(other) => Err(format!(
            "field `{label}` is a {}, not a Float",
            other.type_name()
        )),
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
    r.function("rl_runtime/value-bool", |_, (key,): (String,)| {
        Ok(value_of(&key)?.f32() > 0.5)
    })
    .doc("Fn([String], Bool). The shot's value `key`, on above 0.5.")
    .register()?;
    r.function(
        "rl_runtime/value-vec2",
        |_, (key,): (String,)| match value_of(&key)? {
            LabValue::Vec2(v) => Ok(V2(glam::Vec2::from(v))),
            other => Err(format!("`{key}` is a {}, not a vec2", other.ty().name())),
        },
    )
    .doc("Fn([String], Vec2). The shot's vec2 value `key`.")
    .register()?;
    r.function(
        "rl_runtime/value-vec3",
        |_, (key,): (String,)| match value_of(&key)? {
            LabValue::Vec3(v) => Ok(V3(Vec3::from(v))),
            other => Err(format!("`{key}` is a {}, not a vec3", other.ty().name())),
        },
    )
    .doc("Fn([String], Vec3). The shot's vec3 value `key`.")
    .register()?;
    r.function(
        "rl_runtime/value-vec4",
        |_, (key,): (String,)| match value_of(&key)? {
            LabValue::Vec4(v) => Ok(V4(glam::Vec4::from(v))),
            other => Err(format!("`{key}` is a {}, not a vec4", other.ty().name())),
        },
    )
    .doc("Fn([String], Vec4). The shot's vec4 value `key`.")
    .register()?;
    r.function("rl_runtime/value-choice", |_, (key,): (String,)| {
        let choices = values::entry(&key)
            .and_then(|e| e.choices)
            .ok_or_else(|| format!("`{key}` is not a choice"))?;
        let index = value_of(&key)?.u32() as usize;
        choices
            .options
            .get(index)
            .map(|o| o.name.to_string())
            .ok_or_else(|| format!("`{key}` is {index}, past its options ({})", choices.names()))
    })
    .doc("Fn([String], String). The name of the option the shot's choice `key` holds.")
    .register()?;
    r.function("rl_runtime/text", |_, (key,): (String,)| {
        let package = investigation::current().package().unwrap_or("render_lab");
        crate::i18n::try_package_text(package, &key).or_else(|| crate::i18n::try_package_text("render_lab", &key)).map(str::to_string).ok_or_else(|| format!("no text `{key}` in {package}'s text table or the lab's"))
    })
    .doc("Fn([String], String). The investigation's package's text `key` in the viewer's language, or the lab's.")
    .register()?;
    r.function("rl_runtime/demo", |_, ()| {
        state(|s| s.demo.key().to_string())
    })
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
            // A declared name runs to the next space or bracket; one starting with `_` is private.
            let name: String = source[i + head.len()..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !"{}[]".contains(*c))
                .collect();
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
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
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
    let mut emit = |key: &str,
                    ty: Type,
                    choices: Option<&'static values::Choices>,
                    out: &mut String| {
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
        if let Some(c) = choices.filter(|c| {
            c.options
                .iter()
                .all(|o| lamlet_present::vocab::camel(&[o.name]).is_ok_and(|n| n == o.name))
        }) {
            let t = c.name;
            if types.insert(t) {
                let fields: Vec<String> = c
                    .options
                    .iter()
                    .map(|o| format!("{{std::Field {} std::Unit}}", quoted(o.name)))
                    .collect();
                out.push_str(&format!(
                    "\nThe options of `{t}`.\n$[# {t} [% {{std::Variant {{' {}}}}}]]\n",
                    fields.join(" ")
                ));
                let mut of = format!("{{std::error {{std::++ \"no option of {t} is named \" s}}}}");
                for o in c.options.iter().rev() {
                    of = format!(
                        "{{std::caseBool {{std::= s {}}} {{^ {{}} {{: {{#{} std::unit}} {t}}}}} {{^ {{}} {of}}}}}",
                        quoted(o.name),
                        o.name
                    );
                }
                out.push_str(&format!("$[# {{_of{t} s}} {of}]\n"));
                let handlers: Vec<String> = c
                    .options
                    .iter()
                    .map(|o| format!("{} {{^ {{u}} {}}}", o.name, quoted(o.name)))
                    .collect();
                out.push_str(&format!(
                    "$[# {{_name{t} o}} {{std::match {{: o {t}}} {{. {}}}}}]\n",
                    handlers.join(" ")
                ));
            }
            out.push_str(&format!(
                "\nThe shot's `{key}`.\n$[# {{{name}}} {{_of{t} {{rt::valueChoice {k}}}}}]\n"
            ));
            out.push_str(&format!(
                "Sets `{key}`.\n$[# {{{setter} o}} {{_write {k} {{std::toData {{_name{t} o}}}}}}]\n"
            ));
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
        out.push_str(&format!(
            "\nThe shot's `{key}`.\n$[# {{{name}}} {{rt::{reader} {k}}}]\n"
        ));
        out.push_str(&format!(
            "Sets `{key}`.\n$[# {{{setter} x}} {{_write {k} {{std::toData {{: x {ty}}}}}}}]\n"
        ));
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
    Limits {
        max_instructions: 2_000_000,
        max_frames: 10_000,
        max_stack: 1_000_000,
        max_alloc_bytes: 64 << 20,
        max_native_calls: 100_000,
        ..Limits::default()
    }
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
    let r = crate::resources::find(key)
        .ok_or_else(|| format!("no resource {key} (render-lab cook)"))?;
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
    investigation
        .script_natives(engine.natives_mut())
        .map_err(|e| format!("{}'s natives: {}", investigation.path(), e.render()))?;
    let runtime_stubs = engine
        .natives_mut()
        .declarations(NATIVES)
        .map_err(|e| e.render())?;
    let package_stubs = engine
        .natives_mut()
        .declarations(package)
        .map_err(|e| e.render())?;
    let (library_file, library) = resource_text(LIBRARY)?;
    let mut reserved = declared(lamlet::package::STD_SOURCE);
    reserved.extend(declared(&library));
    reserved.extend(declared(&package_stubs));
    let (values, skipped) = values_source(package, &reserved);
    let mut packages = vec![
        Package::render_lab()?,
        Package {
            name: LIBRARY_PACKAGE.into(),
            file: library_file,
            source: library,
        },
        Package {
            name: NATIVES.into(),
            file: format!("<{NATIVES} natives>"),
            source: runtime_stubs,
        },
        Package {
            name: format!("{package}_natives"),
            file: format!("<{package} natives>"),
            source: package_stubs,
        },
        Package {
            name: format!("{package}_values"),
            file: format!("<{package} values>"),
            source: values,
        },
    ];
    let prefix = format!("{package}.script/");
    let mut scripts = Vec::new();
    for r in crate::resources::all()
        .iter()
        .filter(|r| r.id().starts_with(&prefix))
    {
        let name = format!("{package}_{}", r.id()[prefix.len()..].replace('-', "_"));
        let (file, source) = resource_text(r.id())?;
        packages.push(Package {
            name: name.clone(),
            file,
            source,
        });
        scripts.push((r.id().to_string(), name));
    }
    engine.set_resolver(super::resolver(&packages)?);
    Ok((
        engine,
        Sources {
            packages,
            scripts,
            skipped,
        },
    ))
}

/// The names a document's own declarations bind (`[# name …]`, `[# {name args} …]`): its bindings without those
/// its imports bring.
fn declared_names(document: &Document, source: &str) -> BTreeSet<String> {
    document
        .parts()
        .into_iter()
        .filter_map(|part| {
            let Part::Declaration { region } = part else {
                return None;
            };
            let text = source.get(region.at.start as usize..region.at.end as usize)?;
            let rest = text.trim_start_matches(['$', '[']).strip_prefix('#')?;
            let rest = rest.trim_start().trim_start_matches('{');
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// A Type a runtime library binding holds (`Shape`, `Write`, `Click`).
fn library_type(library: &Document, name: &str) -> Result<TypeDesc, String> {
    match library.binding(name).map_err(|e| e.render())?.value {
        Value::Type(t) => Ok((*t).clone()),
        other => Err(format!(
            "{LIBRARY}: `{name}` is a {}, not a Type",
            other.type_name()
        )),
    }
}

/// The error for a function `named` that returns what a hook returns (`ret`) but takes what none takes
/// (`params`); Ok when it returns no hook's type.
fn wrong_hook(
    key: &str,
    named: &str,
    params: &[TypeDesc],
    ret: &TypeDesc,
    types: &HookTypes,
) -> Result<(), String> {
    let Some(returns) = [
        (&types.shapes, "Shape"),
        (&types.rows, "Row"),
        (&types.writes, "Write"),
    ]
    .into_iter()
    .find(|(t, _)| *t == ret)
    .map(|(_, n)| n) else {
        return Ok(());
    };
    let expected = if returns == "Write" {
        "(Click) for a click handler, (Float) for an animation, or (Float, a record) for an \
         action"
    } else {
        "(Float)"
    };
    let takes = params
        .iter()
        .map(|p| match p == &types.click {
            true => "Click".to_string(),
            false => p.to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "{key}: `{named}` returns List({returns}) as a hook does, but takes ({takes}); a hook \
         returning List({returns}) takes {expected} (render_lab_runtime). A helper's name \
         starts with `_`"
    ))
}

/// The types a hook's signature is told by, from the runtime library.
struct HookTypes {
    shapes: TypeDesc,
    rows: TypeDesc,
    writes: TypeDesc,
    click: TypeDesc,
}

impl HookTypes {
    fn of(engine: &Engine, sources: &Sources) -> Result<HookTypes, String> {
        let library = sources
            .packages
            .iter()
            .find(|p| p.name == LIBRARY_PACKAGE)
            .expect("the runtime library");
        let library = engine
            .check(SourceFile::new(
                library.file.clone(),
                library.source.clone(),
            ))
            .map_err(|e| e.render())?;
        Ok(HookTypes {
            shapes: TypeDesc::List(Rc::new(library_type(&library, "Shape")?)),
            rows: TypeDesc::List(Rc::new(library_type(&library, "Row")?)),
            writes: TypeDesc::List(Rc::new(library_type(&library, "Write")?)),
            click: library_type(&library, "Click")?,
        })
    }
}

impl Runtime {
    /// `investigation`'s scripts checked, or None when its package has none.
    fn load(investigation: &'static dyn Investigation) -> Result<Option<Runtime>, String> {
        let package = investigation.package().unwrap_or("render_lab");
        let prefix = format!("{package}.script/");
        if package == "render_lab"
            || !crate::resources::all()
                .iter()
                .any(|r| r.id().starts_with(&prefix))
        {
            return Ok(None);
        }
        let (engine, sources) = engine(investigation)?;
        let types = HookTypes::of(&engine, &sources)?;
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
            let source = sources
                .packages
                .iter()
                .find(|p| &p.name == name)
                .expect("a script's source");
            runtime.add_module(key, source, &types)?;
        }
        runtime.check_names(investigation)?;
        Ok(Some(runtime))
    }

    /// The script `key` (its package `source`) checked, its hooks added.
    fn add_module(&mut self, key: &str, source: &Package, types: &HookTypes) -> Result<(), String> {
        let document = self
            .engine
            .check(SourceFile::new(source.file.clone(), source.source.clone()))
            .map_err(|e| e.render())?;
        let mut instance = self.engine.instance(&document).map_err(|e| e.render())?;
        instance.set_limits(limits());
        let module = self.modules.len();
        let own = declared_names(&document, &source.source);
        for binding in document.bindings() {
            // A hook is the script's own binding, not one it imports.
            if binding.name.starts_with('_') || !own.contains(&binding.name) {
                continue;
            }
            self.add_hook(key, Hook { module, binding }, types)?;
        }
        self.modules.push(Module {
            name: key.to_string(),
            instance,
            _document: document,
        });
        Ok(())
    }

    /// A binding of the script `key` added as the hook its type makes it, if it is one.
    fn add_hook(&mut self, key: &str, hook: Hook, types: &HookTypes) -> Result<(), String> {
        let binding = &hook.binding;
        let Some(TypeDesc::Func(params, ret)) = &binding.ty else {
            // A generic function (a parameter it never constrains) has no hook's type.
            if binding.ty.is_none() && matches!(binding.value, Value::Closure(_) | Value::Native(_))
            {
                return Err(format!(
                    "{key}: `{}` is a generic function ({}), never called as a hook; ascribe a hook's \
                     type (render_lab_runtime), or start a helper's name with `_`",
                    binding.name, binding.type_text
                ));
            }
            return Ok(());
        };
        let (params, ret) = (params.clone(), ret.clone());
        let named = binding.name.clone();
        let taken = |m: &BTreeMap<String, Hook>| m.contains_key(&named);
        // A hook's type says its kind: from time, shapes make an overlay, rows a readout, writes an animation.
        match params.as_slice() {
            [TypeDesc::Float] if *ret == types.shapes => self.overlays.push(hook),
            [TypeDesc::Float] if *ret == types.rows => self.readouts.push(hook),
            [p] if *ret == types.writes && *p == types.click => {
                if taken(&self.clicks) {
                    return Err(format!("{key}: a second click handler `{named}`"));
                }
                self.clicks.insert(named, hook);
            }
            [TypeDesc::Float] if *ret == types.writes => {
                if taken(&self.animations) {
                    return Err(format!("{key}: a second animation `{named}`"));
                }
                self.animations.insert(named, hook);
            }
            [TypeDesc::Float, TypeDesc::Record(fields)] if *ret == types.writes => {
                if self.actions.contains_key(&named) {
                    return Err(format!("{key}: a second action `{named}`"));
                }
                let args = fields
                    .iter()
                    .map(|(l, t)| ScriptArg::of(l, t))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| format!("{key}: the action `{named}`: {e}"))?;
                self.actions.insert(named, (hook, args));
            }
            // A function that returns what a hook returns but takes what none takes is a hook whose
            // signature is wrong: refused, rather than never called.
            _ => return wrong_hook(key, &named, &params, &ret, types),
        }
        Ok(())
    }

    /// Refuses a name both the scripts and the code take, and a demonstration's animation the scripts lack.
    fn check_names(&self, investigation: &'static dyn Investigation) -> Result<(), String> {
        // A name is the scripts' or the code's, not both.
        let code = investigation.actions();
        if let Some(c) = code.clicks.iter().find(|c| self.clicks.contains_key(c.key)) {
            return Err(format!(
                "the click handler `{}` is both a script's and the investigation's code's (`#[click]`)",
                c.key
            ));
        }
        if let Some(a) = code.specs.iter().find(|a| self.actions.contains_key(a.key)) {
            return Err(format!(
                "the action `{}` is both a script's and the investigation's code's (`#[action]`)",
                a.key
            ));
        }
        // A demonstration's animation is one of the scripts'.
        if let Some(demos) = investigation.demo_specs() {
            for spec in &demos.specs {
                if let Some(name) = spec.animation.filter(|n| !self.animations.contains_key(*n)) {
                    let known: Vec<&str> = self.animations.keys().map(String::as_str).collect();
                    return Err(format!(
                        "demonstration `{}`: no animation `{name}` in the scripts (theirs: {})",
                        spec.id.key(),
                        crate::story_refs::names_or_none(&known)
                    ));
                }
            }
        }
        Ok(())
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
    fn call(
        &mut self,
        kind: &Kind,
        args: Vec<Value>,
        state: &ShotState,
        frame: Option<&FrameContent>,
        statics: Option<&Statics>,
    ) -> Result<Data, String> {
        let h = self.hook(kind).ok_or_else(|| format!("no {kind}"))?;
        let (module, binding) = (h.module, h.binding.clone());
        let instance = &mut self.modules[module].instance;
        let value = with_context(state, frame, statics, || instance.call(&binding, args))
            .map_err(|e| e.render())?;
        Data::from_value(&value).map_err(|e| format!("`{}` returned {e}", binding.name))
    }

    /// Calls a hook that returns writes and applies them to `target` (all or none): whether it set anything; None
    /// when the scripts have no such hook. A failure is reported (`failed`) and sets nothing.
    fn run(
        &mut self,
        kind: Kind,
        args: Vec<Value>,
        state: &ShotState,
        frame: Option<&FrameContent>,
        statics: Option<&Statics>,
        target: &mut ShotState,
    ) -> Option<bool> {
        self.hook(&kind)?;
        let name = kind.to_string();
        let applied = self
            .call(&kind, args, state, frame, statics)
            .and_then(|d| writes(&d))
            .map(|w| apply(&w, target));
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
    let Some(r) = runtime else {
        return Ok(Vec::new());
    };
    let mut hooks: Vec<String> = r
        .overlays
        .iter()
        .map(|h| format!("overlay {} ({})", h.binding.name, r.modules[h.module].name))
        .collect();
    hooks.extend(
        r.readouts
            .iter()
            .map(|h| format!("readout {} ({})", h.binding.name, r.modules[h.module].name)),
    );
    hooks.extend(
        r.clicks
            .iter()
            .map(|(n, h)| format!("click {n} ({})", r.modules[h.module].name)),
    );
    hooks.extend(r.actions.iter().map(|(n, (h, args))| {
        format!(
            "action {n}({}) ({})",
            args.iter()
                .map(ScriptArg::describe)
                .collect::<Vec<_>>()
                .join(", "),
            r.modules[h.module].name
        )
    }));
    hooks.extend(
        r.animations
            .iter()
            .map(|(n, h)| format!("animation {n} ({})", r.modules[h.module].name)),
    );
    Ok(hooks)
}

#[cfg(test)]
mod tests;
