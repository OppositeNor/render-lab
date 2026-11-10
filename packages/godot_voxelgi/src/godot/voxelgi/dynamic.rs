//! Dynamic objects, on the CPU: what `GI::VoxelGIInstance::update` does
//! each frame for a GeometryInstance3D with GI mode Dynamic
//! (servers/rendering/renderer_rd/environment/gi.cpp) with voxel_gi.glsl's
//! `MODE_DYNAMIC` kernels. The object's box, in cells oversampled 16 times,
//! is drawn from six orthographic views (+x, +y, +z, -x, -y, -z) with its
//! material (`_render_material`: albedo, normal, emission, depth); each view's
//! texels are lit like cells (`MODE_DYNAMIC_LIGHTING`, shadows from the baked
//! field), then shrunk two by two (`MODE_DYNAMIC_SHRINK`): 16x, 8x, 4x, 2x
//! written only, then 1x and every coarser mip also plotted into the light
//! texture, blended over what is there. Before the first plot of a frame the
//! texture is cleared and the static light written again from the float
//! buffer; nothing bounces off the dynamic voxels.
//!
//! Where the lab differs: the views are found by casting a ray through the
//! centre of each texel's column (the nearest hit, either face) instead of
//! rasterizing the mesh; the maps are kept unflipped (the engine flips some
//! views' storage and undoes it when it reads them, which changes no value).
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::light::{self, LIGHT_TYPE_AREA, Light, Params};
use super::storage::{LightTexture, Probe};
use glam::{IVec2, IVec3, Vec3, Vec4};
use lab::layout::ObjectData;

/// `dynamic_maps[0].size / max(octree_size)`: the first map's oversampling.
pub const MULTIPLIER: i32 = 16;
/// The oversampled maps before the 1x one: 16x, 8x, 4x, 2x.
pub const OVERSAMPLED: usize = 4;

/// `render_z` and `render_up` of the six views.
pub const RENDER_Z: [Vec3; 6] = [
    Vec3::X,
    Vec3::Y,
    Vec3::Z,
    Vec3::NEG_X,
    Vec3::NEG_Y,
    Vec3::NEG_Z,
];
pub const RENDER_UP: [Vec3; 6] = [Vec3::Y, Vec3::Z, Vec3::Y, Vec3::Y, Vec3::Z, Vec3::Y];

/// One map of a view's shrink chain: its rectangle (in that map's texels of
/// the probe's grid), its light (rgb, alpha the coverage) and depth along the view.
#[derive(Clone, Debug)]
pub struct Map {
    /// -1 for the oversampled maps, else the mip it plots into.
    pub mipmap: i32,
    pub rect_size: IVec2,
    pub light: Vec<Vec4>,
    pub depth: Vec<f32>,
}

impl Map {
    pub fn at(&self, p: IVec2) -> (Vec4, f32) {
        let i = (p.x + p.y * self.rect_size.x) as usize;
        (self.light[i], self.depth[i])
    }
}

/// One of the six views: the first map's albedo, and the chain.
#[derive(Clone, Debug)]
pub struct View {
    /// The first map's albedo (rgb, a = covered).
    pub albedo: Vec<Vec4>,
    pub maps: Vec<Map>,
}

/// One object's plot: its oversampled box and its six views, and the texture after it.
pub struct Plot {
    pub begin: IVec3,
    pub end: IVec3,
    pub views: Vec<View>,
    pub texture: LightTexture,
    /// Texels the plot wrote, per mip.
    pub plotted: Vec<Vec<IVec3>>,
}

/// Godot's `Basis::looking_at(target, up)`: columns x, y, z with -z toward the target.
fn looking_at(target: Vec3, up: Vec3) -> [Vec3; 3] {
    let v_z = -target.normalize();
    let v_x = up.cross(v_z).normalize();
    let v_y = v_z.cross(v_x);
    [v_x, v_y, v_z]
}

fn axis_of(v: Vec3) -> usize {
    let a = v.abs();
    if a.x > 0.5 {
        0
    } else if a.y > 0.5 {
        1
    } else {
        2
    }
}

/// The views' setup for the box `begin..end` (oversampled cells).
fn view_axes(j: usize, begin: IVec3, end: IVec3) -> (Vec3, [Vec3; 3], Vec3) {
    let render_dir = RENDER_Z[j];
    let size = (end - begin).as_vec3();
    let center = begin.as_vec3() + size * 0.5;
    let eye = center - size * 0.5 * render_dir;
    (render_dir, looking_at(center - eye, RENDER_UP[j]), eye)
}

/// What one object's dynamic pass reads: the probe and its light parameters, the lights, the object, and the
/// node's offset (world to the probe's cells).
struct Plotting<'a> {
    probe: &'a Probe,
    params: &'a Params,
    lights: &'a [Light],
    object: &'a ObjectData,
    node: Vec3,
}

/// One view of the oversampled box: the direction it renders along, the texture axes its x, y, and depth run
/// along, those directions in cells, where its depth starts and which way it counts, and its rectangle.
struct ViewFrame {
    render_dir: Vec3,
    axes: [usize; 3],
    dirs: [IVec3; 3],
    z_base: f32,
    z_sign: f32,
    rect_pos: IVec2,
    rect_size: IVec2,
}

impl ViewFrame {
    /// View `j` of the box `begin..end` (oversampled cells).
    fn new(j: usize, begin: IVec3, end: IVec3) -> ViewFrame {
        let size = end - begin;
        let (render_dir, basis, eye) = view_axes(j, begin, end);
        let x_axis = axis_of(basis[0]);
        let y_axis = axis_of(basis[1]);
        let z_dir_f = -basis[2];
        let z_axis = axis_of(z_dir_f);
        let x_dir = basis[0].abs().round().as_ivec3();
        let y_dir = basis[1].abs().round().as_ivec3();
        let z_dir = z_dir_f.round().as_ivec3();
        // A view looking toward the axis's negative end counts its depth down from the near face.
        let z_flip = basis[2].x + basis[2].y + basis[2].z > 0.0;
        let z_base = eye[z_axis];
        let z_sign = if z_flip {
            -1.0
        } else {
            1.0
        };
        ViewFrame {
            render_dir,
            axes: [x_axis, y_axis, z_axis],
            dirs: [x_dir, y_dir, z_dir],
            z_base,
            z_sign,
            rect_pos: IVec2::new(begin[x_axis], begin[y_axis]),
            rect_size: IVec2::new(size[x_axis], size[y_axis]),
        }
    }
}

/// Where a reduction of a view's chain is: its rectangle (position and size) and the mip's limits.
struct Chain {
    rect_p: IVec2,
    rect_s: IVec2,
    limits: IVec3,
}

/// What the plot writes: the light texture, and the texels written per mip.
struct Plotted<'a> {
    tex: &'a mut LightTexture,
    plotted: &'a mut [Vec<IVec3>],
}

/// The dynamic pass for one object: `object` (its world record), lit by
/// `lights` with the field of `probe`, plotted over `texture`.
pub fn plot(
    probe: &Probe,
    params: &Params,
    lights: &[Light],
    object: &ObjectData,
    texture: &LightTexture,
    node: Vec3,
) -> Plot {
    let ctx = Plotting {
        probe,
        params,
        lights,
        object,
        node,
    };
    let (begin, end) = ctx.oversampled_box();
    let mut tex = texture.clone();
    let mut plotted = vec![Vec::new(); probe.mip_count()];
    let mut views = Vec::new();
    for j in 0..6 {
        // Each of the six views: its first map rendered, then shrunk map by map, plotting from 1x on.
        let view = ViewFrame::new(j, begin, end);
        let (albedo, first) = ctx.render_view(&view, end - begin);
        let mut maps = vec![first];
        let map_count = OVERSAMPLED + probe.mip_count();
        let mut chain = Chain {
            rect_p: view.rect_pos,
            rect_s: view.rect_size,
            limits: probe.octree_size,
        };
        for k in 1..map_count {
            let out = Plotted {
                tex: &mut tex,
                plotted: &mut plotted,
            };
            let map = ctx.reduce(&view, k, &mut chain, maps.last().unwrap(), out);
            maps.push(map);
        }
        views.push(View { albedo, maps });
    }
    for p in &mut plotted {
        // Each mip's plotted texels, once each, in order.
        p.sort_by_key(|v| (v.z, v.y, v.x));
        p.dedup();
    }
    Plot {
        begin,
        end,
        views,
        texture: tex,
        plotted,
    }
}

impl Plotting<'_> {
    /// A world point in oversampled cells.
    fn over_of(&self, p: Vec3) -> Vec3 {
        self.probe.to_cell(p - self.node) * MULTIPLIER as f32
    }

    /// An oversampled cell's point in the world.
    fn world_of(&self, c: Vec3) -> Vec3 {
        self.probe.from_cell(c / MULTIPLIER as f32) + self.node
    }

    /// The object's box in oversampled cells, from its world bounds through its transform (its eight corners):
    /// `begin = aabb.position - 1`, `end = aabb.end + 1` (Vector3i truncates), even sizes, clamped.
    fn oversampled_box(&self) -> (IVec3, IVec3) {
        let octree = self.probe.octree_size;
        let o = self.object;
        let l2w = glam::Mat4::from_cols_array_2d(&o.local_to_world);
        let (lc, le) = (Vec3::from(o.local_center), Vec3::from(o.local_extent));
        let corners: Vec<Vec3> = (0..8)
            .map(|i| {
                self.over_of(l2w.transform_point3(
                    lc + le
                        * Vec3::new(
                            if i & 1 == 0 {
                                -1.0
                            } else {
                                1.0
                            },
                            if i & 2 == 0 {
                                -1.0
                            } else {
                                1.0
                            },
                            if i & 4 == 0 {
                                -1.0
                            } else {
                                1.0
                            },
                        ),
                ))
            })
            .collect();
        let min = corners.iter().fold(Vec3::splat(f32::MAX), |a, &c| a.min(c));
        let max = corners.iter().fold(Vec3::splat(f32::MIN), |a, &c| a.max(c));
        let mut begin = (min - Vec3::ONE).as_ivec3();
        let mut end = (min + (max - min) + Vec3::ONE).as_ivec3();
        for j in 0..3 {
            if (end[j] - begin[j]) & 1 != 0 {
                end[j] += 1;
            }
            begin[j] = begin[j].max(0);
            end[j] = end[j].min(octree[j] * MULTIPLIER);
        }
        (begin, end)
    }

    /// The view's first map (16x): each texel's column cast along the view to the object, its albedo, and its
    /// voxel lit like a cell (MODE_DYNAMIC_LIGHTING) at its depth; and the albedo (a = covered).
    fn render_view(&self, view: &ViewFrame, size: IVec3) -> (Vec<Vec4>, Map) {
        let ViewFrame {
            render_dir,
            axes: [x_axis, y_axis, z_axis],
            dirs: [x_dir, y_dir, z_dir],
            z_base,
            z_sign,
            rect_pos,
            rect_size,
        } = *view;
        let (probe, o, m) = (self.probe, self.object, MULTIPLIER as f32);
        let n = (rect_size.x * rect_size.y) as usize;
        let mut albedo = vec![Vec4::ZERO; n];
        let mut lit = vec![Vec4::ZERO; n];
        let mut depth = vec![0.0f32; n];
        let world_per_over = 1.0 / (probe.to_cell_scale * m);
        for py in 0..rect_size.y {
            for px in 0..rect_size.x {
                let i = (px + py * rect_size.x) as usize;
                // The column's centre on the view's near face, the ray along the view.
                let mut c = Vec3::ZERO;
                c[x_axis] = (rect_pos.x + px) as f32 + 0.5;
                c[y_axis] = (rect_pos.y + py) as f32 + 0.5;
                c[z_axis] = z_base;
                let ro = self.world_of(c);
                let t_max = size[z_axis] as f32 * world_per_over;
                let Some((t, nw)) = lab::scene::intersect_object(o, ro, render_dir, t_max) else {
                    continue;
                };
                let d = t / world_per_over;
                let nw = nw.normalize();
                albedo[i] = Vec4::new(
                    o.albedo_roughness[0],
                    o.albedo_roughness[1],
                    o.albedo_roughness[2],
                    1.0,
                );
                // MODE_DYNAMIC_LIGHTING: the voxel from the depth, lit like a cell.
                let z = z_base + d * z_sign;
                depth[i] = z;
                let pos =
                    x_dir * (rect_pos.x + px) + y_dir * (rect_pos.y + py) + z_dir.abs() * z as i32;
                let p = pos.as_vec3() / m;
                let accum = self.light_voxel(p, nw, albedo[i].truncate());
                let emission =
                    Vec3::new(o.emissive[0], o.emissive[1], o.emissive[2]) * o.emissive[3];
                lit[i] = (accum + emission).extend(1.0);
            }
        }
        let map = Map {
            mipmap: -1,
            rect_size,
            light: lit,
            depth,
        };
        (albedo, map)
    }

    /// A dynamic voxel at `p` (cells) with normal `nw` and albedo `albedo`, lit by every light as a cell is.
    fn light_voxel(&self, p: Vec3, nw: Vec3, albedo: Vec3) -> Vec3 {
        let mut accum = Vec3::ZERO;
        for l in self.lights {
            if l.kind != LIGHT_TYPE_AREA {
                let Some((li, light_dir)) =
                    light::light_at_pos(self.probe, self.params, l, p, nw, None)
                else {
                    continue;
                };
                accum += nw.dot(-light_dir).max(0.0) * li * albedo;
            } else {
                let Some(li) = light::area_light(self.params, l, p, nw) else {
                    continue;
                };
                accum += li * albedo;
            }
        }
        accum
    }

    /// Map `k` of the view's chain (MODE_DYNAMIC_SHRINK): `prev` shrunk two by two, written while a coarser
    /// map follows, and from the 1x map on plotted into the texture, blended over what is there.
    fn reduce(
        &self,
        view: &ViewFrame,
        k: usize,
        chain: &mut Chain,
        prev: &Map,
        out: Plotted,
    ) -> Map {
        let (probe, params) = (self.probe, self.params);
        let mipmap = if k < OVERSAMPLED {
            -1
        } else {
            (k - OVERSAMPLED) as i32
        };
        let Chain {
            rect_p,
            rect_s,
            limits,
        } = chain;
        let mut prev_ofs = IVec2::ZERO;
        for a in 0..2 {
            // Halving an odd rectangle: an odd start keeps the cell before it, an odd size rounds up.
            if rect_p[a] & 1 != 0 {
                rect_s[a] += 1;
                prev_ofs[a] = 1;
            }
            if rect_s[a] & 1 != 0 {
                rect_s[a] += 1;
            }
            rect_p[a] >>= 1;
            rect_s[a] = (rect_s[a] >> 1).max(1);
        }
        if mipmap > 0 {
            *limits = (*limits / 2).max(IVec3::ONE);
        }
        let (rect_p, rect_s, limits) = (*rect_p, *rect_s, *limits);
        let on_mipmap = mipmap > 0;
        let n = (rect_s.x * rect_s.y) as usize;
        let mut out_light = vec![Vec4::ZERO; n];
        let mut out_depth = vec![0.0f32; n];
        let write = mipmap < probe.mip_count() as i32 - 1;
        let do_plot = mipmap >= 0;
        let Plotted { tex, plotted } = out;
        for py in 0..rect_s.y {
            for px in 0..rect_s.x {
                let mut accum = Vec4::ZERO;
                let mut accum_z = 0.0f32;
                let mut count = 0.0f32;
                for i in 0..4 {
                    // The 2 x 2 texels of the bigger map under this one; uncovered ones count for nothing.
                    let ofs = IVec2::new(px * 2 + (i & 1), py * 2 + (i >> 1)) - prev_ofs;
                    if ofs.x < 0
                        || ofs.y < 0
                        || ofs.x >= prev.rect_size.x
                        || ofs.y >= prev.rect_size.y
                    {
                        continue;
                    }
                    let (l, z) = prev.at(ofs);
                    if l.w == 0.0 {
                        continue;
                    }
                    accum += l;
                    accum_z += z * 0.5;
                    count += 1.0;
                }
                if on_mipmap {
                    // A mip divides as the light's mips do (by 8, or by the count with propagation).
                    let rgb = accum.truncate() / (8.0 + (count - 8.0) * params.propagation);
                    accum = rgb.extend(accum.w / 8.0);
                } else {
                    // An oversampled map averages its four texels.
                    accum /= 4.0;
                }
                accum_z = if count == 0.0 {
                    // No covered texel: no depth.
                    0.0
                } else {
                    accum_z / count
                };
                let i = (px + py * rect_s.x) as usize;
                if write {
                    out_light[i] = accum;
                    out_depth[i] = accum_z;
                }
                if do_plot && accum.w >= 0.001 {
                    // The texel's voxel, from its place in the map and its depth along the view.
                    let [x_dir, y_dir, z_dir] = view.dirs;
                    let pos = x_dir * (rect_p.x + px)
                        + y_dir * (rect_p.y + py)
                        + z_dir.abs() * accum_z as i32;
                    let at = (mipmap as usize, limits);
                    let out = Plotted {
                        tex: &mut *tex,
                        plotted: &mut *plotted,
                    };
                    self.plot_texel(at, (pos, z_dir.abs()), (accum, accum_z), out);
                }
            }
        }
        Map {
            mipmap,
            rect_size: rect_s,
            light: out_light,
            depth: out_depth,
        }
    }

    /// A shrunk texel `accum` at depth `accum_z`, plotted at `pos` of mip `mipmap` (inside `limits`) and the
    /// texel after it along the view (`step`), split by the depth's fraction and blended over what is there.
    fn plot_texel(
        &self,
        (mipmap, limits): (usize, IVec3),
        (pos, step): (IVec3, IVec3),
        (accum, accum_z): (Vec4, f32),
        out: Plotted,
    ) {
        let params = self.params;
        let z_frac = accum_z.fract();
        for s in 0..2 {
            let pos3d = pos + step * s;
            // The texel splits over the voxel at its depth and the next one, by the depth's fraction.
            if pos3d.cmplt(IVec3::ZERO).any() || pos3d.cmpge(limits).any() {
                continue;
            }
            let blit = accum
                * if s == 0 {
                    1.0 - z_frac
                } else {
                    z_frac
                };
            let mut color = out.tex.texel(mipmap, pos3d);
            // Blend over what is stored: back to light units, "over" by alpha, then back to the stored range.
            color = (color.truncate() * params.dynamic_range).extend(color.w);
            let sa = 1.0 - blit.w;
            let a = color.w * sa + blit.w;
            if a != 0.0 {
                color =
                    ((color.truncate() * color.w * sa + blit.truncate() * blit.w) / a).extend(a);
            }
            color = (color.truncate() / params.dynamic_range).extend(color.w);
            out.tex.store(mipmap, pos3d, color);
            out.plotted[mipmap].push(pos3d);
        }
    }
}
