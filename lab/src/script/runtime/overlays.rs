//! Overlays: the shapes and readout rows a script draws, decoded before any is drawn.

use super::*;

/// A shape an overlay drew, decoded before any is drawn.
#[derive(Clone, Debug)]
pub(super) enum Shape {
    Segment {
        from: Vec3,
        to: Vec3,
        color: Color32,
        alpha: f32,
        width: f32,
        flags: u32,
    },
    Point {
        at: Vec3,
        radius: f32,
        color: Color32,
        alpha: f32,
    },
    Label {
        at: Vec3,
        text: String,
        color: Color32,
    },
}

pub(super) fn field<'a>(d: &'a Data, label: &str) -> Result<&'a Data, String> {
    match d {
        Data::Record(fields) => fields
            .get(label)
            .ok_or_else(|| format!("no field `{label}`")),
        _ => Err(format!("not a record (reading `{label}`)")),
    }
}

pub(super) fn number(d: &Data, label: &str) -> Result<f32, String> {
    match field(d, label)? {
        Data::Float(x) => typed::single(*x, &format!("`{label}`")),
        other => Err(format!("`{label}` is {other:?}, not a Float")),
    }
}

pub(super) fn text<'a>(d: &'a Data, label: &str) -> Result<&'a str, String> {
    match field(d, label)? {
        Data::String(s) => Ok(s),
        other => Err(format!("`{label}` is {other:?}, not a String")),
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_Segment`.
    struct SegmentRecord {
        from: V3 = "from",
        to: V3 = "to",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
        width: Num = "width",
        arrow: bool = "arrow",
        dashed: bool = "dashed",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_Point`.
    struct PointRecord {
        at: V3 = "at",
        radius: Num = "radius",
        color: ScriptColor = "color",
        alpha: Num = "alpha",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_Label`: its text written out, or a text key.
    struct LabelRecord {
        at: V3 = "at",
        text: String = "text",
        color: ScriptColor = "color",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `Row`.
    struct RowRecord {
        name: String = "name",
        value: String = "value",
        color: ScriptColor = "color",
    }
}

/// runtime.lam's `Shape`, as the script returned it.
enum ShapeRecord {
    Segment(SegmentRecord),
    Point(PointRecord),
    Label(LabelRecord),
    LabelText(LabelRecord),
}

impl lamlet::embed::FromLamlet for ShapeRecord {
    fn from_lamlet(v: &Value) -> Result<Self, String> {
        use typed::payload;
        let (tag, p) = typed::variant(v)?;
        Ok(match tag {
            "segment" => ShapeRecord::Segment(payload(tag, p)?),
            "point" => ShapeRecord::Point(payload(tag, p)?),
            "label" => ShapeRecord::Label(payload(tag, p)?),
            "labelText" => ShapeRecord::LabelText(payload(tag, p)?),
            other => return Err(format!("no shape `{other}`")),
        })
    }
}

/// The overlay's shapes, each checked; a label's text key looked up in `package`'s text, then the lab's.
pub(super) fn shapes(list: &Value, package: &str) -> Result<Vec<Shape>, String> {
    typed::items::<ShapeRecord>(list, "shape")?
        .into_iter()
        .map(|record| {
            Ok(match record {
                ShapeRecord::Segment(s) => Shape::Segment {
                    from: s.from.0,
                    to: s.to.0,
                    color: s.color.color32(),
                    alpha: s.alpha.0,
                    width: s.width.0,
                    flags: if s.arrow {
                        FLAG_ARROW
                    } else {
                        0
                    } | if s.dashed {
                        FLAG_DASHED
                    } else {
                        0
                    },
                },
                ShapeRecord::Point(p) => Shape::Point {
                    at: p.at.0,
                    radius: p.radius.0,
                    color: p.color.color32(),
                    alpha: p.alpha.0,
                },
                ShapeRecord::Label(l) => Shape::Label {
                    at: l.at.0,
                    text: l.text,
                    color: l.color.color32(),
                },
                ShapeRecord::LabelText(l) => {
                    // A label's text is its key's, in the package's text table or the lab's.
                    let text = crate::i18n::try_package_text(package, &l.text)
                        .or_else(|| crate::i18n::try_package_text("render_lab", &l.text))
                        .ok_or_else(|| {
                            format!(
                                "no text `{}` in {package}'s text table or the lab's",
                                l.text
                            )
                        })?;
                    Shape::Label {
                        at: l.at.0,
                        text: text.to_string(),
                        color: l.color.color32(),
                    }
                }
            })
        })
        .collect()
}

/// A readout hook's rows, each checked.
pub(super) fn readout_rows(list: &Value) -> Result<Vec<(String, String, Color32)>, String> {
    Ok(typed::items::<RowRecord>(list, "row")?
        .into_iter()
        .map(|r| (r.name, r.value, r.color.color32()))
        .collect())
}

/// Draws `shapes` at `at` in the frame's lists (`FrameContent::overlay_at`), or after everything; returns where they
/// end, so the next overlay follows them.
pub(super) fn draw(
    shapes: &[Shape],
    content: &mut FrameContent,
    at: Option<DrawMark>,
) -> Option<DrawMark> {
    let mut drawn = FrameContent::new(Vec::new(), content.uniforms);
    for s in shapes {
        match s {
            Shape::Segment {
                from,
                to,
                color,
                alpha,
                width,
                flags,
            } => drawn
                .draw
                .segment(*from, *to, *color, *alpha, *width, *flags),
            Shape::Point {
                at,
                radius,
                color,
                alpha,
            } => drawn.draw.point(*at, *radius, *color, *alpha),
            Shape::Label { at, text, color } => drawn.labels.push(Label {
                anchor: *at,
                text: text.clone(),
                color: *color,
            }),
        }
    }
    let Some(at) = at else {
        content.draw.segments.extend(drawn.draw.segments);
        content.draw.points.extend(drawn.draw.points);
        content.labels.extend(drawn.labels);
        return None;
    };
    let end = DrawMark {
        segments: at.segments + drawn.draw.segments.len(),
        points: at.points + drawn.draw.points.len(),
        labels: at.labels + drawn.labels.len(),
    };
    content
        .draw
        .segments
        .splice(at.segments..at.segments, drawn.draw.segments);
    content
        .draw
        .points
        .splice(at.points..at.points, drawn.draw.points);
    content.labels.splice(at.labels..at.labels, drawn.labels);
    Some(end)
}

/// Draws the current investigation's overlays over `content` (after its `build_frame`) and adds its readouts' rows,
/// at `time` seconds, and shows the message of a failing hook. Nothing without scripts.
pub fn overlay(state: &ShotState, statics: &Statics, content: &mut FrameContent, time: f32) {
    let view = crate::view::name();
    let mut at = content.overlay_at.filter(|m| {
        m.segments <= content.draw.segments.len()
            && m.points <= content.draw.points.len()
            && m.labels <= content.labels.len()
    });
    let result = with_runtime(|rt| {
        for i in 0..rt.overlays.len() {
            // Each overlay draws what it last drew in this view when its call fails, so a slip does not blank it.
            let name = Kind::Overlay(i).to_string();
            let drawn = rt
                .call(
                    &Kind::Overlay(i),
                    vec![Value::Float(time as f64)],
                    state,
                    Some(content),
                    Some(statics),
                )
                .and_then(|d| shapes(&d, &rt.package));
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
            // Each readout's rows are added after the rows already in the readout.
            let name = Kind::Readout(i).to_string();
            let rows = rt
                .call(
                    &Kind::Readout(i),
                    vec![Value::Float(time as f64)],
                    state,
                    Some(content),
                    Some(statics),
                )
                .and_then(|d| readout_rows(&d));
            match rows {
                Ok(rows) => {
                    rt.succeeded(&name);
                    content
                        .readout
                        .get_or_insert_with(Default::default)
                        .rows
                        .extend(rows);
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
        content.notes.push((
            crate::i18n::tr_with(
                "script-error",
                crate::args!(message = e.lines().next().unwrap_or_default()),
            ),
            crate::palette::MISMATCH,
        ));
    }
}
