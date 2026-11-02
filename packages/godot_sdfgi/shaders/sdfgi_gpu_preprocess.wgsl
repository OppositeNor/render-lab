// Godot's SDFGI pre-process (sdfgi_preprocess.glsl, dispatched by
// SDFGI::render_region, gi.cpp:2061-2418), ported to WGSL as the lab
// program's passes (src/godot/sdfgi/gpu.rs): the scroll of the kept cells
// and occlusion, the half-size jump flood (initialize, regular steps,
// shared-memory steps, upscale, one full-size step), the probe occlusion, and
// STORE (the distance field, the packed occlusion, and the solid-cell list
// with its indirect arguments). Each entry point binds its own group-3
// resources, named after the pass (gpu.rs PASSES gives the bindings);
// group 2 binding 0 is gSdfgiParams (sdfgi_gpu_common.wgsl), the push constant
// `Params` (sdfgi_preprocess.glsl:158-171):
//   [0..2] scroll, [3] grid_size, [4..6] probe_offset, [7] step_size,
//   [8] half_size, [9] occlusion_index, [10] cascade.
//
// Where the lab differs: `dst_occlusion[8]` (an image array) becomes eight
// images (bindings), one per pass instance in the occlusion pass, a switch in
// the scroll; a shared-memory read outside the occlusion cache (an index
// Godot leaves undefined) is clamped by wgpu's bounds checks.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md).

const SDFGI_OCCLUSION_SIZE: i32 = 8;

fn sdfgiPreScroll() -> vec3<i32> {
    return sdfgiParamI3(0u);
}

fn sdfgiPreGridSize() -> i32 {
    return sdfgiParamI(3u);
}

fn sdfgiPreProbeOffset() -> vec3<i32> {
    return sdfgiParamI3(4u);
}

fn sdfgiPreStepSize() -> i32 {
    return sdfgiParamI(7u);
}

fn sdfgiPreHalfSize() -> bool {
    return sdfgiParam(8u) != 0u;
}

fn sdfgiPreCascade() -> i32 {
    return sdfgiParamI(10u);
}

fn sdfgiInGrid(p: vec3<i32>) -> bool {
    return all(p >= vec3<i32>(0)) && all(p < vec3<i32>(sdfgiPreGridSize()));
}

// `(ivec3(i) >> ivec3(0, 1, 2)) & ivec3(1)`: the corner of a 2x2x2 block.
fn sdfgiCorner(i: i32) -> vec3<i32> {
    return (vec3<i32>(i) >> vec3<u32>(0u, 1u, 2u)) & vec3<i32>(1);
}

// `occlusion_shift` (sdfgi_preprocess.glsl:219, 973).
fn sdfgiOcclusionShift(i: u32) -> u32 {
    var shifts = array<u32, 8>(12u, 8u, 4u, 0u, 28u, 24u, 20u, 16u);
    return shifts[i];
}

// GLSL's sign() of an ivec3.
fn sdfgiSignI(v: vec3<i32>) -> vec3<i32> {
    return select(select(vec3<i32>(0), vec3<i32>(1), v > vec3<i32>(0)), vec3<i32>(-1), v < vec3<i32>(0));
}

// ---------------------------------------------------------------------------
// MODE_SCROLL (sdfgi_preprocess.glsl:174-201): the cascade's old solid cells,
// one thread each (an indirect dispatch over the old list), written back into
// the working images at their scrolled position.

@group(3) @binding(1) var gScrollAlbedo: texture_storage_3d<r16uint, write>;
@group(3) @binding(2) var gScrollFacing: texture_storage_3d<r32uint, write>;
@group(3) @binding(3) var gScrollLight: texture_storage_3d<r32uint, write>;
@group(3) @binding(4) var gScrollLightAniso: texture_storage_3d<r32uint, write>;
@group(3) @binding(5) var<storage, read> gScrollDispatch: SdfgiDispatchData;
@group(3) @binding(6) var<storage, read> gScrollVoxels: array<SdfgiProcessVoxel>;

@compute @workgroup_size(64, 1, 1)
fn sdfgiScrollCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Pixel being shaded.
    let index = gid.x;
    if (index >= gScrollDispatch.totalCount) {
        return; // too big
    }
    let voxel = gScrollVoxels[index];
    let readPos = (vec3<i32>(i32(voxel.position)) >> vec3<u32>(0u, 7u, 14u)) & vec3<i32>(0x7F);
    let writePos = readPos + sdfgiPreScroll();
    if (any(writePos < vec3<i32>(0)) || any(writePos >= vec3<i32>(sdfgiPreGridSize()))) {
        return; // Fits outside the 3D texture, don't do anything.
    }
    let albedo = ((voxel.albedo & 0x7FFFu) << 1u) | 1u; // add solid bit
    textureStore(gScrollAlbedo, writePos, vec4<u32>(albedo));
    let facing = (voxel.albedo >> 15u) & 0x3Fu; // 6 anisotropic facing bits
    textureStore(gScrollFacing, writePos, vec4<u32>(facing));
    let light = voxel.light & 0x3fffffffu; // 30 bits of RGBE8985
    textureStore(gScrollLight, writePos, vec4<u32>(light));
    let lightAniso = voxel.lightAniso & 0x3fffffffu; // 30 bits of 6 anisotropic 5 bits values
    textureStore(gScrollLightAniso, writePos, vec4<u32>(lightAniso));
}

// ---------------------------------------------------------------------------
// MODE_SCROLL_OCCLUSION (sdfgi_preprocess.glsl:203-225): the cascade's packed
// occlusion (eight 4-bit values per cell) back into the eight working images,
// shifted by the scroll.

@group(3) @binding(1) var gScrollOcc0: texture_storage_3d<r8unorm, write>;
@group(3) @binding(2) var gScrollOcc1: texture_storage_3d<r8unorm, write>;
@group(3) @binding(3) var gScrollOcc2: texture_storage_3d<r8unorm, write>;
@group(3) @binding(4) var gScrollOcc3: texture_storage_3d<r8unorm, write>;
@group(3) @binding(5) var gScrollOcc4: texture_storage_3d<r8unorm, write>;
@group(3) @binding(6) var gScrollOcc5: texture_storage_3d<r8unorm, write>;
@group(3) @binding(7) var gScrollOcc6: texture_storage_3d<r8unorm, write>;
@group(3) @binding(8) var gScrollOcc7: texture_storage_3d<r8unorm, write>;
@group(3) @binding(9) var gScrollOccSrc: texture_3d<u32>;

fn sdfgiScrollOccStore(i: u32, p: vec3<i32>, v: vec4<f32>) {
    switch i {
        case 0u: { textureStore(gScrollOcc0, p, v); }
        case 1u: { textureStore(gScrollOcc1, p, v); }
        case 2u: { textureStore(gScrollOcc2, p, v); }
        case 3u: { textureStore(gScrollOcc3, p, v); }
        case 4u: { textureStore(gScrollOcc4, p, v); }
        case 5u: { textureStore(gScrollOcc5, p, v); }
        case 6u: { textureStore(gScrollOcc6, p, v); }
        default: { textureStore(gScrollOcc7, p, v); }
    }
}

@compute @workgroup_size(4, 4, 4)
fn sdfgiScrollOcclusionCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pos = vec3<i32>(gid);
    let scroll = sdfgiPreScroll();
    let grid = sdfgiPreGridSize();
    if (any(pos >= vec3<i32>(grid) - abs(scroll))) {
        return; // too large, do nothing
    }
    var readPos = pos + max(vec3<i32>(0), -scroll);
    let writePos = pos + max(vec3<i32>(0), scroll);
    readPos.z += sdfgiPreCascade() * grid;
    var occlusion = textureLoad(gScrollOccSrc, readPos, 0).r;
    readPos.x += grid;
    occlusion |= textureLoad(gScrollOccSrc, readPos, 0).r << 16u;
    for (var i = 0u; i < 8u; i++) {
        let o = f32((occlusion >> sdfgiOcclusionShift(i)) & 0xFu) / 15.0;
        sdfgiScrollOccStore(i, writePos, vec4<f32>(o));
    }
}

// ---------------------------------------------------------------------------
// MODE_INITIALIZE_JUMP_FLOOD_HALF (sdfgi_preprocess.glsl:245-273): each half
// cell seeds with one solid voxel of its 2x2x2 block, picked by the cell's
// parity ("so the half size jump flood has a bit better chance").

@group(3) @binding(1) var gInitColor: texture_3d<u32>;
@group(3) @binding(2) var gInitPositions: texture_storage_3d<rgba8uint, write>;

@compute @workgroup_size(4, 4, 4)
fn sdfgiJumpFloodInitCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pos = vec3<i32>(gid);
    let basePos = pos * 2;
    var closest: array<vec4<u32>, 8>;
    var closestCount = 0;
    for (var i = 0; i < 8; i++) {
        let srcPos = basePos + sdfgiCorner(i);
        let c = textureLoad(gInitColor, srcPos, 0).r;
        if ((c & 1u) != 0u) {
            closest[closestCount] = vec4<u32>(vec3<u32>(srcPos), 255u);
            closestCount++;
        }
    }
    if (closestCount == 0) {
        textureStore(gInitPositions, pos, vec4<u32>(0u));
    } else {
        let indexv = (pos & vec3<i32>(1)) * vec3<i32>(1, 2, 4);
        let index = (indexv.x | indexv.y | indexv.z) % closestCount;
        textureStore(gInitPositions, pos, closest[index]);
    }
}

// ---------------------------------------------------------------------------
// MODE_JUMPFLOOD (sdfgi_preprocess.glsl:275-349): the regular jump flood,
// "efficient for large steps, inefficient for small steps": each cell keeps
// the nearest seed of itself and its 26 neighbours `step_size` away.

@group(3) @binding(1) var gFloodSrc: texture_3d<u32>;
@group(3) @binding(2) var gFloodDst: texture_storage_3d<rgba8uint, write>;

// The 26 offsets of MODE_JUMPFLOOD (sdfgi_preprocess.glsl:303-329).
fn sdfgiFloodOffset(i: u32) -> vec3<i32> {
    // The 27 offsets in x-major order, the centre (13) skipped.
    let k = select(i32(i), i32(i) + 1, i >= 13u);
    return vec3<i32>(k / 9, (k / 3) % 3, k % 3) - vec3<i32>(1);
}

@compute @workgroup_size(4, 4, 4)
fn sdfgiJumpFloodCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pos = vec3<i32>(gid);
    let halfSize = sdfgiPreHalfSize();
    var posf = vec3<f32>(pos);
    if (halfSize) {
        posf = posf * 2.0 + 0.5;
    }
    var p = textureLoad(gFloodSrc, pos, 0);
    if (!halfSize && all(p == vec4<u32>(vec3<u32>(pos), 255u))) {
        textureStore(gFloodDst, pos, p);
        return; // points to itself and valid, nothing better can be done, just pass
    }
    var pDist = 0.0; // should not matter
    if (p.w != 0u) {
        pDist = distance(posf, vec3<f32>(p.xyz));
    }
    let step = sdfgiPreStepSize();
    for (var i = 0u; i < 26u; i++) {
        let ofs = pos + sdfgiFloodOffset(i) * step;
        if (any(ofs < vec3<i32>(0)) || any(ofs >= vec3<i32>(sdfgiPreGridSize()))) {
            continue;
        }
        let q = textureLoad(gFloodSrc, ofs, 0);
        if (q.w == 0u) {
            continue; // was not initialized yet, ignore
        }
        let qDist = distance(posf, vec3<f32>(q.xyz));
        if (p.w == 0u || qDist < pDist) {
            p = q; // just replace because current is unused
            pDist = qDist;
        }
    }
    textureStore(gFloodDst, pos, p);
}

// ---------------------------------------------------------------------------
// MODE_JUMPFLOOD_OPTIMIZED (sdfgi_preprocess.glsl:351-444): a workgroup is an
// 8x8x8 block of a lattice of spacing `step_size`; its 10x10x10 patch (the
// block and a one-point halo) is loaded into shared memory once, then each
// cell scans its 27 lattice neighbours there.

const SDFGI_GROUP_SIZE: i32 = 8;
const SDFGI_GROUP_SIDE: i32 = SDFGI_GROUP_SIZE + 2;

var<workgroup> gFloodGroupPositions: array<vec4<u32>, 1000>; // (8 + 2)^3, with margins

fn sdfgiGroupOffset(p: vec3<i32>) -> u32 {
    return u32(p.z * SDFGI_GROUP_SIDE * SDFGI_GROUP_SIDE + p.y * SDFGI_GROUP_SIDE + p.x);
}

@compute @workgroup_size(8, 8, 8)
fn sdfgiJumpFloodSharedCS(@builtin(workgroup_id) wid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let step = sdfgiPreStepSize();
    let grid = sdfgiPreGridSize();
    let groupOffset = vec3<i32>(wid) % step;
    let groupPos = groupOffset + (vec3<i32>(wid) / step) * vec3<i32>(SDFGI_GROUP_SIZE * step);
    // Load data into local group memory.
    let local = vec3<i32>(lid);
    if (all(local < vec3<i32>(SDFGI_GROUP_SIDE / 2))) {
        // Use this thread for loading: "this method uses less threads for this but its simpler and less divergent".
        let basePos = local * 2;
        for (var i = 0; i < 8; i++) {
            let loadPos = basePos + sdfgiCorner(i);
            let loadGlobalPos = groupPos + (loadPos - vec3<i32>(1)) * step;
            var q = vec4<u32>(0u); // unused
            if (all(loadGlobalPos >= vec3<i32>(0)) && all(loadGlobalPos < vec3<i32>(grid))) {
                q = textureLoad(gFloodSrc, loadGlobalPos, 0);
            }
            gFloodGroupPositions[sdfgiGroupOffset(loadPos)] = q;
        }
    }
    // Sync.
    workgroupBarrier();
    let globalPos = groupPos + local * step;
    if (any(globalPos < vec3<i32>(0)) || any(globalPos >= vec3<i32>(grid))) {
        return; // do nothing else, end here because outside range
    }
    let localPos = local + vec3<i32>(1);
    // Only makes sense if point is inside screen.
    var closest = vec4<u32>(0u);
    var closestDist = 0.0;
    var posf = vec3<f32>(globalPos);
    if (sdfgiPreHalfSize()) {
        posf = posf * 2.0 + 0.5;
    }
    // The 27 offsets, x-major, the centre included (sdfgi_preprocess.glsl:385-412).
    for (var k = 0; k < 27; k++) {
        let o = vec3<i32>(k / 9, (k / 3) % 3, k % 3) - vec3<i32>(1);
        let point = gFloodGroupPositions[sdfgiGroupOffset(localPos + o)];
        if (point.w == 0u) {
            continue; // was not initialized yet, ignore
        }
        let dist = distance(posf, vec3<f32>(point.xyz));
        if (closest.w == 0u || dist < closestDist) {
            closest = point;
            closestDist = dist;
        }
    }
    textureStore(gFloodDst, globalPos, closest);
}

// ---------------------------------------------------------------------------
// MODE_UPSCALE_JUMP_FLOOD (sdfgi_preprocess.glsl:446-482): back to full size;
// a solid voxel seeds itself, any other takes its half cell's seed, then the
// nearest solid voxel of that seed's 2x2x2 block.

@group(3) @binding(1) var gUpscaleColor: texture_3d<u32>;
@group(3) @binding(2) var gUpscaleHalf: texture_3d<u32>;
@group(3) @binding(3) var gUpscaleDst: texture_storage_3d<rgba8uint, write>;

@compute @workgroup_size(4, 4, 4)
fn sdfgiUpscaleCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pos = vec3<i32>(gid);
    let c = textureLoad(gUpscaleColor, pos, 0).r;
    var v: vec4<u32>;
    if ((c & 1u) != 0u) {
        // Bit set means this is solid.
        v = vec4<u32>(vec3<u32>(pos), 255u); // not zero means used
    } else {
        v = textureLoad(gUpscaleHalf, pos >> vec3<u32>(1u), 0);
        var d = length(vec3<f32>(vec3<i32>(v.xyz) - pos));
        let vbase = vec3<i32>(v.xyz - (v.xyz & vec3<u32>(1u)));
        // Search around if there is a better candidate from the same block.
        for (var i = 0; i < 8; i++) {
            let p = vbase + sdfgiCorner(i);
            let d2 = length(vec3<f32>(p - pos));
            if (d2 < d) {
                // Check valid distance before test so we avoid a read.
                let c2 = textureLoad(gUpscaleColor, p, 0).r;
                if ((c2 & 1u) != 0u) {
                    v = vec4<u32>(vec3<u32>(p), v.w);
                    d = d2;
                }
            }
        }
        // "could validate better position.."
    }
    textureStore(gUpscaleDst, pos, v);
}

// ---------------------------------------------------------------------------
// MODE_OCCLUSION (sdfgi_preprocess.glsl:484-946): one workgroup per probe of
// one parity class (this pass's instance is the class, its image the
// class's); the facing masks of the probe's 16^3 cube in shared memory, then
// visibility spread from the probe shell by shell (22 steps, a barrier each),
// solid voxels biased away, and each free voxel's visibility factor.

@group(3) @binding(1) var gOccDst: texture_storage_3d<r8unorm, read_write>;
@group(3) @binding(2) var gOccFacing: texture_3d<u32>;

// `group_size_offset` (sdfgi_preprocess.glsl:61): per shell, (count, offset) into `group_pos`.
var<private> gOccGroupSizeOffset: array<vec2<u32>, 11> = array<vec2<u32>, 11>(vec2<u32>(1u, 0u), vec2<u32>(3u, 1u), vec2<u32>(6u, 4u), vec2<u32>(10u, 10u), vec2<u32>(15u, 20u), vec2<u32>(21u, 35u), vec2<u32>(28u, 56u), vec2<u32>(36u, 84u), vec2<u32>(42u, 120u), vec2<u32>(46u, 162u), vec2<u32>(48u, 208u));

// `group_pos` (sdfgi_preprocess.glsl:62-73): the voxel offsets (x | y << 8 | z << 16) of the first 11 diagonal shells of one octant.
var<private> gOccGroupPos: array<u32, 256> = array<u32, 256>(0u,
    65536u, 256u, 1u,
    131072u, 65792u, 512u, 65537u, 257u, 2u,
    196608u, 131328u, 66048u, 768u, 131073u, 65793u, 513u, 65538u, 258u, 3u,
    262144u, 196864u, 131584u, 66304u, 1024u, 196609u, 131329u, 66049u, 769u, 131074u, 65794u, 514u, 65539u, 259u, 4u,
    327680u, 262400u, 197120u, 131840u, 66560u, 1280u, 262145u, 196865u, 131585u, 66305u, 1025u, 196610u, 131330u, 66050u, 770u, 131075u, 65795u, 515u, 65540u, 260u, 5u,
    393216u, 327936u, 262656u, 197376u, 132096u, 66816u, 1536u, 327681u, 262401u, 197121u, 131841u, 66561u, 1281u, 262146u, 196866u, 131586u, 66306u, 1026u, 196611u, 131331u, 66051u, 771u, 131076u, 65796u, 516u, 65541u, 261u, 6u,
    458752u, 393472u, 328192u, 262912u, 197632u, 132352u, 67072u, 1792u, 393217u, 327937u, 262657u, 197377u, 132097u, 66817u, 1537u, 327682u, 262402u, 197122u, 131842u, 66562u, 1282u, 262147u, 196867u, 131587u, 66307u, 1027u, 196612u, 131332u, 66052u, 772u, 131077u, 65797u, 517u, 65542u, 262u, 7u,
    459008u, 393728u, 328448u, 263168u, 197888u, 132608u, 67328u, 458753u, 393473u, 328193u, 262913u, 197633u, 132353u, 67073u, 1793u, 393218u, 327938u, 262658u, 197378u, 132098u, 66818u, 1538u, 327683u, 262403u, 197123u, 131843u, 66563u, 1283u, 262148u, 196868u, 131588u, 66308u, 1028u, 196613u, 131333u, 66053u, 773u, 131078u, 65798u, 518u, 65543u, 263u,
    459264u, 393984u, 328704u, 263424u, 198144u, 132864u, 459009u, 393729u, 328449u, 263169u, 197889u, 132609u, 67329u, 458754u, 393474u, 328194u, 262914u, 197634u, 132354u, 67074u, 1794u, 393219u, 327939u, 262659u, 197379u, 132099u, 66819u, 1539u, 327684u, 262404u, 197124u, 131844u, 66564u, 1284u, 262149u, 196869u, 131589u, 66309u, 1029u, 196614u, 131334u, 66054u, 774u, 131079u, 65799u, 519u,
    459520u, 394240u, 328960u, 263680u, 198400u, 459265u, 393985u, 328705u, 263425u, 198145u, 132865u, 459010u, 393730u, 328450u, 263170u, 197890u, 132610u, 67330u, 458755u, 393475u, 328195u, 262915u, 197635u, 132355u, 67075u, 1795u, 393220u, 327940u, 262660u, 197380u, 132100u, 66820u, 1540u, 327685u, 262405u, 197125u, 131845u, 66565u, 1285u, 262150u, 196870u, 131590u, 66310u, 1030u, 196615u, 131335u, 66055u, 775u);

// `occlusion_facing` (sdfgi_preprocess.glsl:75): the cube's facing masks, four per word.
var<workgroup> gOccFacingCache: array<u32, 1024>; // (8 * 2)^3 / 4

// get_facing (sdfgi_preprocess.glsl:77-81).
fn sdfgiGetFacing(p: vec3<i32>) -> u32 {
    let ofs = u32(p.z * SDFGI_OCCLUSION_SIZE * 2 * SDFGI_OCCLUSION_SIZE * 2 + p.y * SDFGI_OCCLUSION_SIZE * 2 + p.x);
    let v = gOccFacingCache[ofs / 4u];
    return (v >> ((ofs % 4u) * 8u)) & 0xFFu;
}

fn sdfgiOccLoad(p: vec3<i32>) -> f32 {
    return textureLoad(gOccDst, p).r;
}

// One read of the "bias solid voxels away" step (sdfgi_preprocess.glsl:701-712 and the six like it).
fn sdfgiOccBiasRead(localOffset: vec3<i32>, regionOffset: vec3<i32>, acc: ptr<function, vec2<f32>>) {
    var readOffset = localOffset;
    let f = sdfgiGetFacing(readOffset);
    if (f == 0u) {
        readOffset += regionOffset;
        if (sdfgiInGrid(readOffset)) {
            (*acc).x += sdfgiOccLoad(readOffset);
            (*acc).y += 1.0;
        }
    }
}

// One test of the visibility step (sdfgi_preprocess.glsl:842-854 and the five like it): (visible, occlude_total).
fn sdfgiOccVisRead(localOffset: vec3<i32>, regionOffset: vec3<i32>, readMask: u32, blockMask: u32, acc: ptr<function, vec2<f32>>) {
    var readOffset = localOffset;
    let mask = sdfgiGetFacing(readOffset);
    if (mask != 0u) {
        readOffset += regionOffset;
        if (sdfgiInGrid(readOffset)) {
            (*acc).y += 1.0;
            if ((mask & readMask) != 0u && (mask & blockMask) == 0u) {
                (*acc).x += 1.0;
            }
        }
    }
}

@compute @workgroup_size(64, 1, 1)
fn sdfgiOcclusionCS(@builtin(workgroup_id) wid: vec3<u32>, @builtin(local_invocation_index) invocationIdx: u32) {
    let region = vec3<i32>(wid);
    let scroll = sdfgiPreScroll();
    let grid = sdfgiPreGridSize();
    var regionOffset = -vec3<i32>(SDFGI_OCCLUSION_SIZE);
    regionOffset += region * SDFGI_OCCLUSION_SIZE * 2;
    regionOffset += sdfgiPreProbeOffset() * SDFGI_OCCLUSION_SIZE;
    var regionOutOfBounds = false;
    if (any(scroll != vec3<i32>(0))) {
        // Validate scroll region (sdfgi_preprocess.glsl:495-505).
        let regionOffsetTo = regionOffset + vec3<i32>(SDFGI_OCCLUSION_SIZE * 2);
        let scrollMask = select(vec3<u32>(0u), vec3<u32>(1u), scroll != vec3<i32>(0)); // save which axes are scrolling
        let scrollFrom = select(vec3<i32>(0), vec3<i32>(grid) + scroll, scroll < vec3<i32>(0));
        let scrollTo = select(vec3<i32>(grid), scroll, scroll > vec3<i32>(0));
        let out = (select(vec3<u32>(0u), vec3<u32>(1u), regionOffsetTo <= scrollFrom) | select(vec3<u32>(0u), vec3<u32>(1u), regionOffset >= scrollTo)) * scrollMask;
        if (all(out == scrollMask)) {
            regionOutOfBounds = true; // all axes that scroll are out: region outside scroll bounds, quit
        }
    }
    // OCC_HALF_SIZE = 4: each thread owns a 4x4x4 block of the cube.
    let ofsHalf = u32(SDFGI_OCCLUSION_SIZE / 2);
    let localOfs = vec3<i32>(vec3<u32>(invocationIdx % ofsHalf, (invocationIdx % (ofsHalf * ofsHalf)) / ofsHalf, invocationIdx / (ofsHalf * ofsHalf))) * 4;
    if (!regionOutOfBounds) {
        for (var i = 0; i < 16; i++) {
            // Skip x, so it can be packed.
            let offset = localOfs + ((vec3<i32>(i * 4) >> vec3<u32>(0u, 2u, 4u)) & vec3<i32>(3));
            var facingPack = 0u;
            for (var j = 0; j < 4; j++) {
                let foffset = regionOffset + offset + vec3<i32>(j, 0, 0);
                if (all(foffset >= vec3<i32>(0)) && all(foffset < vec3<i32>(grid))) {
                    let f = textureLoad(gOccFacing, foffset, 0).r;
                    facingPack |= f << u32(j * 8);
                }
            }
            gOccFacingCache[u32(offset.z * (SDFGI_OCCLUSION_SIZE * 2 * SDFGI_OCCLUSION_SIZE * 2) + offset.y * (SDFGI_OCCLUSION_SIZE * 2) + offset.x) / 4u] = facingPack;
        }
    }
    // Sync occlusion saved.
    workgroupBarrier();
    // Process occlusion: OCC_STEPS = 22 shells out from the probe and back (sdfgi_preprocess.glsl:535-657).
    let occSteps = SDFGI_OCCLUSION_SIZE * 3 - 2;
    let occHalfSteps = occSteps / 2;
    for (var step = 0; step < occSteps; step++) {
        if (!regionOutOfBounds) {
            let shrink = step >= occHalfSteps;
            let occStep = select(step, occHalfSteps - (step - occHalfSteps) - 1, shrink);
            if (invocationIdx < gOccGroupSizeOffset[occStep].x) {
                let pv = gOccGroupPos[gOccGroupSizeOffset[occStep].y + invocationIdx];
                var procAbs = (vec3<i32>(i32(pv)) >> vec3<u32>(0u, 8u, 16u)) & vec3<i32>(0xFF);
                if (shrink) {
                    procAbs = vec3<i32>(SDFGI_OCCLUSION_SIZE) - procAbs - vec3<i32>(1);
                }
                for (var i = 0; i < 8; i++) {
                    let bits = sdfgiCorner(i);
                    let procSign = bits * 2 - 1;
                    let localOffset = vec3<i32>(SDFGI_OCCLUSION_SIZE) + procAbs * procSign - (vec3<i32>(1) - bits);
                    let offset = localOffset + regionOffset;
                    if (all(offset >= vec3<i32>(0)) && all(offset < vec3<i32>(grid))) {
                        var occ: f32;
                        let facing = sdfgiGetFacing(localOffset);
                        if (facing != 0u) {
                            occ = 0.0; // solid
                        } else if (step == 0) {
                            occ = 1.0;
                        } else {
                            let readDir = -procSign;
                            var majorAxis: vec3<i32>;
                            if (procAbs.x < procAbs.y) {
                                if (procAbs.z < procAbs.y) {
                                    majorAxis = vec3<i32>(0, 1, 0);
                                } else {
                                    majorAxis = vec3<i32>(0, 0, 1);
                                }
                            } else {
                                if (procAbs.z < procAbs.x) {
                                    majorAxis = vec3<i32>(1, 0, 0);
                                } else {
                                    majorAxis = vec3<i32>(0, 0, 1);
                                }
                            }
                            var avg = 0.0;
                            occ = 0.0;
                            let readX = offset + vec3<i32>(readDir.x, 0, 0) + select(vec3<i32>(0), majorAxis * readDir, procAbs.x == 0);
                            let readY = offset + vec3<i32>(0, readDir.y, 0) + select(vec3<i32>(0), majorAxis * readDir, procAbs.y == 0);
                            let readZ = offset + vec3<i32>(0, 0, readDir.z) + select(vec3<i32>(0), majorAxis * readDir, procAbs.z == 0);
                            let facingX = sdfgiGetFacing(readX - regionOffset);
                            if (facingX == 0u) {
                                if (sdfgiInGrid(readX)) {
                                    occ += sdfgiOccLoad(readX);
                                    avg += 1.0;
                                }
                            } else if (procAbs.x != 0) {
                                avg += 1.0; // do not occlude from voxels in the opposite octant
                            }
                            let facingY = sdfgiGetFacing(readY - regionOffset);
                            if (facingY == 0u) {
                                if (sdfgiInGrid(readY)) {
                                    occ += sdfgiOccLoad(readY);
                                    avg += 1.0;
                                }
                            } else if (procAbs.y != 0) {
                                avg += 1.0;
                            }
                            let facingZ = sdfgiGetFacing(readZ - regionOffset);
                            if (facingZ == 0u) {
                                if (sdfgiInGrid(readZ)) {
                                    occ += sdfgiOccLoad(readZ);
                                    avg += 1.0;
                                }
                            } else if (procAbs.z != 0) {
                                avg += 1.0;
                            }
                            if (avg > 0.0) {
                                occ /= avg;
                            }
                        }
                        textureStore(gOccDst, offset, vec4<f32>(occ));
                    }
                }
            }
        }
        // The shell just written, visible to the next (the image is read back by the same group).
        textureBarrier();
        workgroupBarrier();
    }
    // Bias solid voxels away (sdfgi_preprocess.glsl:659-795).
    if (!regionOutOfBounds) {
        for (var i = 0; i < 64; i++) {
            let localOffset = localOfs + ((vec3<i32>(i) >> vec3<u32>(0u, 2u, 4u)) & vec3<i32>(3));
            let offset = regionOffset + localOffset;
            if (all(offset >= vec3<i32>(0)) && all(offset < vec3<i32>(grid))) {
                let facing = sdfgiGetFacing(localOffset);
                if (facing != 0u) {
                    // Only work on solids.
                    var procPos = localOffset - vec3<i32>(SDFGI_OCCLUSION_SIZE);
                    procPos += select(vec3<i32>(0), vec3<i32>(1), procPos >= vec3<i32>(0));
                    var acc = vec2<f32>(0.0); // (occ, avg)
                    let readDir = -sdfgiSignI(procPos);
                    let readDirX = vec3<i32>(readDir.x, 0, 0);
                    let readDirY = vec3<i32>(0, readDir.y, 0);
                    let readDirZ = vec3<i32>(0, 0, readDir.z);
                    let facingPos = (vec3<u32>(facing) >> vec3<u32>(0u, 1u, 2u)) & vec3<u32>(1u);
                    let facingNeg = (vec3<u32>(facing) >> vec3<u32>(3u, 4u, 5u)) & vec3<u32>(1u);
                    let readValid = select(facingNeg, facingPos, readDir > vec3<i32>(0)) != vec3<u32>(0u);
                    // Sides.
                    if (readValid.x) {
                        sdfgiOccBiasRead(localOffset + readDirX, regionOffset, &acc);
                    }
                    if (readValid.y) {
                        sdfgiOccBiasRead(localOffset + readDirY, regionOffset, &acc);
                    }
                    if (readValid.z) {
                        sdfgiOccBiasRead(localOffset + readDirZ, regionOffset, &acc);
                    }
                    // Adjacents.
                    if (readValid.y && readValid.z) {
                        sdfgiOccBiasRead(localOffset + readDirY + readDirZ, regionOffset, &acc);
                    }
                    if (readValid.x && readValid.z) {
                        sdfgiOccBiasRead(localOffset + readDirX + readDirZ, regionOffset, &acc);
                    }
                    if (readValid.x && readValid.y) {
                        sdfgiOccBiasRead(localOffset + readDirX + readDirY, regionOffset, &acc);
                    }
                    // Diagonal.
                    if (all(readValid)) {
                        sdfgiOccBiasRead(localOffset + readDir, regionOffset, &acc);
                    }
                    var occ = acc.x;
                    if (acc.y > 0.0) {
                        occ /= acc.y;
                    }
                    textureStore(gOccDst, offset, vec4<f32>(occ));
                }
            }
        }
    }
    textureBarrier();
    workgroupBarrier();
    // "There are no more barriers after this, so we can early return."
    if (regionOutOfBounds) {
        return;
    }
    // Each free voxel's visibility factor (sdfgi_preprocess.glsl:806-925).
    for (var i = 0; i < 64; i++) {
        let localOffset = localOfs + ((vec3<i32>(i) >> vec3<u32>(0u, 2u, 4u)) & vec3<i32>(3));
        let offset = regionOffset + localOffset;
        if (all(offset >= vec3<i32>(0)) && all(offset < vec3<i32>(grid))) {
            let facing = sdfgiGetFacing(localOffset);
            if (facing == 0u) {
                var procPos = localOffset - vec3<i32>(SDFGI_OCCLUSION_SIZE);
                procPos += select(vec3<i32>(0), vec3<i32>(1), procPos >= vec3<i32>(0));
                let procAbs = abs(procPos);
                let readDir = sdfgiSignI(procPos); // opposite direction
                let readDirX = vec3<i32>(readDir.x, 0, 0);
                let readDirY = vec3<i32>(0, readDir.y, 0);
                let readDirZ = vec3<i32>(0, 0, readDir.z);
                // Match positive with negative normals; `block_mask` is zeroed in the GLSL (dead).
                var readMask = select(vec3<u32>(1u, 2u, 4u), vec3<u32>(8u, 16u, 32u), readDir > vec3<i32>(0));
                var blockMask = vec3<u32>(0u);
                var acc = vec2<f32>(0.0); // (visible, occlude_total)
                if (procAbs.x < SDFGI_OCCLUSION_SIZE) {
                    sdfgiOccVisRead(localOffset + readDirX, regionOffset, readMask.x, blockMask.x, &acc);
                }
                if (procAbs.y < SDFGI_OCCLUSION_SIZE) {
                    sdfgiOccVisRead(localOffset + readDirY, regionOffset, readMask.y, blockMask.y, &acc);
                }
                if (procAbs.z < SDFGI_OCCLUSION_SIZE) {
                    sdfgiOccVisRead(localOffset + readDirZ, regionOffset, readMask.z, blockMask.z, &acc);
                }
                // If near the cartesian plane, test in opposite direction too (match negative with positive normals).
                readMask = select(vec3<u32>(1u, 2u, 4u), vec3<u32>(8u, 16u, 32u), readDir < vec3<i32>(0));
                blockMask = vec3<u32>(0u);
                if (procAbs.x == 1) {
                    sdfgiOccVisRead(localOffset - readDirX, regionOffset, readMask.x, blockMask.x, &acc);
                }
                if (procAbs.y == 1) {
                    sdfgiOccVisRead(localOffset - readDirY, regionOffset, readMask.y, blockMask.y, &acc);
                }
                if (procAbs.z == 1) {
                    sdfgiOccVisRead(localOffset - readDirZ, regionOffset, readMask.z, blockMask.z, &acc);
                }
                if (acc.y > 0.0) {
                    var occ = sdfgiOccLoad(offset);
                    occ *= acc.x / acc.y;
                    textureStore(gOccDst, offset, vec4<f32>(occ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MODE_STORE (sdfgi_preprocess.glsl:948-1063): the distance field (1 + the
// distance to the nearest seed, 0 on solid, over 255), the eight occlusions
// packed four bits each, and the solid cells compacted into the list: a
// shared counter gives each solid thread its slot in the group, one global
// atomic per group reserves the group's range ("to reduce pressure"), and
// atomicMax raises the indirect dispatch's group count.

@group(3) @binding(1) var gStorePositions: texture_3d<u32>;
@group(3) @binding(2) var gStoreAlbedo: texture_3d<u32>;
@group(3) @binding(3) var gStoreOcc0: texture_3d<f32>;
@group(3) @binding(4) var gStoreOcc1: texture_3d<f32>;
@group(3) @binding(5) var gStoreOcc2: texture_3d<f32>;
@group(3) @binding(6) var gStoreOcc3: texture_3d<f32>;
@group(3) @binding(7) var gStoreOcc4: texture_3d<f32>;
@group(3) @binding(8) var gStoreOcc5: texture_3d<f32>;
@group(3) @binding(9) var gStoreOcc6: texture_3d<f32>;
@group(3) @binding(10) var gStoreOcc7: texture_3d<f32>;
@group(3) @binding(11) var gStoreLight: texture_3d<u32>;
@group(3) @binding(12) var gStoreLightAniso: texture_3d<u32>;
@group(3) @binding(13) var gStoreFacing: texture_3d<u32>;
@group(3) @binding(14) var gStoreSdf: texture_storage_3d<r8unorm, write>;
@group(3) @binding(15) var gStoreOcclusion: texture_storage_3d<r16uint, write>;
@group(3) @binding(16) var<storage, read_write> gStoreDispatch: SdfgiDispatchDataAtomic;
@group(3) @binding(17) var<storage, read_write> gStoreVoxels: array<SdfgiProcessVoxel>;

var<workgroup> gStoreStaged: array<SdfgiProcessVoxel, 64>; // store_positions[4 * 4 * 4]
var<workgroup> gStoreCount: atomic<u32>; // store_position_count
var<workgroup> gStoreFrom: u32; // store_from_index

fn sdfgiStoreOcc(i: u32, p: vec3<i32>) -> f32 {
    switch i {
        case 0u: { return textureLoad(gStoreOcc0, p, 0).r; }
        case 1u: { return textureLoad(gStoreOcc1, p, 0).r; }
        case 2u: { return textureLoad(gStoreOcc2, p, 0).r; }
        case 3u: { return textureLoad(gStoreOcc3, p, 0).r; }
        case 4u: { return textureLoad(gStoreOcc4, p, 0).r; }
        case 5u: { return textureLoad(gStoreOcc5, p, 0).r; }
        case 6u: { return textureLoad(gStoreOcc6, p, 0).r; }
        default: { return textureLoad(gStoreOcc7, p, 0).r; }
    }
}

@compute @workgroup_size(4, 4, 4)
fn sdfgiStoreCS(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let local = vec3<i32>(lid);
    let pos = vec3<i32>(gid);
    let grid = sdfgiPreGridSize();
    // Store SDF.
    var p = textureLoad(gStorePositions, pos, 0);
    var solid = false;
    var d: f32;
    if (all(vec3<i32>(p.xyz) == pos)) {
        // Solid block.
        d = 0.0;
        solid = true;
    } else {
        // Distance block.
        d = 1.0 + length(vec3<f32>(p.xyz) - vec3<f32>(pos));
    }
    d /= 255.0;
    textureStore(gStoreSdf, pos, vec4<f32>(d));
    // Store occlusion.
    var occlusion = 0u;
    for (var i = 0u; i < 8u; i++) {
        let occ = sdfgiStoreOcc(i, pos);
        occlusion |= u32(clamp(occ * 15.0, 0.0, 15.0)) << sdfgiOcclusionShift(i);
    }
    {
        var occPos = pos;
        occPos.z += sdfgiPreCascade() * grid;
        textureStore(gStoreOcclusion, occPos, vec4<u32>(occlusion & 0xFFFFu));
        occPos.x += grid;
        textureStore(gStoreOcclusion, occPos, vec4<u32>(occlusion >> 16u));
    }
    // Store positions.
    if (all(local == vec3<i32>(0))) {
        atomicStore(&gStoreCount, 0u); // base one stores as zero, the others wait
    }
    workgroupBarrier();
    if (solid) {
        let index = atomicAdd(&gStoreCount, 1u);
        // At least do the conversion work in parallel.
        var record: SdfgiProcessVoxel;
        record.position = u32(pos.x | (pos.y << 7u) | (pos.z << 14u));
        // See around which voxels point to this one, add them to the list.
        var bitIndex = 0u;
        var neighbourBits = 0u;
        for (var i = -1; i <= 1; i++) {
            for (var j = -1; j <= 1; j++) {
                for (var k = -1; k <= 1; k++) {
                    if (i == 0 && j == 0 && k == 0) {
                        continue;
                    }
                    let npos = pos + vec3<i32>(i, j, k);
                    if (all(npos >= vec3<i32>(0)) && all(npos < vec3<i32>(grid))) {
                        p = textureLoad(gStorePositions, npos, 0);
                        if (all(vec3<i32>(p.xyz) == pos)) {
                            neighbourBits |= (1u << bitIndex);
                        }
                    }
                    bitIndex++;
                }
            }
        }
        let rgb = textureLoad(gStoreAlbedo, pos, 0).r;
        let facing = textureLoad(gStoreFacing, pos, 0).r;
        record.albedo = rgb >> 1u; // store as it comes (555) to avoid precision loss (and move away the alpha bit)
        record.albedo |= (facing & 0x3Fu) << 15u; // store facing in bits 15-21
        record.albedo |= neighbourBits << 21u; // store lower 11 bits of neighbors with remaining albedo
        record.position |= (neighbourBits >> 11u) << 21u; // store 11 bits more of neighbors with position
        record.light = textureLoad(gStoreLight, pos, 0).r;
        record.lightAniso = textureLoad(gStoreLightAniso, pos, 0).r;
        // Add neighbors.
        record.light |= (neighbourBits >> 22u) << 30u; // store 2 bits more of neighbors with light
        record.lightAniso |= (neighbourBits >> 24u) << 30u; // store 2 bits more of neighbors with aniso
        gStoreStaged[index] = record;
    }
    workgroupBarrier();
    // Global increment only once per group, to reduce pressure.
    let count = atomicLoad(&gStoreCount);
    if (all(local == vec3<i32>(0)) && count > 0u) {
        gStoreFrom = atomicAdd(&gStoreDispatch.totalCount, count);
        let groupCount = (gStoreFrom + count - 1u) / 64u + 1u;
        atomicMax(&gStoreDispatch.x, groupCount);
    }
    workgroupBarrier();
    let readIndex = u32(local.z * 4 * 4 + local.y * 4 + local.x);
    let writeIndex = gStoreFrom + readIndex;
    if (readIndex < count) {
        gStoreVoxels[writeIndex] = gStoreStaged[readIndex];
    }
    if (all(pos == vec3<i32>(0))) {
        // This thread clears y and z.
        atomicStore(&gStoreDispatch.y, 1u);
        atomicStore(&gStoreDispatch.z, 1u);
    }
}
