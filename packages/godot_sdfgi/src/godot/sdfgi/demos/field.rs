//! Chapters 4 and 5: the solid voxels of cascade 0, one voxel's bits, and the distance field's jump flood.

use super::*;

/// Chapter 4: the solid voxels of cascade 0 and one voxel's bits.
pub(super) fn voxels_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let passes = state.p("voxelPasses");
    if state.p("voxels") > 0.5 {
        stored_voxels(state, world, content);
    }
    let p = worked_voxel(world);
    let w = cell_world(world, 0, p);
    ball(content, w, 0.08, palette::SELECTED, 1.0);
    for (k, dir) in voxels::ANISO_DIR.iter().enumerate() {
        let on = data.voxels.facing[index(p)] & (1 << k) != 0;
        content.draw.segment(
            w,
            w + *dir * 0.25,
            if on {
                palette::NORMAL
            } else {
                palette::BRICK_EMPTY
            },
            if on {
                1.0
            } else {
                0.4
            },
            if on {
                3.0
            } else {
                1.0
            },
            lab::frame::FLAG_ARROW,
        );
    }
    let albedo = data.voxels.albedo[index(p)];
    let facing = data.voxels.facing[index(p)];
    content.readout = Some(Readout {
        title: tr("demos-voxel").into(),
        subtitle: tr_with(
            "demos-voxel_subtitle",
            lab::args!(x = p.x, y = p.y, z = p.z),
        ),
        formula: None,
        formula_lit: None,
        rows: vec![
            row(
                tr("demos-albedo_word"),
                format!("{albedo:#06x} (R16_UINT)"),
                palette::text(),
            ),
            row(
                tr("demos-solid_bit"),
                format!("bit 0 = {}", albedo & 1),
                palette::INSIDE,
            ),
            row(
                "R (bits 11-15)",
                format!("{} / 31", (albedo >> 11) & 31),
                Color32::from_rgb(255, 120, 120),
            ),
            row(
                "G (bits 6-10)",
                format!("{} / 31", (albedo >> 6) & 31),
                Color32::from_rgb(120, 230, 120),
            ),
            row(
                "B (bits 1-5)",
                format!("{} / 31", (albedo >> 1) & 31),
                Color32::from_rgb(130, 160, 255),
            ),
            row(
                tr("demos-facing_bits"),
                format!("{:06b}", facing.reverse_bits() >> 26),
                palette::NORMAL,
            ),
            row(
                tr("demos-emission"),
                format!("{:#010x}", data.voxels.emission[index(p)]),
                palette::text_dim(),
            ),
            row(
                tr("demos-passes_run"),
                format!("{}", passes.round()),
                palette::THEME_ACCENT,
            ),
        ],
    });
}

/// The voxels the store keeps, after the chosen number of passes, below the cut: in their albedo, or in the
/// colours of the faces their fragments faced.
pub(super) fn stored_voxels(state: &ShotState, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let passes = state.p("voxelPasses");
    let cut = state.p("cutHeight");
    let voxels = voxel_passes(passes.round().clamp(0.0, 3.0) as usize);
    let size = world.cascades.cascades[0].cell_size * 0.45;
    for c in &data.stored.cells {
        let p = c.cell();
        if voxels.albedo[index(p)] & 1 == 0 {
            continue;
        }
        let w = cell_world(world, 0, p);
        if w.y > cut {
            continue;
        }
        let color = if state.p("voxelFacing") > 0.5 {
            // One colour per axis, the +/- faces in two shades: the bits a voxel's fragments set.
            const FACE: [Color32; 6] = [
                palette::AXIS_X,
                palette::AXIS_Y,
                palette::AXIS_Z,
                Color32::from_rgb(140, 40, 40),
                Color32::from_rgb(40, 120, 50),
                Color32::from_rgb(40, 70, 150),
            ];
            let f = c.facing();
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for (k, col) in FACE.iter().enumerate() {
                if f & (1 << k) != 0 {
                    r += col.r() as u32;
                    g += col.g() as u32;
                    b += col.b() as u32;
                    n += 1;
                }
            }
            let n = n.max(1);
            Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
        } else {
            let a = c.albedo_rgb();
            Color32::from_rgb(
                (a.x.sqrt() * 255.0) as u8,
                (a.y.sqrt() * 255.0) as u8,
                (a.z.sqrt() * 255.0) as u8,
            )
        };
        ball(content, w, size, color, 1.0);
    }
}

/// Cascade 0's render images after `passes` of the three views (cached).
pub(super) fn voxel_passes(passes: usize) -> &'static voxels::RenderVoxels {
    static PASSES: OnceLock<Vec<voxels::RenderVoxels>> = OnceLock::new();
    &PASSES.get_or_init(|| {
        let scene = Scene::study_room();
        let objects = sim::voxelized_objects(&scene.records());
        let cascades = model::Cascades::create(Settings::default(), ORIGIN);
        let region = cascades.pending_regions()[0];
        (0..=3)
            .map(|n| {
                let mut v = voxels::RenderVoxels::cleared();
                voxels::voxelize_axes(&objects, &region, &mut v, n);
                v
            })
            .collect()
    })[passes]
}

/// A horizontal slice of cascade 0 around the room, as an inset: 48 x 48 cells.
pub(super) const SLICE_CELLS: i32 = 34;

/// What an inset says besides its picture: its title, its picture's key,
/// and lines of text under it.
pub(super) struct Caption {
    pub(super) title: String,
    pub(super) key: String,
    pub(super) lines: Vec<(String, Color32)>,
}

pub(super) fn slice_inset(
    state: &ShotState,
    world: &World,
    caption: Caption,
    pixel: impl Fn(IVec3) -> [u8; 4],
    marker: Option<IVec3>,
) -> PictureInset {
    let min = world_cell(
        world,
        0,
        Vec3::new(-SLICE_CELLS as f32 * 0.1, 0.0, -SLICE_CELLS as f32 * 0.1),
    );
    slice_window(state, world, caption, pixel, marker, min, SLICE_CELLS)
}

/// A slice inset of `cells` x `cells` from cell `min` (x and z).
pub(super) fn slice_window(
    state: &ShotState,
    world: &World,
    caption: Caption,
    pixel: impl Fn(IVec3) -> [u8; 4],
    marker: Option<IVec3>,
    min: IVec3,
    cells: i32,
) -> PictureInset {
    let y = world_cell(world, 0, Vec3::new(0.0, state.p("sliceHeight"), 0.0)).y;
    let n = cells as usize;
    // Each cell drawn as a block of k x k pixels, so the inset's filtering keeps cells sharp.
    let k = (220 / n).max(1);
    let size = n * k;
    let mut rgba = vec![0u8; size * size * 4];
    for row in 0..n {
        for col in 0..n {
            // Rows from the top: +z at the bottom, as the top camera sees the room.
            let p = IVec3::new(min.x + col as i32, y, min.z + row as i32);
            let c = if inside(p) {
                pixel(p)
            } else {
                [0, 0, 0, 255]
            };
            for dy in 0..k {
                for dx in 0..k {
                    let i = ((row * k + dy) * size + col * k + dx) * 4;
                    rgba[i..i + 4].copy_from_slice(&c);
                }
            }
        }
    }
    let marker = marker.map(|m| {
        (
            [
                ((m.x - min.x) as f32 + 0.5) / n as f32,
                1.0 - ((m.z - min.z) as f32 + 0.5) / n as f32,
            ],
            palette::SELECTED,
        )
    });
    let key = format!("{}-{y}", caption.key);
    picture(
        Corner::TopRight,
        Caption { key, ..caption },
        size,
        rgba,
        ["x".into(), "z".into()],
        marker,
    )
}

/// A square picture of `size` pixels in an inset, 220 points a side, with its
/// axes (x, y) and a marked point.
pub(super) fn picture(
    corner: Corner,
    caption: Caption,
    size: usize,
    rgba: Vec<u8>,
    axes: [String; 2],
    marker: Option<([f32; 2], Color32)>,
) -> PictureInset {
    let (Caption { title, key, lines }, [x_axis, y_axis]) = (caption, axes);
    let marker = marker.map(|(at, color)| Marker {
        at,
        color,
        label: String::new(),
    });
    PictureInset {
        corner,
        x_axis,
        y_axis,
        marker,
        lines,
        ..PictureInset::new(
            title,
            Picture::Pixels {
                key,
                size: [size, size],
                rgba: Arc::new(rgba),
            },
            [220.0, 220.0],
        )
    }
}

pub(super) fn grey(v: f32) -> [u8; 4] {
    let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
    [g, g, g, 255]
}

/// Chapter 5: the distance field and the jump flood, step by step.
pub(super) fn field_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let step = state.p("floodStep").round().clamp(0.0, 8.0) as usize;
    let worked = world_cell(world, 0, Vec3::new(-0.2, state.p("sliceHeight"), -0.55));
    let seeds_at = |p: IVec3| -> [u8; 4] {
        match step {
            0 => data.flood.half_init[half_index(p)],
            1..=6 => data.flood.half_passes[step - 1].2[half_index(p)],
            7 => data.flood.upscaled[index(p)],
            _ => data.flood.seeds[index(p)],
        }
    };
    // The slices: an inset while `slice` picks one, an image while the story shows it (super::IMAGES).
    field_slices(state, world, (step, worked), content);
    let seed = seeds_at(worked);
    let byte = data.stored.sdf[index(worked)];
    let seed_cell = IVec3::new(seed[0] as i32, seed[1] as i32, seed[2] as i32);
    let w = cell_world(world, 0, worked);
    ball(content, w, 0.06, palette::SELECTED, 1.0);
    // The worked cell and its seed, for a story's arrows and labels (super::ANNOTATIONS).
    content.points.push(("field.worked".into(), w));
    if seed[3] != 0 {
        let s = cell_world(world, 0, seed_cell);
        content.points.push(("field.seed".into(), s));
        content
            .draw
            .segment(w, s, palette::OUTSIDE, 1.0, 2.0, lab::frame::FLAG_ARROW);
        ball(content, s, 0.05, palette::INSIDE, 1.0);
    }
    // A sphere trace across the slice: from the worked cell along +x, each step the stored distance.
    let march_steps = field_march(state, world, worked, content);
    let schedule = voxels::half_schedule();
    let step_text = match step {
        0 => tr("demos-flood_init").to_string(),
        1..=6 => format!(
            "{} {} ({})",
            tr("demos-flood_step_size"),
            schedule[step - 1].0,
            if schedule[step - 1].1 {
                tr("demos-optimized")
            } else {
                tr("demos-regular")
            }
        ),
        7 => tr("demos-flood_upscale").to_string(),
        _ => tr("demos-flood_final").to_string(),
    };
    content.readout = Some(Readout {
        title: tr("demos-field").into(),
        subtitle: step_text,
        formula: None,
        formula_lit: None,
        rows: vec![
            row(
                tr("demos-cell"),
                format!("{:?}", worked.to_array()),
                palette::SELECTED,
            ),
            row(
                tr("demos-seed"),
                if seed[3] == 0 {
                    tr("demos-unused").to_string()
                } else {
                    format!("{:?}", [seed[0], seed[1], seed[2]])
                },
                palette::INSIDE,
            ),
            row(
                tr("demos-distance_cells"),
                format!("{:.2}", (seed_cell - worked).as_vec3().length()),
                palette::OUTSIDE,
            ),
            row(
                tr("demos-stored_byte"),
                format!("{byte} = 1 + d"),
                palette::OUTSIDE,
            ),
            row(
                tr("demos-marches_read"),
                format!("{byte} - 1 = {}", byte as i32 - 1),
                palette::text(),
            ),
            row(
                tr("demos-solid_cells"),
                format!("{} / {}", data.stored.cells.len(), CELLS / 4),
                palette::INSIDE,
            ),
            row(
                tr("demos-march_steps"),
                march_steps.to_string(),
                palette::OUTSIDE,
            ),
        ],
    });
}

/// The slices: an inset while `slice` picks one, an image while the story shows it.
pub(super) fn field_slices(
    state: &ShotState,
    world: &World,
    (step, worked): (usize, IVec3),
    content: &mut FrameContent,
) {
    let data = &world.data[0];
    let seeds_at = |p: IVec3| -> [u8; 4] {
        match step {
            0 => data.flood.half_init[half_index(p)],
            1..=6 => data.flood.half_passes[step - 1].2[half_index(p)],
            7 => data.flood.upscaled[index(p)],
            _ => data.flood.seeds[index(p)],
        }
    };
    let slice = state.p("slice").round() as i32;
    for (n, key) in [(1, "distanceField"), (2, "floodSeeds")] {
        let (inset_on, image_on) = (slice == n, state.shows_image(key));
        if !inset_on && !image_on {
            continue;
        }
        let title = if n == 1 {
            tr("demos-sdf_slice").to_string()
        } else {
            tr_with("demos-seed_slice", lab::args!(step = step))
        };
        let inset = slice_inset(
            state,
            world,
            Caption {
                title,
                key: format!("sdf-{n}-{step}"),
                lines: vec![],
            },
            |p| {
                if n == 1 {
                    grey(data.stored.sdf[index(p)] as f32 / 24.0)
                } else {
                    let s = seeds_at(p);
                    let changed = step > 0 && {
                        let prev = match step - 1 {
                            0 => data.flood.half_init[half_index(p)],
                            k @ 1..=6 => data.flood.half_passes[k - 1].2[half_index(p)],
                            7 => data.flood.upscaled[index(p)],
                            _ => s,
                        };
                        prev != s
                    };
                    if s[3] == 0 {
                        [0, 0, 0, 255]
                    } else if changed {
                        // A cell this step gave a nearer seed.
                        [255, 120, 60, 255]
                    } else {
                        // The seed's own colour, so cells sharing a seed share a colour.
                        [
                            60 + (s[0] as u32 * 37 % 160) as u8,
                            60 + (s[2] as u32 * 53 % 160) as u8,
                            90 + (s[1] as u32 * 29 % 140) as u8,
                            255,
                        ]
                    }
                }
            },
            Some(worked),
        );
        if image_on {
            content.images.push((key.to_string(), inset.clone()));
        }
        if inset_on {
            content.insets.push(inset);
        }
    }
}

/// A sphere trace across the slice (`fieldMarch`): from the worked cell along +x, each step the stored distance;
/// how many steps it took.
pub(super) fn field_march(
    state: &ShotState,
    world: &World,
    worked: IVec3,
    content: &mut FrameContent,
) -> usize {
    let data = &world.data[0];
    let mut march_steps = 0;
    if state.p("fieldMarch") > 0.5 {
        let field = world.field();
        let y_mult = world.cascades.settings.y_mult();
        let start = (worked.as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
        let pos = (start - field.offset(0)) * field.to_cell(0);
        let dir = Vec3::X;
        let mut advance = 0.0;
        for _ in 0..40 {
            let d = light::sample_sdf(
                &data.stored.sdf,
                (pos + dir * advance) / CASCADE_SIZE as f32,
            ) * 255.0
                - 1.0;
            let at = (pos + dir * advance) / field.to_cell(0) + field.offset(0);
            let real = Vec3::new(at.x, at.y / y_mult, at.z);
            circle(
                content,
                real,
                (d.max(0.0) / field.to_cell(0)).max(0.02),
                palette::OUTSIDE,
                0.9,
            );
            ball(content, real, 0.03, palette::OUTSIDE, 1.0);
            march_steps += 1;
            if d < 0.05 {
                break;
            }
            advance += d;
        }
    }
    march_steps
}

pub(super) fn half_index(p: IVec3) -> usize {
    let h: IVec3 = p >> IVec3::ONE;
    (h.x + h.y * 64 + h.z * 64 * 64) as usize
}
