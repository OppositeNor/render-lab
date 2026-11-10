//! The decals' and the light culling's overlays: the decals' boxes and names, the inspected point's tile, and the lights' markers.

use super::*;

/// The names the demonstration gives the scene's decals (decals::scene_decals).
pub(super) fn decal_name(texture: usize) -> &'static str {
    [tr("demos-splat"), tr("demos-grid")][texture.min(1)]
}

/// The twelve edges of a box.
pub(super) fn obb_edges(
    draw: &mut DrawLists,
    center: Vec3,
    rotation: glam::Quat,
    half: Vec3,
    color: Color32,
    alpha: f32,
    width: f32,
) {
    let [x, y, z] = [
        rotation * Vec3::X * half.x,
        rotation * Vec3::Y * half.y,
        rotation * Vec3::Z * half.z,
    ];
    // Bits 1, 2, and 4 of a corner's index pick its side along x, y, and z.
    let corner = |i: u32| {
        center
            + x * if i & 1 != 0 {
                1.0
            } else {
                -1.0
            }
            + y * if i & 2 != 0 {
                1.0
            } else {
                -1.0
            }
            + z * if i & 4 != 0 {
                1.0
            } else {
                -1.0
            }
    };
    for i in 0..8u32 {
        // Each edge joins a corner to the one across a single bit, drawn once, from the corner with it clear.
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                draw.segment(corner(i), corner(i | bit), color, alpha, width, 0);
            }
        }
    }
}

/// Each decal's box (m_position, m_quaternion, m_halfSize), its projection
/// axis (local z, the decalUp of GetDecalAttenuation), and its sort key.
pub(super) fn decal_boxes(
    draw: &mut DrawLists,
    labels: &mut Vec<Label>,
    decals: &[super::super::decals::Decal],
    alpha: f32,
) {
    for d in decals {
        // Each decal draws its box, an arrow along the axis it projects down, and its name.
        let color = [palette::accent(), palette::REFLECTED][d.texture.min(1)];
        let [x, y, z] = d.axes();
        // Bits 1, 2, and 4 of a corner's index pick its side along x, y, and z.
        let corner = |i: u32| {
            d.position
                + x * d.half_size.x
                    * if i & 1 != 0 {
                        1.0
                    } else {
                        -1.0
                    }
                + y * d.half_size.y
                    * if i & 2 != 0 {
                        1.0
                    } else {
                        -1.0
                    }
                + z * d.half_size.z
                    * if i & 4 != 0 {
                        1.0
                    } else {
                        -1.0
                    }
        };
        for i in 0..8u32 {
            // Each edge joins a corner to the one across a single bit, drawn once, from the corner with it clear.
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    draw.segment(corner(i), corner(i | bit), color, alpha * 0.8, 2.0, 0);
                }
            }
        }
        draw.segment(
            d.position,
            d.position + z * (d.half_size.z + 0.25),
            color,
            alpha,
            3.0,
            FLAG_ARROW,
        );
        labels.push(Label {
            anchor: d.position + z * (d.half_size.z + 0.3),
            text: format!("{} · sortKey {}", decal_name(d.texture), d.sort_key),
            color,
        });
    }
}

/// The inspected point's tile as the culling demonstration shows it.
pub struct CullTile {
    pub tile: [u32; 2],
    pub data: super::super::culling::TileLightData,
    /// The point's bin, and the lights the bin keeps (group 1 of its remapped list).
    pub bin: u32,
    pub kept: Vec<u32>,
    /// The simple point lights the tile's list holds, for any bin.
    pub listed: Vec<u32>,
    pub point: Vec3,
}

/// The culling readout and the inspected point's tile, from the CPU mirror on
/// the tile's ray-cast depths.
pub fn culling_view(
    state: &ShotState,
    shading: &Shading,
    objects: &[ObjectData],
    camera: &lab::layout::CameraData,
) -> (Vec<(String, String, Color32)>, CullTile) {
    use super::super::culling;
    let (p, _) = inspected_point(state);
    let projector = lab::camera::Projector::new(camera);
    let pixel = projector.project(p).map_or(Vec2::ZERO, |(px, _)| px);
    // The point's tile is the TILE_DIM-pixel square of the screen that holds its pixel.
    let tile = [
        (pixel.x.max(0.0) as u32) / culling::TILE_DIM,
        (pixel.y.max(0.0) as u32) / culling::TILE_DIM,
    ];
    let data = culling::cpu_tile(camera, objects, tile);
    let lights = shading.lights();
    let list = culling::cull_tile(
        camera,
        &data,
        tile,
        &lights,
        &shading.decals,
        &shading.sphere_lights,
    );
    // The point's view distance picks its depth bin.
    let distance = -glam::Mat4::from_cols_array_2d(&camera.world_to_view)
        .transform_point3(p)
        .z;
    let bin = data.bin(distance);
    let bins = culling::remap(&list);
    // A bin's list holds the light groups one after another; the point's bin keeps the simple point lights' group.
    let kept: Vec<u32> = bins[bin as usize]
        .split(|&v| v == culling::END_OF_GROUP)
        .nth(culling::GROUP_SIMPLE_POINT as usize)
        .unwrap_or(&[])
        .to_vec();
    // The tile's list holds every group's lights, each group ended by a marker the count leaves out.
    let in_tile = list.len() - culling::LIGHT_GROUPS as usize;
    // Each list entry keeps a light's index in its top 16 bits.
    let listed: Vec<u32> = list
        .split(|&e| e >> 16 == culling::END_OF_GROUP)
        .nth(culling::GROUP_SIMPLE_POINT as usize)
        .unwrap_or(&[])
        .iter()
        .map(|e| e >> 16)
        .collect();
    // The lights whose attenuation radius reaches the point.
    let reach = lights
        .iter()
        .filter(|l| (l.position - p).length_squared() * l.inv_attenuation_radius_squared < 1.0)
        .count();
    let rows = vec![
        (
            tr("demos-tile_16_x_16_px").to_string(),
            format!("({}, {})", tile[0], tile[1]),
            palette::text(),
        ),
        (
            "zNear … zFar".to_string(),
            format!("{:.2} … {:.2} m", data.z_near, data.z_far),
            palette::accent(),
        ),
        (
            tr("demos-depth_bins_2_logmaxbins").to_string(),
            format!("{}", 1u32 << data.log_max_bins),
            palette::accent(),
        ),
        (
            tr("demos-32_slice_mask").to_string(),
            format!("0x{:08X}", data.mask),
            palette::accent(),
        ),
        (
            tr("demos-simple_point_lights").to_string(),
            format!("{}", lights.len()),
            LIGHT,
        ),
        (
            tr("demos-in_the_tile_s_list").to_string(),
            format!("{in_tile}"),
            LIGHT,
        ),
        (
            tr("demos-the_point_s_bin").to_string(),
            format!("{bin}  ({:.2} m)", distance),
            palette::accent(),
        ),
        (
            tr("demos-in_that_bin").to_string(),
            format!("{}", kept.len()),
            LIGHT,
        ),
        (
            tr("demos-lights_reaching_the_point").to_string(),
            format!("{reach}"),
            DIFFUSE,
        ),
    ];
    (
        rows,
        CullTile {
            tile,
            data,
            bin,
            kept,
            listed,
            point: p,
        },
    )
}

/// The tile's view-space box in the world, its depth bins as slices, and the
/// lights its bin keeps.
pub(super) fn tile_overlay(
    draw: &mut DrawLists,
    camera: &lab::layout::CameraData,
    tile: &CullTile,
    alpha: f32,
) {
    use super::super::culling;
    let to_world = glam::Mat4::from_cols_array_2d(&camera.world_to_view).inverse();
    let projection = glam::Mat4::from_cols_array_2d(&camera.view_to_clip);
    let grid = Vec2::splat(culling::TILE_DIM as f32) / Vec2::from(camera.viewport_size);
    // A screen position (uv) at a view distance, carried back into the world.
    let corner = |uv: Vec2, distance: f32| {
        let ndc = Vec2::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
        to_world.transform_point3(Vec3::new(
            ndc.x * distance / projection.x_axis.x,
            ndc.y * distance / projection.y_axis.y,
            -distance,
        ))
    };
    let uv0 = Vec2::new(tile.tile[0] as f32, tile.tile[1] as f32) * grid;
    let uvs = [
        uv0,
        uv0 + Vec2::new(grid.x, 0.0),
        uv0 + grid,
        uv0 + Vec2::new(0.0, grid.y),
    ];
    let bins = 1u32 << tile.data.log_max_bins;
    for k in 0..=bins {
        // Each bin boundary is a ring around the tile at its depth; the two that bound the point's bin are lit.
        let z = tile.data.z_near + (tile.data.z_far - tile.data.z_near) * k as f32 / bins as f32;
        let ring: Vec<Vec3> = uvs
            .iter()
            .chain(std::iter::once(&uvs[0]))
            .map(|uv| corner(*uv, z))
            .collect();
        let color = if k == tile.bin || k == tile.bin + 1 {
            palette::SELECTED
        } else {
            palette::accent()
        };
        draw.polyline(&ring, color, alpha, 2.0);
    }
    for uv in uvs {
        // The tile's four corner edges run from its nearest depth to its farthest.
        draw.segment(
            corner(uv, tile.data.z_near),
            corner(uv, tile.data.z_far),
            palette::accent(),
            alpha * 0.7,
            1.5,
            0,
        );
    }
    draw.point(tile.point, 9.0, palette::SELECTED, alpha);
}

/// A marker on each of the culling demonstration's lights, in its colour:
/// faint when the inspected point's tile does not list it, brighter when the
/// tile lists it for other bins, and lit, ringed, and joined to the point
/// when the point's bin keeps it. List indices count the room's light first;
/// it keeps its own marker, and is ringed and joined the same way.
pub(super) fn light_markers(
    draw: &mut DrawLists,
    shading: &Shading,
    tile: Option<&CullTile>,
    alpha: f32,
) {
    if let Some(t) = tile.filter(|t| shading.light_on && t.kept.contains(&0)) {
        // The room's light is index 0: lit, ringed, and joined to the point when the point's bin keeps it.
        draw.point(shading.light.position, 17.0, palette::text(), alpha);
        draw.point(shading.light.position, 11.0, LIGHT, alpha);
        draw.segment(
            t.point,
            shading.light.position,
            palette::text(),
            alpha * 0.8,
            2.0,
            FLAG_DASHED,
        );
    }
    for (i, light) in shading.extra_lights.iter().enumerate() {
        // The extra lights take list indices from 1, after the room's light.
        let index = i as u32 + 1;
        let [r, g, b] = (EXTRA_LIGHT_COLORS[i % 4] * 255.0).round().to_array();
        let color = Color32::from_rgb(r as u8, g as u8, b as u8);
        let kept = tile.is_some_and(|t| t.kept.contains(&index));
        let listed = tile.is_some_and(|t| t.listed.contains(&index));
        if kept {
            // Kept by the point's bin: lit, ringed, and joined to the point.
            draw.point(light.position, 16.0, palette::text(), alpha);
            draw.point(light.position, 11.0, color, alpha);
            if let Some(t) = tile {
                draw.segment(
                    t.point,
                    light.position,
                    palette::text(),
                    alpha * 0.8,
                    2.0,
                    FLAG_DASHED,
                );
            }
        } else {
            // Not kept: larger and brighter when the tile lists it for another bin, faint when not listed at all.
            draw.point(
                light.position,
                if listed {
                    10.0
                } else {
                    8.0
                },
                color,
                alpha
                    * if listed {
                        0.85
                    } else {
                        0.4
                    },
            );
        }
    }
}
