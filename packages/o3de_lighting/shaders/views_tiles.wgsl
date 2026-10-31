/// Light culling's tiles (LightCullingParent.pass) as the chain holds them:
/// every 16 x 16 tile of the viewport coloured by the lights its depth bins
/// keep (LightListRemapped, without the group markers), with the tile grid,
/// and the picked pixel's tile outlined. `mode` 0 counts the tile's fullest
/// bin, as O3DE's (disabled) LightCullingHeatmap does, from TileLightData.w;
/// 1 the bin each pixel's own depth falls in, the list the forward pass walks.
/// {@ view=tileGrid entry=o3deTileGridView size=384,216 text=views.tile_grid reads=tileData,lightListRemapped,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct TileGridParams {
    /// 0 the tile's fullest bin, 1 the pixel's own bin, 2 how many depth bins
    /// the tile has (1, 2, 4, or 8: dark to bright).
    /// {@ key=mode default=0 range=0,2}
    mode: u32,
    /// The count shown at the top of the ramp.
    /// {@ key=top default=12 range=1,64}
    top: f32,
}

/// {@ view_params=tileGrid}
@group(3) @binding(1) var<uniform> gTileGrid: TileGridParams;

/// The lights in one bin's remapped list: its entries before END_OF_LIST,
/// less the END_OF_GROUP markers.
fn binLightCount(tile: u32, bin: u32) -> u32 {
    let base = (tile * NVLC_MAX_BINS + bin) * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN;
    var count = 0u;
    for (var i = 0u; i < NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN; i++) {
        let v = bLightListRemapped[base + i];
        if (v == NVLC_END_OF_LIST) {
            break;
        }
        if (v != NVLC_END_OF_GROUP) {
            count++;
        }
    }
    return count;
}

/// The bin a view distance falls in (NVLC_GetBin).
fn binOf(data: TileLightData, distance: f32) -> u32 {
    let f = saturate((distance - data.zNear) / (data.zFar - data.zNear));
    return u32(min(f, 0.999999) * f32(1u << data.logMaxBins));
}

@compute @workgroup_size(8, 8)
fn o3deTileGridView(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(gViewOut))) {
        return;
    }
    let pixel = viewPixel(id.xy);
    let tileId = pixel / TILE_DIM;
    let tile = tileId.y * tileGridWidth() + tileId.x;
    let data = tileUnpackData(tile);
    let distance = viewDepthAt(pixel);
    var count = 0u;
    if (gTileGrid.mode == 2u) {
        // 1, 2, 4, 8 bins on the ramp's quarter marks.
        let bins = f32(data.logMaxBins) / 3.0;
        var c = heat(0.15 + 0.85 * bins);
        if (distance <= 0.0) {
            c = vec3<f32>(0.03, 0.04, 0.08);
        }
        textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(c, 1.0));
        return;
    }
    if (gTileGrid.mode == 1u) {
        count = select(0u, binLightCount(tile, binOf(data, distance)), distance > 0.0);
    } else {
        for (var b = 0u; b < (1u << data.logMaxBins); b++) {
            count = max(count, binLightCount(tile, b));
        }
    }
    var color = select(heat(sqrt(f32(count) / gTileGrid.top)), vec3<f32>(0.03, 0.04, 0.08), count == 0u);
    // A faint picture of the scene's depth under the counts.
    if (distance > 0.0) {
        color = mix(color, vec3<f32>(0.6, 0.75, 1.0), 0.12 * (1.0 - saturate(distance / 12.0)));
    }
    // The tile grid: the last view texel of each tile.
    let out = vec2<f32>(textureDimensions(gViewOut));
    let full = vec2<f32>(textureDimensions(tDepth));
    let next = min(vec2<u32>((vec2<f32>(id.xy) + 1.5) / out * full), textureDimensions(tDepth) - 1u) / TILE_DIM;
    // Drawn only where a tile spans a few view texels.
    if (any(next != tileId) && out.x / full.x * f32(TILE_DIM) >= 5.0) {
        color = mix(color, vec3<f32>(0.0), 0.35);
    }
    // The picked pixel's tile, outlined.
    let pick = pickedUv();
    if (pick.x >= 0.0) {
        let pickTile = vec2<u32>(pick * full) / TILE_DIM;
        let lo = vec2<f32>(pickTile * TILE_DIM) / full * out;
        let hi = vec2<f32>((pickTile + 1u) * TILE_DIM) / full * out;
        let p = vec2<f32>(id.xy) + 0.5;
        let inside = all(p >= lo - 1.5) && all(p <= hi + 1.5);
        let inner = all(p >= lo + 0.5) && all(p <= hi - 0.5);
        if (inside && !inner) {
            color = vec3<f32>(1.0, 0.85, 0.2);
        }
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
