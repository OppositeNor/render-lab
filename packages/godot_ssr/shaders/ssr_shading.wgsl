// Godot's Forward+ opaque pass around screen-space reflections, ported to WGSL
// for the lab's scene pass: the depth pre-pass's normal-roughness buffer
// (scene_forward_clustered.glsl:3019-3028, encode24 :1183-1203), the screen-space
// roughness limiter (:1657-1672), and the indirect specular the SSR result is
// blended into (:1690-1733, 2139-2184, 2196-2228, 2246-2260). Composed after the
// scene module (lab/src/shaders.rs), so SceneVaryings, SceneOutput, gFrame,
// gObjects, and common.wgsl are in scope; ssr_passes.wgsl holds the SSR passes.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md). src/godot/ssr/trace.rs mirrors the passes on the CPU, and
// `selftest` compares the two.

// gShading: the investigation's values (the `///` tags; the lab packs them from
// the shot, lab/src/shader_data.rs). The SSR passes read the same keys through
// their plan's parameters (gpu.rs), as Godot pushes the Environment's values.
struct SsrShading {
    /// Environment.ssr_enabled: whether the opaque pass blends the reflections in.
    /// {@ key=ssrEnabled default=1 range=0,1 label=ssr text=controls-ssr_enabled engine=Environment.ssr_enabled help=help-ssrEnabled}
    ssrEnabled: f32,
    /// Environment.ssr_max_steps: the trace's iterations.
    /// {@ key=ssrMaxSteps default=64 range=32,512 label=ssr text=controls-ssr_max_steps engine=Environment.ssr_max_steps help=help-ssrMaxSteps}
    maxSteps: f32,
    /// Environment.ssr_fade_in: the exponent of the fade over the ray's screen length.
    /// {@ key=ssrFadeIn default=0.15 range=0,1 label=ssr text=controls-ssr_fade_in engine=Environment.ssr_fade_in help=help-ssrFadeIn}
    fadeIn: f32,
    /// Environment.ssr_fade_out: the exponent of the fade toward the far end.
    /// {@ key=ssrFadeOut default=2 range=0,8 label=ssr text=controls-ssr_fade_out engine=Environment.ssr_fade_out help=help-ssrFadeOut}
    fadeOut: f32,
    /// Environment.ssr_depth_tolerance, in view-space units.
    /// {@ key=ssrDepthTolerance default=0.5 range=0.01,4 label=ssr text=controls-ssr_depth_tolerance engine=Environment.ssr_depth_tolerance help=help-ssrDepthTolerance}
    depthTolerance: f32,
    /// The floor material's metallic (BaseMaterial3D.metallic; the lab's other objects are dielectric).
    /// {@ key=floorMetallic default=0 range=0,1 label=surface text=controls-floor_metallic engine=BaseMaterial3D.metallic help=help-floorMetallic}
    floorMetallic: f32,
    /// How much of the opaque pass's indirect specular is drawn: 0 none, 1 the sky's reflection after
    /// specular occlusion, 2 the SSR result alone, 3 SSR blended in (the frame).
    /// {@ key=ssrComposeStep default=3 range=0,3 label=compose text=controls-compose_step help=help-ssrComposeStep}
    composeStep: f32,
    /// Environment.reflected_light_source: 1 the sky (Background with a sky background, or Sky), 0 Disabled.
    /// {@ key=skyReflection default=1 range=0,1 label=surface text=controls-sky_reflection engine=Environment.reflected_light_source help=help-skyReflection}
    skyReflection: f32,
    /// rendering/anti_aliasing/screen_space_roughness_limiter/enabled (default on).
    /// {@ key=roughnessLimiter default=1 range=0,1 label=surface text=controls-roughness_limiter engine=ProjectSettings.rendering/anti_aliasing/screen_space_roughness_limiter/enabled help=help-roughnessLimiter}
    roughnessLimiter: f32,
}
/// {@ shader_data}
@group(1) @binding(0) var<uniform> gShading: SsrShading;

// The limiter's amount and limit (rendering_server.cpp:3746-3747 defaults).
const SSR_LIMITER_AMOUNT: f32 = 0.25;
const SSR_LIMITER_LIMIT: f32 = 0.18;
// MAX_ROUGHNESS_LOD: rendering/reflections/sky_reflections/roughness_layers (8) - 1
// (render_forward_clustered.cpp:5110, rendering_server.cpp:3696).
const SSR_MAX_ROUGHNESS_LOD: f32 = 7.0;

// The scene group (gpu.rs PROGRAM.scene_uses): the resolved reflections,
// R16G16B16A16_SFLOAT at the internal size, premultiplied by validity
// (RB_FINAL, ss_effects.cpp:1462-1463); and the best-fit normal table the
// pre-pass reads (best_fit_normal_texture, R8, 1024 x 1024).
@group(3) @binding(1) var gSsrFinal: texture_2d<f32>;
@group(3) @binding(2) var gSsrBestFit: texture_2d<f32>;

struct SsrPrepassOutput {
    @location(0) normalRoughness: vec4<f32>,
}

// The fragment's view-space normal, turned toward the eye (DO_SIDE_CHECK,
// scene_forward_clustered.glsl:1264-1266), as Godot's lighting uses it.
fn ssrViewNormal(input: SceneVaryings) -> vec3<f32> {
    let n = eyeSideNormal(input.worldNormal, input.worldPosition, gFrame.camera.position);
    return normalize((gFrame.camera.worldToView * vec4<f32>(n, 0.0)).xyz);
}

// The screen-space roughness limiter (Kaplanyan et al., "Improved Geometric
// Specular Antialiasing", 2019; scene_forward_clustered.glsl:1657-1672): the
// normal's change across the pixel widens the roughness.
fn ssrLimitRoughness(roughness: f32, normal: vec3<f32>) -> f32 {
    if (gShading.roughnessLimiter < 0.5) {
        return roughness;
    }
    let roughness2 = roughness * roughness;
    let dndu = dpdx(normal);
    let dndv = dpdy(normal);
    let variance = SSR_LIMITER_AMOUNT * (dot(dndu, dndu) + dot(dndv, dndv));
    let kernelRoughness2 = min(2.0 * variance, SSR_LIMITER_LIMIT);
    let filtered = min(1.0, roughness2 + kernelRoughness2);
    var r = sqrt(filtered);
    if (r < 0.00000001) {
        r = 0.0;
    }
    return r;
}

// encode24 (Kaplanyan, "CryENGINE 3: Reaching the Speed of Light", SIGGRAPH 2010;
// scene_forward_clustered.glsl:1183-1203): the normal scaled to touch the unit
// cube, then to the length whose 8-bit quantization errs least (the table).
fn ssrEncode24(v: vec3<f32>) -> vec3<f32> {
    let uns = abs(v);
    let maxAbs = max(uns.z, max(uns.x, uns.y));
    var tc = select(uns.xy, select(uns.xz, uns.yz, uns.y < maxAbs), uns.z < maxAbs);
    tc = tc / maxAbs;
    tc = select(tc.xy, tc.yx, tc.x < tc.y);
    tc.y = tc.y / tc.x;
    // texture(sampler2D(best_fit_normal_texture, SAMPLER_NEAREST_CLAMP), tc).r
    let size = vec2<f32>(textureDimensions(gSsrBestFit));
    let texel = clamp(vec2<i32>(floor(tc * size)), vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1));
    let scale = textureLoad(gSsrBestFit, texel, 0).r;
    return v / maxAbs * scale;
}

// The depth pre-pass with Godot's normal-roughness buffer (PASS_MODE_DEPTH_NORMAL_ROUGHNESS,
// render_forward_clustered.cpp:1925-1940; scene_forward_clustered.glsl:3019-3028): rgb the
// encoded view-space normal * 0.5 + 0.5, alpha the roughness * 127/255 (the lab's objects are
// static: Godot stores 1 minus that for a dynamic instance).
@fragment
fn ssrPrepassFS(input: SceneVaryings) -> SsrPrepassOutput {
    let n = ssrViewNormal(input);
    let roughness = ssrLimitRoughness(gObjects[input.objectIndex].albedoRoughness.w, n);
    return SsrPrepassOutput(vec4<f32>(ssrEncode24(n) * 0.5 + vec3<f32>(0.5), roughness * (127.0 / 255.0)));
}

// The sky's radiance along `dir` at a roughness LOD (the radiance octmap's
// roughness layers in Godot; the lab's environment's mips, box-filtered, blended
// between two levels).
fn ssrSkyRadiance(dir: vec3<f32>, lod: f32) -> vec3<f32> {
    let levels = f32(textureNumLevels(gEnvironment));
    let l = clamp(lod, 0.0, levels - 1.0);
    let l0 = floor(l);
    let l1 = min(l0 + 1.0, levels - 1.0);
    return gFrame.ambientColor.rgb * mix(environment(dir, i32(l0)), environment(dir, i32(l1)), l - l0);
}

// What Godot's opaque pass blends SSR into, and how (the steps a demonstration shows).
struct SsrSpecular {
    // The sky's reflection (horizon-faded), after specular occlusion.
    beforeSsr: vec3<f32>,
    // The SSR result at the pixel, after the 0.6-0.7 roughness fade (premultiplied).
    ssr: vec4<f32>,
    // `beforeSsr * (1 - ssr.a) + ssr.rgb`: the incoming specular radiance.
    blended: vec3<f32>,
    // The environment BRDF and energy compensation that scale it (per channel: a metal's f0 is its albedo).
    scale: vec3<f32>,
}

// The indirect specular of a pixel (scene_forward_clustered.glsl): the sky
// reflection (:1690-1733; ref_vec bent toward the normal by roughness squared,
// horizon squared), specular occlusion from the ambient light's luminance
// (:2174-2183; the lab's materials have no metallic), the SSR blend (:2196-2228),
// then the environment BRDF (:2253-2260; the lab's analytic fit of Karis 2014 for
// Godot's DFG table, f0 0.04, f90 1).
fn ssrIndirectSpecular(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, roughness: f32, metallic: f32, f0: vec3<f32>, ambient: vec3<f32>, pixel: vec2<u32>) -> SsrSpecular {
    var out: SsrSpecular;
    var refVec = reflect(-v, n);
    refVec = normalize(mix(refVec, n, roughness * roughness));
    let horizon = min(1.0 + dot(refVec, n), 1.0);
    // SCENE_DATA_FLAGS_USE_REFLECTION_CUBEMAP (render_scene_data_rd.cpp:200-201): the sky's reflection only from the sky.
    var spec = vec3<f32>(0.0);
    if (gShading.skyReflection > 0.5) {
        spec = ssrSkyRadiance(refVec, sqrt(roughness) * SSR_MAX_ROUGHNESS_LOD) * horizon * horizon;
    }
    var occlusion = (ambient.r * 0.3 + ambient.g * 0.59 + ambient.b * 0.11) * 2.0;
    occlusion = min(occlusion * 4.0, 1.0);
    // Smooth metals keep their reflection (scene_forward_clustered.glsl:2178-2181).
    let reflective = (1.0 - roughness) * metallic;
    occlusion = max(min(reflective * occlusion * 10.0, 1.0), occlusion);
    spec = spec * occlusion;
    out.beforeSsr = spec;
    out.ssr = vec4<f32>(0.0);
    if (gShading.ssrEnabled > 0.5) {
        // The half-size path: the resolve pass picked the level and undid the
        // tone compression, so the pixel's own texel is read (resolve_ssr off).
        var ssr = textureLoad(gSsrFinal, min(pixel, textureDimensions(gSsrFinal) - vec2<u32>(1u)), 0);
        ssr = ssr * smoothstep(0.0, 1.0, 1.0 - clamp((roughness - 0.6) / (0.7 - 0.6), 0.0, 1.0));
        out.ssr = ssr;
        spec = spec * (1.0 - ssr.a) + ssr.rgb;
    }
    out.blended = spec;
    let nv = clamp(dot(n, v), 0.0001, 1.0);
    let ab = ssrKarisAB(roughness, nv);
    // prefiltered_dfg's (x, y) from Karis's (A, B): (f90 - f0) x + f0 y = f0 A + f90 B.
    let dfg = vec2<f32>(ab.y, ab.x + ab.y);
    let energy = vec3<f32>(1.0) + f0 * (1.0 / dfg.y - 1.0);
    // The cheap luminance approximation of f90 (scene_forward_clustered.glsl:2258-2259).
    let f90 = clamp(50.0 * f0.g, metallic, 1.0);
    out.scale = energy * ((vec3<f32>(f90) - f0) * dfg.x + f0 * dfg.y);
    return out;
}

// Karis's analytic environment BRDF ("Physically Based Shading on Mobile", 2014):
// the scale A and bias B of f0.
fn ssrKarisAB(roughness: f32, nv: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * nv)) * r.x + r.y;
    return vec2<f32>(-1.04, 1.04) * a004 + r.zw;
}

// The opaque pass: the lab's direct light from the panel and its ambient, plus
// Godot's indirect specular with SSR blended in.
@fragment
fn ssrForwardFS(input: SceneVaryings) -> SceneOutput {
    let eye = gFrame.camera.position;
    let v = normalize(eye - input.worldPosition);
    let n = eyeSideNormal(input.worldNormal, input.worldPosition, eye);
    let o = gObjects[input.objectIndex];
    let albedo = o.albedoRoughness.rgb;
    let viewNormal = normalize((gFrame.camera.worldToView * vec4<f32>(n, 0.0)).xyz);
    let roughness = ssrLimitRoughness(o.albedoRoughness.w, viewNormal);
    let metallic = select(0.0, gShading.floorMetallic, input.objectIndex == gFrame.counts.y);
    // f0 = mix(vec3(0.16 * specular * specular), albedo, metallic) with the default specular 0.5.
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    // Godot's ambient_light is the sky's radiance over the hemisphere; the lab's ambient term divided by pi.
    let ambient = indirectIrradiance(n) / PI;
    var color = albedo * (1.0 - metallic) * (directIrradiance(input.worldPosition, n, input.objectIndex) / PI + ambient) + o.emissive.rgb * o.emissive.w;
    let size = vec2<u32>(gFrame.camera.viewportSize);
    let pixel = min(vec2<u32>(input.clip.xy), size - vec2<u32>(1u));
    let spec = ssrIndirectSpecular(input.worldPosition, n, v, roughness, metallic, f0, ambient, pixel);
    // The stages a demonstration shows; the frame is stage 3.
    let step = u32(round(gShading.composeStep));
    if (step == 1u) {
        color += spec.beforeSsr * spec.scale;
    } else if (step == 2u) {
        color += spec.ssr.rgb * spec.scale;
    } else if (step >= 3u) {
        color += spec.blended * spec.scale;
    }
    var coverage = 0xffffffffu;
    let presence = objectPresence(input.objectIndex);
    if (presence < 1.0) {
        coverage = (1u << u32(clamp(floor(presence * 4.0 + interleavedGradientNoise(input.clip.xy)), 0.0, 4.0))) - 1u;
    }
    return SceneOutput(vec4<f32>(color, 1.0), coverage);
}
