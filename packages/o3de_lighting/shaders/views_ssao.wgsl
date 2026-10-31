/// SSAO's stages (SsaoParent.pass), each as the chain holds it: 0
/// DepthDownsample's half-size linear depth (R32_FLOAT, the upper-left pixel
/// of each 2 x 2), 1 SsaoCompute's output (R8_UNORM, half size,
/// pow(1 - AO x strength, 2.2): white unoccluded), 2 after the vertical and
/// horizontal depth-aware blurs (half size), 3 the depth-aware upsample (full
/// size), which ModulateWithSsao multiplies the diffuse light by. The picked
/// pixel is ringed.
/// {@ view=ssaoStage entry=o3deSsaoStageView size=384,216 text=views.ssao_stage reads=halfDepth,ssao,ssaoBlur,ssaoFull,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct SsaoStageParams {
    /// 0 half-size depth, 1 SsaoCompute, 2 blurred, 3 upsampled.
    /// {@ key=stage default=3 range=0,3}
    stage: u32,
    /// The depth stage's far end, metres.
    /// {@ key=far default=12 range=1,200}
    far: f32,
}

/// {@ view_params=ssaoStage}
@group(3) @binding(1) var<uniform> gSsaoStage: SsaoStageParams;

@compute @workgroup_size(8, 8)
fn o3deSsaoStageView(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(gViewOut))) {
        return;
    }
    let pixel = viewPixel(id.xy);
    let half = pixel / 2u;
    let hi = half.y * halfSize().x + half.x;
    var color = vec3<f32>(0.0);
    switch (gSsaoStage.stage) {
        case 0u: {
            let z = bitcast<f32>(bHalfDepth[hi]);
            let t = select(0.0, 1.0 - saturate(z / gSsaoStage.far), z > 0.0);
            color = mix(vec3<f32>(0.02, 0.03, 0.08), vec3<f32>(0.55, 0.80, 1.0), t);
        }
        case 1u: { color = vec3<f32>(bitcast<f32>(bSsao[hi])); }
        case 2u: { color = vec3<f32>(bitcast<f32>(bSsaoBlur[hi])); }
        default: { color = vec3<f32>(bitcast<f32>(bSsaoFull[pixel.y * fullSize().x + pixel.x])); }
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(withPick(id.xy, saturate(color)), 1.0));
}
