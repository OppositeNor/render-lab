/// The investigation's image kinds (lab/src/views.rs), GPU images a story
/// makes and drives (a cue `image=maps imageFrom=probeMap image.probe=7,7,8`),
/// composed after the scene shading, whose probe maps (`sdfgiTexel`) they read.
///
/// A probe's two octahedral maps, irradiance and radiance side by side, as the
/// probe pass stored them (E5B9G9R9, each 6 x 6 texels and a one-texel border,
/// dimmed), tone mapped as the CPU picture of them is (`demos.rs`, `maps`).
/// {@ view=probeMap entry=sdfgiProbeMapView size=208,104 text=demos.probe_maps_gpu}
@group(3) @binding(0) var gProbeMapOut: texture_storage_2d<rgba8unorm, write>;

struct ProbeMapParams {
    /// The probe, by its cell in its cascade's probe grid.
    /// {@ key=probe default=7,7,8 range=0,16}
    probe: vec3<f32>,
    /// Its cascade.
    /// {@ key=cascade default=0 range=0,3}
    cascade: u32,
    /// The gain before tone mapping.
    /// {@ key=gain default=3 range=0,16}
    gain: f32,
}

/// {@ view_params=probeMap}
@group(3) @binding(1) var<uniform> gProbeMap: ProbeMapParams;

@compute @workgroup_size(8, 8)
fn sdfgiProbeMapView(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(gProbeMapOut);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    // Each map is 8 x 8 texels (SDFGI_OCT_SIZE and its border), a texel `scale` pixels.
    let side = u32(SDFGI_OCT_SIZE) + 2u;
    let scale = size.y / side;
    let map = id.x / (side * scale);
    let local = vec2<u32>(id.x % (side * scale), id.y);
    let t = local / scale;
    // The probe's maps start at ((x + z * 17) * 8, y * 8) in its cascade's layers.
    let p = vec3<i32>(clamp(round(gProbeMap.probe), vec3<f32>(0.0), vec3<f32>(16.0)));
    let base = vec2<i32>((p.x + p.z * 17) * i32(side), p.y * i32(side));
    let layer = min(gProbeMap.cascade, 3u) * 2u + min(map, 1u);
    let c = sdfgiTexel(layer, base.x + i32(t.x), base.y + i32(t.y)) * gProbeMap.gain;
    var v = sqrt(c / (vec3<f32>(1.0) + c));
    if (t.x == 0u || t.y == 0u || t.x == side - 1u || t.y == side - 1u) {
        v = v * 0.5;
    }
    // A dark line between texels.
    if (local.x % scale == 0u || local.y % scale == 0u) {
        v = vec3<f32>(20.0, 28.0, 40.0) / 255.0;
    }
    textureStore(gProbeMapOut, vec2<i32>(id.xy), vec4<f32>(v, 1.0));
}
