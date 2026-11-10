/// The picked pixel's 16 x 16 tile, zoomed, as LightCullingTilePrepare reads it: one cell per pixel, shaded by
/// the pixel's view depth between the tile's z near (bright) and z far (dark), black where nothing was drawn.
/// The cells read so far are lit and the rest dimmed; the next cell to read is outlined, and the picked pixel's
/// cell ringed.
/// {@ view=tilePixels entry=o3deTilePixelsView size=320,320 text=views-tile_pixels reads=tileData,depth}
@group(3) @binding(0) var gViewOut: texture_storage_2d<rgba8unorm, write>;

struct TilePixelsParams {
    /// How many of the tile's 256 pixels have been read, in row order (the cursor is the next one).
    /// {@ key=read default=256 range=0,256}
    read: f32,
}

/// {@ view_params=tilePixels}
@group(3) @binding(1) var<uniform> gTilePixels: TilePixelsParams;

@compute @workgroup_size(8, 8)
fn o3deTilePixelsView(@builtin(global_invocation_id) id: vec3<u32>) {
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
    let full = textureDimensions(tDepth);
    let pixel = min(vec2<u32>(pick * vec2<f32>(full)), full - 1u);
    let tileId = pixel / TILE_DIM;
    let data = tileUnpackData(tileId.y * tileGridWidth() + tileId.x);
    // The cell this texel shows, and where inside the cell it lies.
    let f = vec2<f32>(id.xy) / vec2<f32>(size) * f32(TILE_DIM);
    let cell = vec2<u32>(f);
    let inside = f - vec2<f32>(cell);
    let px = min(tileId * TILE_DIM + cell, full - 1u);
    let distance = viewDepthAt(px);
    if (distance > 0.0) {
        let t = saturate((distance - data.zNear) / max(data.zFar - data.zNear, 1e-4));
        color = mix(vec3<f32>(0.95, 0.85, 0.55), vec3<f32>(0.25, 0.30, 0.55), t);
    }
    let index = f32(cell.y * TILE_DIM + cell.x);
    if (index >= gTilePixels.read) {
        color = color * 0.25;
    }
    let edge = inside.x < 0.06 || inside.y < 0.06;
    if (edge) {
        color = vec3<f32>(0.02, 0.02, 0.04);
    }
    // The cursor: the next pixel to read, outlined.
    let next = u32(gTilePixels.read);
    if (next < TILE_DIM * TILE_DIM && cell.x == next % TILE_DIM && cell.y == next / TILE_DIM
        && (inside.x < 0.16 || inside.x > 0.84 || inside.y < 0.16 || inside.y > 0.84)) {
        color = vec3<f32>(1.0, 0.85, 0.2);
    }
    // The picked pixel's cell, ringed.
    let picked = pixel - tileId * TILE_DIM;
    if (all(cell == picked)) {
        let d = length(inside - 0.5);
        if (d > 0.28 && d < 0.42) {
            color = vec3<f32>(1.0, 0.3, 0.3);
        }
    }
    textureStore(gViewOut, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
