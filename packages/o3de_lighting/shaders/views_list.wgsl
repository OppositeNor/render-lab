/// The picked pixel's tile's light lists, as the chain holds them: on top
/// its LightList (one entry per light the tile keeps, `index << 16 | bins`,
/// in the culling's seven groups, each closed by END_OF_GROUP), then
/// LightListRemapped, one row per depth bin (8 at most, NVLC_MAX_BINS), each
/// the indices its bin keeps, the groups' markers, and END_OF_LIST. A cell is
/// coloured by its light's group; a marker is dark, END_OF_LIST black, and
/// the row of the bin the picked pixel's depth falls in is marked on the left.
/// {@ view=lightList entry=o3deLightListView size=528,240 text=views.light_list reads=tileData,lightList,lightListRemapped,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct LightListParams {
    /// The entries a row shows.
    /// {@ key=columns default=32 range=8,64}
    columns: u32,
    /// How far the forward pass's iterator has walked the picked pixel's bin:
    /// its first `walked` entries lit, the rest dimmed (-1: all lit).
    /// {@ key=walked default=-1 range=-1,256}
    walked: f32,
}

/// {@ view_params=lightList}
@group(3) @binding(1) var<uniform> gLightList: LightListParams;

/// The culling's groups in list order (LightCulling.azsl): decals, simple
/// point, simple spot, point (sphere), disk, capsule, quad.
fn groupColor(group: u32) -> vec3<f32> {
    switch (group) {
        case 0u: { return vec3<f32>(0.62, 0.62, 0.66); }
        case 1u: { return vec3<f32>(0.98, 0.62, 0.22); }
        case 2u: { return vec3<f32>(0.98, 0.86, 0.30); }
        case 3u: { return vec3<f32>(0.95, 0.45, 0.62); }
        case 4u: { return vec3<f32>(0.50, 0.85, 0.45); }
        case 5u: { return vec3<f32>(0.35, 0.80, 0.90); }
        default: { return vec3<f32>(0.68, 0.55, 0.95); }
    }
}

@compute @workgroup_size(8, 8)
fn o3deLightListView(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(gViewOut);
    if (any(id.xy >= size)) {
        return;
    }
    let pick = pickedUv();
    var color = vec3<f32>(0.03, 0.04, 0.08);
    if (pick.x < 0.0) {
        textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
        return;
    }
    let full = vec2<f32>(textureDimensions(tDepth));
    let pixel = min(vec2<u32>(pick * full), textureDimensions(tDepth) - 1u);
    let tileId = pixel / TILE_DIM;
    let tile = tileId.y * tileGridWidth() + tileId.x;
    let data = tileUnpackData(tile);
    let distance = viewDepthAt(pixel);
    let f = saturate((distance - data.zNear) / (data.zFar - data.zNear));
    let pixelBin = u32(min(f, 0.999999) * f32(1u << data.logMaxBins));
    // Rows: two of LightList, a gap, then the bins.
    let columns = gLightList.columns;
    let cell = f32(size.x - 16u) / f32(columns);
    let rowHeight = f32(size.y) / 11.0;
    let row = u32(f32(id.y) / rowHeight);
    let x = f32(id.x) - 16.0;
    if (row == 2u || x < 0.0) {
        // The bin marker on the left of the picked pixel's bin.
        if (x < 0.0 && x > -12.0 && row >= 3u && row - 3u == pixelBin) {
            color = vec3<f32>(1.0, 0.85, 0.2);
        }
        textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
        return;
    }
    let column = u32(x / cell);
    // The cell's border.
    let fx = x / cell - f32(column);
    let fy = f32(id.y) / rowHeight - f32(row);
    let border = fx < 0.08 || fx > 0.92 || fy < 0.1 || fy > 0.9;
    if (row < 2u) {
        // LightList: walk its groups up to this entry.
        let k = row * columns + column;
        var group = 0u;
        var markers = 0u;
        var value = 0u;
        for (var i = 0u; i <= k; i++) {
            value = bLightList[tile * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN + i];
            if (i < k && (value >> 16u) == NVLC_END_OF_GROUP) {
                group++;
                markers++;
            }
            if (markers == LIGHT_GROUPS) {
                break;
            }
        }
        if (markers < LIGHT_GROUPS) {
            color = select(groupColor(group), vec3<f32>(0.18, 0.20, 0.26), (value >> 16u) == NVLC_END_OF_GROUP);
        }
    } else if (row - 3u < (1u << data.logMaxBins)) {
        let bin = row - 3u;
        let base = (tile * NVLC_MAX_BINS + bin) * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN;
        var group = 0u;
        var ended = false;
        var value = 0u;
        for (var i = 0u; i <= column; i++) {
            value = bLightListRemapped[base + i];
            if (value == NVLC_END_OF_LIST) {
                ended = i < column;
                break;
            }
            if (i < column && value == NVLC_END_OF_GROUP) {
                group++;
            }
        }
        if (!ended) {
            if (value == NVLC_END_OF_LIST) {
                color = vec3<f32>(0.0);
            } else if (value == NVLC_END_OF_GROUP) {
                color = vec3<f32>(0.18, 0.20, 0.26);
            } else {
                color = groupColor(group);
            }
            if (bin == pixelBin) {
                color = color * 1.15;
                if (gLightList.walked >= 0.0 && f32(column) >= gLightList.walked) {
                    color = color * 0.3;
                }
            }
        }
    }
    if (border) {
        color = color * 0.45;
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(saturate(color), 1.0));
}
