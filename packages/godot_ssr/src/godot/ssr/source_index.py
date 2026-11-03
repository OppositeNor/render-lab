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

Entry = tuple[str, str, str, str, str, str, list[tuple[str, str, str]]]

ENTRIES: list[Entry] = [
    # ---- Where it runs ----
    ("gating", RFC, "using_ssr = true;", "RenderForwardClustered::_render_scene",
     "SSR runs when the Environment enables it and the viewport's background is not transparent; a reflection probe's render never computes it.",
     "当 Environment 启用 SSR 且视口背景不透明时运行 SSR；反射探针的渲染从不计算它。", []),
    ("ssr-normal-roughness", RFC, "if (using_ssr ||", "RenderForwardClustered::_render_scene",
     "Using SSR makes the depth pre-pass write the normal-roughness buffer too (PASS_MODE_DEPTH_NORMAL_ROUGHNESS).",
     "启用 SSR 会让深度预渲染同时写入法线-粗糙度缓冲（PASS_MODE_DEPTH_NORMAL_ROUGHNESS）。", []),
    ("process-ssr", RFC, "void RenderForwardClustered::_process_ssr", "RenderForwardClustered::_process_ssr",
     "Before the opaque pass: allocate the SSR buffers, build each view's reprojection matrix, keep this frame's camera for the next, and run the SSR passes.",
     "在不透明渲染之前：分配 SSR 缓冲，为每个视图构建重投影矩阵，保存本帧相机供下一帧使用，并运行 SSR 各渲染阶段。", []),
    ("reprojection", RFC, "reprojections[v] = ", "RenderForwardClustered::_process_ssr",
     "The reprojection: this frame's screen to view space (inverse projection), to the world (camera transform), to last frame's view (its inverse transform), to last frame's screen (its projection). Camera only: no motion vectors.",
     "重投影：从本帧屏幕到视图空间（逆投影），到世界（相机变换），到上一帧视图（其逆变换），再到上一帧屏幕（其投影）。只考虑相机：不读取运动矢量。", []),
    ("copy-last-frame", RFC, "_copy_framebuffer_to_ss_effects(rb, using_ssil, using_ssr);", "RenderForwardClustered::_render_scene",
     "After the transparent pass and the MSAA resolve, the internal HDR colour is copied into the last-frame texture that the next frame's trace reads.",
     "在透明渲染和 MSAA 解析之后，内部 HDR 颜色被复制到上一帧纹理，供下一帧的追踪读取。", []),
    ("resolve-flag", RFC, "ss_flags |= (rb_data.is_valid() && !rb_data->ss_effects_data.ssr.half_size)", "RenderForwardClustered::_setup_environment",
     "The flag named RESOLVE_SSR is set at full size, when no resolve pass ran: the opaque pass then picks the blur level and undoes the compression itself.",
     "名为 RESOLVE_SSR 的标志在全尺寸（没有运行 resolve 阶段）时被设置：此时不透明渲染自己选择模糊级别并撤销压缩。", []),
    ("env-defaults", ENVH, "int ssr_max_steps = 64;", "Environment",
     "The Environment's SSR settings and defaults: off, 64 steps, fade in 0.15, fade out 2.0, depth tolerance 0.5.",
     "Environment 的 SSR 设置及默认值：关闭，64 步，淡入 0.15，淡出 2.0，深度容差 0.5。",
     [("bool ssr_enabled = false;", "Off by default.", "默认关闭。"), ("float ssr_depth_tolerance = 0.5;", "The depth tolerance, in view-space units.", "深度容差，单位为视图空间单位。")]),
    ("half-size-setting", RS, "rendering/environment/screen_space_reflection/half_size", "RenderingServer",
     "The project setting that runs SSR at half resolution, on by default.",
     "让 SSR 以半分辨率运行的项目设置，默认开启。", []),
    # ---- Buffers ----
    ("ssr-size", SSE, "p_ssr_buffers.size = ssr_half_size", "SSEffects::ssr_allocate_buffers",
     "The SSR buffers are half the internal size (rounded down) at half size.",
     "半尺寸时，SSR 缓冲是内部尺寸的一半（向下取整）。", []),
    ("mip-count", SSE, "while (cur_width > 1 && cur_height > 1) {", "SSEffects::ssr_allocate_buffers",
     "The mip count: both sides halved while both are above one texel.",
     "mip 数量：两边都大于一个纹素时，两边各减半。", []),
    ("hiz-texture", SSE, "RB_HIZ, RD::DATA_FORMAT_R32_SFLOAT", "SSEffects::ssr_allocate_buffers",
     "The depth pyramid: R32_SFLOAT at the SSR size, with the full mip chain.",
     "深度金字塔：R32_SFLOAT，SSR 尺寸，带完整 mip 链。", []),
    ("mip-level-texture", SSE, "RB_MIP_LEVEL, RD::DATA_FORMAT_R8_UNORM", "SSEffects::ssr_allocate_buffers",
     "The blur level per pixel: R8_UNORM, storing the level divided by 14.",
     "每个像素的模糊级别：R8_UNORM，存储级别除以 14。", []),
    ("last-frame-size", SSE, "last_frame_size /= 2;", "SSEffects::allocate_last_frame_buffer",
     "With SSR at half size and SSIL off, the last frame is kept at half size.",
     "SSR 为半尺寸且 SSIL 关闭时，上一帧以半尺寸保存。", []),
    # ---- Downsample and pyramid ----
    ("pass-downsample", SSE, '"SSR Downsample"', "SSEffects::screen_space_reflection",
     "The half-size downsample of depth and normal-roughness (half size only), choosing a variant when the full size is odd.",
     "深度和法线-粗糙度的半尺寸下采样（仅半尺寸），全尺寸为奇数时选择变体。", []),
    ("downsample-winner", DOWN, "if (depth < sample_depth) {", "get_sample",
     "Of the four (or six, or nine) full-size depths, the largest wins: with reversed Z that is the nearest surface.",
     "在四个（或六个、九个）全尺寸深度中取最大者：在反向 Z 下即最近的表面。", []),
    ("downsample-normal", DOWN, "imageStore(dest_normal_roughness, pixel_pos, texelFetch(source_normal_roughness, sample_pos, 0));", "main",
     "The half-size normal-roughness is the winner's texel, copied unchanged.",
     "半尺寸的法线-粗糙度是胜出者的纹素，原样复制。", []),
    ("pass-hiz", SSE, '"SSR HI-Z"', "SSEffects::screen_space_reflection",
     "The pyramid's levels 1 and up, one dispatch per level, each choosing an odd variant from its parent's size.",
     "金字塔的第 1 级及以上，每级一次调度，各自根据上一级的尺寸选择奇数变体。", []),
    ("hiz-max", HIZ, "depth = max(depth, texelFetch(source, pixel_pos * 2 + ivec2(1, 0), 0).x);", "main",
     "Each texel of a level is the max of its 2 x 2 parents: the nearest depth anywhere under it.",
     "每级的每个纹素是其 2 x 2 上级纹素的最大值：其下方任何位置的最近深度。", []),
    # ---- The trace ----
    ("pass-main", SSE, '"SSR Main"', "SSEffects::screen_space_reflection",
     "The trace, one thread per SSR pixel, with the Environment's steps, fades, and tolerance as push constants.",
     "追踪，每个 SSR 像素一个线程，Environment 的步数、淡化和容差作为推送常量。", []),
    ("trace-sky", SSR, "bool should_trace = screen_pos.z != 0.0;", "main",
     "A pixel of depth 0 (the far plane: sky) is not traced.",
     "深度为 0 的像素（远平面：天空）不追踪。", []),
    ("trace-rough", SSR, "if (roughness >= 0.7) {", "main",
     "Roughness 0.7 or more is not traced, for speed; it writes zero colour and level.",
     "粗糙度 0.7 及以上不追踪，以节省时间；写入零颜色和零级别。", []),
    ("geom-normal", SSR, "vec3 compute_geometric_normal(", "compute_geometric_normal",
     "The surface's true normal rebuilt from the pyramid's level 0: on each axis, the side whose two depths extrapolate best to the centre.",
     "从金字塔第 0 级重建表面的真实法线：在每个轴上，取两个深度外推到中心最准的一侧。", []),
    ("bias", SSR, "pos += geom_normal * (1.0 - pow(clamp(dot(normal, geom_normal), 0.0, 1.0), 8.0));", "main",
     "The start is pushed off the surface along the geometric normal, by one minus the eighth power of how far the shading normal agrees.",
     "起点沿几何法线推离表面，距离为一减去着色法线与之一致程度的八次方。", []),
    ("bounce", SSR, "ray_dir = normalize(reflect(ray_dir, geom_normal));", "main",
     "A reflected ray pointing into the true surface is reflected again about the geometric normal.",
     "指向真实表面内部的反射射线，会再关于几何法线反射一次。", []),
    ("camera-clip", SSR, "end_pos -= ray_dir / ray_dir.z * (end_pos.z + 0.00001);", "main",
     "The comment says the near plane; the end point is clipped to the camera's plane, z = -0.00001.",
     "注释说的是近平面；终点实际被裁剪到相机平面 z = -0.00001。", []),
    ("parametric-t", SSR, "screen_ray_dir /= abs(screen_ray_dir.z);", "main",
     "The screen-space ray is scaled so its depth changes by exactly one per unit of t (Supnik's parametric t).",
     "屏幕空间射线被缩放，使 t 每变化一个单位深度正好变化一（Supnik 的参数化 t）。", []),
    ("t-max", SSR, "float t_max = min(t2.x, t2.y);", "main",
     "Where the ray leaves the screen: the trace stops there.",
     "射线离开屏幕之处：追踪在此停止。", []),
    ("start-advance", SSR, "// Advance the start point to the closest next cell to prevent immediate self intersection.", "main",
     "The start is advanced out of the pixel's own cell, so the ray never hits itself.",
     "起点被推出像素自身的单元格，因此射线不会击中自己。", []),
    ("loop", SSR, "while (cur_level >= 0 && cur_iteration > 0 && t < t_max) {", "main",
     "The loop runs while the level is not below zero, steps remain, and the ray is on screen.",
     "当级别不低于零、仍有步数、射线仍在屏幕内时，循环继续。", []),
    ("hit-test", SSR, "bool hit = facing_camera ? (t <= depth_t) : (depth_t <= edge_t);", "main",
     "A cell may hold a hit when the ray reaches the cell's nearest depth before leaving it (or, toward the camera, is already behind it).",
     "当射线在离开单元格前到达其最近深度（朝向相机时则是已在其后方）时，该单元格可能包含命中。", []),
    ("tolerance", SSR, "if ((z0 - z1) > params.depth_tolerance) {", "main",
     "At level 0 only: a ray more than the tolerance behind the surface passes under it, staying at level 0.",
     "仅在第 0 级：位于表面后方超过容差的射线从其下方穿过，停留在第 0 级。", []),
    ("level-step", SSR, "cur_level = min(cur_level + mip_offset, params.mipmaps - 1);", "main",
     "A miss climbs one level (cells twice as wide), a hit descends one.",
     "未命中则上升一级（单元格宽一倍），命中则下降一级。", []),
    ("validity", SSR, "// Instead of hard rejecting samples, write sample validity to the alpha channel.", "main",
     "Validity, not a yes or no: it goes into alpha, so invalid pixels still write a blur level.",
     "用有效度代替是或否：它写入 alpha，因此无效像素仍会写入模糊级别。", []),
    ("edge-sky", SSR, "if (t >= t_max || hit_depth == 0.0) {", "main",
     "A ray that left the screen, or ended on the sky, is invalid.",
     "离开屏幕或终止于天空的射线无效。", []),
    ("backface", SSR, "if (all(lessThan(abs(screen_ray_dir.xy * t), 2.0 / params.screen_size))) {", "main",
     "The back-face test runs only when the hit is within two SSR pixels of the start on both axes.",
     "背面测试只在命中点在两个轴上都距起点两个 SSR 像素以内时运行。", []),
    ("confidence", SSR, "float confidence = 1.0 - smoothstep(0.0, params.depth_tolerance, delta);", "main",
     "Confidence falls with the distance between the ray's end and the surface under it, squared.",
     "置信度随射线终点与其下方表面之间的距离下降，并取平方。", []),
    ("margin", SSR, "margin_blend = smoothstep(0.0, margin.x * margin.y, margin_grad.x * margin_grad.y);", "main",
     "A fade near the last frame's edges, on the product of the two edge distances.",
     "在上一帧边缘附近淡出，基于两个边缘距离的乘积。", []),
    ("fades", SSR, "float fade_out = params.distance_fade == 0.0", "main",
     "Fade in and fade out over the ray's screen length: together they peak at 0.58 with the defaults.",
     "随射线屏幕长度淡入和淡出：默认值下二者乘积的峰值为 0.58。",
     [("float fade_in = params.curve_fade_in == 0.0", "The fade in, ray length to the power fade_in.", "淡入：射线长度的 fade_in 次方。")]),
    ("last-frame-color", SSR, "color = vec4(textureLod(source_last_frame, reprojected_pos.xy, 0).xyz, 1.0) * validity;", "main",
     "The colour is last frame's, at the reprojected point, premultiplied by validity.",
     "颜色取自上一帧的重投影点，并预乘有效度。", []),
    ("compress", SSR, "color.rgb /= 1.0 + dot(color.rgb, rec709_luminance_weights);", "main",
     "The colour is compressed by its luminance before filtering, so bright texels do not dominate.",
     "颜色在滤波前按亮度压缩，使明亮的纹素不会占主导。", []),
    ("cone", SSR, "float cone_angle = min(roughness, 0.999) * M_PI * 0.5;", "main",
     "Roughness becomes a cone's half-angle: roughness times ninety degrees.",
     "粗糙度变为圆锥半角：粗糙度乘以九十度。", []),
    ("blur-radius", SSR, "blur_radius = (a * (sqrt(a2 + fh2) - a)) / (4.0 * h);", "main",
     "The radius of the circle inside the cone where the ray ends.",
     "射线终点处圆锥内切圆的半径。", []),
    ("mip-bias", SSR, "mip_level = clamp(log2(blur_radius * max(params.screen_size.x, params.screen_size.y) / 16.0 + 1.0), 0, params.mipmaps - 1);", "main",
     "The circle's radius in pixels becomes a mip level: log2 of radius over 16, plus one, clamped to the mips (and later lowered for long rays).",
     "以像素计的圆半径变为 mip 级别：半径除以 16 加一后取 log2，限制在 mip 范围内（之后对长射线再降低）。", []),
    ("mip-store", SSR, "imageStore(output_mip_level, pixel_pos, vec4(mip_level / 14.0, 0.0, 0.0, 0.0));", "main",
     "The level is stored divided by 14 in eight bits.",
     "级别除以 14 后以八位存储。", []),
    # ---- Filter and resolve ----
    ("pass-filter", SSE, '"SSR Roughness Filter"', "SSEffects::screen_space_reflection",
     "The roughness filter, one dispatch per level, each from the level before.",
     "粗糙度滤波，每级一次调度，各自基于上一级。", []),
    ("filter-weights", FILT, "const float WEIGHTS[7] = float[7](", "WEIGHTS",
     "Seven Gaussian weights, summing to one.",
     "七个高斯权重，总和为一。", []),
    ("filter-weight", FILT, "return mix(clamp(params.mip_level * 0.2, 0.0, 1.0), 1.0, c.a);", "get_weight",
     "An invalid texel weighs min(0.2 x level, 1) of a valid one's weight, blended by alpha.",
     "无效纹素的权重是有效纹素的 min(0.2 x 级别, 1)，按 alpha 混合。", []),
    ("filter-sample", FILT, "return textureLod(source, (vec2(pixel_pos) + 0.5) / params.screen_size, 0);", "get_sample",
     "Each fetch lands on the corner of four parent texels: a 2 x 2 average.",
     "每次取样都落在四个上级纹素的角上：即 2 x 2 平均。", []),
    ("pass-resolve", SSE, '"SSR Resolve"', "SSEffects::screen_space_reflection",
     "The resolve to the internal size (half size only).",
     "解析到内部尺寸（仅半尺寸）。", []),
    ("resolve-taps", RES, "get_sample(depth, normal, roughness, ((pixel_pos - 1) / 2) + ivec2(0, 0), color0, weight0);", "main",
     "Four half-size taps around the full-size pixel, each at its own blur level.",
     "全尺寸像素周围的四个半尺寸取样点，各自使用自己的模糊级别。", []),
    ("resolve-weights", RES, "const float DEPTH_FACTOR = 2048.0;", "get_sample",
     "Each tap is weighted by depth (2048), normal (32), and roughness (16) similarity.",
     "每个取样点按深度（2048）、法线（32）和粗糙度（16）的相似度加权。", []),
    ("resolve-uncompress", RES, "color.rgb /= 1.0 - dot(color.rgb, rec709_luminance_weights);", "get_sample",
     "The compression is undone per tap, before the taps are averaged.",
     "在平均各取样点之前，逐个撤销压缩。", []),
    # ---- The opaque pass ----
    ("nr-write", SCENE, "normal_roughness_output_buffer = vec4(encode24(normal) * 0.5 + 0.5, roughness);", "main",
     "The pre-pass writes the encoded view-space normal and the roughness.",
     "预渲染写入编码后的视图空间法线和粗糙度。",
     [("normal_roughness_output_buffer.w = normal_roughness_output_buffer.w * (127.0 / 255.0);", "Roughness takes seven bits; above one half means a dynamic object.", "粗糙度占七位；大于一半表示动态物体。")]),
    ("encode24", SCENE, "vec3 encode24(vec3 v) {", "encode24",
     "Best-fit normals: the normal scaled to touch the unit cube, then to the length that quantizes best (Kaplanyan, SIGGRAPH 2010).",
     "最佳拟合法线：将法线缩放到接触单位立方体，再缩放到量化误差最小的长度（Kaplanyan，SIGGRAPH 2010）。", []),
    ("best-fit-table", BFN, "float find_minimum_error(vec3 normal) {", "find_minimum_error",
     "The table, made once at start-up: for each direction of a 1024 x 1024 folded-cube grid, the length whose 8-bit quantization errs least.",
     "启动时生成一次的表：对 1024 x 1024 折叠立方体网格的每个方向，取 8 位量化误差最小的长度。", []),
    ("limiter", SCENE, "float kernelRoughness2 = min(2.0 * variance, scene_data.roughness_limiter_limit);", "main",
     "The roughness limiter widens roughness where the normal changes across the pixel.",
     "粗糙度限制器在法线跨像素变化处增大粗糙度。", []),
    ("sky-reflection", SCENE, "float horizon = min(1.0 + dot(ref_vec, normal), 1.0);", "main",
     "The sky's reflection along a direction bent toward the normal by roughness squared, faded below the horizon.",
     "沿按粗糙度平方偏向法线的方向取天空反射，在地平线以下淡出。", []),
    ("spec-occlusion", SCENE, "specular_occlusion = min(specular_occlusion * 4.0, 1.0);", "main",
     "Specular occlusion from the ambient light's luminance dims the reflection before SSR is blended in.",
     "由环境光亮度得出的镜面遮蔽，在混入 SSR 之前使反射变暗。", []),
    ("ssr-fade", SCENE, "ssr *= smoothstep(0.0, 1.0, 1.0 - clamp((roughness - 0.6) / (0.7 - 0.6), 0.0, 1.0));", "main",
     "Between roughness 0.6 and 0.7 the SSR result fades out, smoothing the trace's cut-off.",
     "在粗糙度 0.6 到 0.7 之间 SSR 结果淡出，使追踪的截断变得平滑。", []),
    ("ssr-blend", SCENE, "indirect_specular_light = indirect_specular_light * (1.0 - ssr.a) + ssr.rgb;", "main",
     "SSR covers the indirect specular by its validity (alpha is premultiplied).",
     "SSR 按其有效度覆盖间接镜面光（alpha 已预乘）。", []),
    ("reflection-source", SDATA, "if ((ref_src == RSE::ENV_REFLECTION_SOURCE_BG && env_bg == RSE::ENV_BG_SKY) || ref_src == RSE::ENV_REFLECTION_SOURCE_SKY) {", "RenderSceneDataRD::update_ubo",
     "The sky's reflection is used only when the reflection source is the sky, or the background and the background is the sky; Disabled leaves none for SSR to cover.",
     "只有当反射来源是天空，或是背景且背景为天空时，才使用天空反射；Disabled 时 SSR 没有可覆盖的反射。", []),
    ("f0", LIGHTS, "hvec3 F0(half metallic, half specular, hvec3 albedo) {", "F0",
     "f0: 0.16 times specular squared for a dielectric (0.04 at the default 0.5), the albedo for a metal, mixed by metallic.",
     "f0：电介质为 0.16 乘 specular 的平方（默认 0.5 时为 0.04），金属为反照率，按 metallic 混合。", []),
    ("env-brdf", SCENE, "indirect_specular_light *= energy_compensation * ((f90 - f0) * envBRDF.x + f0 * envBRDF.y);", "main",
     "After the blend, the environment BRDF scales it: SSR is treated as incoming light, like a probe.",
     "混合之后，环境 BRDF 对其缩放：SSR 被视为入射光，与探针相同。", []),
]

CHAPTERS: list[tuple[int, str, str, list[str]]] = [
    (1, "Where SSR runs", "SSR 在哪里运行", ["gating", "ssr-normal-roughness", "process-ssr", "env-defaults", "half-size-setting", "copy-last-frame"]),
    (2, "Depth, normals, and one thread per pixel", "深度、法线与每像素一个线程", ["nr-write", "encode24", "best-fit-table", "limiter", "ssr-size", "pass-downsample", "downsample-winner", "downsample-normal", "geom-normal"]),
    (3, "The depth pyramid", "深度金字塔", ["hiz-texture", "mip-count", "pass-hiz", "hiz-max"]),
    (4, "Marching the ray", "步进射线", ["pass-main", "trace-sky", "trace-rough", "bias", "bounce", "camera-clip", "parametric-t", "t-max", "start-advance", "loop", "hit-test", "tolerance", "level-step"]),
    (5, "Is it a hit, and what colour", "是否命中，以及什么颜色", ["validity", "edge-sky", "backface", "confidence", "margin", "fades", "reprojection", "last-frame-size", "last-frame-color", "compress"]),
    (6, "Blurring by roughness", "按粗糙度模糊", ["cone", "blur-radius", "mip-bias", "mip-store", "mip-level-texture", "pass-filter", "filter-weights", "filter-weight", "filter-sample", "pass-resolve", "resolve-taps", "resolve-weights", "resolve-uncompress", "resolve-flag"]),
    (7, "Into the final image", "进入最终图像", ["reflection-source", "f0", "sky-reflection", "spec-occlusion", "ssr-fade", "ssr-blend", "env-brdf"]),
    (8, "The frame again", "再看一帧", ["process-ssr", "copy-last-frame"]),
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
    {"lab": "SsrNormalRoughness", "engineShort": "RB_TEX_NORMAL_ROUGHNESS", "engineName": "The normal-roughness buffer (RB_TEX_NORMAL_ROUGHNESS)", "engineNameZh": "法线-粗糙度缓冲（RB_TEX_NORMAL_ROUGHNESS）",
     "relationship": "exact", "sources": ["nr-write", "encode24"],
     "exact": "R8G8B8A8_UNORM: rgb the view-space normal through encode24, times 0.5 plus 0.5; a the roughness times 127/255, one minus that for a dynamic instance.",
     "exactZh": "R8G8B8A8_UNORM：rgb 为经 encode24 编码的视图空间法线乘 0.5 加 0.5；a 为粗糙度乘 127/255，动态实例则为一减去它。",
     "labLayout": "The same texel, written by ssrPrepassFS into a multisampled Rgba8Unorm target; the lab's objects are static.",
     "labLayoutZh": "相同的纹素，由 ssrPrepassFS 写入多重采样的 Rgba8Unorm 目标；实验室中的物体都是静态的。"},
    {"lab": "SsrColor", "engineShort": "RB_SSR", "engineName": "The reflection colour (RB_SSR)", "engineNameZh": "反射颜色（RB_SSR）",
     "relationship": "exact", "sources": ["last-frame-color", "compress", "validity"],
     "exact": "R16G16B16A16_SFLOAT at the SSR size with the full mip chain: rgb last frame's colour times validity, divided by one plus its Rec. 709 luminance; a the validity.",
     "exactZh": "R16G16B16A16_SFLOAT，SSR 尺寸，带完整 mip 链：rgb 为上一帧颜色乘有效度，再除以一加其 Rec. 709 亮度；a 为有效度。",
     "labLayout": "The same, the program image ssrColor.",
     "labLayoutZh": "相同，即程序图像 ssrColor。"},
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
    for key, path, token, symbol, inspect, inspect_zh, details in ENTRIES:
        if key in entries:
            raise SystemExit(f"duplicate key {key}")
        if not inspect or not inspect_zh:
            raise SystemExit(f"{key}: empty text")
        line = find(path, token)
        detail_list: list[dict[str, object]] = []
        for d_token, note, note_zh in details:
            d_line = find(path, d_token)
            detail_list.append({"line": d_line, "lineHash": line_hash(line_text(path, d_line)), "note": note, "noteZh": note_zh})
        entries[key] = {"path": path, "line": line, "symbol": symbol, "lineHash": line_hash(line_text(path, line)), "inspect": inspect, "inspectZh": inspect_zh, "details": detail_list}
    for _, _, _, keys in CHAPTERS:
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
        "chapters": [{"number": n, "title": title, "titleZh": zh, "reading": keys} for n, title, zh, keys in CHAPTERS],
        "demos": DEMOS,
        "mappings": MAPPINGS,
        "passes": PASSES,
    }
    out = LAB / "packages/godot_ssr/source-index.json"
    out.write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {out}: {len(entries)} entries at {commit}")


main()
