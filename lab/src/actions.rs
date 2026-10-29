//! Actions: named pieces of a package's work that a story starts and stops,
//! the way a page's script calls its functions. A story writes
//! `{@ run=floodStep run.seconds=0.5}` and `{@ stop=floodStep}`; the
//! timeline (src/shot.rs) keeps each running action with the frame it
//! started, and evaluating a frame calls it with its arguments and the
//! seconds since then (`ActionFrame::elapsed`). An action sets the shot's
//! state (its values, camera, callout) before the frame is built, so it is
//! a function of the frame like everything else a timeline sets, and any
//! frame renders on its own.
//!
//! A package declares its actions with `#[lab::actions]` on an `impl`
//! block, one `#[action]` function each; their parameters after the frame
//! are the action's arguments, their kinds read from the parameters' types
//! (`ActionArg`):
//!
//! ```ignore
//! #[lab::actions]
//! impl Sdfgi {
//!     /// Steps the jump flood, one pass every `seconds`.
//!     #[action]
//!     fn jump_flood(frame: &mut ActionFrame, seconds: f32) {
//!         let step = (frame.elapsed / seconds).floor().min(8.0);
//!         frame.state.set("floodStep", step);
//!     }
//! }
//! ```
//!
//! The same block declares the package's click handlers, `#[click]`
//! functions of a `&mut ClickFrame`: what a click on an image's picture does
//! when a story names it (`{@ image=atlas imageClick=selectPage}`) or an
//! inset the package draws does (`PictureInset::click`). A click happens in
//! the viewer, on what is on screen, while no timeline drives the state (a
//! demonstration, or a timeline paused); a handler sets that state.
//!
//! The macro gives the type `ACTIONS`, which the investigation returns from
//! `Investigation::actions`. An action's key is its function's name in
//! camelCase (`jumpFlood`; `#[action(key = "…")]` names another), and its
//! `///` comment is its doc. An argument is required unless its type is an
//! `Option`.

use crate::shot::ShotState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// What an action runs in: the frame's state, which it sets, and the
/// seconds since the story started it.
pub struct ActionFrame<'a> {
    pub state: &'a mut ShotState,
    pub elapsed: f32,
}

/// What a click handler runs in: the state on screen, the state it sets,
/// and the point clicked on the image's picture (0..1, y down).
pub struct ClickFrame<'a> {
    pub shown: &'a ShotState,
    pub state: &'a mut ShotState,
    pub uv: glam::Vec2,
    pub content: &'a crate::frame::FrameContent,
    pub statics: &'a crate::frame::Statics,
}

/// A click handler a package declares (`#[click]`).
pub struct ClickSpec {
    pub key: &'static str,
    pub doc: &'static str,
}

/// The kind of an action's argument, as a story writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// A number.
    Number,
    /// A whole number, 0 or more.
    Index,
    /// A whole number of either sign.
    Integer,
    /// `true` or `false`.
    Toggle,
    /// This many numbers, comma-separated (`34,52,61`).
    Vector(usize),
    /// A word (`probe`).
    Name,
}

impl ArgKind {
    pub fn describe(self) -> String {
        match self {
            ArgKind::Number => "a number".into(),
            ArgKind::Index => "a whole number, 0 or more".into(),
            ArgKind::Integer => "a whole number".into(),
            ArgKind::Toggle => "true or false".into(),
            ArgKind::Vector(n) => format!("{n} numbers"),
            ArgKind::Name => "a name".into(),
        }
    }
}

/// A type an action's argument can be: how a story writes it, and how its
/// value is read.
pub trait ActionArg: Sized {
    const KIND: ArgKind;
    /// Whether the story must give it (an `Option` need not).
    const REQUIRED: bool = true;
    /// The value the story wrote, or None when it wrote none.
    fn read(value: Option<&Value>) -> Result<Self, String>;

    /// Whether `value` is one (what validation runs).
    fn check(value: Option<&Value>) -> Result<(), String> {
        Self::read(value).map(|_| ())
    }
}

fn given(value: Option<&Value>) -> Result<&Value, String> {
    value.ok_or_else(|| "missing".to_string())
}

fn number(value: &Value) -> Result<f64, String> {
    value.as_f64().ok_or_else(|| format!("{value} is not a number"))
}

fn whole(value: &Value) -> Result<i64, String> {
    let n = number(value)?;
    if n.fract() != 0.0 {
        return Err(format!("{value} is not a whole number"));
    }
    Ok(n as i64)
}

fn numbers<const N: usize>(value: &Value) -> Result<[f32; N], String> {
    let list = value.as_array().filter(|a| a.len() == N).ok_or_else(|| format!("{value} is not {N} numbers"))?;
    let mut out = [0.0; N];
    for (o, v) in out.iter_mut().zip(list) {
        *o = number(v)? as f32;
    }
    Ok(out)
}

impl ActionArg for f32 {
    const KIND: ArgKind = ArgKind::Number;
    fn read(value: Option<&Value>) -> Result<f32, String> {
        Ok(number(given(value)?)? as f32)
    }
}

impl ActionArg for u32 {
    const KIND: ArgKind = ArgKind::Index;
    fn read(value: Option<&Value>) -> Result<u32, String> {
        let value = given(value)?;
        u32::try_from(whole(value)?).map_err(|_| format!("{value} is not 0 or more"))
    }
}

impl ActionArg for i32 {
    const KIND: ArgKind = ArgKind::Integer;
    fn read(value: Option<&Value>) -> Result<i32, String> {
        let value = given(value)?;
        i32::try_from(whole(value)?).map_err(|_| format!("{value} is out of range"))
    }
}

impl ActionArg for bool {
    const KIND: ArgKind = ArgKind::Toggle;
    fn read(value: Option<&Value>) -> Result<bool, String> {
        let value = given(value)?;
        value.as_bool().ok_or_else(|| format!("{value} is not true or false"))
    }
}

impl ActionArg for String {
    const KIND: ArgKind = ArgKind::Name;
    fn read(value: Option<&Value>) -> Result<String, String> {
        let value = given(value)?;
        value.as_str().map(str::to_string).ok_or_else(|| format!("{value} is not a name"))
    }
}

impl ActionArg for glam::Vec2 {
    const KIND: ArgKind = ArgKind::Vector(2);
    fn read(value: Option<&Value>) -> Result<glam::Vec2, String> {
        numbers::<2>(given(value)?).map(glam::Vec2::from)
    }
}

impl ActionArg for glam::Vec3 {
    const KIND: ArgKind = ArgKind::Vector(3);
    fn read(value: Option<&Value>) -> Result<glam::Vec3, String> {
        numbers::<3>(given(value)?).map(glam::Vec3::from)
    }
}

impl ActionArg for glam::IVec3 {
    const KIND: ArgKind = ArgKind::Vector(3);
    fn read(value: Option<&Value>) -> Result<glam::IVec3, String> {
        let v = numbers::<3>(given(value)?)?;
        if v.iter().any(|c| c.fract() != 0.0) {
            return Err(format!("{v:?} are not whole numbers"));
        }
        Ok(glam::IVec3::new(v[0] as i32, v[1] as i32, v[2] as i32))
    }
}

impl<T: ActionArg> ActionArg for Option<T> {
    const KIND: ArgKind = T::KIND;
    const REQUIRED: bool = false;
    fn read(value: Option<&Value>) -> Result<Option<T>, String> {
        value.map(|v| T::read(Some(v))).transpose()
    }
}

/// An argument an action takes.
pub struct ArgSpec {
    /// Its key in a story (`run.<key>=…`).
    pub key: &'static str,
    pub kind: ArgKind,
    pub required: bool,
    pub check: fn(Option<&Value>) -> Result<(), String>,
}

/// An action a package declares (`#[action]`).
pub struct ActionSpec {
    pub key: &'static str,
    pub doc: &'static str,
    pub args: &'static [ArgSpec],
}

/// A package's actions and click handlers: what they are, and how one runs
/// (`#[lab::actions]` writes both).
pub struct Actions {
    pub specs: &'static [ActionSpec],
    pub run: fn(&RunningAction, &mut ActionFrame) -> Result<(), String>,
    pub clicks: &'static [ClickSpec],
    /// Runs click handler `key`: whether it set the state.
    pub click: fn(&str, &mut ClickFrame) -> Result<bool, String>,
}

/// An investigation without actions or click handlers.
pub static NONE: Actions = Actions { specs: &[], run: |action, _| Err(format!("no action `{}`", action.key)), clicks: &[], click: |key, _| Err(format!("no click handler `{key}`")) };

impl Actions {
    /// Why a story cannot name click handler `key`: one these do not declare.
    pub fn check_click(&self, key: &str) -> Result<(), String> {
        if self.clicks.iter().any(|c| c.key == key) {
            return Ok(());
        }
        let known: Vec<&str> = self.clicks.iter().map(|c| c.key).collect();
        Err(format!("no click handler `{key}` (the investigation's: {})", if known.is_empty() { "none".into() } else { known.join(", ") }))
    }

    pub fn spec(&self, key: &str) -> Option<&'static ActionSpec> {
        self.specs.iter().find(|s| s.key == key)
    }

    /// Why `call` cannot run: an action these do not declare, an argument
    /// it does not take, or one missing or of the wrong kind.
    pub fn check(&self, call: &ActionCall) -> Result<(), String> {
        let spec = self.spec(&call.name).ok_or_else(|| {
            let known: Vec<&str> = self.specs.iter().map(|s| s.key).collect();
            format!("no action `{}` (the investigation's actions: {})", call.name, if known.is_empty() { "none".into() } else { known.join(", ") })
        })?;
        if let Some(extra) = call.args.keys().find(|k| !spec.args.iter().any(|a| a.key == k.as_str())) {
            return Err(format!("the action `{}` takes no argument `{extra}`", spec.key));
        }
        for arg in spec.args {
            (arg.check)(call.args.get(arg.key)).map_err(|e| format!("the action `{}`'s argument `{}` ({}): {e}", spec.key, arg.key, arg.kind.describe()))?;
        }
        Ok(())
    }
}

/// A story's call of an action, as a timeline key holds it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionCall {
    pub name: String,
    #[serde(default)]
    pub args: BTreeMap<String, Value>,
}

/// An action the story has started and not stopped, at the frame evaluated.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunningAction {
    pub key: String,
    pub args: BTreeMap<String, Value>,
    /// The frame it started at, and the seconds since then.
    pub started: u64,
    pub elapsed: f32,
}

impl RunningAction {
    /// Its argument `key`, as `T` (what `#[lab::actions]`'s dispatch reads).
    pub fn arg<T: ActionArg>(&self, key: &str) -> Result<T, String> {
        T::read(self.args.get(key)).map_err(|e| format!("the action `{}`'s argument `{key}`: {e}", self.key))
    }
}
