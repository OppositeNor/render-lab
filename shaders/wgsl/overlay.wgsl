// Debug overlays. Wire boxes, vectors, samples, cards, and probes are built
// on the GPU from the same records the struct panel displays: a drawn box is
// literally ObjectData.worldCenter/worldExtent, a drawn arrow is literally
// RayDebugData.reflectedDir, and so on.
//
// Overlays render into their own multisampled RGBA8 layer that shares the
// scene's depth buffer. Colours are display-referred sRGB values written
// with premultiplied alpha; the composite pass lays the layer over the
// tonemapped scene, so overlay colours match the struct panel exactly.

const FLAG_ARROW: u32 = 1u;
const FLAG_DASHED: u32 = 2u;

// Segment kinds (SegmentInstance.rid.x)
const SEG_EXPLICIT: u32 = 0u;     // p0 -> p1 as given
const SEG_RAY: u32 = 1u;          // rid.y ray, rid.z: 0 incident, 1 normal, 2 reflected, 3 view; p0.x = length
const SEG_RADIANCE: u32 = 2u;     // rid.y RadianceSample, rid.z ProbeRecord; p0.x = max length
const SEG_CARD_AXIS: u32 = 3u;    // rid.y card, rid.z column of tangentFrame; p0.x = length
const SEG_OBJECT_AXIS: u32 = 4u;  // rid.y object, rid.z column of localToWorld; p0.x = length

// Box kinds (BoxInstance.rid.x)
const BOX_OBJECT_OBB: u32 = 0u;     // local bounds through localToWorld
const BOX_OBJECT_AABB: u32 = 1u;    // worldCenter +/- worldExtent
const BOX_VIRTUAL_BRICK: u32 = 2u;  // BrickEntry.virtualBrick in the field's local volume
const BOX_PHYSICAL_BRICK: u32 = 3u; // BrickEntry.physicalBrick in the atlas display
const BOX_CARD_VOLUME: u32 = 4u;    // CardData capture volume
const BOX_CARD_RECT: u32 = 5u;      // CardData rectangle (4 edges)
const BOX_PAGE_RECT: u32 = 6u;      // PageEntry.cardUVRect on its card (4 edges)

struct LineVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) along: f32,
    @location(2) @interpolate(flat) flags: u32,
}

struct Segment {
    p0: vec3<f32>,
    p1: vec3<f32>,
    color: vec4<f32>,
    valid: bool,
}

fn offscreen() -> vec4<f32> {
    return vec4<f32>(2.0, 2.0, 2.0, 1.0);
}

fn reflectedEnd(ray: RayDebugData) -> vec3<f32> {
    if (ray.reflectedHitObject != NO_HIT) {
        return ray.reflectedHitPosition;
    }
    return ray.hitPosition + ray.reflectedDir * 2.0;
}

fn recordSegment(rid: vec4<u32>, p0: vec3<f32>, p1: vec3<f32>, color: vec4<f32>) -> Segment {
    var s = Segment(p0, p1, color, true);
    if (rid.x == SEG_RAY) {
        let ray = gRays[rid.y];
        let len = select(0.6, p0.x, p0.x > 0.0);
        if (rid.z == 0u) {
            s.p0 = ray.origin;
            s.p1 = ray.hitPosition;
        } else if (rid.z == 1u) {
            s.p0 = ray.hitPosition;
            s.p1 = ray.hitPosition + ray.normal * len;
        } else if (rid.z == 2u) {
            s.p0 = ray.hitPosition;
            s.p1 = reflectedEnd(ray);
        } else {
            s.p0 = ray.hitPosition;
            s.p1 = ray.hitPosition + ray.viewDir * len;
        }
        // p0.y: the share not drawn yet, while the path is being traced.
        s.p1 = mix(s.p1, s.p0, p0.y);
    } else if (rid.x == SEG_RADIANCE) {
        let r = gRadiance[rid.y];
        let probe = gProbes[rid.z];
        let len = min(r.hitDistance, p0.x);
        s.p0 = probe.position + probe.normal * 0.01;
        s.p1 = s.p0 + r.direction * len;
        if (color.a < 0.0) {
            s.color = vec4<f32>(linearToSrgb(tonemap(r.radiance * 2.0)), -color.a);
        }
    } else if (rid.x == SEG_CARD_AXIS) {
        let card = gCards[rid.y];
        var frame = card.tangentFrame;
        s.p0 = shownCardCenter(card);
        s.p1 = s.p0 + frame[rid.z] * p0.x;
    } else if (rid.x == SEG_OBJECT_AXIS) {
        let o = gObjects[rid.y];
        var m = o.localToWorld;
        let origin = transformPoint(o.localToWorld, o.localCenter);
        s.p0 = origin;
        s.p1 = origin + normalize(m[rid.z].xyz) * p0.x;
    }
    return s;
}

// Expand a world-space segment into a screen-space quad `width` pixels wide,
// with an optional arrowhead: 6 vertices for the body, 3 for the head.
fn expandSegment(seg: Segment, width: f32, flags: u32, vertexId: u32) -> LineVaryings {
    var o: LineVaryings;
    o.flags = flags;
    o.color = seg.color;
    o.along = 0.0;
    o.clip = offscreen();
    if (!seg.valid) {
        return o;
    }
    var c0 = gFrame.camera.worldToClip * vec4<f32>(seg.p0, 1.0);
    var c1 = gFrame.camera.worldToClip * vec4<f32>(seg.p1, 1.0);
    let nearW = 1e-3;
    if (c0.w < nearW && c1.w < nearW) {
        return o;
    }
    if (c0.w < nearW) {
        c0 = mix(c0, c1, (nearW - c0.w) / (c1.w - c0.w));
    }
    if (c1.w < nearW) {
        c1 = mix(c1, c0, (nearW - c1.w) / (c0.w - c1.w));
    }
    let halfSize = gFrame.camera.viewportSize * 0.5;
    let s0 = c0.xy / c0.w * halfSize;
    let s1 = c1.xy / c1.w * halfSize;
    let d = s1 - s0;
    let len = length(d);
    let dir = select(vec2<f32>(1.0, 0.0), d / len, len > 1e-4);
    let perp = vec2<f32>(-dir.y, dir.x);
    let scale = gFrame.overlayParams.y;
    let w = width * scale * 0.5;
    let arrow = (flags & FLAG_ARROW) != 0u;
    var headLen = 0.0;
    var headHalf = 0.0;
    if (arrow) {
        headLen = min(len * 0.45, (14.0 + width * 2.2) * scale);
        headHalf = (5.0 + width * 1.4) * scale;
    }
    let bodyEnd = (len - headLen * 0.85) / max(len, 1e-4);
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0));
    var offsetPx: vec2<f32>;
    var t: f32;
    if (vertexId < 6u) {
        let k = corners[vertexId];
        t = k.x * bodyEnd;
        offsetPx = perp * (k.y * w);
    } else {
        if (!arrow) {
            return o;
        }
        let k = vertexId - 6u;
        let tb = (len - headLen) / max(len, 1e-4);
        t = select(tb, 1.0, k == 0u);
        offsetPx = select(perp * select(headHalf, -headHalf, k == 1u), vec2<f32>(0.0), k == 0u);
    }
    var clip = mix(c0, c1, t);
    clip = vec4<f32>(clip.xy + offsetPx / halfSize * clip.w, clip.z, clip.w);
    // Pull lines slightly toward the camera so edges lying on surfaces win
    // (reversed depth: toward the camera is greater).
    clip.z = min(clip.z + 3e-4 * clip.w, clip.w);
    o.clip = clip;
    o.along = t * len;
    return o;
}

struct SegmentInstance {
    @location(0) p0: vec3<f32>,
    @location(1) width: f32,
    @location(2) p1: vec3<f32>,
    @location(3) flags: u32,
    @location(4) color: vec4<f32>,
    @location(5) rid: vec4<u32>,
}

@vertex
fn segmentVS(input: SegmentInstance, @builtin(vertex_index) vertexId: u32) -> LineVaryings {
    let seg = recordSegment(input.rid, input.p0, input.p1, input.color);
    return expandSegment(seg, input.width, input.flags, vertexId);
}

struct BoxInstance {
    @location(0) rid: vec4<u32>,     // kind, record index, unused, line flags
    @location(1) color: vec4<f32>,
    @location(2) params: vec4<f32>,  // width in pixels
}

// A box given by a centre and three half axes (world space).
struct OrientedBox {
    center: vec3<f32>,
    ax: vec3<f32>,
    ay: vec3<f32>,
    az: vec3<f32>,
    rectOnly: bool,
    valid: bool,
}

// Where a card is drawn: its centre, pulled out along its normal by the
// display-only spread (overlayParams.w) so that cards can be seen apart.
fn shownCardCenter(card: CardData) -> vec3<f32> {
    return card.center + card.tangentFrame[2] * gFrame.overlayParams.w;
}

fn sdfObject() -> ObjectData {
    return gObjects[u32(gFrame.sdfVolume.w)];
}

fn recordBox(rid: vec4<u32>) -> OrientedBox {
    var b = OrientedBox(vec3<f32>(0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), false, true);
    if (rid.x == BOX_OBJECT_OBB) {
        let o = gObjects[rid.y];
        b.center = transformPoint(o.localToWorld, o.localCenter);
        b.ax = transformVector(o.localToWorld, vec3<f32>(o.localExtent.x, 0.0, 0.0));
        b.ay = transformVector(o.localToWorld, vec3<f32>(0.0, o.localExtent.y, 0.0));
        b.az = transformVector(o.localToWorld, vec3<f32>(0.0, 0.0, o.localExtent.z));
    } else if (rid.x == BOX_OBJECT_AABB) {
        let o = gObjects[rid.y];
        b.center = o.worldCenter;
        b.ax = vec3<f32>(o.worldExtent.x, 0.0, 0.0);
        b.ay = vec3<f32>(0.0, o.worldExtent.y, 0.0);
        b.az = vec3<f32>(0.0, 0.0, o.worldExtent.z);
    } else if (rid.x == BOX_VIRTUAL_BRICK) {
        let brick = gBricks[rid.y];
        let o = sdfObject();
        let span = 7.0 * brick.voxelSize;
        let localMin = gFrame.sdfVolume.xyz + vec3<f32>(brick.virtualBrick) * span;
        b.center = transformPoint(o.localToWorld, localMin + span * 0.5);
        b.ax = transformVector(o.localToWorld, vec3<f32>(span * 0.5, 0.0, 0.0));
        b.ay = transformVector(o.localToWorld, vec3<f32>(0.0, span * 0.5, 0.0));
        b.az = transformVector(o.localToWorld, vec3<f32>(0.0, 0.0, span * 0.5));
    } else if (rid.x == BOX_PHYSICAL_BRICK) {
        let brick = gBricks[rid.y];
        let slot = gFrame.atlasDisplay.w;
        b.center = gFrame.atlasDisplay.xyz + (vec3<f32>(brick.physicalBrick) + 0.5) * slot;
        let h = slot * 0.46;
        b.ax = vec3<f32>(h, 0.0, 0.0);
        b.ay = vec3<f32>(0.0, h, 0.0);
        b.az = vec3<f32>(0.0, 0.0, h);
        b.valid = brick.physicalIndex != NO_HIT;
    } else if (rid.x == BOX_CARD_VOLUME || rid.x == BOX_CARD_RECT) {
        let card = gCards[rid.y];
        b.center = shownCardCenter(card);
        b.ax = card.tangentFrame[0] * card.halfExtents.x;
        b.ay = card.tangentFrame[1] * card.halfExtents.y;
        b.az = card.tangentFrame[2] * card.halfExtents.z;
        if (rid.x == BOX_CARD_VOLUME) {
            b.center -= b.az;
        }
        b.rectOnly = rid.x == BOX_CARD_RECT;
    } else if (rid.x == BOX_PAGE_RECT) {
        let page = gPages[rid.y];
        let card = gCards[page.cardIndex];
        let uvMid = (page.cardUVRect.xy + page.cardUVRect.zw) * 0.5;
        let uvHalf = (page.cardUVRect.zw - page.cardUVRect.xy) * 0.5;
        let tx = card.tangentFrame[0] * card.halfExtents.x;
        let ty = card.tangentFrame[1] * card.halfExtents.y;
        // Card UV: u along +X, v along -Y.
        b.center = shownCardCenter(card) + tx * (uvMid.x * 2.0 - 1.0) - ty * (uvMid.y * 2.0 - 1.0) + card.tangentFrame[2] * 0.004;
        // tx spans half the card (UV 0..1 is -tx..+tx), so a page's half size is tx * 2 * uvHalf.
        b.ax = tx * (2.0 * uvHalf.x);
        b.ay = ty * (2.0 * uvHalf.y);
        b.az = vec3<f32>(0.0);
        b.rectOnly = true;
    }
    return b;
}

fn cornerSigns(c: u32) -> vec3<f32> {
    return vec3<f32>(select(-1.0, 1.0, (c & 1u) != 0u), select(-1.0, 1.0, (c & 2u) != 0u), select(-1.0, 1.0, (c & 4u) != 0u));
}

@vertex
fn boxVS(input: BoxInstance, @builtin(vertex_index) vertexId: u32) -> LineVaryings {
    // 12 edges x 9 vertices. Edge e joins corners a and b (bit i = sign on axis i).
    var edges = array<vec2<u32>, 12>(
        vec2<u32>(0u, 1u), vec2<u32>(2u, 3u), vec2<u32>(4u, 5u), vec2<u32>(6u, 7u),
        vec2<u32>(0u, 2u), vec2<u32>(1u, 3u), vec2<u32>(4u, 6u), vec2<u32>(5u, 7u),
        vec2<u32>(0u, 4u), vec2<u32>(1u, 5u), vec2<u32>(2u, 6u), vec2<u32>(3u, 7u));
    let edge = edges[vertexId / 9u];
    let b = recordBox(input.rid);
    var seg: Segment;
    seg.color = input.color;
    // A rectangle keeps only the four edges of its z = -1 face.
    seg.valid = b.valid && !(b.rectOnly && ((edge.x & 4u) != 0u || (edge.y & 4u) != 0u));
    let sa = cornerSigns(edge.x);
    let sb = cornerSigns(edge.y);
    let az = select(b.az, vec3<f32>(0.0), b.rectOnly);
    seg.p0 = b.center + b.ax * sa.x + b.ay * sa.y + az * sa.z;
    seg.p1 = b.center + b.ax * sb.x + b.ay * sb.y + az * sb.z;
    return expandSegment(seg, input.params.x, input.rid.w, vertexId % 9u);
}

@fragment
fn lineFS(input: LineVaryings) -> @location(0) vec4<f32> {
    if ((input.flags & FLAG_DASHED) != 0u && fract(input.along / (14.0 * gFrame.overlayParams.y)) > 0.55) {
        discard;
    }
    return vec4<f32>(input.color.rgb * input.color.a, input.color.a);
}

// ---------------------------------------------------------------------------
// Sample cubes: dense voxel samples and decoded brick samples.

const SAMPLE_VOXEL: u32 = 0u;          // rid.y = VoxelSample index; rid.z = 1 colours by the stored byte
const SAMPLE_BRICK_VIRTUAL: u32 = 1u;  // rid.y = BrickEntry, rid.z = sample 0..511
const SAMPLE_BRICK_ATLAS: u32 = 2u;    // the same sample, placed in the atlas display

struct SampleInstance {
    @location(0) rid: vec4<u32>,
    @location(1) params: vec4<f32>,  // cube size, alpha, band for colouring, highlight
}

struct SampleVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

// Decode one brick sample exactly as SampleSparseMeshSignedDistanceField
// does: encoded * DistanceFieldToVolumeScaleBias.x + ...y.
fn decodeBrickSample(brick: BrickEntry, v: vec3<u32>) -> f32 {
    let encoded = textureLoad(gBrickAtlas, vec3<i32>(brick.physicalBrick * 8u + v), 0).r;
    return encoded * brick.distanceScaleBias.x + brick.distanceScaleBias.y;
}

@vertex
fn sampleVS(input: SampleInstance, @builtin(vertex_index) vertexId: u32) -> SampleVaryings {
    var o: SampleVaryings;
    o.clip = offscreen();
    o.color = vec4<f32>(0.0);
    var center: vec3<f32>;
    var dist: f32;
    var local = vec3<f32>(0.0);
    var cutTest = true;
    if (input.rid.x == SAMPLE_VOXEL) {
        let s = gVoxels[input.rid.y];
        local = s.localPosition;
        dist = s.signedDistance;
        if (input.rid.z == 1u) {
            // What the builder stores: +band beyond the band, inside or out.
            dist = f32(s.encoded) / 255.0 * 2.0 * input.params.z - input.params.z;
        }
        center = transformPoint(sdfObject().localToWorld, local);
    } else {
        let brick = gBricks[input.rid.y];
        let i = input.rid.z;
        let v = vec3<u32>(i % 8u, (i / 8u) % 8u, i / 64u);
        dist = decodeBrickSample(brick, v);
        if (input.rid.x == SAMPLE_BRICK_VIRTUAL) {
            local = gFrame.sdfVolume.xyz + vec3<f32>(brick.virtualBrick * 7u + v) * brick.voxelSize;
            center = transformPoint(sdfObject().localToWorld, local);
        } else {
            let slot = gFrame.atlasDisplay.w;
            center = gFrame.atlasDisplay.xyz + (vec3<f32>(brick.physicalBrick) + (vec3<f32>(v) + 0.5) / 8.0) * slot;
            cutTest = false;
        }
    }
    if (cutTest && dot(gFrame.cutPlane.xyz, local) > gFrame.cutPlane.w) {
        return o;
    }
    // 36 vertices: 6 faces x 2 triangles.
    let face = vertexId / 6u;
    var quad = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let q = quad[vertexId % 6u];
    let axis = face / 2u;
    let side = select(-1.0, 1.0, (face & 1u) != 0u);
    var n = vec3<f32>(0.0, 0.0, side);
    if (axis == 0u) {
        n = vec3<f32>(side, 0.0, 0.0);
    } else if (axis == 1u) {
        n = vec3<f32>(0.0, side, 0.0);
    }
    let a = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), axis == 0u);
    let bvec = cross(n, a);
    let p = center + (n + a * q.x + bvec * q.y) * (input.params.x * 0.5);
    var color = distanceColor(dist / max(input.params.z, 1e-6));
    color = mix(color, vec3<f32>(1.0), input.params.w * 0.35);
    let shade = 0.72 + 0.28 * dot(n, normalize(vec3<f32>(0.4, 0.8, 0.45)));
    o.color = vec4<f32>(color * shade, input.params.y);
    o.clip = gFrame.camera.worldToClip * vec4<f32>(p, 1.0);
    return o;
}

@fragment
fn sampleFS(input: SampleVaryings) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color.rgb * input.color.a, input.color.a);
}

// ---------------------------------------------------------------------------
// Card quads, textured through the page table:
// card UV -> virtual page -> PageEntry.physicalPage -> Surface Cache atlas,
// showing the picture of one atlas layer.

struct CardInstance {
    @location(0) rid: vec4<u32>,     // card index, highlighted page, 1 = the whole card is selected, unused
    @location(1) params: vec4<f32>,  // alpha, page-grid strength, layer (src/lab/surface_cache.rs Layer), unused
}

struct CardVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) cardUV: vec2<f32>,
    @location(1) @interpolate(flat) rid: vec4<u32>,
    @location(2) @interpolate(flat) params: vec4<f32>,
    // 1 when the camera sees the card's front (the side it captures from).
    @location(3) @interpolate(flat) front: f32,
}

@vertex
fn cardVS(input: CardInstance, @builtin(vertex_index) vertexId: u32) -> CardVaryings {
    let card = gCards[input.rid.x];
    var quad = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0));
    let uv = quad[vertexId];
    let tx = card.tangentFrame[0] * card.halfExtents.x;
    let ty = card.tangentFrame[1] * card.halfExtents.y;
    let center = shownCardCenter(card);
    let p = center + tx * (uv.x * 2.0 - 1.0) - ty * (uv.y * 2.0 - 1.0);
    var o: CardVaryings;
    o.clip = gFrame.camera.worldToClip * vec4<f32>(p, 1.0);
    o.cardUV = uv;
    o.rid = input.rid;
    o.params = input.params;
    o.front = select(0.0, 1.0, dot(card.tangentFrame[2], gFrame.camera.position - center) > 0.0);
    return o;
}

struct AtlasLookup {
    uv: vec2<f32>,
    pageTableIndex: u32,
}

// The same addressing as a surface-cache lookup (surfaceCacheCoord in
// src/commercial/shaders/surface_lookup.wgsl), as a UV of the layer pictures.
fn cardUVToAtlasUV(card: CardData, cardUV: vec2<f32>) -> AtlasLookup {
    let uv = clamp(cardUV, vec2<f32>(0.0), vec2<f32>(0.999999));
    let pageCoord = min(vec2<u32>(uv * vec2<f32>(card.sizeInPages)), card.sizeInPages - vec2<u32>(1u));
    let index = card.pageIndex + pageCoord.y * card.sizeInPages.x + pageCoord.x;
    let texel = surfaceCacheCoord(card, cardUV);
    return AtlasLookup(texel / vec2<f32>(textureDimensions(gSurfaceDisplay).xy), index);
}

@fragment
fn cardFS(input: CardVaryings) -> @location(0) vec4<f32> {
    let card = gCards[input.rid.x];
    let pageCoordF = input.cardUV * vec2<f32>(card.sizeInPages);
    let gridWidth = fwidth(pageCoordF);
    let uvWidth = fwidth(input.cardUV);
    let lookup = cardUVToAtlasUV(card, input.cardUV);
    // The atlas texture is sRGB, so the sample is linear. Its alpha is the
    // capture's coverage, zero where the card saw past its mesh; filtering
    // mixes covered and empty texels as premultiplied colour.
    let texel = textureSampleLevel(gSurfaceDisplay, gLinearSampler, lookup.uv, 0.0);
    let coverage = texel.a;
    var color = mix(srgbToLinear(vec3<f32>(0.04, 0.07, 0.11)), texel.rgb / max(coverage, 1e-4), coverage);
    let f = abs(fract(pageCoordF + 0.5) - 0.5) / max(gridWidth, vec2<f32>(1e-6));
    let grid = saturate(1.6 - min(f.x, f.y));
    let edge = min(input.cardUV, 1.0 - input.cardUV) / max(uvWidth, vec2<f32>(1e-6));
    let border = saturate(2.2 - min(edge.x, edge.y));
    color = mix(color, srgbToLinear(vec3<f32>(0.95, 0.97, 1.0)), grid * input.params.y);
    // Empty texels keep a faint fill, so the whole rectangle reads as the card.
    var opacity = max(mix(0.22, 1.0, coverage), max(grid * input.params.y, border));
    if (lookup.pageTableIndex == input.rid.y) {
        // palette::PAGE (#ffd166)
        color = mix(color, srgbToLinear(vec3<f32>(1.0, 0.8196, 0.4)), 0.35 + 0.65 * saturate(grid * 2.0 + border));
        opacity = max(opacity, 0.6);
    } else if (input.rid.z == 1u) {
        // The selected card, evenly over all its pages: palette::CARD (#b6e3ff)
        color = mix(color, srgbToLinear(vec3<f32>(0.714, 0.890, 1.0)), 0.2 + 0.8 * border);
        opacity = max(opacity, 0.4);
    }
    color = mix(color, srgbToLinear(vec3<f32>(0.85, 0.92, 1.0)), border * 0.8);
    // Seen from behind, a card shows its capture mirrored; keep it faint.
    let alpha = input.params.x * opacity * mix(0.35, 1.0, input.front);
    return vec4<f32>(linearToSrgb(color) * alpha, alpha);
}

// ---------------------------------------------------------------------------
// Probes and point markers: screen-facing discs.

const POINT_EXPLICIT: u32 = 0u;  // position as given
const POINT_RAY: u32 = 1u;       // rid.y ray, rid.z: 0 hit, 1 reflected hit, 2 origin
const POINT_PROBE: u32 = 2u;     // rid.y probe; colour from irradiance when color.a < 0

struct PointInstance {
    @location(0) position: vec3<f32>,
    @location(1) radius: f32,
    @location(2) color: vec4<f32>,
    @location(3) rid: vec4<u32>,
}

struct PointVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) offset: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) radius: f32,
}

@vertex
fn pointVS(input: PointInstance, @builtin(vertex_index) vertexId: u32) -> PointVaryings {
    var p = input.position;
    var color = input.color;
    if (input.rid.x == POINT_RAY) {
        let ray = gRays[input.rid.y];
        if (input.rid.z == 0u) {
            p = ray.hitPosition;
        } else if (input.rid.z == 1u) {
            p = reflectedEnd(ray);
        } else {
            p = ray.origin;
        }
    } else if (input.rid.x == POINT_PROBE) {
        let probe = gProbes[input.rid.y];
        p = probe.position + probe.normal * 0.012;
        if (color.a < 0.0) {
            // Irradiance (E/pi) is dim; probes display it at 3x (demos::PROBE_DISPLAY_GAIN).
            color = vec4<f32>(linearToSrgb(tonemap(probe.irradiance.rgb * 3.0)), -color.a);
        }
    }
    var quad = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let q = quad[vertexId];
    var clip = gFrame.camera.worldToClip * vec4<f32>(p, 1.0);
    let halfSize = gFrame.camera.viewportSize * 0.5;
    let r = input.radius * gFrame.overlayParams.y;
    clip = vec4<f32>(clip.xy + q * r / halfSize * clip.w, clip.z, clip.w);
    var o: PointVaryings;
    o.clip = select(offscreen(), clip, clip.w > 1e-3);
    o.offset = q * r;
    o.color = color;
    o.radius = r;
    return o;
}

@fragment
fn pointFS(input: PointVaryings) -> @location(0) vec4<f32> {
    let r = length(input.offset);
    let aa = fwidth(r);
    if (r > input.radius) {
        discard;
    }
    let ringWidth = 3.0 * gFrame.overlayParams.y;
    let ring = smoothstep(input.radius - ringWidth - aa, input.radius - ringWidth, r);
    let c = mix(input.color.rgb, vec3<f32>(0.03, 0.06, 0.1), ring * 0.85);
    return vec4<f32>(c * input.color.a, input.color.a);
}

// ---------------------------------------------------------------------------
// Ghost shell: a translucent rim-lit copy of a scene object, used when a demo
// needs to look inside it (the opaque copy is hidden in the scene pass).

struct GhostVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) objectIndex: u32,
}

struct GhostVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) worldPosition: vec3<f32>,
    @location(1) worldNormal: vec3<f32>,
}

@vertex
fn ghostVS(input: GhostVertex) -> GhostVaryings {
    let o = gObjects[input.objectIndex];
    var v: GhostVaryings;
    v.worldPosition = transformPoint(o.localToWorld, input.position);
    v.worldNormal = normalToWorld(o, input.normal);
    v.clip = gFrame.camera.worldToClip * vec4<f32>(v.worldPosition, 1.0);
    return v;
}

@fragment
fn ghostFS(input: GhostVaryings) -> @location(0) vec4<f32> {
    let n = normalize(input.worldNormal);
    let v = normalize(gFrame.camera.position - input.worldPosition);
    let rim = pow(1.0 - abs(dot(n, v)), 2.0);
    let a = (0.06 + 0.5 * rim) * gFrame.deformParams.z;
    return vec4<f32>(vec3<f32>(0.75, 0.9, 1.0) * a, a);
}
