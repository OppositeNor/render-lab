/// The sun's screen-space shadow (ShadowParent.pass): FullscreenShadow's
/// output, each pixel's visibility of the sun from the cascades (R8_UNORM;
/// the lab keeps it as floats rounded to 8 bits), or FullscreenShadowBlur's,
/// the buffer the forward pass reads. White lit, black shadowed; the picked
/// pixel ringed.
/// {@ view=shadowBuffer entry=o3deShadowBufferView size=384,216 text=views.shadow_buffer reads=shadowRaw,shadowBuffer,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct ShadowBufferParams {
    /// 0 FullscreenShadow's output, 1 the blurred buffer.
    /// {@ key=blurred default=1 range=0,1}
    blurred: u32,
}

/// {@ view_params=shadowBuffer}
@group(3) @binding(1) var<uniform> gShadowBufferView: ShadowBufferParams;

@compute @workgroup_size(8, 8)
fn o3deShadowBufferView(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(gViewOut))) {
        return;
    }
    let pixel = viewPixel(id.xy);
    let i = pixel.y * fullSize().x + pixel.x;
    let v = bitcast<f32>(select(bShadowRaw[i], bShadowBuffer[i], gShadowBufferView.blurred == 1u));
    var color = vec3<f32>(0.06, 0.05, 0.12) + vec3<f32>(0.92, 0.92, 0.88) * v;
    if (!covered(textureLoad(tDepth, pixel, 0))) {
        color = vec3<f32>(0.02, 0.03, 0.08);
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(withPick(id.xy, color), 1.0));
}
