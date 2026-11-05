/// The frame graph's inspector (lab/src/graph/, lab/src/renderer/): a
/// resource of the frame, as it stood right after the node that wrote it,
/// drawn into a display texture by a visualizer chosen for its format. One
/// entry point per kind of resource; each binds its source at its own
/// binding of group 0, beside the parameters (0) and the output (1).
struct InspectParams {
    /// The visualizer (`graph::Show`, in its order: color, unorm, normal,
    /// depth, ramp, signed, count, float, hash).
    mode: u32,
    /// The channel shown: 0 all, 1 r, 2 g, 3 b, 4 a.
    channel: u32,
    /// A normal's encoding: 0 signed xyz, 1 unsigned xyz, 2 octahedral xy;
    /// 3, an integer image's word as RGB9E5 (shown as colour).
    encoding: u32,
    /// Multisampled: 0 sample 0, 1 the samples' average.
    average: u32,
    /// The range a ramp maps to 0..1 (depth: a logarithmic one).
    lo: f32,
    hi: f32,
    /// Colour: the exposure in stops.
    exposure: f32,
    /// A volume's slice, 0..1 along `axis`.
    slice: f32,
    out_size: vec2<u32>,
    src_size: vec2<u32>,
    /// A buffer: its pixels per tile side (0: no screen shape), u32 per
    /// tile (or element), the word shown, and tiles per row.
    tile: u32,
    words: u32,
    word: u32,
    tiles_x: u32,
    /// A volume's mip, or a depth array's layer.
    level: u32,
    /// A volume's slicing axis: 0 x, 1 y, 2 z.
    axis: u32,
    samples: u32,
    /// An integer image's bit field: its first bit (bits 0-7) and width (8-15; 0, the whole value).
    bits: u32,
}

@group(0) @binding(0) var<uniform> gInspect: InspectParams;
@group(0) @binding(1) var gInspectOut: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var gInspectTexture: texture_2d<f32>;
@group(0) @binding(3) var gInspectTextureMs: texture_multisampled_2d<f32>;
@group(0) @binding(4) var gInspectDepthMs: texture_depth_multisampled_2d;
@group(0) @binding(5) var gInspectDepthArray: texture_depth_2d_array;
@group(0) @binding(6) var<storage, read> gInspectBuffer: array<u32>;
@group(0) @binding(7) var gInspectVolume: texture_3d<f32>;
@group(0) @binding(8) var gInspectVolumeUint: texture_3d<u32>;
@group(0) @binding(9) var gInspectVolumeSint: texture_3d<i32>;
@group(0) @binding(10) var gInspectArray: texture_2d_array<f32>;
@group(0) @binding(11) var gInspectArrayUint: texture_2d_array<u32>;
@group(0) @binding(12) var gInspectArraySint: texture_2d_array<i32>;

fn inspectSrgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3(0.0), vec3(1.0));
    return select(1.055 * pow(x, vec3(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3(0.0031308));
}

/// A perceptual ramp (dark blue, teal, yellow) of t in 0..1, display colours.
fn inspectRamp(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0);
    let a = vec3(0.07, 0.04, 0.33);
    let b = vec3(0.13, 0.57, 0.55);
    let c = vec3(0.99, 0.91, 0.14);
    return select(mix(b, c, (x - 0.5) * 2.0), mix(a, b, x * 2.0), x < 0.5);
}

fn inspectChannel(v: vec4<f32>) -> f32 {
    switch gInspect.channel {
        case 2u: { return v.g; }
        case 3u: { return v.b; }
        case 4u: { return v.a; }
        default: { return v.r; }
    }
}

fn inspectHash(x: u32) -> vec3<f32> {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    h = (h >> 22u) ^ h;
    return vec3(f32(h & 255u), f32((h >> 8u) & 255u), f32((h >> 16u) & 255u)) / 255.0 * 0.8 + 0.2;
}

/// A value (`v`; `raw`, a buffer's word) in display colours.
fn inspectShow(v: vec4<f32>, raw: u32) -> vec4<f32> {
    let range = max(gInspect.hi - gInspect.lo, 1e-20);
    switch gInspect.mode {
        case 0u: {
            let c = select(v.rgb, vec3(inspectChannel(v)), gInspect.channel != 0u);
            return vec4(inspectSrgb(c * exp2(gInspect.exposure)), 1.0);
        }
        case 1u: {
            return vec4(select(v.rgb, vec3(inspectChannel(v)), gInspect.channel != 0u), 1.0);
        }
        case 2u: {
            var n = v.xyz;
            if gInspect.encoding == 1u {
                n = n * 2.0 - 1.0;
            } else if gInspect.encoding == 2u {
                let e = v.xy * 2.0 - 1.0;
                n = vec3(e, 1.0 - abs(e.x) - abs(e.y));
                let t = max(-n.z, 0.0);
                n = vec3(n.x + select(t, -t, n.x >= 0.0), n.y + select(t, -t, n.y >= 0.0), n.z);
            }
            let len = length(n);
            return vec4(select(vec3(0.0), n / len * 0.5 + 0.5, len > 1e-6), 1.0);
        }
        case 3u: {
            // Reversed depth: 1 near, 0 at infinity (the background stays black).
            let d = v.r;
            if d <= 0.0 {
                return vec4(0.0, 0.0, 0.0, 1.0);
            }
            let t = log2(max(d, gInspect.lo) / gInspect.lo) / log2(max(gInspect.hi / gInspect.lo, 1.0001));
            return vec4(vec3(clamp(t, 0.0, 1.0)), 1.0);
        }
        case 4u: {
            return vec4(inspectRamp((inspectChannel(v) - gInspect.lo) / range), 1.0);
        }
        case 5u: {
            let m = max(max(abs(gInspect.lo), abs(gInspect.hi)), 1e-20);
            let t = clamp(inspectChannel(v) / m, -1.0, 1.0);
            let c = select(mix(vec3(0.95), vec3(0.85, 0.2, 0.15), t), mix(vec3(0.95), vec3(0.15, 0.35, 0.85), -t), t < 0.0);
            return vec4(c, 1.0);
        }
        case 6u: {
            if raw == 0u {
                return vec4(0.0, 0.0, 0.0, 1.0);
            }
            return vec4(inspectRamp((f32(raw) - gInspect.lo) / range), 1.0);
        }
        case 7u: {
            return vec4(inspectRamp((bitcast<f32>(raw) - gInspect.lo) / range), 1.0);
        }
        default: {
            let id = select(u32(round(inspectChannel(v) * 255.0)), raw, gInspect.words > 0u);
            return vec4(select(inspectHash(id), vec3(0.0), id == 0u), 1.0);
        }
    }
}

/// The source texel under output texel `id` (nearest).
fn inspectSource(id: vec2<u32>) -> vec2<u32> {
    let size = max(gInspect.src_size, vec2(1u));
    return min(id * size / max(gInspect.out_size, vec2(1u)), size - 1u);
}

fn inspectInside(id: vec2<u32>) -> bool {
    return all(id < gInspect.out_size);
}

@compute @workgroup_size(8, 8)
fn inspectTextureCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let v = textureLoad(gInspectTexture, inspectSource(gid.xy), 0);
    textureStore(gInspectOut, gid.xy, inspectShow(v, 0u));
}

@compute @workgroup_size(8, 8)
fn inspectTextureMsCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let at = inspectSource(gid.xy);
    var v = textureLoad(gInspectTextureMs, at, 0);
    if gInspect.average != 0u {
        for (var s = 1u; s < gInspect.samples; s++) {
            v += textureLoad(gInspectTextureMs, at, s);
        }
        v /= f32(max(gInspect.samples, 1u));
    }
    textureStore(gInspectOut, gid.xy, inspectShow(v, 0u));
}

@compute @workgroup_size(8, 8)
fn inspectDepthMsCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let at = inspectSource(gid.xy);
    var d = textureLoad(gInspectDepthMs, at, 0);
    if gInspect.average != 0u {
        for (var s = 1u; s < gInspect.samples; s++) {
            d += textureLoad(gInspectDepthMs, at, s);
        }
        d /= f32(max(gInspect.samples, 1u));
    }
    textureStore(gInspectOut, gid.xy, inspectShow(vec4(d, 0.0, 0.0, 1.0), 0u));
}

@compute @workgroup_size(8, 8)
fn inspectDepthArrayCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let d = textureLoad(gInspectDepthArray, inspectSource(gid.xy), gInspect.level, 0);
    textureStore(gInspectOut, gid.xy, inspectShow(vec4(d, 0.0, 0.0, 1.0), 0u));
}

@compute @workgroup_size(8, 8)
fn inspectBufferCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    // The pixel's tile, and the word shown of its words.
    // (Tile 0: a buffer of no screen shape, its elements row by row.)
    let pixel = inspectSource(gid.xy);
    let tile = pixel / max(gInspect.tile, 1u);
    let element = select(tile.y * gInspect.tiles_x + tile.x, gid.y * gInspect.out_size.x + gid.x, gInspect.tile == 0u);
    let index = element * max(gInspect.words, 1u) + min(gInspect.word, max(gInspect.words, 1u) - 1u);
    let raw = select(0u, gInspectBuffer[index], index < arrayLength(&gInspectBuffer));
    let v = bitcast<f32>(raw);
    textureStore(gInspectOut, gid.xy, inspectShow(vec4(v, v, v, 1.0), raw));
}

@compute @workgroup_size(8, 8)
fn inspectVolumeCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let size = textureDimensions(gInspectVolume, gInspect.level);
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(gInspect.out_size);
    let depth = clamp(gInspect.slice, 0.0, 1.0);
    var p: vec3<f32>;
    switch gInspect.axis {
        case 0u: { p = vec3(depth, 1.0 - uv.y, uv.x); }
        case 2u: { p = vec3(uv.x, 1.0 - uv.y, depth); }
        default: { p = vec3(uv.x, depth, uv.y); }
    }
    let at = min(vec3<u32>(p * vec3<f32>(size)), size - 1u);
    let v = textureLoad(gInspectVolume, at, gInspect.level);
    textureStore(gInspectOut, gid.xy, inspectShow(v, 0u));
}

/// A volume's texel under output texel `gid` (the slice along `axis`).
fn inspectVolumeAt(gid: vec2<u32>, size: vec3<u32>) -> vec3<u32> {
    let uv = (vec2<f32>(gid) + 0.5) / vec2<f32>(gInspect.out_size);
    let depth = clamp(gInspect.slice, 0.0, 1.0);
    var p: vec3<f32>;
    switch gInspect.axis {
        case 0u: { p = vec3(depth, 1.0 - uv.y, uv.x); }
        case 2u: { p = vec3(uv.x, 1.0 - uv.y, depth); }
        default: { p = vec3(uv.x, depth, uv.y); }
    }
    return min(vec3<u32>(p * vec3<f32>(size)), size - 1u);
}

/// The shared-exponent RGB of a word (E5B9G9R9).
fn inspectRgb9e5(w: u32) -> vec3<f32> {
    let scale = exp2(f32(w >> 27u) - 24.0);
    return vec3(f32(w & 511u), f32((w >> 9u) & 511u), f32((w >> 18u) & 511u)) * scale;
}

/// An integer texel shown: its channel, its bit field, and its word decoded as asked.
fn inspectInteger(raw4: vec4<u32>, signed: bool) -> vec4<f32> {
    var raw = raw4.r;
    switch gInspect.channel {
        case 2u: { raw = raw4.g; }
        case 3u: { raw = raw4.b; }
        case 4u: { raw = raw4.a; }
        default: {}
    }
    let shift = gInspect.bits & 255u;
    let width = (gInspect.bits >> 8u) & 255u;
    if width > 0u {
        raw = (raw >> shift) & select((1u << width) - 1u, 0xffffffffu, width >= 32u);
    }
    if gInspect.encoding == 3u {
        return inspectShow(vec4(inspectRgb9e5(raw), 1.0), raw);
    }
    let value = select(f32(raw), f32(bitcast<i32>(raw)), signed);
    return inspectShow(vec4(value), raw);
}

@compute @workgroup_size(8, 8)
fn inspectVolumeUintCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let at = inspectVolumeAt(gid.xy, textureDimensions(gInspectVolumeUint));
    textureStore(gInspectOut, gid.xy, inspectInteger(textureLoad(gInspectVolumeUint, at, 0), false));
}

@compute @workgroup_size(8, 8)
fn inspectVolumeSintCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let at = inspectVolumeAt(gid.xy, textureDimensions(gInspectVolumeSint));
    textureStore(gInspectOut, gid.xy, inspectInteger(bitcast<vec4<u32>>(textureLoad(gInspectVolumeSint, at, 0)), true));
}

@compute @workgroup_size(8, 8)
fn inspectArrayCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    let v = textureLoad(gInspectArray, inspectSource(gid.xy), gInspect.level, 0);
    textureStore(gInspectOut, gid.xy, inspectShow(v, 0u));
}

@compute @workgroup_size(8, 8)
fn inspectArrayUintCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    textureStore(gInspectOut, gid.xy, inspectInteger(textureLoad(gInspectArrayUint, inspectSource(gid.xy), gInspect.level, 0), false));
}

@compute @workgroup_size(8, 8)
fn inspectArraySintCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    if !inspectInside(gid.xy) {
        return;
    }
    textureStore(gInspectOut, gid.xy, inspectInteger(bitcast<vec4<u32>>(textureLoad(gInspectArraySint, inspectSource(gid.xy), gInspect.level, 0)), true));
}
