//! A picture in a corner of the viewport (`FrameContent::insets`): its title
//! and a status line above it; axes, a grid, outlined regions, and a marked
//! point on it; a dashed link from a point of the scene to one region; and
//! lines under it. The picture is pixels the investigation computes
//! (`Picture::Pixels`, put in a texture that is rewritten when they change), a
//! PNG resource (`Picture::Resource`, read once), or a GPU texture the
//! renderer keeps under a key (`Picture::Texture`, drawn as it holds when the
//! frame is drawn, so passes that write it each frame show).
//!
//! A story shows an image by its key (`{@ image=key}`, `StoryImage`): one
//! the investigation declares (`Investigation::images`) and publishes in the
//! frames that show it (`FrameContent::images`), or a PNG resource of its
//! package or one an imported package exports. The investigation describes
//! the picture; the story says when it shows, in which corner, and with what
//! title.

use crate::camera::Projector;
use crate::layout::CameraData;
use crate::palette;
use crate::rich_text;
use crate::text::Ink;
use crate::ui::Layout;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, Painter, Pos2, Rect, Stroke, StrokeKind};
use glam::Vec3;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum Picture {
    /// RGBA, row by row from the top. `key` names the inset's texture.
    Pixels { key: String, size: [usize; 2], rgba: Arc<Vec<u8>> },
    /// The texture the renderer keeps under this key (`Textures`); a key it
    /// does not keep is drawn as an empty picture.
    Texture(String),
    /// The PNG resource of this full key (`<package>.<key>`).
    Resource(String),
}

/// An image an investigation publishes for its story (`Investigation::images`).
pub struct ImageSpec {
    /// Its key in a story (`{@ image=<key>}`) and in `FrameContent::images`.
    pub key: &'static str,
    pub doc: &'static str,
}

/// The image a story shows (`{@ image=key imageTitle=text-key imageCorner=top-left}`).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct StoryImage {
    pub source: ImageSource,
    /// A text key of the package of the investigation playing the story, for its title.
    pub title: Option<String>,
    pub corner: Corner,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub enum ImageSource {
    /// One the investigation publishes (`ImageSpec`), by its key.
    Published(String),
    /// A PNG resource, by its full key.
    Resource(String),
}

/// The inset of the image the story shows, faded in by `alpha`: the
/// investigation's picture of it with the story's corner and title (`package`'s
/// text), or the PNG at its own size. None when it shows none, or an image the
/// frame does not publish.
pub fn story(image: &StoryImage, alpha: f32, images: &[(String, PictureInset)], package: &str) -> Option<PictureInset> {
    let mut inset = match &image.source {
        ImageSource::Published(key) => images.iter().find(|(k, _)| k == key)?.1.clone(),
        ImageSource::Resource(key) => PictureInset::new(String::new(), Picture::Resource(key.clone()), [0.0, 0.0]),
    };
    inset.alpha *= alpha;
    inset.corner = image.corner;
    if let Some(title) = &image.title {
        inset.title = vec![crate::i18n::package_text(package, title).to_string()];
    }
    Some(inset)
}

/// The GPU textures a picture can show, by key, as the renderer registered
/// them with egui.
pub type Textures = HashMap<String, egui::TextureId>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    fn top(self) -> bool {
        matches!(self, Corner::TopLeft | Corner::TopRight)
    }

    fn right(self) -> bool {
        matches!(self, Corner::TopRight | Corner::BottomRight)
    }
}

/// A point on the picture, 0..1 along each axis (y up), its colour, and its
/// label (empty: none).
#[derive(Clone, Debug)]
pub struct Marker {
    pub at: [f32; 2],
    pub color: Color32,
    pub label: String,
}

/// A region of the picture, `[min x, min y, max x, max y]` in 0..1 with y
/// down (as its rows run), outlined inside its edge or, when `outside`, just
/// outside it.
#[derive(Clone, Debug)]
pub struct Outline {
    pub rect: [f32; 4],
    pub color: Color32,
    pub width: f32,
    pub outside: bool,
}

/// A dashed line in the outline's colour from a point of the scene to
/// `outlines[outline]`, while the point is in the viewport.
#[derive(Clone, Debug)]
pub struct Link {
    pub from: Vec3,
    pub outline: usize,
}

#[derive(Clone, Debug)]
pub struct PictureInset {
    pub alpha: f32,
    pub corner: Corner,
    /// Joined with " · " on one line when that fits the picture's width, else a line each.
    pub title: Vec<String>,
    /// A dimmer line under the title.
    pub status: Option<String>,
    pub picture: Picture,
    /// The picture's size, points; shrunk, keeping its shape, so the inset
    /// fits in half the viewport's width. `[0, 0]`: its own size in pixels, its
    /// longer side at most 360.
    pub size: [f32; 2],
    /// The horizontal axis, left to right, and the vertical one, bottom to
    /// top (empty: not drawn).
    pub x_axis: String,
    pub y_axis: String,
    /// Columns and rows of grid lines over the picture.
    pub grid: Option<[u32; 2]>,
    pub outlines: Vec<Outline>,
    pub marker: Option<Marker>,
    pub link: Option<Link>,
    /// Lines under the picture.
    pub lines: Vec<(String, Color32)>,
}

impl PictureInset {
    /// `picture` at `size` in the top-right corner, titled, with nothing else.
    pub fn new(title: impl Into<String>, picture: Picture, size: [f32; 2]) -> PictureInset {
        PictureInset {
            alpha: 1.0,
            corner: Corner::TopRight,
            title: vec![title.into()],
            status: None,
            picture,
            size,
            x_axis: String::new(),
            y_axis: String::new(),
            grid: None,
            outlines: Vec::new(),
            marker: None,
            link: None,
            lines: Vec::new(),
        }
    }
}

/// Draw `inset` over the viewport; returns the rectangle it covers.
pub fn draw(ctx: &egui::Context, ink: &Ink, l: &Layout, camera: &CameraData, textures: &Textures, inset: &PictureInset) -> Rect {
    let painter = ink.painter;
    let fade = |c: Color32| c.gamma_multiply(inset.alpha.clamp(0.0, 1.0));
    let measure = |t: &str| rich_text::measure(ink, t, 24.0, FontFamily::Proportional).x;
    let axes = !inset.y_axis.is_empty();
    // Room on the left for the vertical axis' name.
    let (left, right) = if axes { (62.0, 30.0) } else { (10.0, 10.0) };
    let room = l.viewport.width() * 0.5;
    let (id, own) = texture(ctx, textures, &inset.picture).unzip();
    let mut size = match (inset.size, own.flatten()) {
        ([0.0, 0.0], Some(own)) => own * (360.0 / own.max_elem()).min(1.0),
        ([0.0, 0.0], None) => vec2(220.0, 220.0),
        (size, _) => vec2(size[0], size[1]),
    };
    if size.x + left + right > room {
        size *= ((room - left - right) / size.x).max(0.1);
    }
    let base = size.x + left + right;
    let joined = inset.title.join(" · ");
    let title = if inset.title.iter().all(String::is_empty) {
        Vec::new()
    } else if inset.title.len() > 1 && measure(&joined) + 28.0 > base {
        inset.title.clone()
    } else {
        vec![joined]
    };
    let above: Vec<(&String, Color32)> = title.iter().map(|t| (t, palette::text())).chain(inset.status.iter().map(|s| (s, palette::text_dim()))).collect();
    // As wide as its text needs, within half the viewport.
    let widest = above.iter().map(|(t, _)| *t).chain(inset.lines.iter().map(|(t, _)| t)).map(|t| measure(t)).fold(0.0, f32::max);
    let width = (widest + 28.0).clamp(base, room.max(base));
    let top = 10.0 + 30.0 * above.len() as f32 + 8.0;
    let under = if inset.x_axis.is_empty() { 8.0 } else { 40.0 };
    let below = if inset.x_axis.is_empty() && inset.lines.is_empty() { 10.0 } else { under + 32.0 * inset.lines.len() as f32 + 6.0 };
    let frame_size = vec2(width, top + size.y + below);
    let x = if inset.corner.right() { l.viewport.max.x - 16.0 - width } else { l.viewport.min.x + 16.0 };
    let y = if inset.corner.top() { l.viewport.min.y + 16.0 } else { l.viewport.max.y - 16.0 - frame_size.y };
    let frame = Rect::from_min_size(pos2(x, y), frame_size);
    painter.rect_filled(frame, CornerRadius::same(10), fade(Color32::from_rgba_premultiplied(7, 18, 31, 238)));
    painter.rect_stroke(frame, CornerRadius::same(10), Stroke::new(1.0, fade(palette::border())), StrokeKind::Inside);
    for (k, (text, color)) in above.iter().enumerate() {
        let text = ink.fit_right(text, width - 28.0, 24.0, FontFamily::Proportional);
        ink.text(frame.min + vec2(14.0, 10.0 + 30.0 * k as f32), Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, fade(*color));
    }

    let image = Rect::from_min_size(frame.min + vec2(left, top), size);
    match id {
        Some(id) => {
            painter.image(id, image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), fade(Color32::WHITE));
        }
        None => {
            painter.rect_filled(image, CornerRadius::ZERO, fade(Color32::from_rgb(0x18, 0x2a, 0x3c)));
        }
    }
    match inset.grid {
        Some([columns, rows]) => {
            let grid = Stroke::new(1.0, fade(Color32::from_rgba_unmultiplied(160, 190, 215, 89)));
            for i in 0..=columns {
                let x = image.min.x + i as f32 * image.width() / columns.max(1) as f32;
                painter.line_segment([pos2(x, image.min.y), pos2(x, image.max.y)], grid);
            }
            for j in 0..=rows {
                let y = image.min.y + j as f32 * image.height() / rows.max(1) as f32;
                painter.line_segment([pos2(image.min.x, y), pos2(image.max.x, y)], grid);
            }
        }
        None => {
            painter.rect_stroke(image, CornerRadius::ZERO, Stroke::new(1.0, fade(palette::border())), StrokeKind::Outside);
        }
    }
    let region = |r: [f32; 4]| Rect::from_min_max(image.min + vec2(r[0], r[1]) * image.size(), image.min + vec2(r[2], r[3]) * image.size());
    for o in &inset.outlines {
        let (rect, kind) = if o.outside { (region(o.rect).expand(2.0), StrokeKind::Outside) } else { (region(o.rect), StrokeKind::Inside) };
        painter.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(o.width, fade(o.color)), kind);
    }
    if let Some((link, o)) = inset.link.as_ref().and_then(|link| Some((link, inset.outlines.get(link.outline)?))) {
        if let Some((px, _)) = Projector::new(camera).project(link.from) {
            let a = l.viewport_pixel_to_canvas(px);
            if l.viewport.contains(a) {
                let r = region(o.rect);
                dashed(painter, a, if inset.corner.top() { r.center_bottom() } else { r.center_top() }, Stroke::new(2.5, fade(o.color)));
            }
        }
    }

    if !inset.x_axis.is_empty() {
        ink.text(pos2(image.center().x, image.max.y + 6.0), Align2::CENTER_TOP, &inset.x_axis, 24.0, FontFamily::Proportional, fade(palette::text_dim()));
    }
    if axes {
        let galley = painter.layout_no_wrap(inset.y_axis.clone(), egui::FontId::proportional(24.0), fade(palette::text_dim()));
        let at = pos2(image.min.x - 34.0, image.center().y + galley.size().x / 2.0);
        painter.add(egui::epaint::TextShape::new(at, galley, fade(palette::text_dim())).with_angle(-std::f32::consts::FRAC_PI_2));
    }
    if let Some(m) = &inset.marker {
        let p = pos2(image.min.x + m.at[0].clamp(0.0, 1.0) * image.width(), image.max.y - m.at[1].clamp(0.0, 1.0) * image.height());
        painter.circle_stroke(p, 9.0, Stroke::new(3.0, fade(Color32::BLACK)));
        painter.circle_stroke(p, 9.0, Stroke::new(2.0, fade(m.color)));
        if !m.label.is_empty() {
            let right = p.x < image.center().x;
            let (anchor, offset) = if right { (Align2::LEFT_CENTER, 14.0) } else { (Align2::RIGHT_CENTER, -14.0) };
            let size = rich_text::measure(ink, &m.label, 24.0, FontFamily::Proportional);
            painter.rect_filled(anchor.anchor_size(pos2(p.x + offset, p.y), size).expand(3.0), CornerRadius::same(3), fade(Color32::from_rgba_premultiplied(7, 18, 31, 220)));
            ink.text(pos2(p.x + offset, p.y), anchor, &m.label, 24.0, FontFamily::Proportional, fade(m.color));
        }
    }
    let mut y = image.max.y + under;
    for (line, color) in &inset.lines {
        ink.text(pos2(frame.min.x + 14.0, y), Align2::LEFT_TOP, ink.fit_right(line, width - 28.0, 24.0, FontFamily::Proportional), 24.0, FontFamily::Proportional, fade(*color));
        y += 32.0;
    }
    frame.expand(4.0)
}

/// The picture's texture, and its own size in pixels when known: the
/// renderer's; a resource's, read the first time; or the pixels' own, made on
/// first use and rewritten when they change.
fn texture(ctx: &egui::Context, textures: &Textures, picture: &Picture) -> Option<(egui::TextureId, Option<egui::Vec2>)> {
    let (key, size, rgba) = match picture {
        Picture::Texture(key) => return textures.get(key).map(|&id| (id, None)),
        Picture::Resource(key) => return resource_texture(ctx, key).map(|t| (t.id(), Some(t.size_vec2()))),
        Picture::Pixels { key, size, rgba } => (key, *size, rgba),
    };
    let own = Some(vec2(size[0] as f32, size[1] as f32));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    size.hash(&mut hasher);
    rgba.hash(&mut hasher);
    let hash = hasher.finish();
    let id = egui::Id::unique(("picture-inset", key));
    let image = || egui::ColorImage::from_rgba_unmultiplied(size, rgba);
    let texture = match ctx.data(|d| d.get_temp::<(egui::TextureHandle, u64)>(id)) {
        Some((texture, made)) if made == hash => return Some((texture.id(), own)),
        Some((mut texture, _)) => {
            texture.set(image(), egui::TextureOptions::LINEAR);
            texture
        }
        None => ctx.load_texture(format!("inset {key}"), image(), egui::TextureOptions::LINEAR),
    };
    ctx.data_mut(|d| d.insert_temp(id, (texture.clone(), hash)));
    Some((texture.id(), own))
}

/// The PNG resource `key` as a texture, decoded the first time it is drawn;
/// None (and the reason, once) when it is not a PNG.
fn resource_texture(ctx: &egui::Context, key: &str) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique(("picture-resource", key));
    if let Some(texture) = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return texture;
    }
    let texture = match crate::resources::find(key).ok_or_else(|| anyhow::anyhow!("no resource `{key}`")).and_then(|r| crate::record::decode_png(std::io::Cursor::new(r.bytes()))) {
        Ok(image) => Some(ctx.load_texture(format!("image {key}"), image, egui::TextureOptions::LINEAR)),
        Err(e) => {
            eprintln!("image `{key}`: {e:#}");
            None
        }
    };
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

fn dashed(painter: &Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    let d = b - a;
    let len = d.length();
    let n = (len / 14.0).ceil() as usize;
    for i in (0..n).step_by(2) {
        let t0 = i as f32 / n as f32;
        let t1 = ((i + 1) as f32 / n as f32).min(1.0);
        painter.line_segment([a + d * t0, a + d * t1], stroke);
    }
}
