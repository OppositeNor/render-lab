//! Writes: what click handlers, actions, and animations return, checked whole and then applied.

use super::*;

/// A hook's writes, each checked against the key's declaration (its type, a whole number's range, a choice's
/// options) before any is applied: the key and its value, or a component's number.
pub(super) fn writes(list: &Data) -> Result<Vec<(String, Written)>, String> {
    let Data::List(items) = list else {
        return Err("a hook returns a list of Writes".into());
    };
    items
        .iter()
        .map(|w| {
            let key = text(w, "key")?.to_string();
            let value = field(w, "value")?;
            let entry =
                values::entry(&key).ok_or_else(|| format!("the shot has no value `{key}`"))?;
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
pub(super) enum Written {
    Value(LabValue),
    /// A vector's component, set by itself.
    Number(f32),
}

/// `value` as `entry`'s type.
pub(super) fn typed(key: &str, entry: &values::Entry, value: &Data) -> Result<LabValue, String> {
    let wrong = || format!("`{key}` is a {}, not {value:?}", entry.ty.name());
    let whole = |n: i64, lo: i64, hi: i64| {
        if (lo..=hi).contains(&n) {
            Ok(n)
        } else {
            Err(format!(
                "`{key}` is a {}: {n} is out of its range",
                entry.ty.name()
            ))
        }
    };
    if let Some(c) = entry.choices {
        // A choice is written by its option's name, and stored as that option's index.
        let index = match value {
            Data::String(name) => c.index(name).ok_or_else(|| {
                format!(
                    "`{key}` has no option `{name}` (its options: {})",
                    c.names()
                )
            })? as i64,
            _ => return Err(wrong()),
        };
        return Ok(match entry.ty {
            Type::I32 => LabValue::I32(index as i32),
            _ => LabValue::U32(index as u32),
        });
    }
    let vector = |n: usize| -> Result<Vec<f32>, String> {
        ["x", "y", "z", "w"][..n]
            .iter()
            .map(|c| number(value, c))
            .collect()
    };
    Ok(match (entry.ty, value) {
        (Type::F32, Data::Float(x)) => LabValue::F32(*x as f32),
        (Type::I32, Data::Int(n)) => {
            LabValue::I32(whole(*n, i32::MIN as i64, i32::MAX as i64)? as i32)
        }
        (Type::U32, Data::Int(n)) => LabValue::U32(whole(*n, 0, u32::MAX as i64)? as u32),
        (Type::Bool, Data::Bool(b)) => LabValue::Bool(*b),
        (Type::Vec2, Data::Record(_)) => {
            LabValue::Vec2(vector(2)?.try_into().expect("two numbers"))
        }
        (Type::Vec3, Data::Record(_)) => {
            LabValue::Vec3(vector(3)?.try_into().expect("three numbers"))
        }
        (Type::Vec4, Data::Record(_)) => {
            LabValue::Vec4(vector(4)?.try_into().expect("four numbers"))
        }
        _ => return Err(wrong()),
    })
}

/// Applies checked writes, in order: whether there were any.
pub(super) fn apply(writes: &[(String, Written)], state: &mut ShotState) -> bool {
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
pub(super) fn trace(u: f64, v: f64, skip: &[String]) -> Result<Option<Hit>, String> {
    let names: Vec<&'static str> = statics(|s| s.scene.objects.iter().map(|o| o.name).collect())?;
    frame(|content| {
        let camera = &content.uniforms.camera;
        let (origin, dir) = crate::camera::Projector::new(camera)
            .ray(glam::Vec2::new(u as f32, v as f32) * glam::Vec2::from(camera.viewport_size));
        let mut best: Option<(f32, Vec3, usize)> = None;
        for (i, o) in content.objects.iter().enumerate() {
            // The nearest object the ray hits, leaving out the named ones to skip.
            if names.get(i).is_some_and(|n| skip.iter().any(|s| s == n)) {
                continue;
            }
            if let Some((t, n)) =
                crate::scene::intersect_object(o, origin, dir, best.map_or(100.0, |b| b.0))
            {
                best = Some((t, n, i));
            }
        }
        best.map(|(t, n, i)| Hit {
            point: V3(origin + dir * t),
            normal: V3(n),
            dir: V3(dir),
            object: i as i64,
            name: names.get(i).copied().unwrap_or_default().to_string(),
        })
    })
}

/// Registers the natives that read the frame (`rl_runtime/trace`, `rl_runtime/viewport`).
pub(super) fn register_frame_natives(engine: &mut Engine) -> Result<(), lamlet::embed::Error> {
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
    with_runtime(|rt| rt.clicks.keys().cloned().collect())
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Runs the scripts' click handler `key` on `frame` (a click at `frame.uv` on the picture of `frame.content`):
/// whether it set anything; None when the scripts have no such handler. The handler named `scene` takes a click
/// on the scene itself, before `Investigation::click`.
pub fn click(key: &str, frame: &mut crate::actions::ClickFrame) -> Option<bool> {
    let click = lamlet::embed::record()
        .field("u", frame.uv.x as f64)
        .field("v", frame.uv.y as f64)
        .build()
        .ok()?;
    let (shown, content, statics) = (frame.shown, frame.content, frame.statics);
    with_runtime(|rt| {
        rt.run(
            Kind::Click(key.to_string()),
            vec![click],
            shown,
            Some(content),
            Some(statics),
            frame.state,
        )
    })
    .ok()
    .flatten()
    .flatten()
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
    pub(super) fn of(key: &str, ty: &TypeDesc) -> Result<ScriptArg, String> {
        use crate::actions::ArgKind;
        let (inner, required) = match ty {
            TypeDesc::Variant(alts)
                if alts.len() == 2
                    && &*alts[0].0 == "none"
                    && alts[0].1 == TypeDesc::Unit
                    && &*alts[1].0 == "some" =>
            {
                (&alts[1].1, false)
            }
            other => (other, true),
        };
        let kind = match inner {
            TypeDesc::Float => ArgKind::Number,
            TypeDesc::Int => ArgKind::Integer,
            TypeDesc::Bool => ArgKind::Toggle,
            TypeDesc::String => ArgKind::Name,
            TypeDesc::Record(fields)
                if *inner == floats(&["x", "y"]) || *inner == floats(&["x", "y", "z"]) =>
            {
                ArgKind::Vector(fields.len())
            }
            other => {
                return Err(format!(
                    "the argument `{key}` is a {other}; an argument is a Float, an Int, a Bool, a String, a Vec2 or Vec3, or an Option of one"
                ));
            }
        };
        Ok(ScriptArg {
            key: key.to_string(),
            kind,
            required,
            ty: inner.clone(),
        })
    }

    pub fn describe(&self) -> String {
        format!(
            "{}: {}{}",
            self.key,
            self.kind.describe(),
            if self.required {
                ""
            } else {
                " (optional)"
            }
        )
    }

    /// The story's value for it (JSON), as Lamlet.
    pub(super) fn value(&self, given: Option<&serde_json::Value>) -> Result<Value, String> {
        let Some(v) = given.filter(|v| !v.is_null()) else {
            return if self.required {
                Err("missing".into())
            } else {
                Ok(Value::variant("none", Value::Unit))
            };
        };
        // A number at the lab's precision (f32), as the investigation's code reads it.
        let number = |v: &serde_json::Value| {
            v.as_f64()
                .map(|n| n as f32 as f64)
                .ok_or_else(|| format!("{v} is not a number"))
        };
        let value = match &self.ty {
            TypeDesc::Float => Value::Float(number(v)?),
            TypeDesc::Int => {
                let n = number(v)?;
                if n.fract() != 0.0 || n.abs() > 9.0e15 {
                    return Err(format!("{v} is not a whole number"));
                }
                Value::Int(n as i64)
            }
            TypeDesc::Bool => Value::Bool(
                v.as_bool()
                    .ok_or_else(|| format!("{v} is not true or false"))?,
            ),
            TypeDesc::String => Value::Str(Rc::from(
                v.as_str().ok_or_else(|| format!("{v} is not a name"))?,
            )),
            TypeDesc::Record(fields) => {
                let list = v
                    .as_array()
                    .filter(|a| a.len() == fields.len())
                    .ok_or_else(|| format!("{v} is not {} numbers", fields.len()))?;
                let labels = ["x", "y", "z"];
                Value::record(
                    list.iter()
                        .zip(labels)
                        .map(|(n, l)| Ok((Rc::from(l), Value::Float(number(n)?))))
                        .collect::<Result<Vec<_>, String>>()?,
                )
            }
            other => return Err(format!("an argument of type {other}")),
        };
        Ok(if self.required {
            value
        } else {
            Value::variant("some", value)
        })
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
    with_runtime(|rt| {
        rt.actions
            .iter()
            .map(|(n, (_, args))| ScriptAction {
                name: n.clone(),
                args: args.clone(),
            })
            .collect()
    })
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// The argument record a story's call gives the action `args` takes.
pub(super) fn arguments(
    name: &str,
    args: &[ScriptArg],
    given: &BTreeMap<String, serde_json::Value>,
) -> Result<Value, String> {
    if let Some(extra) = given.keys().find(|k| !args.iter().any(|a| &a.key == *k)) {
        return Err(format!("the action `{name}` takes no argument `{extra}`"));
    }
    let fields = args.iter().map(|a| {
        Ok((
            Rc::from(a.key.as_str()),
            a.value(given.get(&a.key)).map_err(|e| {
                format!(
                    "the action `{name}`'s argument `{}` ({}): {e}",
                    a.key,
                    a.kind.describe()
                )
            })?,
        ))
    });
    Ok(Value::record(fields.collect::<Result<Vec<_>, String>>()?))
}

/// Why a story's call of a script action cannot run (an argument it does not take, one missing or of the wrong
/// kind); None when the scripts have no such action.
pub fn check_action(call: &crate::actions::ActionCall) -> Option<Result<(), String>> {
    with_runtime(|rt| {
        rt.actions
            .get(&call.name)
            .map(|(_, args)| arguments(&call.name, args, &call.args).map(|_| ()))
    })
    .ok()
    .flatten()
    .flatten()
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
                rt.run(
                    kind,
                    vec![Value::Float(action.elapsed as f64), record],
                    &snapshot,
                    None,
                    None,
                    state,
                );
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
pub(super) fn animation_of(demo: crate::shot::DemoId) -> Option<&'static str> {
    investigation::current()
        .demo_specs()?
        .specs
        .iter()
        .find(|s| s.id == demo)?
        .animation
}

/// Applies `demo`'s script animation at `t` seconds to `state` (after the investigation's own animation): the keys
/// it set, or None when the demonstration has no script animation. Its writes apply all or none.
pub fn animate(demo: crate::shot::DemoId, t: f32, state: &mut ShotState) -> Option<Vec<String>> {
    let name = animation_of(demo)?;
    let snapshot = state.clone();
    with_runtime(|rt| {
        let kind = Kind::Animation(name.to_string());
        let written = rt
            .call(&kind, vec![Value::Float(t as f64)], &snapshot, None, None)
            .and_then(|d| writes(&d));
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
