"""Write packages/godot_ssr/source-index.json from the Godot checkout.

Every entry names a file and a token; the line is the token's only
occurrence in the file, so no line number is typed by hand. A token
`anchor >>> text` is the first line holding `text` after the anchor's only
line, for lines that repeat. Run it from anywhere after changing an entry or
moving to another Godot commit:

    python3 packages/godot_ssr/src/godot/ssr/source_index.py

The Godot source is GODOT_SOURCE_DIR, or where the user says when asked.
"""
import hashlib
import json
import os
import subprocess
from pathlib import Path

LAB = Path(__file__).resolve().parents[5]


def ask_checkout() -> Path:
    """The Godot source: GODOT_SOURCE_DIR, else where the user says."""
    given = os.environ.get("GODOT_SOURCE_DIR", "")
    if not given:
        try:
            given = input("Where is the Godot source (the checkout's root)? ").strip()
        except EOFError:
            raise SystemExit("set GODOT_SOURCE_DIR to the Godot source (the checkout's root)") from None
    path = Path(given).expanduser().resolve()
    if not (path / "version.py").is_file():
        raise SystemExit(f"{path}: not the Godot source (no version.py)")
    return path


CHECKOUT = ask_checkout()

RD = "servers/rendering/renderer_rd/"
RFC = RD + "forward_clustered/render_forward_clustered.cpp"
SSE = RD + "effects/ss_effects.cpp"
SH = RD + "shaders/effects/"
SSR = SH + "screen_space_reflection.glsl"
DOWN = SH + "screen_space_reflection_downsample.glsl"
HIZ = SH + "screen_space_reflection_hiz.glsl"
FILT = SH + "screen_space_reflection_filter.glsl"
RES = SH + "screen_space_reflection_resolve.glsl"
SCENE = RD + "shaders/forward_clustered/scene_forward_clustered.glsl"
BFN = RD + "shaders/forward_clustered/best_fit_normal.glsl"
ENVH = "scene/resources/environment.h"
RS = "servers/rendering/rendering_server.cpp"
SDATA = RD + "storage_rd/render_scene_data_rd.cpp"
LIGHTS = RD + "shaders/scene_forward_lights_inc.glsl"

Entry = tuple[str, str, str, str, str, list[tuple[str, str]]]

ENTRIES: list[Entry] = [
    # ---- Where it runs ----
    ("gating", RFC, "using_ssr = true;", "RenderForwardClustered::_render_scene",
     "SSR runs when the Environment enables it and the viewport's background is not transparent; a reflection probe's render never computes it.", []),
    ("ssr-normal-roughness", RFC, "if (using_ssr ||", "RenderForwardClustered::_render_scene",
     "Using SSR makes the depth pre-pass write the normal-roughness buffer too (PASS_MODE_DEPTH_NORMAL_ROUGHNESS).", []),
    ("process-ssr", RFC, "void RenderForwardClustered::_process_ssr", "RenderForwardClustered::_process_ssr",
     "Before the opaque pass: allocate the SSR buffers, build each view's reprojection matrix, keep this frame's camera for the next, and run the SSR passes.", []),
    ("reprojection", RFC, "reprojections[v] = ", "RenderForwardClustered::_process_ssr",
     "The reprojection: this frame's screen to view space (inverse projection), to the world (camera transform), to last frame's view (its inverse transform), to last frame's screen (its projection). Camera only: no motion vectors.", []),
    ("copy-last-frame", RFC, "_copy_framebuffer_to_ss_effects(rb, using_ssil, using_ssr);", "RenderForwardClustered::_render_scene",
     "After the transparent pass and the MSAA resolve, the internal HDR colour is copied into the last-frame texture that the next frame's trace reads.", []),
    ("resolve-flag", RFC, "ss_flags |= (rb_data.is_valid() && !rb_data->ss_effects_data.ssr.half_size)", "RenderForwardClustered::_setup_environment",
     "The flag named RESOLVE_SSR is set at full size, when no resolve pass ran: the opaque pass then picks the blur level and undoes the compression itself.", []),
    ("env-defaults", ENVH, "int ssr_max_steps = 64;", "Environment",
     "The Environment's SSR settings and defaults: off, 64 steps, fade in 0.15, fade out 2.0, depth tolerance 0.5.",
     [("bool ssr_enabled = false;", "Off by default."), ("float ssr_depth_tolerance = 0.5;", "The depth tolerance, in view-space units.")]),
    ("half-size-setting", RS, "rendering/environment/screen_space_reflection/half_size", "RenderingServer",
     "The project setting that runs SSR at half resolution, on by default.", []),
    # ---- Buffers ----
    ("ssr-size", SSE, "p_ssr_buffers.size = ssr_half_size", "SSEffects::ssr_allocate_buffers",
     "The SSR buffers are half the internal size (rounded down) at half size.", []),
    ("mip-count", SSE, "while (cur_width > 1 && cur_height > 1) {", "SSEffects::ssr_allocate_buffers",
     "The mip count: both sides halved while both are above one texel.", []),
    ("hiz-texture", SSE, "RB_HIZ, RD::DATA_FORMAT_R32_SFLOAT", "SSEffects::ssr_allocate_buffers",
     "The depth pyramid: R32_SFLOAT at the SSR size, with the full mip chain.", []),
    ("mip-level-texture", SSE, "RB_MIP_LEVEL, RD::DATA_FORMAT_R8_UNORM", "SSEffects::ssr_allocate_buffers",
     "The blur level per pixel: R8_UNORM, storing the level divided by 14.", []),
    ("last-frame-size", SSE, "last_frame_size /= 2;", "SSEffects::allocate_last_frame_buffer",
     "With SSR at half size and SSIL off, the last frame is kept at half size.", []),
    # ---- Downsample and pyramid ----
    ("pass-downsample", SSE, '"SSR Downsample"', "SSEffects::screen_space_reflection",
     "The half-size downsample of depth and normal-roughness (half size only), choosing a variant when the full size is odd.", []),
    ("downsample-winner", DOWN, "if (depth < sample_depth) {", "get_sample",
     "Of the four (or six, or nine) full-size depths, the largest wins: with reversed Z that is the nearest surface.", []),
    ("downsample-normal", DOWN, "imageStore(dest_normal_roughness, pixel_pos, texelFetch(source_normal_roughness, sample_pos, 0));", "main",
     "The half-size normal-roughness is the winner's texel, copied unchanged.", []),
    ("pass-hiz", SSE, '"SSR HI-Z"', "SSEffects::screen_space_reflection",
     "The pyramid's levels 1 and up, one dispatch per level, each choosing an odd variant from its parent's size.", []),
    ("hiz-max", HIZ, "depth = max(depth, texelFetch(source, pixel_pos * 2 + ivec2(1, 0), 0).x);", "main",
     "Each texel of a level is the max of its 2 x 2 parents: the nearest depth anywhere under it.", []),
    # ---- The trace ----
    ("pass-main", SSE, '"SSR Main"', "SSEffects::screen_space_reflection",
     "The trace, one thread per SSR pixel, with the Environment's steps, fades, and tolerance as push constants.", []),
    ("trace-sky", SSR, "bool should_trace = screen_pos.z != 0.0;", "main",
     "A pixel of depth 0 (the far plane: sky) is not traced.", []),
    ("trace-rough", SSR, "if (roughness >= 0.7) {", "main",
     "Roughness 0.7 or more is not traced, for speed; it writes zero colour and level.", []),
    ("geom-normal", SSR, "vec3 compute_geometric_normal(", "compute_geometric_normal",
     "The surface's true normal rebuilt from the pyramid's level 0: on each axis, the side whose two depths extrapolate best to the centre.", []),
    ("bias", SSR, "pos += geom_normal * (1.0 - pow(clamp(dot(normal, geom_normal), 0.0, 1.0), 8.0));", "main",
     "The start is pushed off the surface along the geometric normal, by one minus the eighth power of how far the shading normal agrees.", []),
    ("bounce", SSR, "ray_dir = normalize(reflect(ray_dir, geom_normal));", "main",
     "A reflected ray pointing into the true surface is reflected again about the geometric normal.", []),
    ("camera-clip", SSR, "end_pos -= ray_dir / ray_dir.z * (end_pos.z + 0.00001);", "main",
     "The comment says the near plane; the end point is clipped to the camera's plane, z = -0.00001.", []),
    ("parametric-t", SSR, "screen_ray_dir /= abs(screen_ray_dir.z);", "main",
     "The screen-space ray is scaled so its depth changes by exactly one per unit of t (Supnik's parametric t).", []),
    ("t-max", SSR, "float t_max = min(t2.x, t2.y);", "main",
     "Where the ray leaves the screen: the trace stops there.", []),
    ("start-advance", SSR, "// Advance the start point to the closest next cell to prevent immediate self intersection.", "main",
     "The start is advanced out of the pixel's own cell, so the ray never hits itself.", []),
    ("loop", SSR, "while (cur_level >= 0 && cur_iteration > 0 && t < t_max) {", "main",
     "The loop runs while the level is not below zero, steps remain, and the ray is on screen.", []),
    ("hit-test", SSR, "bool hit = facing_camera ? (t <= depth_t) : (depth_t <= edge_t);", "main",
     "A cell may hold a hit when the ray reaches the cell's nearest depth before leaving it (or, toward the camera, is already behind it).", []),
    ("tolerance", SSR, "if ((z0 - z1) > params.depth_tolerance) {", "main",
     "At level 0 only: a ray more than the tolerance behind the surface passes under it, staying at level 0.", []),
    ("level-step", SSR, "cur_level = min(cur_level + mip_offset, params.mipmaps - 1);", "main",
     "A miss climbs one level (cells twice as wide), a hit descends one.", []),
    ("validity", SSR, "// Instead of hard rejecting samples, write sample validity to the alpha channel.", "main",
     "Validity, not a yes or no: it goes into alpha, so invalid pixels still write a blur level.", []),
    ("edge-sky", SSR, "if (t >= t_max || hit_depth == 0.0) {", "main",
     "A ray that left the screen, or ended on the sky, is invalid.", []),
    ("backface", SSR, "if (all(lessThan(abs(screen_ray_dir.xy * t), 2.0 / params.screen_size))) {", "main",
     "The back-face test runs only when the hit is within two SSR pixels of the start on both axes.", []),
    ("confidence", SSR, "float confidence = 1.0 - smoothstep(0.0, params.depth_tolerance, delta);", "main",
     "Confidence falls with the distance between the ray's end and the surface under it, squared.", []),
    ("margin", SSR, "margin_blend = smoothstep(0.0, margin.x * margin.y, margin_grad.x * margin_grad.y);", "main",
     "A fade near the last frame's edges, on the product of the two edge distances.", []),
    ("fades", SSR, "float fade_out = params.distance_fade == 0.0", "main",
     "Fade in and fade out over the ray's screen length: together they peak at 0.58 with the defaults.",
     [("float fade_in = params.curve_fade_in == 0.0", "The fade in, ray length to the power fade_in.")]),
    ("last-frame-color", SSR, "color = vec4(textureLod(source_last_frame, reprojected_pos.xy, 0).xyz, 1.0) * validity;", "main",
     "The colour is last frame's, at the reprojected point, premultiplied by validity.", []),
    ("compress", SSR, "color.rgb /= 1.0 + dot(color.rgb, rec709_luminance_weights);", "main",
     "The colour is compressed by its luminance before filtering, so bright texels do not dominate.", []),
    ("cone", SSR, "float cone_angle = min(roughness, 0.999) * M_PI * 0.5;", "main",
     "Roughness becomes a cone's half-angle: roughness times ninety degrees.", []),
    ("blur-radius", SSR, "blur_radius = (a * (sqrt(a2 + fh2) - a)) / (4.0 * h);", "main",
     "The radius of the circle inside the cone where the ray ends.", []),
    ("mip-bias", SSR, "mip_level = clamp(log2(blur_radius * max(params.screen_size.x, params.screen_size.y) / 16.0 + 1.0), 0, params.mipmaps - 1);", "main",
     "The circle's radius in pixels becomes a mip level: log2 of radius over 16, plus one, clamped to the mips (and later lowered for long rays).", []),
    ("mip-store", SSR, "imageStore(output_mip_level, pixel_pos, vec4(mip_level / 14.0, 0.0, 0.0, 0.0));", "main",
     "The level is stored divided by 14 in eight bits.", []),
    # ---- Filter and resolve ----
    ("pass-filter", SSE, '"SSR Roughness Filter"', "SSEffects::screen_space_reflection",
     "The roughness filter, one dispatch per level, each from the level before.", []),
    ("filter-weights", FILT, "const float WEIGHTS[7] = float[7](", "WEIGHTS",
     "Seven Gaussian weights, summing to one.", []),
    ("filter-weight", FILT, "return mix(clamp(params.mip_level * 0.2, 0.0, 1.0), 1.0, c.a);", "get_weight",
     "An invalid texel weighs min(0.2 x level, 1) of a valid one's weight, blended by alpha.", []),
    ("filter-sample", FILT, "return textureLod(source, (vec2(pixel_pos) + 0.5) / params.screen_size, 0);", "get_sample",
     "Each fetch lands on the corner of four parent texels: a 2 x 2 average.", []),
    ("pass-resolve", SSE, '"SSR Resolve"', "SSEffects::screen_space_reflection",
     "The resolve to the internal size (half size only).", []),
    ("resolve-taps", RES, "get_sample(depth, normal, roughness, ((pixel_pos - 1) / 2) + ivec2(0, 0), color0, weight0);", "main",
     "Four half-size taps around the full-size pixel, each at its own blur level.", []),
    ("resolve-weights", RES, "const float DEPTH_FACTOR = 2048.0;", "get_sample",
     "Each tap is weighted by depth (2048), normal (32), and roughness (16) similarity.", []),
    ("resolve-uncompress", RES, "color.rgb /= 1.0 - dot(color.rgb, rec709_luminance_weights);", "get_sample",
     "The compression is undone per tap, before the taps are averaged.", []),
    # ---- The opaque pass ----
    ("nr-write", SCENE, "normal_roughness_output_buffer = vec4(encode24(normal) * 0.5 + 0.5, roughness);", "main",
     "The pre-pass writes the encoded view-space normal and the roughness.",
     [("normal_roughness_output_buffer.w = normal_roughness_output_buffer.w * (127.0 / 255.0);", "Roughness takes seven bits; above one half means a dynamic object.")]),
    ("encode24", SCENE, "vec3 encode24(vec3 v) {", "encode24",
     "Best-fit normals: the normal scaled to touch the unit cube, then to the length that quantizes best (Kaplanyan, SIGGRAPH 2010).", []),
    ("best-fit-table", BFN, "float find_minimum_error(vec3 normal) {", "find_minimum_error",
     "The table, made once at start-up: for each direction of a 1024 x 1024 folded-cube grid, the length whose 8-bit quantization errs least.", []),
    ("limiter", SCENE, "float kernelRoughness2 = min(2.0 * variance, scene_data.roughness_limiter_limit);", "main",
     "The roughness limiter widens roughness where the normal changes across the pixel.", []),
    ("sky-reflection", SCENE, "float horizon = min(1.0 + dot(ref_vec, normal), 1.0);", "main",
     "The sky's reflection along a direction bent toward the normal by roughness squared, faded below the horizon.", []),
    ("spec-occlusion", SCENE, "specular_occlusion = min(specular_occlusion * 4.0, 1.0);", "main",
     "Specular occlusion from the ambient light's luminance dims the reflection before SSR is blended in.", []),
    ("ssr-fade", SCENE, "ssr *= smoothstep(0.0, 1.0, 1.0 - clamp((roughness - 0.6) / (0.7 - 0.6), 0.0, 1.0));", "main",
     "Between roughness 0.6 and 0.7 the SSR result fades out, smoothing the trace's cut-off.", []),
    ("ssr-blend", SCENE, "indirect_specular_light = indirect_specular_light * (1.0 - ssr.a) + ssr.rgb;", "main",
     "SSR covers the indirect specular by its validity (alpha is premultiplied).", []),
    ("reflection-source", SDATA, "if ((ref_src == RSE::ENV_REFLECTION_SOURCE_BG && env_bg == RSE::ENV_BG_SKY) || ref_src == RSE::ENV_REFLECTION_SOURCE_SKY) {", "RenderSceneDataRD::update_ubo",
     "The sky's reflection is used only when the reflection source is the sky, or the background and the background is the sky; Disabled leaves none for SSR to cover.", []),
    ("f0", LIGHTS, "hvec3 F0(half metallic, half specular, hvec3 albedo) {", "F0",
     "f0: 0.16 times specular squared for a dielectric (0.04 at the default 0.5), the albedo for a metal, mixed by metallic.", []),
    ("env-brdf", SCENE, "indirect_specular_light *= energy_compensation * ((f90 - f0) * envBRDF.x + f0 * envBRDF.y);", "main",
     "After the blend, the environment BRDF scales it: SSR is treated as incoming light, like a probe.", []),
]

CHAPTERS: list[tuple[int, str, list[str]]] = [
    (1, "Where SSR runs", ["gating", "ssr-normal-roughness", "process-ssr", "env-defaults", "half-size-setting", "copy-last-frame"]),
    (2, "Depth, normals, and one thread per pixel", ["nr-write", "encode24", "best-fit-table", "limiter", "ssr-size", "pass-downsample", "downsample-winner", "downsample-normal", "geom-normal"]),
    (3, "The depth pyramid", ["hiz-texture", "mip-count", "pass-hiz", "hiz-max"]),
    (4, "Marching the ray", ["pass-main", "trace-sky", "trace-rough", "bias", "bounce", "camera-clip", "parametric-t", "t-max", "start-advance", "loop", "hit-test", "tolerance", "level-step"]),
    (5, "Is it a hit, and what colour", ["validity", "edge-sky", "backface", "confidence", "margin", "fades", "reprojection", "last-frame-size", "last-frame-color", "compress"]),
    (6, "Blurring by roughness", ["cone", "blur-radius", "mip-bias", "mip-store", "mip-level-texture", "pass-filter", "filter-weights", "filter-weight", "filter-sample", "pass-resolve", "resolve-taps", "resolve-weights", "resolve-uncompress", "resolve-flag"]),
    (7, "Into the final image", ["reflection-source", "f0", "sky-reflection", "spec-occlusion", "ssr-fade", "ssr-blend", "env-brdf"]),
    (8, "The frame again", ["process-ssr", "copy-last-frame"]),
]

DEMOS: dict[str, list[str]] = {
    "room": ["gating", "process-ssr", "ssr-blend", "reflection-source", "f0"],
    "inputs": ["nr-write", "encode24", "best-fit-table", "downsample-normal"],
    "pyramid": ["hiz-texture", "mip-count", "downsample-winner", "hiz-max"],
    "trace": ["loop", "hit-test", "tolerance", "level-step", "parametric-t", "start-advance"],
    "hit": ["validity", "edge-sky", "backface", "confidence", "margin", "fades"],
    "blur": ["cone", "blur-radius", "mip-bias", "mip-store"],
    "compose": ["sky-reflection", "spec-occlusion", "ssr-fade", "ssr-blend", "env-brdf"],
}

PASSES: list[dict[str, object]] = [
    {"lab": "ssrBestFit", "engine": "best_fit_normal (once, at start-up)", "sources": ["best-fit-table"]},
    {"lab": "ssrPrepassFS", "engine": "Depth pre-pass (PASS_MODE_DEPTH_NORMAL_ROUGHNESS)", "sources": ["ssr-normal-roughness", "nr-write"]},
    {"lab": "ssrDownsample", "engine": "SSR Downsample", "sources": ["pass-downsample", "downsample-winner"]},
    {"lab": "ssrHiz", "engine": "SSR HI-Z", "sources": ["pass-hiz", "hiz-max"]},
    {"lab": "ssrTrace", "engine": "SSR Main", "sources": ["pass-main", "loop"]},
    {"lab": "ssrFilter", "engine": "SSR Roughness Filter", "sources": ["pass-filter", "filter-weights"]},
    {"lab": "ssrResolve", "engine": "SSR Resolve", "sources": ["pass-resolve", "resolve-taps"]},
    {"lab": "ssrForwardFS", "engine": "Render Opaque Pass", "sources": ["ssr-blend", "env-brdf"]},
    {"lab": "ssrCopyLastFrame", "engine": "Copy Framebuffer for SSIL/SSR", "sources": ["copy-last-frame"]},
]

MAPPINGS: list[dict[str, object]] = [
    {"lab": "SsrNormalRoughness", "engineShort": "RB_TEX_NORMAL_ROUGHNESS", "engineName": "The normal-roughness buffer (RB_TEX_NORMAL_ROUGHNESS)",
     "relationship": "exact", "sources": ["nr-write", "encode24"],
     "exact": "R8G8B8A8_UNORM: rgb the view-space normal through encode24, times 0.5 plus 0.5; a the roughness times 127/255, one minus that for a dynamic instance.",
     "labLayout": "The same texel, written by ssrPrepassFS into a multisampled Rgba8Unorm target; the lab's objects are static."},
    {"lab": "SsrColor", "engineShort": "RB_SSR", "engineName": "The reflection colour (RB_SSR)",
     "relationship": "exact", "sources": ["last-frame-color", "compress", "validity"],
     "exact": "R16G16B16A16_SFLOAT at the SSR size with the full mip chain: rgb last frame's colour times validity, divided by one plus its Rec. 709 luminance; a the validity.",
     "labLayout": "The same, the program image ssrColor."},
]


def line_hash(text: str) -> str:
    return hashlib.sha256(text.strip().encode()).hexdigest()[:16]


def find(path: str, token: str) -> int:
    lines = (CHECKOUT / path).read_text(encoding="utf-8", errors="replace").splitlines()
    if " >>> " in token:
        anchor, text = token.split(" >>> ", 1)
        start = find(path, anchor)
        after = [i + 1 for i, l in enumerate(lines) if i + 1 > start and text in l]
        if not after:
            raise SystemExit(f"{path}: `{text}` not found after `{anchor}`")
        return after[0]
    hits = [i + 1 for i, l in enumerate(lines) if token in l]
    if not hits:
        raise SystemExit(f"{path}: `{token}` not found")
    if len(hits) > 1:
        raise SystemExit(f"{path}: `{token}` is ambiguous (lines {hits})")
    return hits[0]


def line_text(path: str, line: int) -> str:
    return (CHECKOUT / path).read_text(encoding="utf-8", errors="replace").splitlines()[line - 1]


def main() -> None:
    commit = subprocess.run(["git", "-C", str(CHECKOUT), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    entries: dict[str, dict[str, object]] = {}
    for key, path, token, symbol, inspect, details in ENTRIES:
        if key in entries:
            raise SystemExit(f"duplicate key {key}")
        if not inspect:
            raise SystemExit(f"{key}: empty text")
        line = find(path, token)
        detail_list: list[dict[str, object]] = []
        for d_token, note in details:
            d_line = find(path, d_token)
            detail_list.append({"line": d_line, "lineHash": line_hash(line_text(path, d_line)), "note": note})
        entries[key] = {"path": path, "line": line, "symbol": symbol, "lineHash": line_hash(line_text(path, line)), "inspect": inspect, "details": detail_list}
    for _, _, keys in CHAPTERS:
        for k in keys:
            assert k in entries, k
    for keys in DEMOS.values():
        for k in keys:
            assert k in entries, k
    for m in MAPPINGS + PASSES:
        for k in m["sources"]:
            assert k in entries, k
    index = {
        "schema": 1,
        "engine": {
            "name": "Godot Engine",
            "version": "4.7.2-stable",
            "commit": commit,
            "license": "MIT",
            "note": "Paths are relative to the Godot checkout (godot/). lineHash is the first 16 hex digits of the SHA-256 of the cited line with surrounding whitespace removed.",
        },
        "entries": entries,
        "chapters": [{"number": n, "title": title, "reading": keys} for n, title, keys in CHAPTERS],
        "demos": DEMOS,
        "mappings": MAPPINGS,
        "passes": PASSES,
    }
    out = LAB / "packages/godot_ssr/source-index.json"
    out.write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {out}: {len(entries)} entries at {commit}")


main()
