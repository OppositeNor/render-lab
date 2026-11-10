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
        Data::Float(x) => Ok(*x as f32),
        other => Err(format!("`{label}` is {other:?}, not a Float")),
    }
}

pub(super) fn flag(d: &Data, label: &str) -> Result<bool, String> {
    match field(d, label)? {
        Data::Bool(b) => Ok(*b),
        other => Err(format!("`{label}` is {other:?}, not a Bool")),
    }
}

pub(super) fn text<'a>(d: &'a Data, label: &str) -> Result<&'a str, String> {
    match field(d, label)? {
        Data::String(s) => Ok(s),
        other => Err(format!("`{label}` is {other:?}, not a String")),
    }
}

pub(super) fn point(d: &Data, label: &str) -> Result<Vec3, String> {
    let p = field(d, label)?;
    Ok(Vec3::new(number(p, "x")?, number(p, "y")?, number(p, "z")?))
}

/// `#rrggbb`, or a colour of the lab's theme by name: `text`, `textDim`, `textFaint`, `accent`.
pub(super) fn color(d: &Data) -> Result<Color32, String> {
    let s = text(d, "color")?;
    use crate::palette;
    match s {
        "text" => return Ok(palette::text()),
        "textDim" => return Ok(palette::text_dim()),
        "textFaint" => return Ok(palette::text_faint()),
        "accent" => return Ok(palette::accent()),
        _ => {}
    }
    let hex = s
        .strip_prefix('#')
        .filter(|h| h.len() == 6)
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .ok_or_else(|| format!("the colour {s:?} is not #rrggbb"))?;
    Ok(crate::palette::rgb(hex))
}

/// The overlay's shapes, each checked; a label's text key looked up in `package`'s text, then the lab's.
pub(super) fn shapes(list: &Data, package: &str) -> Result<Vec<Shape>, String> {
    let Data::List(items) = list else {
        return Err("an overlay returns a list".into());
    };
    items
        .iter()
        .map(|item| {
            let Data::Variant(tag, d) = item else {
                return Err(format!("{item:?} is not a Shape"));
            };
            Ok(match tag.as_str() {
                "segment" => {
                    let flags = if flag(d, "arrow")? {
                        FLAG_ARROW
                    } else {
                        0
                    } | if flag(d, "dashed")? {
                        FLAG_DASHED
                    } else {
                        0
                    };
                    Shape::Segment {
                        from: point(d, "from")?,
                        to: point(d, "to")?,
                        color: color(d)?,
                        alpha: number(d, "alpha")?,
                        width: number(d, "width")?,
                        flags,
                    }
                }
                "point" => Shape::Point {
                    at: point(d, "at")?,
                    radius: number(d, "radius")?,
                    color: color(d)?,
                    alpha: number(d, "alpha")?,
                },
                "label" => Shape::Label {
                    at: point(d, "at")?,
                    text: text(d, "text")?.to_string(),
                    color: color(d)?,
                },
                "labelText" => {
                    let key = text(d, "text")?;
                    let text = crate::i18n::try_package_text(package, key)
                        .or_else(|| crate::i18n::try_package_text("render_lab", key))
                        .ok_or_else(|| {
                            format!("no text `{key}` in {package}'s text table or the lab's")
                        })?;
                    Shape::Label {
                        at: point(d, "at")?,
                        text: text.to_string(),
                        color: color(d)?,
                    }
                }
                other => return Err(format!("no shape `{other}`")),
            })
        })
        .collect()
}

/// A readout hook's rows, each checked.
pub(super) fn readout_rows(list: &Data) -> Result<Vec<(String, String, Color32)>, String> {
    let Data::List(items) = list else {
        return Err("a readout returns a list of Rows".into());
    };
    items
        .iter()
        .map(|r| {
            Ok((
                text(r, "name")?.to_string(),
                text(r, "value")?.to_string(),
                color(r)?,
            ))
        })
        .collect()
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
