// Study-room raster pass: direct light from the emissive panel, analytic
// shadows, and a one-bounce analytic reflection on the floor. Also the
// composite pass and the viewer's swapchain blit.

struct SceneVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) objectIndex: u32,
}

struct SceneVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) worldPosition: vec3<f32>,
    @location(1) worldNormal: vec3<f32>,
    @location(2) @interpolate(flat) objectIndex: u32,
}

@vertex
fn sceneVS(input: SceneVertex) -> SceneVaryings {
    let o = gObjects[input.objectIndex];
    var output: SceneVaryings;
    output.worldPosition = transformPoint(o.localToWorld, input.position);
    output.worldNormal = normalToWorld(o, input.normal);
    output.objectIndex = input.objectIndex;
    output.clip = gFrame.camera.worldToClip * vec4<f32>(output.worldPosition, 1.0);
    // Objects in the hidden mask are skipped by the raster pass only; they
    // still cast shadows and appear in reflections.
    if (input.objectIndex < 32u && (gFrame.counts.w & (1u << input.objectIndex)) != 0u) {
        output.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    return output;
}

struct SceneOutput {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) coverage: u32,
}

@fragment
fn sceneFS(input: SceneVaryings) -> SceneOutput {
    var n = normalize(input.worldNormal);
    let eye = gFrame.camera.position;
    let v = normalize(eye - input.worldPosition);
    if (dot(n, v) < 0.0) {
        if (gObjects[input.objectIndex].shapeKind == SHAPE_TERRAIN) {
            // The terrain is seen from above: past a ridge its smooth normal
            // leans away from the eye, so turn it back to grazing instead.
            n = normalize(n - v * (dot(n, v) - 1e-3));
        } else {
            n = -n;
        }
    }
    var color: vec3<f32>;
    if (input.objectIndex == gFrame.counts.y) {
        color = shadeFloor(input.worldPosition, n, eye, input.objectIndex);
    } else {
        color = shadeSurface(input.worldPosition, n, input.objectIndex);
    }
    // An object fading out of the frame covers that share of the pixel's
    // MSAA samples, dithered from pixel to pixel.
    var coverage = 0xffffffffu;
    let presence = objectPresence(input.objectIndex);
    if (presence < 1.0) {
        coverage = (1u << u32(clamp(floor(presence * 4.0 + interleavedGradientNoise(input.clip.xy)), 0.0, 4.0))) - 1u;
    }
    return SceneOutput(vec4<f32>(color, 1.0), coverage);
}

// ---------------------------------------------------------------------------
// Composite: tonemap the resolved HDR scene into the viewport rectangle of
// the RGBA8 frame and lay the resolved overlay layer (premultiplied sRGB)
// on top. The frame target is UNORM, so this pass writes sRGB-encoded values
// explicitly and the egui pass can draw in gamma space on top.

@group(1) @binding(0) var gSceneColor: texture_2d<f32>;
@group(1) @binding(1) var gOverlayColor: texture_2d<f32>;

struct FullscreenVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn fullscreenVS(@builtin(vertex_index) vertexId: u32) -> FullscreenVaryings {
    let uv = vec2<f32>(f32((vertexId << 1u) & 2u), f32(vertexId & 2u));
    var output: FullscreenVaryings;
    output.clip = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    output.uv = uv;
    return output;
}

@fragment
fn compositeFS(input: FullscreenVaryings) -> @location(0) vec4<f32> {
    let hdr = textureSampleLevel(gSceneColor, gLinearSampler, input.uv, 0.0).rgb;
    let overlay = textureSampleLevel(gOverlayColor, gLinearSampler, input.uv, 0.0);
    let scene = linearToSrgb(tonemap(hdr));
    return vec4<f32>(overlay.rgb + scene * (1.0 - overlay.a), 1.0);
}

// The backdrop: the sky along each pixel's view ray, drawn before the
// objects without depth, so they cover it.
@fragment
fn skyFS(input: FullscreenVaryings) -> @location(0) vec4<f32> {
    return vec4<f32>(skyRadiance(viewRay(input.uv)), 1.0);
}

// Viewer blit from the RGBA8 frame (already sRGB-encoded) to the swapchain.
@fragment
fn blitRawFS(input: FullscreenVaryings) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSampleLevel(gSceneColor, gLinearSampler, input.uv, 0.0).rgb, 1.0);
}

// The same blit for an sRGB swapchain, which re-encodes on write.
@fragment
fn blitDecodeFS(input: FullscreenVaryings) -> @location(0) vec4<f32> {
    return vec4<f32>(srgbToLinear(textureSampleLevel(gSceneColor, gLinearSampler, input.uv, 0.0).rgb), 1.0);
}
