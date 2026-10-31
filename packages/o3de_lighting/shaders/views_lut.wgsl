/// The BRDF look-up texture (A:/Textures/BRDFTexture.attimage, built once at
/// start-up by BRDFTextureCS: 256 x 256 R16G16_FLOAT, 1024 GGX samples a
/// texel): its red channel, the split sum's scale A, and its green channel,
/// the bias B, side by side; x is N.V (0 to 1), y runs from rough (top,
/// 1 - roughness = 0... the texture's rows are flipped: v = 1 - roughness,
/// so its top row is roughness 1) to smooth. The picked pixel's texel,
/// (N.V, 1 - roughness), is ringed in both.
/// {@ view=brdfLut entry=o3deBrdfLutView size=512,256 text=views.brdf_lut reads=normal,specularF0,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct BrdfLutParams {
    /// The channels' brightness.
    /// {@ key=gain default=1 range=0.5,4}
    gain: f32,
}

/// {@ view_params=brdfLut}
@group(3) @binding(1) var<uniform> gBrdfLutView: BrdfLutParams;

@compute @workgroup_size(8, 8)
fn o3deBrdfLutView(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(gViewOut);
    if (any(id.xy >= size)) {
        return;
    }
    let side = min(size.x / 2u, size.y);
    let channel = id.x / side;
    let local = vec2<u32>(id.x % side, id.y);
    if (channel > 1u || local.y >= side) {
        textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(0.02, 0.03, 0.08, 1.0));
        return;
    }
    let uv = (vec2<f32>(local) + 0.5) / f32(side);
    let texel = vec2<u32>(uv * f32(BRDF_MAP_SIZE));
    let ab = gShadingData[texel.y * BRDF_MAP_SIZE + texel.x].xy;
    let v = saturate(select(ab.x, ab.y, channel == 1u) * gBrdfLutView.gain);
    var color = select(vec3<f32>(v, v * 0.35, v * 0.25), vec3<f32>(v * 0.3, v, v * 0.4), channel == 1u);
    // The picked pixel's texel: GetBRDFTexCoords, (N.V, 1 - roughness).
    let pick = pickedUv();
    if (pick.x >= 0.0) {
        let full = vec2<f32>(textureDimensions(tDepth));
        let pixel = min(vec2<u32>(pick * full), textureDimensions(tDepth) - 1u);
        let d = textureLoad(tDepth, pixel, 0);
        if (covered(d)) {
            let p = reconstructWorldPosition(pixel, d);
            let n = decodeNormalSignedOctahedron(textureLoad(tNormal, pixel, 0).xyz);
            let roughness = textureLoad(tSpecularF0, pixel, 0).a;
            let nv = saturate(dot(n, normalize(gFrame.camera.position - p)));
            let at = vec2<f32>(nv, 1.0 - roughness) * f32(side);
            let r = length(vec2<f32>(local) + 0.5 - at);
            if (r > 4.0 && r < 6.5) {
                color = vec3<f32>(1.0, 0.85, 0.2);
            }
        }
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
