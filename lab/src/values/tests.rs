use super::*;

/// A struct as a package would declare it.
#[derive(Clone, Debug, PartialEq, lab::Values)]
#[values(scope = "probe", package = "render_lab")]
struct Probe {
    /// How far it reaches.
    #[value(default = 2.5, range = 0.0..=10.0, step = 0.1, unit = m, label(light, size))]
    radius: f32,
    #[value(default = true, label(light))]
    on: bool,
    #[value(default = 3, range = 0.0..=8.0, unit = index)]
    count: u32,
    #[value(key = "where", components(px, py, pz), default = [0.0, 1.0, 0.0])]
    position: [f32; 3],
}

#[test]
fn derived_schema() {
    let schema = Probe::schema();
    let keys: Vec<&str> = schema.iter().map(|e| e.key).collect();
    assert_eq!(
        keys,
        ["probe.radius", "probe.on", "probe.count", "probe.where"]
    );
    let radius = &schema[0];
    assert_eq!(
        (
            radius.ty,
            radius.default,
            radius.range,
            radius.step,
            radius.unit
        ),
        (
            Type::F32,
            Some(Value::F32(2.5)),
            Some((0.0, 10.0)),
            Some(0.1),
            Unit::Metres
        )
    );
    assert_eq!(radius.labels, ["light", "size"]);
    assert_eq!(radius.doc, "How far it reaches.");
    assert_eq!(schema[1].default, Some(Value::Bool(true)));
    assert_eq!(schema[2].default, Some(Value::U32(3)));
    assert_eq!(schema[3].components, ["probe.px", "probe.py", "probe.pz"]);
    assert_eq!(schema[3].channels(), ["probe.px", "probe.py", "probe.pz"]);
    assert_eq!(schema[0].channels(), ["probe.radius"]);
}

#[test]
fn get_and_set_by_key() {
    let mut p = Probe {
        radius: 1.0,
        on: false,
        count: 2,
        position: [1.0, 2.0, 3.0],
    };
    assert_eq!(p.get("probe.radius"), Some(Value::F32(1.0)));
    assert_eq!(p.get("probe.where"), Some(Value::Vec3([1.0, 2.0, 3.0])));
    assert_eq!(p.get("probe.py"), Some(Value::F32(2.0)));
    assert_eq!(p.get("probe.nothing"), None);
    // A timeline's numbers: a flag above 0.5 is on; a count is the nearest whole number.
    assert!(p.set("probe.on", Value::F32(0.7)));
    assert!(p.set("probe.count", Value::F32(4.6)));
    assert!(p.set("probe.pz", Value::F32(9.0)));
    assert!(p.set("probe.radius", 4.0.into()));
    assert!(!p.set("probe.nothing", 1.0.into()));
    assert_eq!(
        p,
        Probe {
            radius: 4.0,
            on: true,
            count: 5,
            position: [1.0, 2.0, 9.0]
        }
    );
}

/// A struct with camelCase keys and every default declared.
#[derive(Clone, Debug, PartialEq, lab::Values)]
#[values(rename_all = "camelCase")]
struct Lights {
    #[value(default = 6, name = "cd", toggle)]
    light_intensity: f32,
    #[value(default = [1.0, 0.5])]
    uv_scale: [f32; 2],
}

#[test]
fn camel_case_and_defaults() {
    let keys: Vec<&str> = Lights::schema().iter().map(|e| e.key).collect();
    assert_eq!(keys, ["lightIntensity", "uvScale"]);
    assert_eq!(
        Lights::defaults(),
        Lights {
            light_intensity: 6.0,
            uv_scale: [1.0, 0.5]
        }
    );
    let e = &Lights::schema()[0];
    assert_eq!((e.label_text(), e.toggle), ("cd", true));
    assert_eq!(Lights::schema()[1].label_text(), "uvScale");
}

/// An investigation whose demonstrations declare `Probe`'s values and one
/// plain parameter (`exposure`); the shot tests run with it current.
struct Test;

/// A choice, as an investigation declares one.
#[derive(Clone, Copy, Debug, PartialEq, lab::Choice, serde::Serialize, serde::Deserialize)]
#[choice(wgsl = "MODE")]
#[serde(rename_all = "camelCase")]
enum Mode {
    First,
    #[choice(label = "Second one")]
    SecondOne,
    // A renamed option is renamed for serde too, so a typed view reads it.
    #[choice(name = "last", wgsl = "END")]
    #[serde(rename = "last")]
    Third,
}

fn demos() -> &'static crate::demo::Demos {
    static DEMOS: std::sync::OnceLock<crate::demo::Demos> = std::sync::OnceLock::new();
    DEMOS.get_or_init(|| {
        // Probe's values, and a choice of Mode as a shader's tag declares one.
        let mut entries = Probe::schema().to_vec();
        entries.push(Entry {
            key: "mode",
            ty: Type::U32,
            default: Some(Value::U32(0)),
            range: Some((0.0, 2.0)),
            unit: Unit::Index,
            choices: Some(Mode::choices()),
            ..Probe::schema()[0].clone()
        });
        crate::demo::Demos::for_tests(Box::leak(entries.into_boxed_slice()), &[("exposure", 1.0)])
    })
}

impl crate::investigation::Investigation for Test {
    fn engine(&self) -> &'static crate::investigation::Engine {
        unimplemented!()
    }
    fn id(&self) -> &'static str {
        "test"
    }
    fn title(&self) -> &'static str {
        "test"
    }
    fn summary(&self) -> &'static str {
        ""
    }
    fn wording(&self) -> &'static crate::investigation::Wording {
        unimplemented!()
    }
    fn footer(&self) -> &'static str {
        ""
    }
    fn sources(&self) -> &'static crate::sources::SourceIndex {
        unimplemented!()
    }
    fn listings(&self) -> &'static [crate::pseudocode::Listing] {
        &[]
    }
    fn build_frame(
        &self,
        _: &ShotState,
        _: &crate::frame::Statics,
        _: [f32; 2],
        _: f32,
        _: f32,
    ) -> crate::frame::FrameContent {
        unimplemented!()
    }
    fn dock(&self, _: &mut crate::controls::Dock, _: &mut egui::Ui) {}
    fn click(
        &self,
        _: &ShotState,
        _: &mut ShotState,
        _: &crate::frame::FrameContent,
        _: &crate::frame::Statics,
        _: &crate::camera::Projector,
        _: glam::Vec2,
    ) -> bool {
        false
    }
    fn selftest(&self, _: &crate::investigation::SelftestOptions) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn demo_specs(&self) -> Option<&'static crate::demo::Demos> {
        Some(demos())
    }
    fn actions(&self) -> &'static crate::actions::Actions {
        &Self::ACTIONS
    }
    fn demos(&self) -> &'static [crate::shot::DemoId] {
        static IDS: std::sync::OnceLock<[crate::shot::DemoId; 1]> = std::sync::OnceLock::new();
        IDS.get_or_init(|| [crate::shot::DemoId::new("test")])
    }
    fn shot_state(&self, _: crate::shot::DemoId) -> ShotState {
        shot()
    }
    fn annotations(&self) -> &'static [crate::utility::annotations::AnnotationSpec] {
        use crate::utility::annotations::{AnnotationKind, AnnotationSpec};
        &[
            AnnotationSpec {
                key: "probe.top",
                kind: AnnotationKind::Point,
                doc: "",
            },
            AnnotationSpec {
                key: "rays",
                kind: AnnotationKind::ArrowSet,
                doc: "",
            },
        ]
    }
    fn images(&self) -> &'static [crate::utility::inset::ImageSpec] {
        &[
            crate::utility::inset::ImageSpec {
                key: "chart",
                doc: "",
            },
            crate::utility::inset::ImageSpec {
                key: "table",
                doc: "",
            },
        ]
    }
}

#[lab::actions]
impl Test {
    /// Raises the exposure to `to` over `seconds`.
    #[action]
    fn raise_exposure(
        frame: &mut crate::actions::ActionFrame,
        to: f32,
        seconds: f32,
        at: Option<glam::IVec3>,
    ) {
        let t = (frame.elapsed / seconds).min(1.0);
        frame.state.set("exposure", 1.0 + (to - 1.0) * t);
        if let Some(at) = at {
            frame.state.set("probe.count", at.x as f32);
        }
    }

    /// Selects the probe count the click's x picks (0 to 8).
    #[click]
    fn pick_count(frame: &mut crate::actions::ClickFrame) -> bool {
        frame.state.set("probe.count", (frame.uv.x * 8.0).round());
        true
    }

    /// Fails, to show its error.
    #[action(key = "fail")]
    fn failing(_frame: &mut crate::actions::ActionFrame) -> Result<(), String> {
        Err("on purpose".into())
    }
}

/// A shot of `Test` at its defaults.
fn shot() -> ShotState {
    ShotState {
        demo: crate::shot::DemoId::new("test"),
        camera: crate::camera::OrbitCamera::default(),
        free_camera: Default::default(),
        frame: Default::default(),
        record: crate::shot::RecordRef::None,
        highlight: Vec::new(),
        panel: crate::shot::PanelMode::Values,
        values: demos().defaults(),
        callout: None,
        code: None,
        formula: None,
        formula_term: 0,
        formula_lit: 0.0,
        actions: Vec::new(),
        images: Vec::new(),
        notes: Vec::new(),
    }
}

/// Runs `f` with `Test` current on this thread.
fn with_test<R>(f: impl FnOnce() -> R) -> R {
    crate::investigation::scoped(&Test, f)
}

use crate::shot::ShotState;

#[test]
fn two_motions_of_one_key_cannot_drive_one_channel() {
    with_test(|| {
        let cue: crate::shot::Cue = serde_json::from_str("{}").unwrap();
        let motions =
            |json: &str| serde_json::from_str::<Vec<crate::animation::Motion>>(json).unwrap();
        let apart = motions(
            r#"[{"path":{"params":["camera.yaw"],"through":[[1]],"seconds":1}},{"path":{"params":["camera.pitch"],"through":[[1]],"seconds":1}}]"#,
        );
        assert!(cue.check_motions(&apart).is_ok());
        let twice = motions(
            r#"[{"path":{"params":["camera.yaw"],"through":[[1]],"seconds":1}},{"wave":{"param":"camera.yaw","amplitude":1,"period":2}}]"#,
        );
        assert_eq!(
            cue.check_motions(&twice).unwrap_err().code,
            "story.motion.duplicate"
        );
    })
}

#[test]
fn a_shot_stores_declared_values_in_their_types() {
    with_test(|| {
        let mut s = shot();
        // Defaults, each in its type; a vector is one value; a plain parameter a number.
        assert_eq!(s["probe.radius"], Value::F32(2.5));
        assert_eq!(s["probe.on"], Value::Bool(true));
        assert_eq!(s["probe.count"], Value::U32(3));
        assert_eq!(s["probe.where"], Value::Vec3([0.0, 1.0, 0.0]));
        assert_eq!(s["exposure"], Value::F32(1.0));
        // A timeline's numbers snap where the type does: a flag at 0.5, a count to the nearest whole number.
        s.set("probe.on", 0.4);
        s.set("probe.count", 4.6);
        assert_eq!(
            (s["probe.on"], s["probe.count"]),
            (Value::Bool(false), Value::U32(5))
        );
        assert_eq!((s.p("probe.on"), s.p("probe.count")), (0.0, 5.0));
        // A component by its own key.
        s.set("probe.py", 7.0);
        assert_eq!(s["probe.where"], Value::Vec3([0.0, 7.0, 0.0]));
        assert_eq!((s.p("probe.py"), s.get("probe.py")), (7.0, Value::F32(7.0)));
        // The camera's, by key too.
        s.set("camera.yaw", 33.0);
        assert_eq!((s.camera.yaw, s.p("camera.yaw")), (33.0, 33.0));
    });
}

#[test]
fn index_set_value_and_their_checks() {
    with_test(|| {
        let mut s = shot();
        s["probe.radius"] = 4.0.into();
        assert_eq!(s.p("probe.radius"), 4.0);
        // set_value converts to the declared type.
        s.set_value("probe.count", 2.0);
        s.set_value("probe.where", [1.0, 2.0, 3.0]);
        s.set_value("probe.on", false);
        assert_eq!(
            (s["probe.count"], s["probe.where"], s["probe.on"]),
            (
                Value::U32(2),
                Value::Vec3([1.0, 2.0, 3.0]),
                Value::Bool(false)
            )
        );
        assert_eq!(s.try_get("probe.nothing"), None);
        let panics = |f: fn(&mut ShotState)| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_test(|| f(&mut shot()))
            }))
            .is_err()
        };
        assert!(panics(|s| s.set("probe.nothing", 1.0)), "an unknown key");
        assert!(
            panics(|s| s.set_value("probe.where", 1.0)),
            "a vector is not one number"
        );
        assert!(
            panics(|s| s.set("probe.where", 1.0)),
            "a vector is set by component"
        );
        assert!(
            panics(|s| {
                let _ = s["probe.px"];
            }),
            "a component is not stored"
        );
    });
}

#[test]
fn a_timeline_blends_every_value_by_channel() {
    with_test(|| {
        let channels: Vec<String> = shot()
            .channels_for_tests()
            .into_iter()
            .map(|c| c.0)
            .collect();
        for c in [
            "camera.x",
            "camera.yaw",
            "probe.radius",
            "probe.on",
            "probe.px",
            "probe.py",
            "probe.pz",
            "exposure",
        ] {
            assert!(channels.iter().any(|k| k == c), "{c} in {channels:?}");
        }
        assert!(
            !channels.iter().any(|k| k == "probe.where"),
            "a vector blends by component"
        );
    });
}

/// The typed view of `Probe`'s values.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct ProbeView {
    #[serde(rename = "probe.radius")]
    radius: f32,
    #[serde(rename = "probe.on")]
    on: bool,
    #[serde(rename = "probe.count")]
    count: u32,
    #[serde(rename = "probe.where")]
    position: [f32; 3],
}

#[test]
fn typed_views() {
    with_test(|| {
        let mut s = shot();
        assert_eq!(
            s.view::<ProbeView>(),
            ProbeView {
                radius: 2.5,
                on: true,
                count: 3,
                position: [0.0, 1.0, 0.0]
            }
        );
        s.store(&ProbeView {
            radius: 9.0,
            on: false,
            count: 1,
            position: [3.0, 2.0, 1.0],
        });
        assert_eq!(
            (
                s["probe.radius"],
                s["probe.on"],
                s["probe.count"],
                s.p("probe.pz")
            ),
            (Value::F32(9.0), Value::Bool(false), Value::U32(1), 1.0)
        );
    });
}

#[test]
fn choices() {
    let c = Mode::choices();
    assert_eq!(
        (c.name, c.names()),
        ("Mode", "first, secondOne, last".to_string())
    );
    assert_eq!(
        (c.index("secondOne"), c.index("last"), c.index("nope")),
        (Some(1), Some(2), None)
    );
    assert_eq!(
        (
            Mode::SecondOne.index(),
            Mode::from_index(2),
            Mode::from_index(3)
        ),
        (1, Some(Mode::Third), None)
    );
    assert_eq!(
        (c.option_text(0), c.option_text(1)),
        ("first", "Second one")
    );
    assert_eq!(c.wgsl_file(), "build/generated/mode.wgsl");
    let wgsl = c.wgsl_source().unwrap();
    assert!(wgsl.contains("const MODE_FIRST: u32 = 0u;\nconst MODE_SECOND_ONE: u32 = 1u;\nconst MODE_END: u32 = 2u;\nconst MODE_COUNT: u32 = 3u;\n"), "{wgsl}");
    // The constants parse as WGSL.
    naga::front::wgsl::parse_str(&wgsl).unwrap();
}

#[test]
fn fields_by_short_or_full_key() {
    let mut p = Probe {
        radius: 1.0,
        on: false,
        count: 2,
        position: [1.0, 2.0, 3.0],
    };
    p.set_field("radius", 3.0);
    p.set_field("probe.on", 0.7);
    p.set_field("px", 9.0);
    p.set_field("where", [4.0, 5.0, 6.0]);
    assert_eq!((p.radius, p.on, p.position), (3.0, true, [4.0, 5.0, 6.0]));
    assert_eq!(
        (p.get_field("count"), p.get_field("probe.py")),
        (Value::U32(2), Value::F32(5.0))
    );
    assert!(
        std::panic::catch_unwind(|| Probe {
            radius: 1.0,
            on: false,
            count: 2,
            position: [0.0; 3]
        }
        .set_field("nothing", 1.0))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| Probe {
            radius: 1.0,
            on: false,
            count: 2,
            position: [0.0; 3]
        }
        .set_field("where", 1.0))
        .is_err()
    );
}

#[test]
fn a_story_value_resolves_against_its_declaration() {
    with_test(|| {
        let r = |name: &str, json: serde_json::Value| resolve(name, &json);
        let channels = |pairs: &[(&str, f32)]| {
            Ok(pairs
                .iter()
                .map(|(c, n)| (c.to_string(), *n))
                .collect::<Vec<_>>())
        };
        assert_eq!(
            r("probe.where", serde_json::json!([1.0, 2.0, 3.0])),
            channels(&[("probe.px", 1.0), ("probe.py", 2.0), ("probe.pz", 3.0)])
        );
        assert_eq!(
            r("probe.on", serde_json::json!(true)),
            channels(&[("probe.on", 1.0)])
        );
        assert_eq!(
            r("probe.count", serde_json::json!(2.6)),
            channels(&[("probe.count", 3.0)])
        );
        assert_eq!(
            r("mode", serde_json::json!("last")),
            channels(&[("mode", 2.0)])
        );
        assert_eq!(r("mode", serde_json::json!(1)), channels(&[("mode", 1.0)]));
        assert_eq!(
            r("probe.py", serde_json::json!(7)),
            channels(&[("probe.py", 7.0)])
        );
        assert_eq!(
            r("exposure", serde_json::json!(1.5)),
            channels(&[("exposure", 1.5)])
        );
        assert_eq!(
            r("camera.yaw", serde_json::json!(40)),
            channels(&[("camera.yaw", 40.0)])
        );
        for (name, json, error) in [
            ("mode", serde_json::json!("nope"), "first, secondOne, last"),
            ("exposure", serde_json::json!("high"), "is a number"),
            ("probe.where", serde_json::json!(1.0), "vec3"),
            ("nothing", serde_json::json!(1), "unknown"),
        ] {
            let e = r(name, json).unwrap_err();
            assert!(e.contains(error), "{name}: {e}");
        }
    });
}

#[test]
fn a_timeline_key_sets_channels() {
    with_test(|| {
        let key: crate::shot::Key = serde_json::from_value(serde_json::json!({"frame": 0, "params": {"mode": "secondOne", "probe.where": [1.0, 2.0, 3.0], "exposure": 2}})).unwrap();
        let mut targets = key.targets().unwrap().to_vec();
        targets.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            targets,
            [
                ("exposure".to_string(), 2.0),
                ("mode".to_string(), 1.0),
                ("probe.px".to_string(), 1.0),
                ("probe.py".to_string(), 2.0),
                ("probe.pz".to_string(), 3.0)
            ]
        );
        let bad: crate::shot::Key =
            serde_json::from_value(serde_json::json!({"frame": 0, "params": {"mode": "nope"}}))
                .unwrap();
        assert!(bad.targets().is_err());
        // A typed view holds a choice as its enum.
        #[derive(serde::Deserialize)]
        struct View {
            mode: Mode,
        }
        let mut s = shot();
        s.set("mode", 2.0);
        assert_eq!(s.view::<View>().mode, Mode::Third);
    });
}

#[test]
fn json() {
    for v in [
        Value::F32(0.25),
        Value::Bool(true),
        Value::U32(7),
        Value::Vec3([1.0, 2.0, 3.0]),
    ] {
        assert_eq!(Value::from_json(v.ty(), &v.to_json()), Some(v));
    }
    assert_eq!(
        Value::from_json(Type::Vec2, &serde_json::json!([1.0])),
        None
    );
    assert_eq!(
        serde_json::to_string(&ValueMap(BTreeMap::from([(
            "a".to_string(),
            Value::F32(0.5)
        )])))
        .unwrap(),
        r#"{"a":0.5}"#
    );
}

#[test]
fn components() {
    assert_eq!(
        Value::from_components(Type::Bool, &[0.4]),
        Value::Bool(false)
    );
    assert_eq!(Value::from_components(Type::U32, &[-2.0]), Value::U32(0));
    assert_eq!(
        Value::from_components(Type::Vec2, &[1.0, 2.0]).components(),
        [1.0, 2.0]
    );
    assert_eq!(Value::Bool(true).f32(), 1.0);
}

fn timeline(keys: serde_json::Value) -> crate::shot::Timeline {
    serde_json::from_value(serde_json::json!({
        "fps": 10, "width": 64, "height": 64, "totalFrames": 100, "title": "test", "engineLabel": "",
        "chapters": [{"number": 1, "title": "Test", "startFrame": 0, "endFrame": 100}],
        "beats": [{"id": "all", "chapter": 1, "kind": "3d", "startFrame": 0, "frames": 100, "headline": "", "captions": []}],
        "keys": keys
    }))
    .unwrap()
}

#[test]
fn actions_declare_their_arguments_from_their_parameters() {
    let specs = Test::ACTIONS.specs;
    assert_eq!(
        specs.iter().map(|s| s.key).collect::<Vec<_>>(),
        ["raiseExposure", "fail"]
    );
    let args = specs[0].args;
    assert_eq!(
        args.iter()
            .map(|a| (a.key, a.kind, a.required))
            .collect::<Vec<_>>(),
        [
            ("to", crate::actions::ArgKind::Number, true),
            ("seconds", crate::actions::ArgKind::Number, true),
            ("at", crate::actions::ArgKind::Vector(3), false)
        ]
    );
    assert_eq!(specs[0].doc, "Raises the exposure to `to` over `seconds`.");
}

#[test]
fn a_story_runs_and_stops_an_action() {
    with_test(|| {
        let t = timeline(serde_json::json!([
            {"frame": 10, "run": {"name": "raiseExposure", "args": {"to": 3, "seconds": 2, "at": [5, 0, 0]}}},
            {"frame": 60, "stop": ["raiseExposure"]}
        ]));
        t.validate().unwrap();
        // Before it starts, half way (1 s of 2), done, and stopped.
        assert_eq!(t.state_at(5).p("exposure"), 1.0);
        assert_eq!(t.state_at(20).p("exposure"), 2.0);
        assert_eq!(t.state_at(20).p("probe.count"), 5.0);
        assert_eq!(t.state_at(40).p("exposure"), 3.0);
        assert_eq!(t.state_at(40).actions[0].elapsed, 3.0);
        assert_eq!(t.state_at(70).p("exposure"), 1.0);
        assert!(t.state_at(70).actions.is_empty());
        // Any frame alone is the same as after the frames before it.
        assert_eq!(t.state_at(20), t.state_at(20));
    });
}

#[test]
fn a_timeline_with_a_wrong_action_does_not_load() {
    with_test(|| {
        let error = |run: serde_json::Value| {
            timeline(serde_json::json!([{"frame": 0, "run": run}]))
                .validate()
                .unwrap_err()
                .to_string()
        };
        assert!(
            error(serde_json::json!({"name": "nope"})).contains("unknown action `nope`"),
            "{}",
            error(serde_json::json!({"name": "nope"}))
        );
        assert!(
            error(serde_json::json!({"name": "raiseExposure", "args": {"to": 1}}))
                .contains("argument `seconds`")
        );
        assert!(error(serde_json::json!({"name": "raiseExposure", "args": {"to": 1, "seconds": 1, "speed": 2}})).contains("no argument `speed`"));
        assert!(
            error(
                serde_json::json!({"name": "raiseExposure", "args": {"to": "high", "seconds": 1}})
            )
            .contains("not a number")
        );
        assert!(error(serde_json::json!({"name": "raiseExposure", "args": {"to": 1, "seconds": 1, "at": [1.5, 0, 0]}})).contains("whole numbers"));
    });
}

#[test]
#[should_panic(expected = "the action `fail`: on purpose")]
fn an_action_s_error_names_it() {
    with_test(|| {
        timeline(serde_json::json!([{"frame": 0, "run": {"name": "fail"}}])).state_at(1);
    });
}

#[test]
fn a_story_shows_and_hides_published_images() {
    use crate::utility::inset::{Corner, ImageSource};
    with_test(|| {
        let t = timeline(serde_json::json!([
            {"frame": 10, "blend": 10, "image": "chart", "imageCorner": "bottom-left", "imageTitle": "ui-play"},
            {"frame": 30, "blend": 10, "image": "table"},
            {"frame": 50, "blend": 10, "imageHide": ["chart"]},
            {"frame": 80, "blend": 10, "image": ""}
        ]));
        t.validate().unwrap();
        assert!(t.state_at(5).images.is_empty());
        let half = t.state_at(15);
        let image = &half.images[0];
        assert_eq!(
            (
                image.name.as_str(),
                &image.source,
                image.corner,
                image.title.as_deref()
            ),
            (
                "chart",
                &ImageSource::Published("chart".into()),
                Corner::BottomLeft,
                Some("ui-play")
            )
        );
        assert!(image.alpha > 0.0 && image.alpha < 1.0, "{}", image.alpha);
        assert!(half.shows_image("chart") && !half.shows_image("table"));
        // Two at once, then the first hidden, then all.
        let both = t.state_at(45);
        assert_eq!(
            both.images
                .iter()
                .map(|i| (i.name.as_str(), i.alpha))
                .collect::<Vec<_>>(),
            [("chart", 1.0), ("table", 1.0)]
        );
        assert_eq!(
            t.state_at(70)
                .images
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>(),
            ["table"]
        );
        assert!(t.state_at(95).images.is_empty());
    });
}

#[test]
fn a_story_shows_a_resource_of_the_frame_graph() {
    use crate::utility::inset::ImageSource;
    with_test(|| {
        // The test investigation has no shading: its graph is the lab's scene pass, overlay, and composite.
        let t = timeline(serde_json::json!([
            {"frame": 0, "panel": "graph", "image": "hdr", "imageGraph": "scene.hdr", "imageShow": "color", "imageParams": {"exposure": 1}},
            {"frame": 20, "blend": 10, "image": "hdr", "imageParams": {"exposure": -1}}
        ]));
        t.validate().unwrap();
        let state = t.state_at(40);
        assert_eq!(state.panel, crate::shot::PanelMode::Graph);
        let ImageSource::Graph { key, show, params } = &state.images[0].source else {
            panic!("a graph image: {:?}", state.images[0].source)
        };
        assert_eq!(
            (key.as_str(), show.as_deref(), params["exposure"].clone()),
            ("scene.hdr", Some("color"), vec![-1.0])
        );
        let error = |key: serde_json::Value| {
            timeline(serde_json::json!([key]))
                .validate()
                .unwrap_err()
                .to_string()
        };
        assert!(
            error(serde_json::json!({"frame": 0, "image": "x", "imageGraph": "nowhere.hdr"}))
                .contains("no node `nowhere`")
        );
        assert!(
            error(serde_json::json!({"frame": 0, "image": "x", "imageGraph": "overlay.hdr"}))
                .contains("neither reads nor writes")
        );
        assert!(error(serde_json::json!({"frame": 0, "image": "x", "imageGraph": "scene.hdr", "imageShow": "rainbow"})).contains("unknown graph image display `rainbow`"));
        assert!(error(serde_json::json!({"frame": 0, "image": "x", "imageGraph": "scene.hdr", "imageParams": {"gain": 1}})).contains("a graph image takes"));
        assert!(
            error(serde_json::json!({"frame": 0, "image": "chart", "imageShow": "color"}))
                .contains("imageShow draws a graph image")
        );
        // The same graph as a DAG, a pass called out.
        let t = timeline(
            serde_json::json!([{"frame": 0, "panel": "dag", "highlight": [{"kind": "pass", "name": "scene"}]}]),
        );
        t.validate().unwrap();
        assert_eq!(t.state_at(0).panel, crate::shot::PanelMode::Dag);
        let t = timeline(serde_json::json!([{"frame": 0, "panel": "resources"}]));
        t.validate().unwrap();
        assert_eq!(t.state_at(0).panel, crate::shot::PanelMode::Resources);
    });
}

#[test]
fn image_parameters_need_an_instance() {
    with_test(|| {
        let error = timeline(
            serde_json::json!([{"frame": 0, "image": "chart", "imageParams": {"probe": 1}}]),
        )
        .validate()
        .unwrap_err()
        .to_string();
        assert!(error.contains("not one (make it with imageFrom"), "{error}");
    });
}

#[test]
fn an_image_names_a_declared_click_handler() {
    with_test(|| {
        assert_eq!(
            Test::ACTIONS
                .clicks
                .iter()
                .map(|c| c.key)
                .collect::<Vec<_>>(),
            ["pickCount"]
        );
        let t = timeline(
            serde_json::json!([{"frame": 0, "image": "chart", "imageClick": "pickCount"}]),
        );
        t.validate().unwrap();
        assert_eq!(t.state_at(5).images[0].click.as_deref(), Some("pickCount"));
        let error =
            timeline(serde_json::json!([{"frame": 0, "image": "chart", "imageClick": "pick"}]))
                .validate()
                .unwrap_err()
                .to_string();
        assert!(
            error.contains("no click handler `pick` (the investigation's: pickCount)"),
            "{error}"
        );
    });
}

#[test]
fn a_story_shows_moves_and_hides_arrows_and_labels() {
    use crate::utility::annotations::{Anchor, NoteKind};
    with_test(|| {
        let t = timeline(serde_json::json!([
            {"frame": 10, "blend": 10, "arrow": "a", "arrowFrom": [0, 0, 0], "arrowTo": "probe.top", "arrowColor": "#50d7ff"},
            {"frame": 30, "blend": 10, "arrow": "a", "arrowFrom": [2, 0, 0]},
            {"frame": 30, "label": "l", "labelAt": [1, 2, 3], "labelText": "ui-play"},
            {"frame": 40, "arrows": "rays"},
            {"frame": 60, "blend": 10, "arrowHide": ["a"], "labelHide": ["l"]},
            {"frame": 80, "arrow": ""}
        ]));
        t.validate().unwrap();
        assert!(t.state_at(5).notes.is_empty());
        let half = t.state_at(15);
        assert_eq!(half.notes[0].color, Some([0x50, 0xd7, 0xff]));
        assert!(half.notes[0].alpha > 0.0 && half.notes[0].alpha < 1.0);
        // The fixed end blends to its new place; the named one stays named.
        let moving = t.state_at(35);
        let NoteKind::Arrow { from, to, .. } = &moving.notes[0].kind else {
            panic!("an arrow")
        };
        assert!(
            matches!(from, Anchor::At(p) if p[0] > 0.0 && p[0] < 2.0),
            "{from:?}"
        );
        assert_eq!(to, &Anchor::Named("probe.top".into()));
        let all = t.state_at(50);
        assert_eq!(
            all.notes
                .iter()
                .map(|n| n.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "l", "rays"]
        );
        assert!(all.shows_arrows("rays") && !all.shows_arrows("a"));
        assert_eq!(
            t.state_at(75)
                .notes
                .iter()
                .map(|n| n.name.as_str())
                .collect::<Vec<_>>(),
            ["rays"]
        );
        assert!(t.state_at(85).notes.is_empty());
    });
}

#[test]
fn a_timeline_with_wrong_annotations_does_not_load() {
    with_test(|| {
        let error = |key: serde_json::Value| {
            timeline(serde_json::json!([key]))
                .validate()
                .unwrap_err()
                .to_string()
        };
        assert!(
            error(serde_json::json!({"frame": 0, "arrow": "a", "arrowFrom": [0, 0, 0]}))
                .contains("names both its ends")
        );
        assert!(error(serde_json::json!({"frame": 0, "arrow": "a", "arrowFrom": [0, 0, 0], "arrowTo": "nowhere"})).contains("no point `nowhere`"));
        assert!(
            error(serde_json::json!({"frame": 0, "arrows": "beams"}))
                .contains("no arrow set `beams` (the investigation's: rays)")
        );
        assert!(
            error(
                serde_json::json!({"frame": 0, "label": "l", "labelAt": [0, 0], "labelText": "x"})
            )
            .contains("not a point")
        );
        assert!(error(serde_json::json!({"frame": 0, "arrow": "a", "arrowFrom": [0, 0, 0], "arrowTo": [1, 1, 1], "arrowColor": "cyan"})).contains("not a colour"));
    });
}

#[test]
fn cleanup_is_idempotent_and_note_kind_changes_need_complete_geometry() {
    with_test(|| {
        let cleanup = serde_json::json!({"frame": 0, "stop": ["raiseExposure"], "imageHide": ["view"], "arrowHide": ["ray"], "labelHide": ["caption"]});
        let t = timeline(serde_json::json!([
            cleanup,
            {"frame": 10, "image": "view", "imageGraph": "scene.hdr"},
            {"frame": 10, "arrow": "ray", "arrowFrom": [0, 0, 0], "arrowTo": [1, 1, 1]},
            {"frame": 10, "label": "caption", "labelAt": [0, 0, 0], "labelText": "ui-play"},
            {"frame": 20, "label": "ray", "labelAt": [0, 0, 0], "labelText": "ui-play"},
            // ray is now a label; cleaning up its old arrow is harmless.
            {"frame": 30, "arrowHide": ["ray"]},
            {"frame": 40, "labelHide": ["ray"]}
        ]));
        t.validate().unwrap();
        assert!(t.state_at(0).actions.is_empty());
        assert_eq!(t.state_at(35).notes.len(), 2);
        assert_eq!(t.state_at(45).notes.len(), 1);
        let error = timeline(serde_json::json!([
            {"frame": 0, "arrow": "rays", "arrowFrom": [0, 0, 0], "arrowTo": [1, 1, 1]},
            {"frame": 1, "arrows": "rays"},
            {"frame": 2, "arrow": "rays", "arrowFrom": [2, 2, 2]}
        ]))
        .validate()
        .unwrap_err()
        .to_string();
        assert!(error.contains("names both its ends"), "{error}");
        let error = timeline(serde_json::json!([
            {"frame": 0, "arrow": "same", "arrowFrom": [0, 0, 0], "arrowTo": [1, 1, 1], "label": "same", "labelAt": [0, 0, 0], "labelText": "ui-play"}
        ])).validate().unwrap_err().to_string();
        assert!(error.contains("defined more than once"), "{error}");
    });
}

#[test]
fn scoped_highlights_validate_in_runtime_and_keep_the_rendered_names() {
    with_test(|| {
        let t = timeline(serde_json::json!([
            {"frame": 0, "panel": "graph", "highlight": [{"kind": "pass", "name": "scene"}]},
            {"frame": 10, "panel": "performance", "highlight": [{"kind": "pass", "name": "scene pass"}]}
        ]));
        t.validate().unwrap();
        assert_eq!(t.state_at(0).highlight, ["scene"]);
        assert_eq!(t.state_at(10).highlight, ["scene pass"]);
        let error = timeline(serde_json::json!([
            {"frame": 0, "panel": "graph", "highlight": [{"kind": "pass", "name": "scne"}]}
        ]))
        .validate()
        .unwrap_err()
        .to_string();
        assert!(error.contains("did you mean `scene`?"), "{error}");
    });
}

#[test]
fn image_resources_must_be_pngs_and_obey_package_visibility() {
    with_test(|| {
        let t = timeline(serde_json::json!([
            {"frame": 0, "imageHide": ["icon/globe"]},
            {"frame": 1, "image": "icon/globe"},
            {"frame": 2, "imageHide": ["icon/globe"]}
        ]));
        t.validate().unwrap();
        assert_eq!(t.state_at(1).images.len(), 1);
        assert!(t.state_at(3).images.is_empty());
        let error = timeline(serde_json::json!([{"frame": 0, "image": "text/en"}]))
            .validate()
            .unwrap_err()
            .to_string();
        assert!(error.contains("not a PNG image"), "{error}");
    });
}

#[test]
fn beats_accept_only_3d_and_2d_scene_types() {
    use crate::shot::{SceneType, TimelineBeat};
    let beat = |kind: &str, diagram: Option<serde_json::Value>| -> TimelineBeat {
        serde_json::from_value(serde_json::json!({"id": "b", "chapter": 1, "kind": kind, "startFrame": 0, "frames": 1, "headline": "", "diagram": diagram})).unwrap()
    };
    let two = || serde_json::json!({"nodes": [{"id": "A", "title": ["A"]}, {"id": "B", "title": ["B"]}], "edges": [{"from": "A", "to": "B"}]});
    assert_eq!(beat("3d", None).kind, SceneType::ThreeD);
    assert!(beat("2d", Some(two())).shows_diagram());
    assert_eq!(
        beat("2d", Some(two()))
            .beat_diagram()
            .unwrap()
            .graph
            .edges
            .len(),
        1
    );
    assert!(!beat("3d", Some(two())).shows_diagram());
    assert!(!beat("2d", None).shows_diagram());
    for kind in ["scene", "flowchart", "sources"] {
        assert!(serde_json::from_value::<SceneType>(serde_json::json!(kind)).is_err());
    }
    assert_eq!(
        serde_json::to_value(SceneType::TwoD).unwrap(),
        serde_json::json!("2d")
    );
}
