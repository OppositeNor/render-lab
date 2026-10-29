// The homepage's frosted glass (src/lab/frost.rs): the rendered scene is
// halved twice, blurred with a separable Gaussian at quarter resolution, then
// tinted, grained, and vignetted behind the page. A standalone module: it
// shares no records with the scene.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linearSampler: sampler;

struct FullscreenOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn fullscreenVS(@builtin(vertex_index) index: u32) -> FullscreenOut {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullscreenOut;
    out.position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

fn texel() -> vec2<f32> {
    return 1.0 / vec2<f32>(textureDimensions(source));
}

// Half resolution: four bilinear taps cover a 4 x 4 source footprint.
@fragment
fn downsampleFS(in: FullscreenOut) -> @location(0) vec4<f32> {
    let t = texel();
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let offset = vec2<f32>(select(-1.0, 1.0, (i & 1) == 1), select(-1.0, 1.0, i >= 2));
        sum += textureSample(source, linearSampler, in.uv + offset * t);
    }
    return sum * 0.25;
}

// Nine-tap Gaussian (sigma about 2 texels) in five bilinear taps.
fn gaussian(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
    let step = direction * texel();
    var sum = textureSample(source, linearSampler, uv) * 0.2270270270;
    sum += (textureSample(source, linearSampler, uv + step * 1.3846153846) + textureSample(source, linearSampler, uv - step * 1.3846153846)) * 0.3162162162;
    sum += (textureSample(source, linearSampler, uv + step * 3.2307692308) + textureSample(source, linearSampler, uv - step * 3.2307692308)) * 0.0702702703;
    return sum;
}

@fragment
fn blurHorizontalFS(in: FullscreenOut) -> @location(0) vec4<f32> {
    return gaussian(in.uv, vec2<f32>(1.0, 0.0));
}

@fragment
fn blurVerticalFS(in: FullscreenOut) -> @location(0) vec4<f32> {
    return gaussian(in.uv, vec2<f32>(0.0, 1.0));
}

fn grain(pixel: vec2<f32>) -> f32 {
    let p = vec2<u32>(pixel);
    var h = p.x * 1973u + p.y * 9277u + 26699u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h & 0xffffu) / 65535.0 - 0.5;
}

// The blurred scene behind the page: darkened toward the palette's
// background, with a fine static grain (the frost) and a soft vignette.
// Values stay sRGB-encoded, as in the lab's frame target.
@fragment
fn frostFS(in: FullscreenOut) -> @location(0) vec4<f32> {
    let blurred = textureSample(source, linearSampler, in.uv).rgb;
    let tint = vec3<f32>(0.027, 0.055, 0.086);
    var color = mix(blurred, tint, 0.38);
    let centered = in.uv - 0.5;
    color *= 1.0 - 0.55 * dot(centered, centered);
    color += grain(in.position.xy) * 0.035;
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
