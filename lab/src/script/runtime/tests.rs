use super::*;

/// An engine with the runtime library (from its source: the built resource is not there in a unit test) and the
/// lab's runtime natives' stubs.
fn engine() -> Engine {
    let mut engine = Engine::new();
    register(&mut engine).unwrap();
    register_frame_natives(&mut engine).unwrap();
    register_math_natives(&mut engine).unwrap();
    let stubs = engine.natives_mut().declarations(NATIVES).unwrap();
    let language =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../packages/render_lab/language");
    let library = Package::read(LIBRARY_PACKAGE, &language.join("runtime.lam")).unwrap();
    let math = Package::read(MATH_PACKAGE, &language.join("math.lam")).unwrap();
    let packages = [
        library,
        math,
        Package {
            name: NATIVES.into(),
            file: "<stubs>".into(),
            source: stubs,
        },
    ];
    engine.set_resolver(super::super::resolver(&packages).unwrap());
    engine
}

/// Each expression region's value of a document that imports std and the runtime library.
fn values(regions: &[&str]) -> Vec<Data> {
    evaluated(regions, "")
}

/// Each expression region's value of a document that imports std, the runtime library, and `more`.
fn evaluated(regions: &[&str], more: &str) -> Vec<Data> {
    raw(regions, more)
        .iter()
        .map(|v| Data::from_value(v).unwrap())
        .collect()
}

/// Each expression region's value, as a hook returns it (typed), of a document that imports std, the runtime
/// library, and `more`.
fn raw(regions: &[&str], more: &str) -> Vec<Value> {
    checked(regions, more).unwrap_or_else(|e| panic!("{e}"))
}

/// Each expression region's value of a document that imports std, the runtime library, and `more`; or the error
/// that stopped it (when it is checked, or evaluated).
fn checked(regions: &[&str], more: &str) -> Result<Vec<Value>, String> {
    let engine = engine();
    let body: String = regions.iter().map(|r| format!("${{{r}}}\n")).collect();
    let document = engine
        .check(SourceFile::new(
            "test.lam",
            format!("$[Import* std]$[Import* {LIBRARY_PACKAGE}]{more}\n{body}"),
        ))
        .map_err(|e| e.render())?;
    let mut instance = engine.instance(&document).map_err(|e| e.render())?;
    Ok(instance
        .evaluate()
        .map_err(|e| e.render())?
        .iter()
        .map(|(_, v)| v.clone())
        .collect())
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
        "length {grownArrows {vec3 0.0 0.0 0.0} {' {named \"N\" {vec3 0.0 1.0 0.0} textColor} {named \"L\" {vec3 1.0 0.0 0.0} textColor}} 0.5 1.0 4.0 1.5}",
        "length {lobeLine {vec3 0.0 0.0 0.0} {' {vec3 0.0 1.0 0.0} {vec3 1.0 0.0 0.0}} {' 1.0 2.0} {rgb \"#ffffff\"} 0.8 1.0 3.0 none}",
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
    assert_eq!(
        v[9],
        Data::Float(crate::animation::Ease::InOut.apply(0.25) as f64)
    );
    // N whole with its label; L half grown, its label not yet out.
    assert_eq!(v[10], Data::Int(3));
    // Two values through the point: three segments.
    assert_eq!(v[11], Data::Int(3));
    // Along the normal, any perpendicular unit vector.
    assert!(matches!(&v[12], Data::Record(f) if f["y"] == Data::Float(0.0)));
}

#[test]
fn shapes_are_checked_before_use() {
    let v = raw(
        &[
            "' {arrow {vec3 0.0 0.0 0.0} {vec3 1.0 0.0 0.0} {rgb \"#ff0000\"} 1.0 4.0} {point {vec3 0.0 1.0 0.0} 8.0 accentColor 0.5}",
            "' {row \"a\" \"1\" textDimColor}",
            "' {point {vec3 0.0 0.0 0.0} 8.0 accentColor 1000000000000000000000000000000000000000.0}",
        ],
        "",
    );
    let s = shapes(&v[0], "render_lab").unwrap();
    assert!(
        matches!(&s[0], Shape::Segment { flags, width, color, .. } if *flags == FLAG_ARROW && *width == 4.0 && *color == crate::palette::rgb(0xff0000))
    );
    assert!(
        matches!(&s[1], Shape::Point { alpha, color, .. } if *alpha == 0.5 && *color == crate::palette::accent())
    );
    assert_eq!(
        readout_rows(&v[1]).unwrap(),
        vec![("a".into(), "1".into(), crate::palette::text_dim())]
    );
    // A finite number past f32's range is refused, the shape and the field named.
    let far = shapes(&v[2], "render_lab").unwrap_err();
    assert!(
        far.contains("shape 0") && far.contains("`alpha`") && far.contains("32-bit"),
        "{far}"
    );
}

#[test]
fn a_colour_is_checked_when_its_script_loads() {
    assert_eq!(
        values(&["rgb \"#0A0b0c\""])[0],
        Data::Variant("rgb".into(), Box::new(Data::Int(0x0a0b0c)))
    );
    // A named colour is a static binding: a wrong one stops the document before anything runs.
    for wrong in ["red", "#12345", "#12345g", "#1234567"] {
        let e = checked(&["1"], &format!("$[# _c [% {{rgb \"{wrong}\"}}]]")).unwrap_err();
        assert!(e.contains("is not #rrggbb"), "{wrong}: {e}");
    }
}

#[test]
fn action_arguments_take_their_kinds_from_their_types() {
    use crate::actions::ArgKind;
    let int = TypeDesc::Variant(vec![
        (Rc::from("none"), TypeDesc::Unit),
        (Rc::from("some"), TypeDesc::Int),
    ]);
    let from = ScriptArg::of("from", &int).unwrap();
    assert!(!from.required && from.kind == ArgKind::Integer);
    let at = ScriptArg::of("at", &floats(&["x", "y", "z"])).unwrap();
    assert!(at.required && at.kind == ArgKind::Vector(3));
    assert!(ScriptArg::of("list", &TypeDesc::List(Rc::new(TypeDesc::Int))).is_err());
    // A story's numbers: a whole number for an Int, the lab's precision for a Float, none for an option left out.
    assert!(
        matches!(from.value(Some(&serde_json::json!(3.0))).unwrap(), Value::Variant(v) if v.0.as_ref() == "some" && matches!(v.1, Value::Int(3)))
    );
    assert!(from.value(Some(&serde_json::json!(2.5))).is_err());
    assert!(matches!(from.value(None).unwrap(), Value::Variant(v) if v.0.as_ref() == "none"));
    let seconds = ScriptArg::of("seconds", &TypeDesc::Float).unwrap();
    assert!(
        matches!(seconds.value(Some(&serde_json::json!(0.7))).unwrap(), Value::Float(x) if x == 0.7f32 as f64)
    );
    assert!(seconds.value(None).is_err());
    let args = [seconds, from];
    let given: BTreeMap<String, serde_json::Value> =
        [("secs".to_string(), serde_json::json!(1.0))].into();
    assert!(
        arguments("marchCone", &args, &given)
            .unwrap_err()
            .contains("no argument `secs`")
    );
}

#[test]
fn the_math_library_computes_what_it_says() {
    let v = evaluated(
        &[
            "fractionLatex 6 16",
            "fractionLatex 8 4",
            "tickLatex 0.30000000000000004",
            "tickLatex 2.0",
            "ticksOf {axisRange 0.0 1.0 0.25}",
            "lagged 0.5 1 3 0.5",
            "phase 3.5 3.0 4.0",
            "thereAndBack 0.5",
            "length {texParts \"w\" f\"{\\lit{1}{a} + \\lit{2}{b}}\" 0.5 textColor {' \"a\" \"b\"} {'}}",
            "map {withId {texParts \"w\" f\"{\\lit{1}{a} + \\lit{2}{b}}\" 0.5 textColor {' \"a\" \"b\"} {'}} \"b\"} idOf",
            "length {transform 0.0 {words \"a\" \"x\" 0.5 textColor} {words \"b\" \"y\" 0.5 textColor}}",
            "map {transform 1.0 {words \"a\" \"x\" 0.5 textColor} {words \"b\" \"y\" 0.5 textColor}} idOf",
            "boxCenter {boxOf {moveTo {filled \"r\" {box {v2 0.0 0.0} {v2 2.0 1.0}} textColor 1.0} {v2 3.0 3.0}}}",
            "length {plot \"f\" {axesIn {box {v2 0.0 0.0} {v2 4.0 4.0}} {axisRange -1.0 1.0 1.0} {axisRange -1.0 1.0 1.0}} {^ {x} {!if {< x 0.0} -0.5 0.5}} -1.0 1.0 textColor 2.0}",
            "length {rt::graphLayout 4 {' {pair 0 1} {pair 1 2} {pair 2 3}} \"layered\" 0}",
            "length {cellOf {table \"t\" {' {' {words \"a\" \"a\" 0.4 textColor} {words \"b\" \"bb\" 0.4 textColor}}} 0.1 textColor} 0 1}",
            "runningSums {' 1.0 2.0 3.0}",
            "boxWidth {boxOf {nth {strip \"s\" {' 1.0 3.0} {box {v2 0.0 0.0} {v2 4.0 1.0}} {' textColor} 0.0} 1}}",
            "length {chanceBar \"c\" {box {v2 0.0 0.0} {v2 4.0 0.3}} 0.5 0.2 true 1.0 textColor}",
            "map {' 0.0 0.5 0.75} {^ {h} {hsvColor h 1.0 1.0}}",
            "length {spectrum 24}",
        ],
        &format!("$[Import* {MATH_PACKAGE}]$[ImportAs {NATIVES} rt]"),
    );
    let text = |d: &Data| match d {
        Data::String(s) => s.clone(),
        other => panic!("{other:?}"),
    };
    let float = |d: &Data| match d {
        Data::Float(x) => *x,
        other => panic!("{other:?}"),
    };
    let int = |d: &Data| match d {
        Data::Int(n) => *n,
        other => panic!("{other:?}"),
    };
    assert_eq!(text(&v[0]), r"\frac{3}{8}");
    assert_eq!(text(&v[1]), "2");
    assert_eq!(text(&v[2]), "0.3");
    assert_eq!(text(&v[3]), "2");
    assert!(matches!(&v[4], Data::List(t) if t.len() == 5));
    // The second of three items lagged by half starts at 0.5 of 2: it is at 0.5.
    assert!((float(&v[5]) - 0.5).abs() < 1e-9);
    assert!((float(&v[6]) - 0.5).abs() < 1e-9);
    assert!((float(&v[7]) - 1.0).abs() < 1e-9);
    // A formula's rest and its two terms; the term named `b`.
    assert_eq!(int(&v[8]), 3);
    assert!(matches!(&v[9], Data::List(ids) if ids.len() == 1 && text(&ids[0]) == "b"));
    // A transform at its start is the first figure, at its end the second.
    assert_eq!(int(&v[10]), 1);
    assert!(matches!(&v[11], Data::List(ids) if text(&ids[0]) == "b"));
    assert!(matches!(&v[12], Data::Record(c) if float(&c["x"]) == 3.0 && float(&c["y"]) == 3.0));
    // A step function's graph breaks at its jump: two runs.
    assert_eq!(int(&v[13]), 2);
    assert_eq!(int(&v[14]), 4);
    assert_eq!(int(&v[15]), 1);
    // The running sums of shares, and a strip's segment as long as its share of the whole.
    assert!(
        matches!(&v[16], Data::List(x) if x.iter().map(float).collect::<Vec<_>>() == vec![0.0, 1.0, 3.0, 6.0])
    );
    assert!((float(&v[17]) - 3.0).abs() < 1e-9);
    // A chance bar's track, its lit share, its arrow and number, and its ends' labels.
    assert!(int(&v[18]) >= 5);
    // Red, cyan, and violet; and as many colours as asked.
    let rgb = |n: i64| Data::Variant("rgb".into(), Box::new(Data::Int(n)));
    assert_eq!(
        v[19],
        Data::List(vec![rgb(0xff0000), rgb(0x00ffff), rgb(0x8000ff)])
    );
    assert_eq!(int(&v[20]), 24);
}

#[test]
fn clips_compose_and_read_into_the_lab_s() {
    use crate::clip::{Track, sample_keys};
    let v = raw(
        &[
            "clip {' {animate \"a\"} {tweenTo \"camera.yaw\" 30.0 2.0 \"linear\"}}",
            "after {clip {' {fade \"a\"}}} {clip {' {animate \"b\"}}}",
            "laggedClips 0.25 {' {clip {' {animate \"a\"}}} {clip {' {animate \"b\"}}} {clip {' {animate \"c\"}}}}",
            "reversed {clip {' {progressKeys \"a\" {' {keyframe 0.0 0.0 \"linear\"} {keyframe 2.0 1.0 \"in\"}}}}}",
            "clipSeconds {sequence {' {clip {' {fade \"a\"}}} {clip {' {animate \"b\"}}} {delayed 1.0 {clip {' {fade \"c\"}}}}}}",
            "clip {' {runTrack 0.5 2.0 \"playIt\"} {waveTrack 0.0 \"camera.pitch\" 2.0 4.0 8.0}}",
        ],
        "",
    );
    let clip = |i: usize| clips::clip_of("c", &v[i]).unwrap_or_else(|e| panic!("{e}"));
    let first = clip(0);
    assert_eq!(first.seconds(), 2.0);
    assert!(
        matches!(&first.tracks[0], Track::Keys { channel, keys } if channel == "progress.a" && keys.len() == 2)
    );
    let after = clip(1);
    assert_eq!(after.seconds(), 1.5);
    let Track::Keys { keys, .. } = &after.tracks[1] else {
        panic!("keys")
    };
    assert_eq!((keys[0].at, keys[1].at), (0.5, 1.5));
    assert_eq!(clip(2).seconds(), 1.5);
    let Track::Keys { keys, .. } = &clip(3).tracks[0] else {
        panic!("keys")
    };
    // Backwards: from 1 at 0 to 0 at 2, by the ease that brought it up.
    assert_eq!(sample_keys(keys, 0.0, 0.5), 1.0);
    assert_eq!(sample_keys(keys, 2.0, 0.5), 0.0);
    assert_eq!(keys[1].ease, crate::animation::Ease::In);
    assert!(
        matches!(v[4], Value::Float(x) if x == 3.0),
        "{:?}",
        v[4].type_name()
    );
    let runs = clip(5);
    assert!(
        matches!(&runs.tracks[0], Track::Run { at, seconds, animation } if *at == 0.5 && *seconds == 2.0 && animation == "playIt")
    );
    assert_eq!(runs.seconds(), 8.0);
}

#[test]
fn a_clip_s_ease_and_its_keyframes_order_are_checked() {
    let v = raw(
        &[
            "clip {' {progressKeys \"a\" {' {keyframe 1.0 1.0 \"bounce\"}}}}",
            "clip {' {progressKeys \"a\" {' {keyframe 1.0 1.0 \"linear\"} {keyframe 0.5 0.0 \"linear\"}}}}",
        ],
        "",
    );
    let e = clips::clip_of("c", &v[0]).unwrap_err();
    assert!(e.contains("no ease `bounce`"), "{e}");
    let e = clips::clip_of("c", &v[1]).unwrap_err();
    assert!(e.contains("not in time order"), "{e}");
}
