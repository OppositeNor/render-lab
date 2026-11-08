//! Entities: what a scene holds that the viewer lists, selects, and moves. The Scene window lists them
//! (src/ui/scene_window.rs), the gizmo moves the selected one in the viewport (src/gizmo.rs), and its rows
//! set the same values with sliders.
//!
//! An entity's place is always values (src/values.rs), so the dock, the gizmo, a timeline, and a story all
//! move it the same way, and a shot keeps it:
//!
//! - A scene's object marked `movable` (packages/render_lab/scenes/) has values of the lab's own:
//!   `object.<name>.x`, `.y`, `.z` (where its pivot stands, metres), `.yaw` (a turn about +Y, degrees),
//!   and `.scale` (a factor of its size), `<name>` its name in camel case (`object.rotatedBox.x`). Their
//!   defaults are the scene's; the lab poses the frame's records from them (`pose`), and the scene's light
//!   moves with the object it is. An investigation whose frame follows them says so
//!   (`Investigation::movable_objects`): one that bakes the scene into data of its own does not, and its
//!   objects are listed as fixed.
//! - An investigation declares the other things its values place, a light or a probe
//!   (`Investigation::entities`): a name, a kind, and the three values of its position.

use crate::frame::{FrameContent, Statics};
use crate::investigation;
use crate::layout::ObjectData;
use crate::palette;
use crate::scene::SceneObject;
use crate::shot::ShotState;
use crate::values::{Entry, Type, Unit, Value};
use glam::{Mat3, Mat4, Quat, Vec3};
use std::collections::HashMap;
use std::sync::Mutex;

/// What an entity is, for its icon in the Scene window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    /// A scene's object.
    Object,
    Light,
    /// A point the demonstration places: a probe, a sample.
    Point,
}

/// An entity an investigation declares (`Investigation::entities`).
#[derive(Clone, Copy, Debug)]
pub struct EntitySpec {
    /// Its name on screen: a key of the investigation's package's text.
    pub text: &'static str,
    pub kind: EntityKind,
    /// The values of its position: x, y, z in metres.
    pub position: [&'static str; 3],
    /// The value that shows it (on above 0.5), if it is not always there.
    pub shown: Option<&'static str>,
}

/// An entity as the Scene window lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    /// What the viewer's selection names it by: `object.<name>`, or `value.<x key>`.
    pub id: String,
    /// Its name on screen.
    pub name: String,
    pub kind: EntityKind,
    /// The values that place it (none: it stays where it is), and an object's turn and size.
    pub position: Option<[&'static str; 3]>,
    pub yaw: Option<&'static str>,
    pub scale: Option<&'static str>,
    /// The value that shows it, and whether it shows.
    pub shown: Option<&'static str>,
    pub visible: bool,
    /// Where it is: its position's values, a fixed object's place.
    pub at: Vec3,
    /// Its turn about +Y and its size, as its values set them (0 and 1 for a fixed one).
    pub yaw_degrees: f32,
    pub scale_factor: f32,
}

impl Entity {
    /// Whether the viewer may move it.
    pub fn movable(&self) -> bool {
        self.position.is_some()
    }
}

/// The values of a movable object.
#[derive(Clone, Copy, Debug)]
pub struct ObjectKeys {
    pub x: &'static str,
    pub y: &'static str,
    pub z: &'static str,
    pub yaw: &'static str,
    pub scale: &'static str,
}

/// An object's name in camel case, as its values' keys have it (`rotated box`: `rotatedBox`).
pub fn camel(name: &str) -> String {
    let mut out = String::new();
    for (i, word) in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .enumerate()
    {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(if i == 0 {
                first.to_ascii_lowercase()
            } else {
                first.to_ascii_uppercase()
            });
            out.extend(chars);
        }
    }
    out
}

/// The keys of the values of the object named `name`.
pub fn object_keys(name: &str) -> ObjectKeys {
    static KEYS: Mutex<Option<HashMap<String, ObjectKeys>>> = Mutex::new(None);
    let mut keys = KEYS.lock().unwrap();
    *keys
        .get_or_insert_with(HashMap::new)
        .entry(name.to_string())
        .or_insert_with(|| {
            let id = camel(name);
            let leak = |part: &str| -> &'static str {
                Box::leak(format!("object.{id}.{part}").into_boxed_str())
            };
            ObjectKeys {
                x: leak("x"),
                y: leak("y"),
                z: leak("z"),
                yaw: leak("yaw"),
                scale: leak("scale"),
            }
        })
}

/// The entries of an object's values, its defaults the scene's.
fn object_entries(o: &SceneObject) -> Vec<&'static Entry> {
    let keys = object_keys(o.name);
    let entry = |key: &'static str, default: f32, range: (f32, f32), unit: Unit| Entry {
        key,
        ty: Type::F32,
        default: Some(Value::F32(default)),
        range: Some(range),
        component_ranges: &[],
        step: None,
        unit,
        wrap: false,
        labels: &["object"],
        text: None,
        name: None,
        color: None,
        toggle: false,
        choices: None,
        package: "render_lab",
        doc: "",
        engine: None,
        help: None,
        components: &[],
    };
    let t = o.translation;
    let axis = |key, default: f32, range, name: &'static str, color| Entry {
        name: Some(name),
        color: Some(color),
        help: Some("help-object_position"),
        ..entry(key, default, range, Unit::Metres)
    };
    let entries = [
        axis(keys.x, t.x, (-10.0, 10.0), "x", palette::AXIS_X),
        axis(keys.y, t.y, (-1.0, 6.0), "y", palette::AXIS_Y),
        axis(keys.z, t.z, (-10.0, 10.0), "z", palette::AXIS_Z),
        Entry {
            wrap: true,
            text: Some("values-object_yaw"),
            help: Some("help-object_yaw"),
            ..entry(keys.yaw, 0.0, (-180.0, 180.0), Unit::Degrees)
        },
        Entry {
            text: Some("values-object_scale"),
            help: Some("help-object_scale"),
            ..entry(keys.scale, 1.0, (0.25, 3.0), Unit::Factor)
        },
    ];
    entries
        .into_iter()
        .map(|e| &*Box::leak(Box::new(e)))
        .collect()
}

/// The entries of the values of every movable object in the current investigation's scenes (none when
/// its frame does not follow them), each object's once.
pub fn entries() -> Vec<&'static Entry> {
    static SCENES: Mutex<Option<HashMap<&'static str, Vec<&'static Entry>>>> = Mutex::new(None);
    let current = investigation::current();
    if !current.movable_objects() {
        return Vec::new();
    }
    let mut scenes = SCENES.lock().unwrap();
    let scenes = scenes.get_or_insert_with(HashMap::new);
    let mut all: Vec<&'static Entry> = Vec::new();
    for id in current.scenes() {
        let entries = scenes.entry(id.key()).or_insert_with(|| {
            crate::scene::Scene::build(id)
                .objects
                .iter()
                .filter(|o| o.movable)
                .flat_map(object_entries)
                .collect()
        });
        for e in entries.iter() {
            if !all.iter().any(|a| a.key == e.key) {
                all.push(e);
            }
        }
    }
    all
}

/// Whether `key` is one of a movable object's values (`entries`).
pub fn is_key(key: &str) -> bool {
    key.starts_with("object.") && entries().iter().any(|e| e.key == key)
}

/// Whether the shown scene has anything the Scene window lists.
pub fn any(state: &ShotState, statics: &Statics) -> bool {
    !statics.scene.objects.is_empty() || !investigation::current().entities(state.demo).is_empty()
}

/// What the shown scene holds: the investigation's entities (its lights, its points), then the scene's
/// objects, each where `state` has it.
pub fn list(state: &ShotState, statics: &Statics) -> Vec<Entity> {
    let current = investigation::current();
    let package = current.package().unwrap_or("render_lab");
    let number = |key: &str, default: f32| state.try_get(key).map_or(default, |v| v.f32());
    let mut out: Vec<Entity> = current
        .entities(state.demo)
        .into_iter()
        .map(|spec| Entity {
            id: format!("value.{}", spec.position[0]),
            name: crate::i18n::package_text(package, spec.text).to_string(),
            kind: spec.kind,
            position: Some(spec.position),
            yaw: None,
            scale: None,
            shown: spec.shown,
            visible: spec.shown.is_none_or(|key| number(key, 1.0) > 0.5),
            at: Vec3::from(spec.position.map(|k| number(k, 0.0))),
            yaw_degrees: 0.0,
            scale_factor: 1.0,
        })
        .collect();
    let movable = current.movable_objects();
    for o in &statics.scene.objects {
        let kind =
            if o.emissive_strength > 0.0 && statics.scene.lighting.light.center == o.translation {
                EntityKind::Light
            } else {
                EntityKind::Object
            };
        let mut entity = Entity {
            id: format!("object.{}", camel(o.name)),
            name: crate::i18n::object_name(o.name),
            kind,
            position: None,
            yaw: None,
            scale: None,
            shown: None,
            visible: true,
            at: o.translation,
            yaw_degrees: 0.0,
            scale_factor: 1.0,
        };
        if movable && o.movable {
            let keys = object_keys(o.name);
            let t = o.translation;
            entity.position = Some([keys.x, keys.y, keys.z]);
            entity.yaw = Some(keys.yaw);
            entity.scale = Some(keys.scale);
            entity.at = Vec3::new(
                number(keys.x, t.x),
                number(keys.y, t.y),
                number(keys.z, t.z),
            );
            entity.yaw_degrees = number(keys.yaw, 0.0);
            entity.scale_factor = number(keys.scale, 1.0);
        }
        out.push(entity);
    }
    out
}

/// How `state` moves the object `o` from where its scene has it: the matrix that takes its scene pose to
/// the pose its values give (its pivot moved, turned about +Y and scaled about it), or None where they are
/// the scene's.
pub fn moved(state: &ShotState, o: &SceneObject) -> Option<Mat4> {
    let keys = object_keys(o.name);
    let number = |key: &str, default: f32| state.try_get(key).map_or(default, |v| v.f32());
    let t = o.translation;
    let at = Vec3::new(
        number(keys.x, t.x),
        number(keys.y, t.y),
        number(keys.z, t.z),
    );
    let (yaw, scale) = (number(keys.yaw, 0.0), number(keys.scale, 1.0));
    if at == t && yaw == 0.0 && scale == 1.0 {
        return None;
    }
    Some(
        Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            Quat::from_rotation_y(yaw.to_radians()),
            at,
        ) * Mat4::from_translation(-t),
    )
}

/// `record` moved by `delta` (`moved`): its matrices, its world bounds, and its bounding radius.
pub fn repose(record: &mut ObjectData, delta: Mat4) {
    let m = delta * Mat4::from_cols_array_2d(&record.local_to_world);
    let (center, extent) = crate::scene::transform_box(
        m,
        Vec3::from(record.local_center),
        Vec3::from(record.local_extent),
    );
    let a = Mat3::from_mat4(delta);
    let grow = a
        .x_axis
        .length()
        .max(a.y_axis.length())
        .max(a.z_axis.length());
    record.local_to_world = m.to_cols_array_2d();
    record.world_to_local = m.inverse().to_cols_array_2d();
    record.world_center = center.to_array();
    record.world_extent = extent.to_array();
    record.bounding_radius = (record.bounding_radius * grow).min(extent.length());
}

/// The frame's records posed where `state` has the scene's movable objects (`moved`), when the
/// investigation follows them; the scene's light moves with the object it is, where the frame carries
/// the scene's light.
pub fn pose(state: &ShotState, statics: &Statics, content: &mut FrameContent) {
    if !investigation::current().movable_objects() {
        return;
    }
    let scene = &statics.scene;
    let mut light = None;
    for record in &mut content.objects {
        let Some(o) = scene
            .objects
            .get(record.object_index as usize)
            .filter(|o| o.movable)
        else {
            continue;
        };
        let Some(delta) = moved(state, o) else {
            continue;
        };
        repose(record, delta);
        if o.emissive_strength > 0.0 && scene.lighting.light.center == o.translation {
            light = Some(delta);
        }
    }
    let u = &mut content.uniforms;
    if let Some(delta) = light
        && Vec3::from_slice(&u.light_center_radiance[..3]) == scene.lighting.light.center
    {
        let center = delta.transform_point3(scene.lighting.light.center);
        u.light_center_radiance[..3].copy_from_slice(&center.to_array());
        u.light_axis_u[..3].copy_from_slice(
            &delta
                .transform_vector3(scene.lighting.light.axis_u)
                .to_array(),
        );
        u.light_axis_v[..3].copy_from_slice(
            &delta
                .transform_vector3(scene.lighting.light.axis_v)
                .to_array(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_in_camel_case() {
        assert_eq!(camel("rotated box"), "rotatedBox");
        assert_eq!(camel("teapot"), "teapot");
        assert_eq!(camel("emissive panel"), "emissivePanel");
        assert_eq!(object_keys("small box").yaw, "object.smallBox.yaw");
    }

    #[test]
    fn a_moved_record_keeps_its_shape() {
        let scene = crate::scene::Scene::build(crate::scene::SceneId::STUDY_ROOM);
        let i = scene.index("rotated box").unwrap();
        let o = &scene.objects[i];
        let mut record = crate::scene::object_record(o, i as u32);
        // Up 1 m, a quarter turn, twice the size: the pivot is where the values say.
        let delta = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_y(90f32.to_radians()),
            o.translation + Vec3::Y,
        ) * Mat4::from_translation(-o.translation);
        let before = record;
        repose(&mut record, delta);
        let m = Mat4::from_cols_array_2d(&record.local_to_world);
        assert!(
            m.transform_point3(Vec3::ZERO)
                .abs_diff_eq(o.translation + Vec3::Y, 1e-5)
        );
        assert!((Mat3::from_mat4(m).x_axis.length() - 2.0 * o.scale.x).abs() < 1e-5);
        assert!((record.bounding_radius - 2.0 * before.bounding_radius).abs() < 1e-4);
        assert!(
            Mat4::from_cols_array_2d(&record.world_to_local)
                .mul_mat4(&m)
                .abs_diff_eq(Mat4::IDENTITY, 1e-5)
        );
    }
}
