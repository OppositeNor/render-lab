use super::*;

/// An engine with the runtime library (from its source: the built resource is not there in a unit test) and the
/// lab's runtime natives' stubs.
fn engine() -> Engine {
    let mut engine = Engine::new();
    register(&mut engine).unwrap();
    register_frame_natives(&mut engine).unwrap();
    let stubs = engine.natives_mut().declarations(NATIVES).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../packages/render_lab/language/runtime.lam");
    let library = Package::read(LIBRARY_PACKAGE, &path).unwrap();
    let packages = [
        library,
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
    let engine = engine();
    let body: String = regions.iter().map(|r| format!("${{{r}}}\n")).collect();
    let document = engine
        .check(SourceFile::new(
            "test.lam",
            format!("$[Import* std]$[Import* {LIBRARY_PACKAGE}]\n{body}"),
        ))
        .unwrap_or_else(|e| panic!("{}", e.render()));
    let mut instance = engine.instance(&document).unwrap();
    instance
        .evaluate()
        .unwrap_or_else(|e| panic!("{}", e.render()))
        .iter()
        .map(|(_, v)| Data::from_value(v).unwrap())
        .collect()
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
    let v = values(&[
        "' {arrow {vec3 0.0 0.0 0.0} {vec3 1.0 0.0 0.0} \"#ff0000\" 1.0 4.0} {point {vec3 0.0 1.0 0.0} 8.0 \"accent\" 0.5}",
    ]);
    let s = shapes(&v[0], "render_lab").unwrap();
    assert!(
        matches!(&s[0], Shape::Segment { flags, width, color, .. } if *flags == FLAG_ARROW && *width == 4.0 && *color == crate::palette::rgb(0xff0000))
    );
    assert!(matches!(&s[1], Shape::Point { alpha, .. } if *alpha == 0.5));
    let bad = values(&["' {segment {vec3 0.0 0.0 0.0} {vec3 1.0 0.0 0.0} \"red\" 1.0 4.0}"]);
    assert!(
        shapes(&bad[0], "render_lab")
            .unwrap_err()
            .contains("#rrggbb")
    );
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
