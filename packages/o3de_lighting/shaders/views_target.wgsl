/// The investigation's image kind of its pass chain's screen-sized resources
/// (lab/src/views.rs), composed after standard_lighting.wgsl, whose group-2
/// declarations of the chain's targets it reads (`reads=`). An instance shows
/// one O3DE resource as the chain holds it: Forward.pass's five targets, the
/// depth pre-pass's depth, or the stencil reference each pixel was drawn with
/// (the lab carries it as data: its depth has no stencil). Unorm texels show
/// as stored; float colours are tone mapped. A story lays instances side by
/// side as O3DE keeps them apart, each titled (`imageTitle`), and every one
/// rings the picked pixel (gShading.pick).
/// {@ view=chainTarget entry=o3deChainTargetView size=384,216 text=views.chain_target reads=diffuse,specular,albedo,specularF0,normal,stencil,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct ChainTargetParams {
    /// The resource: 0 DiffuseOutput (R16G16B16A16_FLOAT), 1 SpecularOutput
    /// (R16G16B16A16_FLOAT), 2 AlbedoOutput (R8G8B8A8_UNORM), 3
    /// SpecularF0Output (R8G8B8A8_UNORM), 4 NormalOutput (R10G10B10A2_UNORM,
    /// the encoded normal), 5 the depth (linear, near bright), 6 the stencil
    /// reference.
    /// {@ key=target default=0 range=0,6}
    which: u32,
    /// What of it: 0 rgb, 1 r, 2 g, 3 b, 4 a (each as grey); 5 the normal
    /// target's normal decoded (as n / 2 + 1 / 2).
    /// {@ key=channel default=0 range=0,5}
    channel: u32,
    /// A float target's exposure before tone mapping, in stops.
    /// {@ key=stops default=0 range=-8,8}
    stops: f32,
    /// The depth view's far end, metres (the ramp runs to black there).
    /// {@ key=far default=12 range=1,200}
    far: f32,
}

/// {@ view_params=chainTarget}
@group(3) @binding(1) var<uniform> gChainTarget: ChainTargetParams;

const TARGET_DIFFUSE: u32 = 0u;
const TARGET_SPECULAR: u32 = 1u;
const TARGET_ALBEDO: u32 = 2u;
const TARGET_SPECULAR_F0: u32 = 3u;
const TARGET_NORMAL: u32 = 4u;
const TARGET_DEPTH: u32 = 5u;
const TARGET_STENCIL: u32 = 6u;

/// A texel as the view shows it: one channel as grey, or its colour.
fn channelColor(texel: vec4<f32>) -> vec3<f32> {
    switch (gChainTarget.channel) {
        case 1u: { return vec3<f32>(texel.r); }
        case 2u: { return vec3<f32>(texel.g); }
        case 3u: { return vec3<f32>(texel.b); }
        case 4u: { return vec3<f32>(texel.a); }
        default: { return texel.rgb; }
    }
}

@compute @workgroup_size(8, 8)
fn o3deChainTargetView(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(gViewOut))) {
        return;
    }
    let pixel = viewPixel(id.xy);
    var color = vec3<f32>(0.0);
    switch (gChainTarget.which) {
        case TARGET_DIFFUSE, TARGET_SPECULAR: {
            let t = select(meanSamples(tSpecular, pixel), meanSamples(tDiffuse, pixel), gChainTarget.which == TARGET_DIFFUSE);
            let c = channelColor(t);
            // The diffuse alpha (-1, or the packed SSS factor) and the specular
            // alpha (1) are numbers, not light: shown as stored.
            if (gChainTarget.channel == 4u) {
                color = vec3<f32>(saturate(c.r * 0.5 + 0.5));
            } else {
                color = linearToSrgb(tonemap(max(c, vec3<f32>(0.0)) * exp2(gChainTarget.stops)));
            }
        }
        case TARGET_ALBEDO: {
            color = saturate(channelColor(meanSamples(tAlbedo, pixel)));
        }
        case TARGET_SPECULAR_F0: {
            color = saturate(channelColor(meanSamples(tSpecularF0, pixel)));
        }
        case TARGET_NORMAL: {
            let t = meanSamples(tNormal, pixel);
            if (gChainTarget.channel == 5u) {
                color = decodeNormalSignedOctahedron(t.xyz) * 0.5 + 0.5;
            } else {
                color = saturate(channelColor(t));
            }
        }
        case TARGET_DEPTH: {
            let z = viewDepthAt(pixel);
            let t = select(0.0, 1.0 - saturate(z / gChainTarget.far), z > 0.0);
            color = mix(vec3<f32>(0.02, 0.03, 0.08), vec3<f32>(0.55, 0.80, 1.0), t);
        }
        default: {
            let s = u32(round(textureLoad(tStencil, pixel, 0u).r * 255.0));
            color = stencilViewColor(s);
        }
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(withPick(id.xy, color), 1.0));
}

/// A stencil reference as a colour: 0x80 (UseDiffuseGIPass) alone dark
/// violet, with 0x3 (UseIBLSpecularPass) teal, and each probe volume the
/// pixel lies in (the low bits above 3) brighter; 0 (nothing drawn) black.
fn stencilViewColor(s: u32) -> vec3<f32> {
    if (s == 0u) {
        return vec3<f32>(0.0);
    }
    let low = s & STENCIL_REFLECTION_MASK;
    var c = vec3<f32>(0.30, 0.18, 0.45);
    if (low >= STENCIL_USE_IBL_SPECULAR_PASS) {
        c = vec3<f32>(0.10, 0.55, 0.55) * (1.0 + 0.35 * f32(low - STENCIL_USE_IBL_SPECULAR_PASS));
    }
    return saturate(c);
}
