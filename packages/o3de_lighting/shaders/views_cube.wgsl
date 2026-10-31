/// The global skylight's cubemaps (ImageBasedLightFeatureProcessor): the
/// specular cubemap's mips (R9G9B9E5 in O3DE, six mips, mip m prefiltered
/// with GGX for perceptual roughness m / 5) or the diffuse cubemap
/// (cosine-convolved, one mip), as an unfolded cross: +Y on top, then -X,
/// +Z, +X, -Z, then -Y (the lab's +Y up; O3DE addresses its cubes with
/// GetCubemapCoords, (-x, z, -y) of its +Z-up vector). The lab builds them
/// from its sky at the sizes `ibl.rs` names. The picked pixel's lookup
/// direction is ringed: its reflection vector in the specular cubemap, its
/// normal in the diffuse one.
/// {@ view=cubemap entry=o3deCubemapView size=384,288 text=views.cubemap reads=normal,specularF0,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct CubemapParams {
    /// 0 the specular cubemap, 1 the diffuse one.
    /// {@ key=diffuse default=0 range=0,1}
    diffuse: u32,
    /// The specular cubemap's mip, 0 to 5 (a fraction blends two, as
    /// SampleLevel does); -1: the picked pixel's, 5 x its roughness.
    /// {@ key=mip default=0 range=-1,5}
    mip: f32,
    /// Exposure before tone mapping, in stops.
    /// {@ key=stops default=0 range=-8,8}
    stops: f32,
}

/// {@ view_params=cubemap}
@group(3) @binding(1) var<uniform> gCubemapView: CubemapParams;

// The cross's cells: (column, row) of faces +X, -X, +Y, -Y, +Z, -Z (cubeFace order).
const CROSS_CELLS = array<vec2<u32>, 6>(vec2<u32>(2u, 1u), vec2<u32>(0u, 1u), vec2<u32>(1u, 0u), vec2<u32>(1u, 2u), vec2<u32>(1u, 1u), vec2<u32>(3u, 1u));

/// The picked pixel's surface: (its position, whether there is one), its
/// normal (NormalOutput decoded), and its roughness (SpecularF0Output.a).
struct PickedSurface {
    hit: bool,
    position: vec3<f32>,
    normal: vec3<f32>,
    roughness: f32,
}

fn pickedSurface() -> PickedSurface {
    var out: PickedSurface;
    let pick = pickedUv();
    if (pick.x < 0.0) {
        return out;
    }
    let full = vec2<f32>(textureDimensions(tDepth));
    let pixel = min(vec2<u32>(pick * full), textureDimensions(tDepth) - 1u);
    let d = textureLoad(tDepth, pixel, 0);
    out.hit = covered(d);
    out.position = reconstructWorldPosition(pixel, d);
    out.normal = decodeNormalSignedOctahedron(textureLoad(tNormal, pixel, 0).xyz);
    out.roughness = textureLoad(tSpecularF0, pixel, 0).a;
    return out;
}

@compute @workgroup_size(8, 8)
fn o3deCubemapView(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(gViewOut);
    if (any(id.xy >= size)) {
        return;
    }
    let side = min(size.x / 4u, size.y / 3u);
    let cell = id.xy / side;
    let local = id.xy % side;
    var face = 6u;
    for (var f = 0u; f < 6u; f++) {
        if (all(CROSS_CELLS[f] == cell)) {
            face = f;
        }
    }
    if (face == 6u || any(id.xy >= vec2<u32>(4u * side, 3u * side))) {
        textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(0.02, 0.03, 0.08, 1.0));
        return;
    }
    let picked = pickedSurface();
    var mip = gCubemapView.mip;
    if (mip < 0.0) {
        mip = picked.roughness * 5.0;
    }
    let st = (vec2<f32>(local) + 0.5) / f32(side) * 2.0 - 1.0;
    let axes = cubeFace(face);
    let dir = normalize(axes[0] * st.x + axes[1] * st.y + axes[2]);
    var radiance = vec3<f32>(0.0);
    if (gCubemapView.diffuse == 1u) {
        radiance = sampleDiffuseEnv(dir);
    } else {
        radiance = sampleSpecularEnv(dir, mip);
    }
    var color = linearToSrgb(tonemap(radiance * exp2(gCubemapView.stops)));
    // The lookup direction of the picked pixel.
    if (picked.hit) {
        let v = normalize(gFrame.camera.position - picked.position);
        let lookup = select(reflect(-v, picked.normal), picked.normal, gCubemapView.diffuse == 1u);
        let angle = acos(clamp(dot(dir, normalize(lookup)), -1.0, 1.0));
        let texel = 2.0 / f32(side);
        if (angle > 4.0 * texel && angle < 6.5 * texel) {
            color = vec3<f32>(1.0, 0.85, 0.2);
        }
    }
    // The faces' edges.
    if (local.x == 0u || local.y == 0u) {
        color = color * 0.5;
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
