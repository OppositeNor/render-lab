//! What a hook returns, read by its type (runtime.lam's): a number the lab keeps as an `f32`, a colour, a variant's
//! alternative, and a list whose item names itself in an error.

use super::*;
use lamlet::embed::FromLamlet;

/// `x` as an `f32`. A Lamlet Float is finite, but one past `f32`'s range would turn infinite: it is refused,
/// `what` named.
pub(super) fn single(x: f64, what: &str) -> Result<f32, String> {
    let y = x as f32;
    if y.is_finite() {
        Ok(y)
    } else {
        Err(format!("{what} is {x}, past the range of a 32-bit float"))
    }
}

/// A Float the lab keeps as an `f32` (a shape's alpha, a mob's size), refused past `f32`'s range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Num(pub f32);

impl LamletType for Num {
    fn lamlet_type() -> TypeDesc {
        TypeDesc::Float
    }
}

impl IntoLamlet for Num {
    fn into_lamlet(self) -> Result<Value, String> {
        (self.0 as f64).into_lamlet()
    }
}

impl FromLamlet for Num {
    fn from_lamlet(v: &Value) -> Result<Self, String> {
        match v {
            Value::Float(x) => Ok(Num(single(*x, "the number")?)),
            other => Err(format!("a {} is not a Float", other.type_name())),
        }
    }
}

/// runtime.lam's `Color`: one of the theme's colours, which follow the viewer's theme, or a fixed `0xrrggbb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptColor {
    Text,
    TextDim,
    TextFaint,
    Accent,
    Rgb(u32),
}

impl ScriptColor {
    /// The colour it names, in the viewer's theme now.
    pub fn color32(self) -> Color32 {
        use crate::palette;
        match self {
            ScriptColor::Text => palette::text(),
            ScriptColor::TextDim => palette::text_dim(),
            ScriptColor::TextFaint => palette::text_faint(),
            ScriptColor::Accent => palette::accent(),
            ScriptColor::Rgb(hex) => palette::rgb(hex),
        }
    }
}

impl LamletType for ScriptColor {
    fn lamlet_type() -> TypeDesc {
        let theme = ["accent", "text", "textDim", "textFaint"]
            .map(|l| (Rc::from(l), TypeDesc::Unit))
            .into_iter();
        let mut alternatives: Vec<(Rc<str>, TypeDesc)> = theme
            .chain(std::iter::once((Rc::from("rgb"), TypeDesc::Int)))
            .collect();
        alternatives.sort_by(|a, b| a.0.cmp(&b.0));
        TypeDesc::Variant(alternatives)
    }
}

impl IntoLamlet for ScriptColor {
    fn into_lamlet(self) -> Result<Value, String> {
        Ok(match self {
            ScriptColor::Text => Value::variant("text", Value::Unit),
            ScriptColor::TextDim => Value::variant("textDim", Value::Unit),
            ScriptColor::TextFaint => Value::variant("textFaint", Value::Unit),
            ScriptColor::Accent => Value::variant("accent", Value::Unit),
            ScriptColor::Rgb(hex) => Value::variant("rgb", Value::Int(hex as i64)),
        })
    }
}

impl FromLamlet for ScriptColor {
    fn from_lamlet(v: &Value) -> Result<Self, String> {
        let (tag, payload) = variant(v)?;
        Ok(match (tag, payload) {
            ("text", _) => ScriptColor::Text,
            ("textDim", _) => ScriptColor::TextDim,
            ("textFaint", _) => ScriptColor::TextFaint,
            ("accent", _) => ScriptColor::Accent,
            ("rgb", Value::Int(n)) if (0..=0xff_ffff).contains(n) => ScriptColor::Rgb(*n as u32),
            ("rgb", Value::Int(n)) => return Err(format!("the colour {n:#x} is not 0xrrggbb")),
            (other, _) => return Err(format!("no colour `{other}`")),
        })
    }
}

/// A variant's alternative and its payload.
pub(super) fn variant(v: &Value) -> Result<(&str, &Value), String> {
    match v {
        Value::Variant(p) => Ok((&p.0, &p.1)),
        other => Err(format!("a {} is not a variant", other.type_name())),
    }
}

/// A variant's payload as `T`, its alternative named in an error.
pub(super) fn payload<T: FromLamlet>(tag: &str, payload: &Value) -> Result<T, String> {
    T::from_lamlet(payload).map_err(|e| format!("`{tag}`: {e}"))
}

/// A list of `T`, each item numbered (from 0) in its error, as `what` (`shape`, `row`) names it.
pub(super) fn items<T: FromLamlet>(v: &Value, what: &str) -> Result<Vec<T>, String> {
    let Value::List(list) = v else {
        return Err(format!("a {} is not a list of {what}s", v.type_name()));
    };
    list.iter()
        .enumerate()
        .map(|(i, x)| T::from_lamlet(&x).map_err(|e| format!("{what} {i}: {e}")))
        .collect()
}
