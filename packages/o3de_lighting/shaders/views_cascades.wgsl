/// The sun's cascaded shadow map (CascadedShadowmaps.pass): one
/// Texture2DArray of D32_FLOAT, 1024 x 1024 texels a slice, one slice per
/// cascade, O3DE's depth from the light (0 near, 1 far, cleared to 1 where
/// nothing was drawn; the lab stores it reversed, 1 - depth). `cascade`
/// shows one slice, or 4 all four side by side; a depth shows from bright
/// (near the light) to dark, empty texels black. The picked pixel's point
/// is ringed in the slices that hold it.
/// {@ view=cascadeMap entry=o3deCascadeMapView size=512,128 text=views.cascade_map reads=cascades,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct CascadeMapParams {
    /// The slice, 0 to 3, or 4 for all four side by side.
    /// {@ key=cascade default=4 range=0,4}
    cascade: u32,
    /// The depth range the ramp spans, `from` to `range` (depths outside it
    /// clamp to its ends): a narrow range stretches the room's depths, which
    /// lie close together in a long cascade.
    /// {@ key=range default=1 range=0.05,1}
    range: f32,
    /// {@ key=from default=0 range=0,0.99}
    start: f32,
}

/// {@ view_params=cascadeMap}
@group(3) @binding(1) var<uniform> gCascadeMapView: CascadeMapParams;

@compute @workgroup_size(8, 8)
fn o3deCascadeMapView(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(gViewOut);
    if (any(id.xy >= size)) {
        return;
    }
    let together = gCascadeMapView.cascade >= 4u;
    let slices = select(1u, 4u, together);
    let side = min(size.x / slices, size.y);
    let slot = id.x / side;
    let local = vec2<u32>(id.x % side, id.y);
    if (slot >= slices || local.y >= side) {
        textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(0.02, 0.03, 0.08, 1.0));
        return;
    }
    let cascade = select(gCascadeMapView.cascade, slot, together);
    let uv = (vec2<f32>(local) + 0.5) / f32(side);
    let texel = vec2<i32>(uv * CASCADE_MAP_SIZE);
    let depth = cascadeDepth(cascade, texel);
    var color = vec3<f32>(0.0);
    if (depth < 1.0) {
        let t = 1.0 - saturate((depth - gCascadeMapView.start) / max(gCascadeMapView.range - gCascadeMapView.start, 1e-4));
        color = mix(vec3<f32>(0.04, 0.06, 0.16), vec3<f32>(0.62, 0.84, 1.0), t) * CASCADE_DEBUG_COLORS[cascade] * 0.35 + mix(vec3<f32>(0.04, 0.06, 0.16), vec3<f32>(0.62, 0.84, 1.0), t) * 0.65;
    }
    // A border in the cascade's debug colour.
    if (local.x == 0u || local.y == 0u || local.x == side - 1u || local.y == side - 1u) {
        color = CASCADE_DEBUG_COLORS[cascade];
    }
    // The picked pixel's point in this slice.
    let pick = pickedUv();
    if (pick.x >= 0.0) {
        let full = vec2<f32>(textureDimensions(tDepth));
        let pixel = min(vec2<u32>(pick * full), textureDimensions(tDepth) - 1u);
        let d = textureLoad(tDepth, pixel, 0);
        if (covered(d)) {
            let p = reconstructWorldPosition(pixel, d);
            let t = shadowCoord(cascade, p, vec3<f32>(0.0));
            let r = length((vec2<f32>(local) + 0.5) - t.xy * f32(side));
            if (all(t.xy >= vec2<f32>(0.0)) && all(t.xy <= vec2<f32>(1.0)) && r > 3.0 && r < 5.0) {
                color = vec3<f32>(1.0, 0.85, 0.2);
            }
        }
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
