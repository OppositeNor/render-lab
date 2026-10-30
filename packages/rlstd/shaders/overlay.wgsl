// Debug overlays: wire boxes, vectors, points, and ghost shells. Boxes and
// axes of objects are built on the GPU from the same records the struct
// panel displays: a drawn box is literally ObjectData.worldCenter/worldExtent.
//
// Overlays render into their own multisampled RGBA8 layer that shares the
// scene's depth buffer. Colours are display-referred sRGB values written
// with premultiplied alpha; the composite pass lays the layer over the
// tonemapped scene, so overlay colours match the struct panel exactly.

const FLAG_ARROW: u32 = 1u;
const FLAG_DASHED: u32 = 2u;

// Segment kinds (SegmentInstance.rid.x)
const SEG_EXPLICIT: u32 = 0u;     // p0 -> p1 as given
const SEG_OBJECT_AXIS: u32 = 1u;  // rid.y object, rid.z column of localToWorld; p0.x = length

// Box kinds (BoxInstance.rid.x)
const BOX_OBJECT_OBB: u32 = 0u;     // local bounds through localToWorld
const BOX_OBJECT_AABB: u32 = 1u;    // worldCenter +/- worldExtent

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

fn recordSegment(rid: vec4<u32>, p0: vec3<f32>, p1: vec3<f32>, color: vec4<f32>) -> Segment {
    var s = Segment(p0, p1, color, true);
    if (rid.x == SEG_OBJECT_AXIS) {
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
    valid: bool,
}

fn recordBox(rid: vec4<u32>) -> OrientedBox {
    var b = OrientedBox(vec3<f32>(0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), true);
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
    seg.valid = b.valid;
    let sa = cornerSigns(edge.x);
    let sb = cornerSigns(edge.y);
    seg.p0 = b.center + b.ax * sa.x + b.ay * sa.y + b.az * sa.z;
    seg.p1 = b.center + b.ax * sb.x + b.ay * sb.y + b.az * sb.z;
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
// Point markers: screen-facing discs.


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
    let p = input.position;
    let color = input.color;
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
    let a = (0.06 + 0.5 * rim) * gFrame.viewParams.z;
    return vec4<f32>(vec3<f32>(0.75, 0.9, 1.0) * a, a);
}
