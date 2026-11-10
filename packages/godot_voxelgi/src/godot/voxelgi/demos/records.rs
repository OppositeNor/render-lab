//! Chapter 3: the cells' records, the sort, the distance field, and the upload.

use super::*;

// ---------------------------------------------------------------------------
// Chapter 3: the cells' records, the sort, the field, the upload.

/// The cells demonstration's worked cell: the bake's worked leaf, or the picked one.
pub fn cells_cell(state: &ShotState, w: &World) -> Option<(usize, IVec3)> {
    nearest_leaf(&w.probe, picked_cell(state, worked_leaf(w).1))
}

pub(super) fn cells_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let Some((cell, pos)) = cells_cell(state, w) else {
        return;
    };
    let level = depth(probe);
    let (min, size) = cell_box(probe, level, pos);
    wire_box(content, min, size, palette::SELECTED, 1.0, 2.5);
    content
        .points
        .push(("cells.worked".into(), min + size * 0.5));
    mark(content, state, min + size * 0.5, 0.06);
    let d = probe.data[cell];
    // Its normal: the samples' sum before the fixup, the stored normal after.
    let n = if state.p("fixupStep") < 0.5 {
        super::super::bake::normalized(Vec3::from(w.sums[cell].normal))
    } else {
        d.normal()
    };
    content.draw.segment(
        min + size * 0.5,
        min + size * 0.5 + n * 0.12,
        palette::NORMAL,
        1.0,
        3.0,
        lab::frame::FLAG_ARROW,
    );
    let mut rows = vec![row(
        tr("demos-cell_index"),
        tr_with(
            "demos-cell_index_value",
            lab::args!(index = cell, level = level),
        ),
        palette::text(),
    )];
    if state.p("fixupStep") < 0.5 {
        // Before the fixup: the bake's raw sums for the cell.
        let s = &w.sums[cell];
        rows.push(row(
            tr("demos-albedo_sum"),
            v3(Vec3::from(s.albedo)),
            palette::AABB,
        ));
        rows.push(row(
            tr("demos-normal_sum"),
            v3(Vec3::from(s.normal)),
            palette::NORMAL,
        ));
        rows.push(row(
            tr("demos-alpha_sum"),
            format!("{:.4}", s.alpha),
            palette::VIEW,
        ));
    } else {
        rows.extend(record_rows(w, (cell, pos)));
    }
    content.readout = Some(readout(
        tr("demos-cell_records"),
        tr("demos-cell_records_subtitle"),
        lit_rows(state, rows),
    ));
    if state.p("cellArray") > 0.5 || state.shows_image("cellArray") {
        let inset = cell_array_inset(probe, cell);
        if state.shows_image("cellArray") {
            content.images.push(("cellArray".into(), inset.clone()));
        }
        if state.p("cellArray") > 0.5 {
            content.insets.push(inset);
        }
    }
    slices(state, w, content, Some(pos));
}

/// The rows of cell `cell` (at `pos`) after the fixup: its children, its four record words and what they decode
/// to, its texel of the light texture, and its distance-field byte.
pub(super) fn record_rows(
    w: &World,
    (cell, pos): (usize, IVec3),
) -> Vec<(String, String, Color32)> {
    let probe = &w.probe;
    let d = probe.data[cell];
    let mut rows = Vec::new();
    let children = probe.octree[cell];
    let none = children.iter().all(|&c| c == CHILD_EMPTY);
    rows.push(row(
        tr("demos-children"),
        if none {
            tr("demos-children_none").to_string()
        } else {
            children
                .iter()
                .map(|c| {
                    if *c == CHILD_EMPTY {
                        "-".to_string()
                    } else {
                        c.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        },
        palette::text_dim(),
    ));
    let word = |name: &str, offset: u32, value: u32| {
        (format!("{name} +{offset}"), format!("0x{value:08X}"))
    };
    let decoded = |v: &[f32]| {
        v.iter()
            .map(|x| format!("{x:.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let (n, v) = word(tr("demos-position_word"), 0, d.position);
    rows.push(row(n, v, palette::AABB));
    rows.push(row(
        tr("demos-decoded_xyz"),
        format!("{} {} {}", pos.x, pos.y, pos.z),
        palette::AABB,
    ));
    let (n, v) = word(tr("demos-albedo_word"), 4, d.albedo);
    rows.push(row(n, v, palette::AABB));
    rows.push(row(
        tr("demos-decoded_rgba"),
        decoded(&d.albedo().to_array()),
        palette::AABB,
    ));
    let (n, v) = word(tr("demos-emission_word"), 8, d.emission);
    rows.push(row(n, v, palette::INCIDENT));
    rows.push(row(
        tr("demos-decoded_rgb"),
        decoded(&d.emission().to_array()),
        palette::INCIDENT,
    ));
    let (n, v) = word(tr("demos-normal_word"), 12, d.normal);
    rows.push(row(n, v, palette::NORMAL));
    rows.push(row(
        tr("demos-decoded_xyz"),
        decoded(&d.normal().to_array()),
        palette::NORMAL,
    ));
    let t = w.lit.texture.texel(0, pos);
    rows.push(row(
        tr("demos-texel"),
        decoded(&t.to_array()),
        palette::INCIDENT,
    ));
    rows.push(row(
        tr("demos-texel_light"),
        decoded(&(t.truncate() * world::DYNAMIC_RANGE).to_array()),
        palette::INCIDENT,
    ));
    rows.push(row(
        tr("demos-sdf_byte"),
        format!("{}", probe.sdf_at(pos)),
        palette::OUTSIDE,
    ));
    rows
}

pub(super) const LEVEL_COLORS: [Color32; 8] = [
    palette::AXIS_X,
    palette::AXIS_Y,
    palette::AXIS_Z,
    palette::OBB,
    palette::REFLECTED,
    palette::PROBE,
    palette::VIEW,
    palette::AABB,
];

/// The sorted cell array as a bar: each level's run in its colour (widths
/// as the counts, every level at least a few pixels), the worked cell marked.
pub(super) fn cell_array_inset(probe: &Probe, cell: usize) -> PictureInset {
    let (wpx, hpx) = (440usize, 40usize);
    let total: f32 = probe
        .level_counts
        .iter()
        .map(|&c| (c as f32).max(1.0).ln_1p())
        .sum();
    let mut edges = vec![0usize];
    let mut acc = 0.0;
    for &c in &probe.level_counts {
        // Each level's share of the strip grows with the log of its cell count, so small levels stay visible.
        acc += (c as f32).max(1.0).ln_1p();
        edges.push((acc / total * wpx as f32) as usize);
    }
    let mut rgba = vec![0u8; wpx * hpx * 4];
    for x in 0..wpx {
        // Each column takes its level's colour.
        let l = edges
            .windows(2)
            .position(|e| x >= e[0] && x < e[1])
            .unwrap_or(7);
        let c = LEVEL_COLORS[l % 8];
        for y in 0..hpx {
            let i = (y * wpx + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[c.r(), c.g(), c.b(), 255]);
        }
    }
    let level = probe.level_of(cell);
    // The cell's mark: its level's span, as far along it as the cell is in its level.
    let within =
        (cell - probe.level_offset(level)) as f32 / probe.level_counts[level].max(1) as f32;
    let at = (edges[level] as f32 + within * (edges[level + 1] - edges[level]) as f32) / wpx as f32;
    let mut inset = PictureInset::new(
        tr("demos-cell_array"),
        Picture::Pixels {
            key: "cell-array".into(),
            size: [wpx, hpx],
            rgba: Arc::new(rgba),
        },
        [440.0, 40.0],
    );
    inset.corner = Corner::BottomLeft;
    inset.marker = Some(Marker {
        at: [at, 0.5],
        color: palette::SELECTED,
        label: String::new(),
    });
    inset.x_axis = tr("demos-cell_array_axis").into();
    inset.lines = probe
        .level_counts
        .iter()
        .enumerate()
        .map(|(l, c)| (format!("{l}: {c}"), LEVEL_COLORS[l % 8]))
        .collect();
    inset
}

/// The distance transform's squared distances after `passes` (cached).
pub(super) fn edt_passes(w: &World, passes: usize) -> &'static [f32] {
    static P: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    &P.get_or_init(|| {
        (0..=3)
            .map(|n| sdf::squared_distances(&w.voxelizer, n))
            .collect()
    })[passes]
}

/// Slice insets: the field or the light texture's mip at `sliceHeight`.
pub(super) fn slices(
    state: &ShotState,
    w: &World,
    content: &mut FrameContent,
    mark: Option<IVec3>,
) {
    let slice = state.p("slice").round() as i32;
    let want_field = slice == 1 || state.shows_image("fieldSlice");
    let want_mip = slice == 2 || state.shows_image("mipSlice");
    let probe = &w.probe;
    let y_leaf = leaf_of(probe, Vec3::new(0.0, state.p("sliceHeight"), 0.0))
        .y
        .clamp(0, probe.octree_size.y - 1);
    let s = probe.octree_size;
    if want_field {
        // The field after `edtPass` of its three passes, at the slice's height.
        let passes = state.p("edtPass").round().clamp(0.0, 3.0) as usize;
        let sq = edt_passes(w, passes);
        let max = 16.0f32;
        let inset = slice_inset(
            tr_with("demos-field_slice", lab::args!(passes = passes)),
            format!("sdf-{passes}-{y_leaf}"),
            s.x as usize,
            s.z as usize,
            |x, z| {
                let v = sq[(x + y_leaf * s.x + z * s.x * s.y) as usize];
                if v == 0.0 {
                    // A leaf: distance 0.
                    return [10, 10, 10, 255];
                }
                if v >= sdf::BIG_VAL * 0.5 {
                    // Not reached by the passes so far.
                    return [235, 235, 240, 255];
                }
                // Grey by the distance, a darker band every two cells.
                let d = v.sqrt();
                let band = if (d / 2.0).floor() as i32 % 2 == 0 {
                    1.0
                } else {
                    0.82
                };
                let g = ((70.0 + 170.0 * (d / max).min(1.0)) * band) as u8;
                [g, g, g, 255]
            },
            mark.map(|m| [m.x, m.z]),
            tr("demos-field_legend").into(),
        );
        if state.shows_image("fieldSlice") {
            content.images.push(("fieldSlice".into(), inset.clone()));
        }
        if slice == 1 {
            content.insets.push(inset);
        }
    }
    if want_mip {
        // The light texture's chosen mip at the slice's height.
        let settings = light_settings(state);
        let lit = world::lit_for(&settings);
        let mip = state
            .p("mipLevel")
            .round()
            .clamp(0.0, (probe.mip_count() - 1) as f32) as usize;
        let ms = probe.mip_size(mip);
        let y = (y_leaf >> mip).min(ms.y - 1);
        let tex = &lit.texture;
        let inset = slice_inset(
            tr_with("demos-mip_slice", lab::args!(mip = mip, x = ms.x, z = ms.z)),
            format!(
                "mip-{mip}-{y}-{}-{}-{}",
                settings.dynamic_range, settings.propagation, settings.two_bounces
            ),
            ms.x as usize,
            ms.z as usize,
            |x, z| {
                let t = tex.texel(mip, IVec3::new(x, y, z));
                if t.w == 0.0 {
                    // An empty texel: a dark checkerboard.
                    let c = if (x + z) % 2 == 0 {
                        22
                    } else {
                        34
                    };
                    return [c, c, c + 6, 255];
                }
                let c = light_color(t.truncate() * settings.dynamic_range);
                [c.r(), c.g(), c.b(), 255]
            },
            mark.map(|m| [m.x >> mip, m.z >> mip]),
            tr("demos-mip_legend").into(),
        );
        if state.shows_image("mipSlice") {
            content.images.push(("mipSlice".into(), inset.clone()));
        }
        if slice == 2 {
            content.insets.push(inset);
        }
    }
}

/// An x-z slice of `nx` x `nz` texels, each a block of pixels, +z at the bottom.
pub(super) fn slice_inset(
    title: String,
    key: String,
    nx: usize,
    nz: usize,
    pixel: impl Fn(i32, i32) -> [u8; 4],
    mark: Option<[i32; 2]>,
    legend: String,
) -> PictureInset {
    let k = (256 / nx.max(nz)).max(1);
    let (wpx, hpx) = (nx * k, nz * k);
    let mut rgba = vec![0u8; wpx * hpx * 4];
    for z in 0..nz {
        for x in 0..nx {
            // Each texel fills a k x k block.
            let c = pixel(x as i32, z as i32);
            for dy in 0..k {
                for dx in 0..k {
                    let i = ((z * k + dy) * wpx + x * k + dx) * 4;
                    rgba[i..i + 4].copy_from_slice(&c);
                }
            }
        }
    }
    // The marker is y up (inset.rs); the rows run down in z.
    let marker = mark.map(|m| Marker {
        at: [
            (m[0] as f32 + 0.5) / nx as f32,
            1.0 - (m[1] as f32 + 0.5) / nz as f32,
        ],
        color: palette::SELECTED,
        label: String::new(),
    });
    let mut inset = PictureInset::new(
        title,
        Picture::Pixels {
            key,
            size: [wpx, hpx],
            rgba: Arc::new(rgba),
        },
        [240.0, 240.0 * hpx as f32 / wpx as f32],
    );
    inset.corner = Corner::TopRight;
    inset.x_axis = "x".into();
    inset.y_axis = "z".into();
    inset.marker = marker;
    inset.lines = vec![(legend, palette::text_dim())];
    inset
}

pub(super) fn field_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let fallback = leaf_of(probe, Vec3::new(-0.35, state.p("sliceHeight"), -0.1));
    let p = picked_cell(state, fallback);
    let passes = state.p("edtPass").round().clamp(0.0, 3.0) as usize;
    let sq = edt_passes(w, passes)[(p.x
        + p.y * probe.octree_size.x
        + p.z * probe.octree_size.x * probe.octree_size.y)
        as usize];
    let (min, size) = cell_box(probe, depth(probe), p);
    wire_box(content, min, size, palette::SELECTED, 1.0, 2.0);
    content
        .points
        .push(("field.worked".into(), min + size * 0.5));
    // The slice's plane, outlined in the room.
    let y = state.p("sliceHeight");
    let b = world::bounds();
    let c0 = Vec3::new(b.position.x + NODE.x, y, b.position.z + NODE.z);
    let pts = [
        c0,
        c0 + Vec3::X * b.size.x,
        c0 + Vec3::new(b.size.x, 0.0, b.size.z),
        c0 + Vec3::Z * b.size.z,
        c0,
    ];
    content.draw.polyline(&pts, palette::OUTSIDE, 0.7, 1.5);
    let byte = sdf::encode(sq);
    let rows = vec![
        row(
            tr("demos-worked_cell"),
            format!("({}, {}, {})", p.x, p.y, p.z),
            palette::SELECTED,
        ),
        row(
            tr("demos-passes"),
            format!("{passes} / 3"),
            palette::OUTSIDE,
        ),
        row(
            tr("demos-squared"),
            if sq >= sdf::BIG_VAL * 0.5 {
                "1e20".to_string()
            } else {
                format!("{sq:.0}")
            },
            palette::OUTSIDE,
        ),
        row(
            tr("demos-distance_cells"),
            if sq >= sdf::BIG_VAL * 0.5 {
                "-".to_string()
            } else {
                format!("{:.2}", sq.sqrt())
            },
            palette::OUTSIDE,
        ),
        row(tr("demos-byte"), format!("{byte}"), palette::OUTSIDE),
        row(
            tr("demos-advance"),
            format!("{} {}", byte as i32 - 1, tr("demos-cells")),
            palette::OUTSIDE,
        ),
    ];
    content.readout = Some(readout(
        tr("demos-field"),
        tr("demos-field_subtitle"),
        lit_rows(state, rows),
    ));
    slices(state, w, content, Some(p));
}
