//! The CPU mirror of Godot's SSR passes (screen_space_reflection*.glsl, as
//! shaders/ssr_passes.wgsl ports them): the Hi-Z reduction and the trace, in
//! f32 as the GPU computes them, over images read back from the GPU or built on
//! the CPU. The selftest compares them with the GPU's results; the trace
//! demonstrations step through `trace` one iteration at a time.

use glam::{Mat4, Vec2, Vec3, Vec4};

/// A float image, row-major, `channels` floats a texel.
#[derive(Clone, Debug)]
pub struct Image {
    pub size: [u32; 2],
    pub channels: usize,
    pub data: Vec<f32>,
}

impl Image {
    pub fn new(size: [u32; 2], channels: usize) -> Image {
        Image {
            size,
            channels,
            data: vec![0.0; size[0] as usize * size[1] as usize * channels],
        }
    }

    /// Texel `p` (texelFetch): channel 0 and on; zero outside the image (the
    /// robust access the GPU gives an out-of-range load).
    pub fn load(&self, p: [i32; 2]) -> Vec4 {
        if p[0] < 0 || p[1] < 0 || p[0] >= self.size[0] as i32 || p[1] >= self.size[1] as i32 {
            return Vec4::ZERO;
        }
        let i = (p[1] as usize * self.size[0] as usize + p[0] as usize) * self.channels;
        let mut v = [0.0f32; 4];
        v[..self.channels].copy_from_slice(&self.data[i..i + self.channels]);
        Vec4::from_array(v)
    }

    pub fn store(&mut self, p: [u32; 2], v: Vec4) {
        let i = (p[1] as usize * self.size[0] as usize + p[0] as usize) * self.channels;
        self.data[i..i + self.channels].copy_from_slice(&v.to_array()[..self.channels]);
    }

    /// textureLod(sampler(LINEAR, CLAMP), uv, 0): bilinear, clamped to the edge.
    pub fn sample(&self, uv: Vec2) -> Vec4 {
        // The texel coordinate of `uv`, half a texel back, so the four texels around it are b and b + 1.
        let f = uv * Vec2::new(self.size[0] as f32, self.size[1] as f32) - Vec2::splat(0.5);
        let b = f.floor();
        let w = f - b;
        let hi = [self.size[0] as i32 - 1, self.size[1] as i32 - 1];
        let c = |x: i32, y: i32| self.load([x.clamp(0, hi[0]), y.clamp(0, hi[1])]);
        let (x0, y0) = (b.x as i32, b.y as i32);
        // Blend along x on the two rows, then between the rows along y.
        let top = c(x0, y0).lerp(c(x0 + 1, y0), w.x);
        let bottom = c(x0, y0 + 1).lerp(c(x0 + 1, y0 + 1), w.x);
        top.lerp(bottom, w.y)
    }
}

/// One Hi-Z level from the one before (screen_space_reflection_hiz.glsl):
/// each texel the max of its 2 x 2 parents, 3 wide along an odd side of the parent.
pub fn hiz_level(parent: &Image, size: [u32; 2]) -> Image {
    let mut out = Image::new(size, 1);
    let odd = [parent.size[0] % 2 == 1, parent.size[1] % 2 == 1];
    for y in 0..size[1] {
        for x in 0..size[0] {
            // Each texel starts from the max of the 2 x 2 parents at twice its coordinate.
            let p = [x as i32 * 2, y as i32 * 2];
            let mut d = parent.load(p).x;
            let mut take = |dx: i32, dy: i32| d = d.max(parent.load([p[0] + dx, p[1] + dy]).x);
            take(1, 0);
            take(0, 1);
            take(1, 1);
            if odd[0] {
                // An odd width leaves a column the halving would drop, so the texel takes it too.
                take(2, 0);
                take(2, 1);
            }
            if odd[1] {
                // Likewise an odd height's last row.
                take(0, 2);
                take(1, 2);
            }
            if odd[0] && odd[1] {
                // And with both odd, the corner the extra row and column share.
                take(2, 2);
            }
            out.store([x, y], Vec4::new(d, 0.0, 0.0, 0.0));
        }
    }
    out
}

/// The trace's inputs: the Hi-Z levels, the half-size normal-roughness, the
/// last frame, the UBO's matrices, and the parameters (as the plan pushes them).
pub struct Inputs<'a> {
    pub hiz: &'a [Image],
    pub normal_roughness: &'a Image,
    pub last_frame: &'a Image,
    pub projection: Mat4,
    pub inv_projection: Mat4,
    pub reprojection: Mat4,
    pub num_steps: i32,
    pub distance_fade: f32,
    pub curve_fade_in: f32,
    pub depth_tolerance: f32,
}

/// Why the loop stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// The pixel is sky (depth 0) or rough (roughness 0.7 or more): not traced.
    NotTraced,
    /// A hit confirmed at level 0 (the level went below 0).
    Hit,
    /// The step budget ran out.
    Steps,
    /// The ray left the screen.
    Screen,
}

/// One iteration of the loop, as a demonstration shows it.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub level: i32,
    pub t: f32,
    pub cell: [i32; 2],
    pub cell_depth: f32,
    pub depth_t: f32,
    pub edge_t: f32,
    pub hit: bool,
    /// The level-0 tolerance test turned a would-be result into "stay at level 0".
    pub held: bool,
    pub next_level: i32,
    pub next_t: f32,
}

/// What one pixel's trace computed, its steps, and its outputs.
#[derive(Clone, Debug)]
pub struct Trace {
    pub exit: Exit,
    pub roughness: f32,
    /// The view-space start, before and after the bias along the geometric normal.
    pub start: Vec3,
    pub biased: Vec3,
    pub normal: Vec3,
    pub geom_normal: Vec3,
    pub ray_dir: Vec3,
    pub bounced: bool,
    pub screen_pos: Vec3,
    pub screen_ray: Vec3,
    pub t_max: f32,
    pub steps: Vec<Step>,
    pub t: f32,
    pub hit_screen: Vec3,
    pub reprojected_uv: Vec2,
    pub validity: f32,
    /// The validity terms in the shader's order: edge or sky, back-face, confidence (squared), margin, fade.
    pub terms: [f32; 5],
    pub ray_len: f32,
    /// The colour written (premultiplied by validity, then compressed) and the blur level.
    pub color: Vec4,
    pub mip_level: f32,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The roughness of a normal-roughness texel's alpha.
pub fn decode_roughness(w: f32) -> f32 {
    let r = if w > 0.5 {
        1.0 - w
    } else {
        w
    };
    r / (127.0 / 255.0)
}

impl Inputs<'_> {
    fn size(&self) -> [i32; 2] {
        [self.hiz[0].size[0] as i32, self.hiz[0].size[1] as i32]
    }

    fn hiz(&self, p: [i32; 2], level: i32) -> f32 {
        self.hiz[level as usize].load(p).x
    }

    fn view_pos(&self, s: Vec3) -> Vec3 {
        let p = self.inv_projection * Vec4::new(s.x * 2.0 - 1.0, s.y * 2.0 - 1.0, s.z, 1.0);
        p.truncate() / p.w
    }

    fn screen_pos(&self, pos: Vec3) -> Vec3 {
        let s = self.projection * pos.extend(1.0);
        let p = s.truncate() / s.w;
        Vec3::new(p.x * 0.5 + 0.5, p.y * 0.5 + 0.5, p.z)
    }

    fn linearize(&self, depth: f32) -> f32 {
        let p = self.inv_projection * Vec4::new(0.0, 0.0, depth, 1.0);
        p.z / p.w
    }

    fn geometric_normal(&self, pixel: [i32; 2], depth_c: f32, view_c: Vec3) -> Vec3 {
        let h = Vec4::new(
            self.hiz([pixel[0] - 1, pixel[1]], 0),
            self.hiz([pixel[0] - 2, pixel[1]], 0),
            self.hiz([pixel[0] + 1, pixel[1]], 0),
            self.hiz([pixel[0] + 2, pixel[1]], 0),
        );
        let v = Vec4::new(
            self.hiz([pixel[0], pixel[1] - 1], 0),
            self.hiz([pixel[0], pixel[1] - 2], 0),
            self.hiz([pixel[0], pixel[1] + 1], 0),
            self.hiz([pixel[0], pixel[1] + 2], 0),
        );
        // How far each side's linear extrapolation misses the centre's depth: the smoother side is the same surface.
        let he = (Vec2::new(2.0 * h.x - h.y, 2.0 * h.z - h.w) - Vec2::splat(depth_c)).abs();
        let ve = (Vec2::new(2.0 * v.x - v.y, 2.0 * v.z - v.w) - Vec2::splat(depth_c)).abs();
        let h_sign: i32 = if he.x < he.y {
            -1
        } else {
            1
        };
        let v_sign: i32 = if ve.x < ve.y {
            -1
        } else {
            1
        };
        let s = self.size();
        let fs = Vec2::new(s[0] as f32, s[1] as f32);
        let px = Vec2::new(pixel[0] as f32, pixel[1] as f32);
        // The chosen neighbours' view positions; the cross of their differences is the normal.
        let hv = h.to_array()[(1 + h_sign) as usize];
        let vv = v.to_array()[(1 + v_sign) as usize];
        let view_h = self
            .view_pos(((px + Vec2::new(h_sign as f32, 0.0) + Vec2::splat(0.5)) / fs).extend(hv));
        let view_v = self
            .view_pos(((px + Vec2::new(0.0, v_sign as f32) + Vec2::splat(0.5)) / fs).extend(vv));
        let h_der = h_sign as f32 * (view_h - view_c);
        let v_der = v_sign as f32 * (view_v - view_c);
        v_der.cross(h_der)
    }

    /// The trace of SSR pixel `pixel` (screen_space_reflection.glsl main).
    pub fn trace(&self, pixel: [i32; 2]) -> Trace {
        let s = self.size();
        let fs = Vec2::new(s[0] as f32, s[1] as f32);
        let mut screen_pos = ((Vec2::new(pixel[0] as f32, pixel[1] as f32) + Vec2::splat(0.5))
            / fs)
            .extend(self.hiz(pixel, 0));
        let mut out = Trace::untraced(screen_pos);
        if screen_pos.z == 0.0 {
            // Depth 0 is the sky, which reflects nothing.
            return out;
        }
        let mut pos = self.view_pos(screen_pos);
        out.start = pos;
        let nr = self.normal_roughness.load(pixel);
        let normal = (nr.truncate() * 2.0 - Vec3::ONE).normalize();
        let roughness = decode_roughness(nr.w);
        out.normal = normal;
        out.roughness = roughness;
        if roughness >= 0.7 {
            // Rough surfaces are left to other reflections: the trace stops here.
            return out;
        }
        let geom_normal = self.geometric_normal(pixel, screen_pos.z, pos).normalize();
        out.geom_normal = geom_normal;
        // Push the start off the surface along its geometric normal, more where the normal map bends away from it.
        pos += geom_normal * (1.0 - normal.dot(geom_normal).clamp(0.0, 1.0).powf(8.0));
        out.biased = pos;
        screen_pos = self.screen_pos(pos);
        out.screen_pos = screen_pos;
        let view_dir = pos.normalize();
        let reflect = |i: Vec3, n: Vec3| i - 2.0 * n.dot(i) * n;
        let mut ray_dir = reflect(view_dir, normal).normalize();
        if ray_dir.dot(geom_normal) < 0.0 {
            // A reflection that points into the surface is mirrored back out by its geometric normal.
            ray_dir = reflect(ray_dir, geom_normal).normalize();
            out.bounced = true;
        }
        out.ray_dir = ray_dir;
        let mut end_pos = pos + ray_dir;
        if end_pos.z > 0.0 {
            // An end behind the camera is pulled back along the ray to just in front of it.
            end_pos -= ray_dir / ray_dir.z * (end_pos.z + 0.00001);
        }
        let screen_end = self.screen_pos(end_pos);
        let mut ray = screen_end - screen_pos;
        // The screen-space ray, scaled so a step of t moves one unit of depth.
        ray /= ray.z.abs();
        out.screen_ray = ray;
        let facing = ray.z >= 0.0;
        // The ray's t where it leaves the screen: the nearer of the two far edges.
        let t0 = (Vec2::ZERO - screen_pos.truncate()) / ray.truncate();
        let t1 = (Vec2::ONE - screen_pos.truncate()) / ray.truncate();
        let t2 = t0.max(t1);
        let t_max = t2.x.min(t2.y);
        out.t_max = t_max;
        // Which way the ray crosses cells on each axis.
        let cell_step = Vec2::new(
            if ray.x < 0.0 {
                -1.0
            } else {
                1.0
            },
            if ray.y < 0.0 {
                -1.0
            } else {
                1.0
            },
        );
        let (level, t) = self.march(&mut out, (facing, t_max), cell_step);
        out.exit = if level < 0 {
            Exit::Hit
        } else if t >= t_max {
            Exit::Screen
        } else {
            Exit::Steps
        };
        out.t = t;
        let cur = screen_pos + ray * t;
        out.hit_screen = cur;
        let r = self.reprojection * Vec4::new(cur.x * 2.0 - 1.0, cur.y * 2.0 - 1.0, cur.z, 1.0);
        let uv = Vec2::new(r.x, r.y) / r.w * 0.5 + Vec2::splat(0.5);
        out.reprojected_uv = uv;
        self.weigh(&mut out, t_max);
        let validity = out.validity;
        if validity > 0.0 {
            // The last frame's colour at the reprojected hit, weighed by validity, then compressed by its luminance.
            let c = self.last_frame.sample(uv).truncate().extend(1.0) * validity;
            let l = c.truncate().dot(Vec3::new(0.2126, 0.7152, 0.0722));
            out.color = (c.truncate() / (1.0 + l)).extend(c.w);
        }
        let (mips, ray_len) = (self.hiz.len() as i32, out.ray_len);
        let mut mip = 0.0;
        if roughness > 0.001 {
            // A rough surface reads a blurrier level, by the cone its roughness spreads over the ray's length.
            mip = blur_level(roughness, ray_len, fs.x.max(fs.y), mips);
        }
        out.mip_level = mip * (1.25 - ray_len).clamp(0.0, 1.0).powf(0.2);
        out
    }

    /// The Hi-Z march (screen_space_reflection.glsl's loop): from `t`, a cell at a time on the level, a miss climbing
    /// a level and a hit descending, until it hits, runs out of steps, or leaves the screen; each iteration kept. The
    /// level it ends on and its t.
    fn march(&self, out: &mut Trace, (facing, t_max): (bool, f32), cell_step: Vec2) -> (i32, f32) {
        let s = self.size();
        let fs = Vec2::new(s[0] as f32, s[1] as f32);
        let (screen_pos, ray) = (out.screen_pos, out.screen_ray);
        let mips = self.hiz.len() as i32;
        let mut level = 0i32;
        let mut iteration = self.num_steps;
        let mut t = {
            // Start at the edge of the starting cell, so the march never tests the pixel against itself.
            let cell = (screen_pos.truncate() * fs).floor();
            let next = cell + cell_step.clamp(Vec2::ZERO, Vec2::ONE);
            let next_pos = next / fs + cell_step * 0.000001;
            let pos_t = (next_pos - screen_pos.truncate()) / ray.truncate();
            pos_t.x.min(pos_t.y)
        };
        while level >= 0 && iteration > 0 && t < t_max {
            // The cell the ray is in at this level, its depth, and the t where the ray reaches that depth.
            let cur = screen_pos + ray * t;
            let count = Vec2::new((s[0] >> level).max(1) as f32, (s[1] >> level).max(1) as f32);
            let cell = (cur.truncate() * count).floor();
            let cell_depth = self.hiz([cell.x as i32, cell.y as i32], level);
            let depth_t = (cell_depth - screen_pos.z) * ray.z;
            // The t where the ray leaves the cell, nudged across its edge.
            let next = cell + cell_step.clamp(Vec2::ZERO, Vec2::ONE);
            let next_pos = next / count + cell_step * 0.000001;
            let pos_t = (next_pos - screen_pos.truncate()) / ray.truncate();
            let edge_t = pos_t.x.min(pos_t.y);
            let mut hit = if facing {
                // Moving toward the camera (reversed Z), the ray hits while still behind the cell's nearest depth.
                t <= depth_t
            } else {
                // Moving away, it hits when it reaches the cell's depth before it leaves the cell.
                depth_t <= edge_t
            };
            let mut offset = if hit {
                // A hit descends a level to look closer; a miss climbs one to skip further.
                -1
            } else {
                1
            };
            let mut held = false;
            if level == 0 {
                // At level 0, a hit beyond the depth tolerance behind the surface is no hit: the ray passed behind it.
                let z0 = self.linearize(cell_depth);
                let z1 = self.linearize(cur.z);
                if z0 - z1 > self.depth_tolerance {
                    hit = false;
                    offset = 0;
                    held = true;
                }
            }
            let before = t;
            if hit {
                if !facing {
                    // A ray moving away advances to the depth it hit.
                    t = t.max(depth_t);
                }
            } else {
                // A miss advances to the cell's edge.
                t = edge_t;
            }
            let next_level = (level + offset).min(mips - 1);
            out.steps.push(Step {
                level,
                t: before,
                cell: [cell.x as i32, cell.y as i32],
                cell_depth,
                depth_t,
                edge_t,
                hit,
                held,
                next_level,
                next_t: t,
            });
            level = next_level;
            iteration -= 1;
        }
        (level, t)
    }

    /// The hit weighed (screen_space_reflection.glsl's validity): the screen's edge, a back face, the depth's
    /// confidence, the margin, and the fade over the ray's screen length; with it, the reflected colour.
    fn weigh(&self, out: &mut Trace, t_max: f32) {
        let s = self.size();
        let fs = Vec2::new(s[0] as f32, s[1] as f32);
        let (cur, uv, ray, t, ray_dir) = (
            out.hit_screen,
            out.reprojected_uv,
            out.screen_ray,
            out.t,
            out.ray_dir,
        );
        let mut validity = 1.0f32;
        let cur_pixel = [(cur.x * fs.x) as i32, (cur.y * fs.y) as i32];
        let hit_depth = self.hiz(cur_pixel, 0);
        let edge = if t >= t_max || hit_depth == 0.0 {
            // A ray that left the screen, or landed on the sky, reflects nothing.
            0.0
        } else {
            1.0
        };
        validity *= edge;
        let mut back = 1.0;
        if (ray.truncate() * t)
            .abs()
            .cmplt(Vec2::splat(2.0) / fs)
            .all()
        {
            // A hit within two pixels of the start that faces away from the ray is the surface's own back face.
            let hn = self.normal_roughness.load(cur_pixel).truncate() * 2.0 - Vec3::ONE;
            if ray_dir.dot(hn) >= 0.0 {
                back = 0.0;
            }
        }
        validity *= back;
        let cur_pos = self.view_pos(cur);
        let hit_pos = self.view_pos(cur.truncate().extend(hit_depth));
        let delta = (cur_pos - hit_pos).length();
        let confidence = 1.0 - smoothstep(0.0, self.depth_tolerance, delta);
        let conf2 = (confidence * confidence).clamp(0.0, 1.0);
        validity *= conf2;
        // Fade near the screen's border: the margin grows with the distance to the nearest two edges.
        let rp = uv * fs;
        let margin = (fs.x + fs.y) * 0.05;
        let grad = Vec2::new(
            if rp.x < fs.x * 0.5 {
                rp.x
            } else {
                fs.x - rp.x
            },
            if rp.y < fs.y * 0.5 {
                rp.y
            } else {
                fs.y - rp.y
            },
        );
        let margin_blend = smoothstep(0.0, margin * margin, grad.x * grad.y);
        let ray_len = (ray.truncate() * t).length();
        out.ray_len = ray_len;
        let fade = fade(ray_len, self.curve_fade_in, self.distance_fade);
        validity *= fade * margin_blend;
        out.terms = [edge, back, conf2, margin_blend, fade];
        out.validity = validity;
    }
}

impl Trace {
    /// A trace that has not been traced yet, from `screen_pos`: nothing but its start.
    fn untraced(screen_pos: Vec3) -> Trace {
        Trace {
            exit: Exit::NotTraced,
            roughness: 0.0,
            start: Vec3::ZERO,
            biased: Vec3::ZERO,
            normal: Vec3::ZERO,
            geom_normal: Vec3::ZERO,
            ray_dir: Vec3::ZERO,
            bounced: false,
            screen_pos,
            screen_ray: Vec3::ZERO,
            t_max: 0.0,
            steps: Vec::new(),
            t: 0.0,
            hit_screen: Vec3::ZERO,
            reprojected_uv: Vec2::ZERO,
            validity: 0.0,
            terms: [0.0; 5],
            ray_len: 0.0,
            color: Vec4::ZERO,
            mip_level: 0.0,
        }
    }
}

/// The fade over the ray's screen length (screen_space_reflection.glsl:270-282).
pub fn fade(ray_len: f32, fade_in: f32, fade_out: f32) -> f32 {
    let a = if fade_in == 0.0 {
        // No fade-in when its curve is 0; otherwise the ray's length raised to the curve.
        1.0
    } else {
        ray_len.clamp(0.0, 1.0).powf(fade_in)
    };
    let b = if fade_out == 0.0 {
        // Likewise the fade-out over the rest of the length.
        1.0
    } else {
        (1.0 - ray_len).clamp(0.0, 1.0).powf(fade_out)
    };
    let f = a * b;
    if f > 0.999 {
        // Almost no fade is no fade.
        1.0
    } else {
        f
    }
}

/// The blur level for a roughness and a ray length (screen_space_reflection.glsl:294-318):
/// the cone's half-angle, the circle inside it where it ends, its radius in pixels.
pub fn blur_level(roughness: f32, ray_len: f32, largest: f32, mips: i32) -> f32 {
    let cone_angle = roughness.min(0.999) * std::f32::consts::PI * 0.5;
    let a = 2.0 * cone_angle.tan() * ray_len;
    let h = ray_len;
    // The radius of the circle inscribed in the isosceles triangle of base a and height h.
    let radius = (a * ((a * a + 4.0 * h * h).sqrt() - a)) / (4.0 * h);
    (radius * largest / 16.0 + 1.0)
        .log2()
        .clamp(0.0, (mips - 1) as f32)
}
