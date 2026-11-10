//! Chapters 4 and 5: the solid voxels of cascade 0, one voxel's bits, and the distance field's jump flood.

use super::*;

/// Chapter 4: the solid voxels of cascade 0 and one voxel's bits.
pub(super) fn voxels_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    emission_chart(state, content);
    let data = &world.data[0];
    let passes = state.p("voxelPasses");
    if state.p("voxels") > 0.5 {
        stored_voxels(state, world, content);
    }
    let p = worked_voxel(world);
    let w = cell_world(world, 0, p);
    ball(content, w, 0.08, palette::SELECTED, 1.0);
    neighbour_flags(state, world, p, content);
    for (k, dir) in voxels::ANISO_DIR.iter().enumerate() {
        // An arrow per direction, bright where the worked voxel's facing bit is set.
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
        // Each solid voxel the chosen passes made, below the cut height.
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
            // Mix the colours of the faces it has.
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
            // Or its albedo, at `albedoLevels` levels a channel (the stored 5 bits are 32), brightened to read.
            let levels = (state.p("albedoLevels").round().clamp(2.0, 32.0) - 1.0).max(1.0);
            let a = (c.albedo_rgb() * levels + Vec3::splat(0.5)).floor() / levels;
            Color32::from_rgb(
                (a.x.sqrt() * 255.0) as u8,
                (a.y.sqrt() * 255.0) as u8,
                (a.z.sqrt() * 255.0) as u8,
            )
        };
        ball(content, w, size, color, 1.0);
    }
}

/// With `emissionChart`, an emissive colour at 10^`emissionLevel` packed as the SDF pass packs emission (RGBE8985):
/// its three mantissas and its shared exponent as bars, and the colour read back beside the true one.
pub(super) fn emission_chart(state: &ShotState, content: &mut FrameContent) {
    if state.p("emissionChart") <= 0.5 {
        return;
    }
    let light = Vec3::new(1.0, 0.55, 0.2) * 10f32.powf(state.p("emissionLevel"));
    let word = voxels::encode_rgbe8985(light);
    let back = voxels::decode_rgbe8985(word);
    // The word's fields: red's 8 bits, green's 9, blue's 8, the exponent's 5, each as a share of its range.
    let fields = [
        ((word & 0xFF) as f32 / 255.0, [230u8, 90, 80]),
        (((word >> 8) & 0x1FF) as f32 / 511.0, [90, 200, 110]),
        (((word >> 17) & 0xFF) as f32 / 255.0, [90, 140, 230]),
        (((word >> 25) & 0x1F) as f32 / 31.0, [230, 200, 90]),
    ];
    let n = 120usize;
    let mut rgba = vec![0u8; n * n * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[12, 20, 32, 255]);
    }
    for (k, (v, c)) in fields.iter().enumerate() {
        // Each field a bar, as tall as its value is of its range.
        let top = n - 1 - (v.clamp(0.0, 1.0) * (n - 8) as f32) as usize;
        for y in top..n {
            for x in 8 + k * 28..8 + k * 28 + 20 {
                let i = (y * n + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }
    content.insets.push(picture(
        Corner::TopLeft,
        Caption {
            title: tr("demos-emission_word").into(),
            key: format!("emission-{word}"),
            lines: vec![
                (
                    tr_with(
                        "demos-emission_red",
                        lab::args!(value = format!("{:.4}", light.x)),
                    ),
                    palette::text(),
                ),
                (
                    tr_with(
                        "demos-emission_back",
                        lab::args!(value = format!("{:.4}", back.x)),
                    ),
                    palette::text_dim(),
                ),
            ],
        },
        n,
        rgba,
        [tr("demos-emission_fields").into(), String::new()],
        None,
    ));
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
                // The room voxelized along 0 to 3 axes, to show what each pass adds.
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
    let k = (300 / n).max(1);
    let size = n * k;
    let mut rgba = vec![0u8; size * size * 4];
    for row in 0..n {
        for col in 0..n {
            // Rows from the top: +z at the bottom, as the top camera sees the room.
            let p = IVec3::new(min.x + col as i32, y, min.z + row as i32);
            // Each cell's colour, outside the cascade black, filling its k x k block.
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
        // The marker in the picture's fractions, y up.
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
    let worked = world_cell(
        world,
        0,
        Vec3::new(state.p("fieldX"), state.p("sliceHeight"), state.p("fieldZ")),
    );
    let seeds_at = |p: IVec3| -> [u8; 4] {
        match step {
            // Step 0 is the half-size seeds, 1 to 6 the half-size passes, 7 the upscale, then the full-size pass.
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
        // A seed found: an arrow from the worked cell to it.
        let s = cell_world(world, 0, seed_cell);
        content.points.push(("field.seed".into(), s));
        content
            .draw
            .segment(w, s, palette::OUTSIDE, 1.0, 2.0, lab::frame::FLAG_ARROW);
        ball(content, s, 0.05, palette::INSIDE, 1.0);
    }
    // A sphere trace across the slice: from the worked cell along +x, each step the stored distance.
    let march_steps = field_march(state, world, worked, content);
    byte_profile(state, world, worked, content);
    flood_reach(state, world, (step, worked, seed), content);
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
        // Each slice is drawn while it is picked, or while a story shows it.
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
                    // The distance field: grey by distance (square root, so cells near a surface read), white from 24.
                    grey((data.stored.sdf[index(p)] as f32 / 24.0).sqrt())
                } else {
                    // The seeds: whether this step changed the cell's seed, from the step before.
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
                        // No seed yet: dark slate.
                        [44, 48, 62, 255]
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
        // A click picks the cell under it (script/logic.lam `pickFieldCell`); the worked cell outlined, linked to the scene.
        let n = SLICE_CELLS as f32;
        let min = world_cell(world, 0, Vec3::new(-n * 0.1, 0.0, -n * 0.1));
        let (u, v) = ((worked.x - min.x) as f32 / n, (worked.z - min.z) as f32 / n);
        let inset = PictureInset {
            click: Some("pickFieldCell".into()),
            outlines: vec![Outline {
                rect: [u, 1.0 - v - 1.0 / n, u + 1.0 / n, 1.0 - v],
                color: palette::SELECTED,
                width: 2.0,
                outside: false,
            }],
            link: Some(Link {
                from: cell_world(world, 0, worked),
                outline: 0,
            }),
            size: [300.0, 300.0],
            ..inset
        };
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
        // March from the worked cell along +x through the stored field, each step a circle of its distance.
        let field = world.field();
        let y_mult = world.cascades.settings.y_mult();
        let start = (worked.as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
        let pos = (start - field.offset(0)) * field.to_cell(0);
        let dir = Vec3::X;
        let mut advance = 0.0;
        let shown = state.p("fieldSteps").round().clamp(0.0, 40.0) as usize;
        for _ in 0..shown {
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

/// The stored byte along the march's line (`byteProfile` cells of it, from the worked cell along +x): a bar per
/// cell of the byte, less `byteRead` (1: the byte minus one, as a ray reads it, so a solid voxel's zero reads -1),
/// and with `byteCurve` the trilinear read between the cells as a curve, which crosses zero halfway between a solid
/// voxel and its empty neighbour.
fn byte_profile(state: &ShotState, world: &World, worked: IVec3, content: &mut FrameContent) {
    const CELLS: usize = 24;
    const PX: usize = 6;
    let shown = state.p("byteProfile").round().clamp(0.0, CELLS as f32) as usize;
    if shown == 0 {
        return;
    }
    let sdf = &world.data[0].stored.sdf;
    let (w, h) = (CELLS * PX, 120usize);
    let (low, high) = (-1.5f32, 8.0f32);
    let y_of = |v: f32| -> usize {
        let t = ((v.clamp(low, high) - low) / (high - low)).clamp(0.0, 1.0);
        ((1.0 - t) * (h - 1) as f32) as usize
    };
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[12, 20, 32, 255]);
    }
    let put = |x: usize, y: usize, c: [u8; 4], rgba: &mut Vec<u8>| {
        if x < w && y < h {
            let i = (y * w + x) * 4;
            rgba[i..i + 4].copy_from_slice(&c);
        }
    };
    let zero = y_of(0.0);
    for x in 0..w {
        put(x, zero, [90, 100, 120, 255], &mut rgba);
    }
    for c in 0..shown {
        // Each cell a bar from zero to its byte minus one, orange where the voxel is solid.
        let p = worked + IVec3::new(c as i32, 0, 0);
        if p.x >= CASCADE_SIZE {
            break;
        }
        let v = sdf[index(p)] as f32 - state.p("byteRead").clamp(0.0, 1.0);
        let colour = if v < 0.0 {
            [255, 150, 70, 255]
        } else {
            [90, 160, 230, 255]
        };
        let (a, b) = (zero.min(y_of(v)), zero.max(y_of(v)));
        for x in c * PX + 1..(c + 1) * PX - 1 {
            for y in a..=b {
                put(x, y, colour, &mut rgba);
            }
        }
    }
    for x in 0..shown * PX * usize::from(state.p("byteCurve") > 0.5) {
        // The trilinear read a ray takes between the cell centres, a white curve.
        let at = worked.as_vec3() + Vec3::new(x as f32 / PX as f32, 0.5, 0.5);
        if at.x >= CASCADE_SIZE as f32 {
            break;
        }
        let v = light::sample_sdf(sdf, at / CASCADE_SIZE as f32) * 255.0 - 1.0;
        put(x, y_of(v), [240, 240, 240, 255], &mut rgba);
    }
    // A click on the chart switches its bars between the stored byte and the byte a ray reads (`toggleByteRead`).
    content.insets.push(PictureInset {
        click: Some("toggleByteRead".into()),
        ..picture(
            Corner::BottomLeft,
            Caption {
                title: tr("demos-byte_profile").into(),
                key: format!("byte-profile-{shown}"),
                lines: vec![(tr("demos-byte_profile_note").into(), palette::text_dim())],
            },
            w.max(h),
            square(rgba, w, h),
            [tr("demos-byte_axis").into(), tr("demos-read_axis").into()],
            None,
        )
    });
}

/// A `w` x `h` picture centred in a square one, for `picture`.
fn square(rgba: Vec<u8>, w: usize, h: usize) -> Vec<u8> {
    let n = w.max(h);
    let mut out = vec![0u8; n * n * 4];
    for px in out.chunks_mut(4) {
        px.copy_from_slice(&[12, 20, 32, 255]);
    }
    let (ox, oy) = ((n - w) / 2, (n - h) / 2);
    for y in 0..h {
        let src = y * w * 4;
        let dst = ((y + oy) * n + ox) * 4;
        out[dst..dst + w * 4].copy_from_slice(&rgba[src..src + w * 4]);
    }
    out
}

/// With `floodNeighbours`, the cells the worked cell compares at `floodStep`: at a half-size pass, the 26 cells a
/// step away along the axes and diagonals (the step in full cells); at the upscale, the eight voxels of the block its
/// seed lies in.
fn flood_reach(
    state: &ShotState,
    world: &World,
    (step, worked, seed): (usize, IVec3, [u8; 4]),
    content: &mut FrameContent,
) {
    if state.p("floodNeighbours") <= 0.5 {
        return;
    }
    let from = cell_world(world, 0, worked);
    let mut cells = Vec::new();
    if (1..=6).contains(&step) {
        // A half-size pass: its step is in half-size cells, so twice that in full cells.
        let reach = 2 * voxels::half_schedule()[step - 1].0;
        for i in -1..=1 {
            for j in -1..=1 {
                for k in -1..=1 {
                    if (i, j, k) != (0, 0, 0) {
                        cells.push(worked + IVec3::new(i, j, k) * reach);
                    }
                }
            }
        }
    } else if step == 7 && seed[3] != 0 {
        // The upscale: the 2 x 2 x 2 block of full cells the seed lies in.
        let base = (IVec3::new(seed[0] as i32, seed[1] as i32, seed[2] as i32) >> IVec3::ONE)
            << IVec3::ONE;
        for k in 0..8 {
            cells.push(base + IVec3::new(k & 1, (k >> 1) & 1, (k >> 2) & 1));
        }
    }
    for c in cells.into_iter().filter(|&c| inside(c)) {
        // A neighbour inside the room gets its line; one beyond it a short stub toward it, so lines stay in the room.
        let at = cell_world(world, 0, c);
        if Scene::inside_room(at, 0.0) {
            content
                .draw
                .segment(from, at, palette::OUTSIDE, 0.6, 1.5, 0);
            ball(content, at, 0.05, palette::OUTSIDE, 1.0);
        } else {
            let tip = from + (at - from).normalize_or_zero() * 0.5;
            content.draw.segment(
                from,
                tip,
                palette::OUTSIDE,
                0.6,
                1.5,
                lab::frame::FLAG_ARROW,
            );
        }
    }
}

/// With `neighbourFlags`, the worked voxel's 26 neighbour flags from its solid-cell record: a short line toward each
/// neighbour, bright where the flag is set (that neighbour is empty and its nearest surface is this voxel).
pub(crate) fn neighbour_flags(
    state: &ShotState,
    world: &World,
    p: IVec3,
    content: &mut FrameContent,
) {
    if state.p("neighbourFlags") <= 0.5 {
        return;
    }
    let Some(cell) = world.data[0].stored.cells.iter().find(|c| c.cell() == p) else {
        return;
    };
    let flags = cell.neighbours();
    let w = cell_world(world, 0, p);
    // One cell's box in the room (its height unsquashed), each neighbour drawn as a box of that size.
    let cell = (cell_world(world, 0, p + IVec3::ONE) - w).abs();
    wire_box(
        content,
        w - cell * 0.45,
        cell * 0.9,
        palette::SELECTED,
        1.0,
        3.0,
    );
    let mut bit = 0;
    for i in -1..=1 {
        for j in -1..=1 {
            for k in -1..=1 {
                if (i, j, k) == (0, 0, 0) {
                    continue;
                }
                // The flags in the order STORE sets them: x, then y, then z, from -1 to 1; a set flag's box lit.
                let on = flags & (1 << bit) != 0;
                let at = cell_world(world, 0, p + IVec3::new(i, j, k));
                let (colour, alpha, width) = if on {
                    (palette::INCIDENT, 1.0, 3.0)
                } else {
                    (palette::BRICK_EMPTY, 0.3, 1.0)
                };
                wire_box(content, at - cell * 0.42, cell * 0.84, colour, alpha, width);
                bit += 1;
            }
        }
    }
}
