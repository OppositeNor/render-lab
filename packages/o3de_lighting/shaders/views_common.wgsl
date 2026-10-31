// Helpers of the image kinds (views*.wgsl), each composed after this file
// and the scene shading, and each declaring its own output `gViewOut`
// (group 3, binding 0), which these read.

/// The viewport pixel a texel of the view shows.
fn viewPixel(id: vec2<u32>) -> vec2<u32> {
    let out = vec2<f32>(textureDimensions(gViewOut));
    let full = vec2<f32>(textureDimensions(tDepth));
    return min(vec2<u32>((vec2<f32>(id) + 0.5) / out * full), textureDimensions(tDepth) - 1u);
}

/// The picked point's pixel this frame, 0..1 of the viewport with y down,
/// or a negative x when nothing is picked or the point is off screen.
fn pickedUv() -> vec2<f32> {
    if (gShading.pick.w < 0.5) {
        return vec2<f32>(-1.0);
    }
    let clip = gFrame.camera.worldToClip * vec4<f32>(gShading.pick.xyz, 1.0);
    if (clip.w <= 0.0) {
        return vec2<f32>(-1.0);
    }
    let ndc = clip.xy / clip.w;
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
}

/// The picked pixel's ring over the view's colour.
fn withPick(id: vec2<u32>, color: vec3<f32>) -> vec3<f32> {
    let pick = pickedUv();
    if (pick.x < 0.0 || pick.y < 0.0) {
        return color;
    }
    let out = vec2<f32>(textureDimensions(gViewOut));
    let d = length(vec2<f32>(id) + 0.5 - pick * out);
    if (d > 5.0 && d < 7.5) {
        return vec3<f32>(1.0, 0.85, 0.2);
    }
    if (d >= 7.5 && d < 8.5) {
        return vec3<f32>(0.0);
    }
    return color;
}

/// The mean of a multisampled target's samples at a pixel.
fn meanSamples(t: texture_multisampled_2d<f32>, pixel: vec2<u32>) -> vec4<f32> {
    var sum = vec4<f32>(0.0);
    let n = textureNumSamples(t);
    for (var i = 0u; i < n; i++) {
        sum += textureLoad(t, pixel, i);
    }
    return sum / f32(n);
}

/// Linear view depth, metres, of a reversed-depth texel (0: nothing drawn).
fn viewDepthAt(pixel: vec2<u32>) -> f32 {
    let d = textureLoad(tDepth, pixel, 0u);
    if (d <= 0.0) {
        return -1.0;
    }
    // Clip z = A z_view + B over w = -z_view: z_view = -B / (d + A).
    let m = gFrame.camera.viewToClip;
    return m[3][2] / (d + m[2][2]);
}

