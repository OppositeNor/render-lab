"""Write packages/godot_sdfgi/source-index.json from the Godot checkout.

Every entry names a file and a token; the line is the token's only
occurrence in the file, so no line number is typed by hand. A token
`anchor >>> text` is the first line holding `text` after the anchor's only
line, for lines that repeat (a table that two functions both declare). Run it
from anywhere after changing an entry or moving to another Godot commit:

    python3 packages/godot_sdfgi/src/godot/sdfgi/source_index.py

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
        # No variable: ask, and stop when there is no one to answer.
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
GI = RD + "environment/gi.cpp"
GIH = RD + "environment/gi.h"
FOGC = RD + "environment/fog.cpp"
SKY = RD + "environment/sky.cpp"
RFC = RD + "forward_clustered/render_forward_clustered.cpp"
RSR = RD + "renderer_scene_render_rd.cpp"
CULL = "servers/rendering/renderer_scene_cull.cpp"
RS = "servers/rendering/rendering_server.cpp"
MOBILE = RD + "forward_mobile/render_forward_mobile.h"
GLES = "drivers/gles3/rasterizer_scene_gles3.h"
ENVH = "scene/resources/environment.h"
VI3D = "scene/3d/visual_instance_3d.cpp"
SH = RD + "shaders/environment/"
PRE = SH + "sdfgi_preprocess.glsl"
DL = SH + "sdfgi_direct_light.glsl"
INT = SH + "sdfgi_integrate.glsl"
GIG = SH + "gi.glsl"
FOGG = SH + "volumetric_fog_process.glsl"
SCENE = RD + "shaders/forward_clustered/scene_forward_clustered.glsl"
FWDGI = RD + "shaders/scene_forward_gi_inc.glsl"

Entry = tuple[str, str, str, str, str, list[tuple[str, str]]]

ENTRIES: list[Entry] = [
    # ---- 1. Where SDFGI runs ------------------------------------------------------------------------------------------
    ("frame-order", RFC, "_update_sdfgi(p_render_data);", "RenderForwardClustered::_render_scene (SDFGI steps)",
     "SDFGI's work begins at the top of the Forward+ frame: _update_sdfgi voxelizes this frame's dirty regions and bakes static lights, before anything else is set up. The same function then calls update_cascades, pre_process_gi (the dynamic lights) and update_light, and later store_probes and process_gi.",
     [("sdfgi->update_cascades();", "the cascade UBO is written here, after _update_sdfgi (finding F10)"),
      ("sdfgi->update_light();", "pre_process_gi (the dynamic lights, the SDFGIData UBO) just before it, then the light pass over the solid cells"),
      ("\t_post_prepass_render(p_render_data, using_sdfgi || using_voxelgi);", "update_probes runs from here, next to the depth pre-pass"),
      ("sdfgi->store_probes();", "the octahedral maps are stored every frame, then gi.process_gi runs the screen pass")]),
    ("sdfgi-update", RFC, "void RenderForwardClustered::sdfgi_update(", "RenderForwardClustered::sdfgi_update",
     "The per-viewport SDFGI object is freed when SDFGI is off, created when it is on, and rebuilt (every texture cleared, every cascade DIRTY_ALL) when the cascade count, minimum cell size, history size, occlusion flag or y scale differs; otherwise SDFGI::update runs, which moves the cascades.",
     [("if (!needs_sdfgi || needs_reset) {", "off, or a reset, frees the object"),
      ("history_frames_to_converge[RSE::ENV_SDFGI_CONVERGE_MAX] = { 5, 10, 15, 20, 25, 30 };", "the history sizes; the default index 5 gives 30"),
      ("sdfgi->num_cascades != environment_get_sdfgi_cascades(p_environment)", "the settings whose change rebuilds everything"),
      ("sdfgi->update(p_environment, p_world_position);", "the only call that scrolls a cascade")]),
    ("settings", ENVH, "bool sdfgi_enabled = false;", "Environment (SDFGI properties)",
     "The environment's SDFGI properties and their defaults: SDFGI off, 4 cascades, a minimum cell size of 0.2 m, the y scale at 75 %, occlusion off, bounce feedback 0.5, read sky light on, energy 1.0, normal and probe bias 1.1.",
     [("int sdfgi_cascades = 4;", "four cascades"),
      ("float sdfgi_min_cell_size = 0.2;", "the finest cell, in metres"),
      ("SDFGIYScale sdfgi_y_scale = SDFGI_Y_SCALE_75_PERCENT;", "75 %, which the engine turns into y_mult 1.5 (see y-mult-table)"),
      ("float sdfgi_bounce_feedback = 0.5;", "the bounce feedback")]),
    ("project-settings", GI, "sdfgi_ray_count = RSE::EnvironmentSDFGIRayCount(CLAMP(", "GI::GI (project settings)",
     "The three project settings are read once, when the GI object is made: probe_ray_count, frames_to_converge and frames_to_update_lights, each clamped to its enum range. At run time only the RenderingServer setters change them (environment_set_sdfgi_ray_count and the frame settings).",
     [("GLOBAL_GET(\"rendering/global_illumination/sdfgi/frames_to_converge\")", "frames_to_converge, an index into {5, 10, 15, 20, 25, 30}"),
      ("GLOBAL_GET(\"rendering/global_illumination/sdfgi/frames_to_update_lights\")", "frames_to_update_lights, an index into {1, 2, 4, 8, 16}")]),
    ("mobile-stub", MOBILE, "virtual void sdfgi_update(const Ref<RenderSceneBuffers> &p_render_buffers, RID p_environment, const Vector3 &p_world_position) override {}", "RenderForwardMobile::sdfgi_update",
     "The Mobile renderer's sdfgi_update has an empty body and its pending-region count returns 0, so SDFGI does nothing there (its settings' setters stay real); the Compatibility renderer has the same empty stubs (gles3-stub).",
     [("virtual int sdfgi_get_pending_region_count(const Ref<RenderSceneBuffers> &p_render_buffers) const override { return 0; }", "no region is ever pending")]),
    ("gles3-stub", GLES, "void sdfgi_update(const Ref<RenderSceneBuffers> &p_render_buffers, RID p_environment, const Vector3 &p_world_position) override {}", "RasterizerSceneGLES3::sdfgi_update",
     "The Compatibility (GLES3) renderer's SDFGI entry points are empty stubs as well: sdfgi_update does nothing and the pending-region count is 0.",
     [("int sdfgi_get_pending_region_count(const Ref<RenderSceneBuffers> &p_render_buffers) const override {", "returns 0")]),
    ("prepass-gate", RFC, "bool depth_pre_pass = (force_depth_pre_pass ||", "RenderForwardClustered::_render_scene (depth pre-pass)",
     "The depth pre-pass runs when the project setting rendering/driver/depth_prepass/enable is on (the default) or an opaque stencil forces it; SDFGI never forces it. The probe update is started only inside this branch, so with the setting off the probes stop updating, read from the code (finding F8).",
     [("bool force_depth_pre_pass = scene_state.used_opaque_stencil;", "the only thing that forces the pre-pass in stable"),
      ("if (depth_pre_pass) { //depth pre pass", "the branch that holds the probe update"),
      ("\t_post_prepass_render(p_render_data, using_sdfgi || using_voxelgi);", "update_probes starts here, beside the depth pre-pass")]),
    ("prepass-call", RSR, "sdfgi->update_probes(", "RendererSceneRenderRD::_post_prepass_render",
     "_post_prepass_render is the only caller of update_probes; it returns at once when the viewport has no SDFGI object.",
     [("if (!p_render_data->render_buffers->has_custom_data(RB_SCOPE_SDFGI)) {", "no SDFGI object, no probe update")]),
    ("ray-count-table", GI, "void GI::SDFGI::update_probes( >>> static const uint32_t ray_count[RSE::ENV_SDFGI_RAY_COUNT_MAX] = { 4, 8, 16, 32, 64, 96, 128 };", "GI::SDFGI::update_probes (ray_count)",
     "The code's ray-count table has seven entries, {4, 8, 16, 32, 64, 96, 128}; the default setting, index 1, gives 8 rays per probe per frame, not the 16 that the editor's label for index 1 names (finding F7, read from the code).",
     [("void GI::SDFGI::update_probes( >>> push_constant.ray_count = ray_count[gi->sdfgi_ray_count];", "the table is indexed by the project setting")]),
    ("ray-count-hint", RS, "\"rendering/global_illumination/sdfgi/probe_ray_count\"", "RenderingServer project setting probe_ray_count",
     "The editor's hint for probe_ray_count lists six entries, \"8 (Fastest),16,32,64,96,128 (Slowest)\", with default 1, so the editor labels the default \"16\" and index 0 \"8\"; the code's table (ray-count-table) has seven entries starting at 4, so they are 8 and 4 rays (finding F7).",
     [("\"rendering/global_illumination/sdfgi/frames_to_converge\"", "for contrast, the next setting's hint \"5 ... 30\" does match its table (history sizes 5 to 30; default index 5 is 30)")]),

    # ---- 2. Cascades that follow the camera ---------------------------------------------------------------------------
    ("cascade-create", GI, "void GI::SDFGI::create(", "GI::SDFGI::create",
     "Builds one SDFGI object: it reads the environment, sets probe_axis_count to PROBE_DIVISOR + 1 = 17, and allocates the scratch and per-cascade textures, each cleared to 0. Every cascade starts at DIRTY_ALL, so the first frame voxelizes the whole grid.",
     [("probe_axis_count = SDFGI::PROBE_DIVISOR + 1;", "17 probes per axis"),
      ("probe_pos = Vector3i((world_position / probe_size + Vector3(0.5, 0.5, 0.5)).floor());", "the first position is the camera rounded to a whole probe"),
      ("cascade.position = probe_pos * probe_cells;", "so the position is a multiple of 8 cells"),
      ("void GI::SDFGI::create( >>> cascade.dirty_regions = SDFGI::Cascade::DIRTY_ALL;", "every cascade starts fully dirty")]),
    ("cell-size-double", GI, "base_cell_size *= 2.0;", "GI::SDFGI::create (cell size)",
     "After each cascade is set up, the cell size doubles, so cascade i has cells of min_cell_size * 2^i (0.2, 0.4, 0.8, 1.6 m) and always 128 cells across: boxes 25.6, 51.2, 102.4 and 204.8 m wide.",
     [("cascade.cell_size = base_cell_size;", "the cascade takes the current size"),
      ("float base_cell_size = min_cell_size;", "the first cascade starts at the minimum")]),
    ("y-mult-table", GI, "static const float y_scale[3] = { 2.0, 1.5, 1.0 };", "GI::SDFGI::create (y_scale)",
     "The environment's y scale 50 %, 75 %, 100 % becomes y_mult 2.0, 1.5, 1.0. World y is multiplied by it before gridding, so at the default 75 % a voxel is cell_size / 1.5 tall and a cascade is two thirds as tall as it is wide: a vertical squash, not \"75 % of the height\" (finding F3, read from the code).",
     [("y_mult = y_scale[y_scale_mode];", "y_mult as the engine uses it everywhere"),
      ("void GI::SDFGI::create( >>> world_position.y *= y_mult;", "the first position squashes the camera's y")]),
    ("probe-axis-count", GI, "probe_axis_count = SDFGI::PROBE_DIVISOR + 1;", "GI::SDFGI::create (probe_axis_count)",
     "A cascade has 17 probes along each axis: the 16 intervals of PROBE_DIVISOR enclose both ends, so probes sit every 8 cells (128 / 16) and probe 16 lies on the far face; 17 x 17 x 17 = 4913 probes per cascade.",
     [("int32_t probe_cells = cascade_size / SDFGI::PROBE_DIVISOR;", "128 / 16 = 8 cells between probes"),
      ("tf_probes.width = probe_axis_count * probe_axis_count;", "the probe maps are 17 * 17 columns wide")]),
    ("scroll-update", GI, "void GI::SDFGI::update(RID p_env, const Vector3 &p_world_position) {", "GI::SDFGI::update",
     "The only place a cascade moves. The camera's cell is its position (y times y_mult) divided by the cell size, truncated toward zero; if it is more than drag_margin = 4 cells from the cascade's position, the cascade steps 8 cells (one probe) at a time, and dirty_regions records the signed total on that axis.",
     [("int32_t drag_margin = (cascade_size / SDFGI::PROBE_DIVISOR) / 2;", "(128 / 16) / 2 = 4 cells"),
      ("Vector3i pos_in_cascade = Vector3i((world_position + probe_half_size) / cascade.cell_size);", "the Vector3i conversion truncates toward zero"),
      ("cascade.position[j] += drag_margin * 2;", "toward +axis, while more than 4 cells away, one step is 8 cells"),
      ("cascade.dirty_regions[j] -= drag_margin * 2;", "a +axis move gives a negative dirty amount (fill from the end)")]),
    ("dirty-all-rule", GI, "//more than half the volume is dirty, make all dirty so its only rendered once", "GI::SDFGI::update (DIRTY_ALL)",
     "A partial scroll becomes DIRTY_ALL when the dirty volume exceeds half of the kept volume: for a move of d cells on one axis, 3d > 128, so from 43 cells, which in 8-cell steps is 48 cells (9.6 m for cascade 0). A move of 128 cells or more on an axis is DIRTY_ALL at once.",
     [("} else if (uint32_t(Math::abs(cascade.dirty_regions[j])) >= cascade_size) {", "128 cells or more on one axis"),
      ("if (dirty_volume > (safe_volume / 2)) {", "the dirty volume against half of the kept volume")]),

    # ---- 3. What is voxelized, and which lights -----------------------------------------------------------------------
    ("pending-regions", RFC, "int RenderForwardClustered::sdfgi_get_pending_region_count(", "RenderForwardClustered::sdfgi_get_pending_region_count",
     "A pending region is one entry for each cascade that is DIRTY_ALL, or for each axis of a scrolled cascade with a non-zero dirty amount, so a cascade has at most 3; regions are numbered over the cascades in order.",
     [("if (c.dirty_regions == RendererRD::GI::SDFGI::Cascade::DIRTY_ALL) {", "a full redraw is one region"),
      ("if (c.dirty_regions[j] != 0) {", "each dirty axis is one region")]),
    ("region-bounds", GI, "int GI::SDFGI::get_pending_region_data(", "GI::SDFGI::get_pending_region_data",
     "Turns a region number into the cascade, the cell range to fill and a world-space box. A region on a dirty axis covers 0..dirty cells (from the start) or 128 + dirty..128 (from the end); axes before it are chipped so a corner is not voxelized twice. The box is cell range times cell size, with y divided by y_mult.",
     [("to[j] = c.dirty_regions[j];", "positive: fill from the start"),
      ("from[j] = to[j] + c.dirty_regions[j];", "negative: fill from the end"),
      ("// \"chip\" away previous regions to avoid re-voxelizing the same thing", "earlier axes are chipped away"),
      ("r_bounds.position = Vector3(from + Vector3i(1, 1, 1) * -int32_t(cascade_size >> 1) + c.position) * c.cell_size * Vector3(1, 1.0 / y_mult, 1);", "the box in metres, y scaled by 1 / y_mult")]),
    ("sdfgi-cull", CULL, "if (cull_data.scenario->instance_aabbs[i].in_aabb(cull_data.cull->sdfgi.region_aabb[j])) {", "RendererSceneCull::_scene_cull (SDFGI regions)",
     "Every visible instance of the scenario (hidden ones are not in instance_data) is tested against every region's box, with no frustum, occlusion or visibility-range test: a mesh behind the camera is voxelized as well. A geometry instance qualifies when it has FLAG_USES_BAKED_LIGHT and matches the camera's layers; the test does not look at cast-shadows-only (finding F17, read from the code).",
     [("if ((idata.flags & InstanceData::FLAG_USES_BAKED_LIGHT) && (cull_data.visible_layers & idata.layer_mask)) {", "the geometry condition"),
      ("cull_result.sdfgi_region_geometry_instances[j].push_back(idata.instance_geometry);", "the instance joins that region's list")]),
    ("gi-mode", VI3D, "void GeometryInstance3D::set_gi_mode(GIMode p_mode) {", "GeometryInstance3D::set_gi_mode",
     "Only GI mode Static sets the baked-light flag the cull tests, so only Static meshes are voxelized; Dynamic keeps the mesh out of the voxels but lets it receive SDFGI, and Disabled does neither.",
     [("case GI_MODE_STATIC: {", "Static: USE_BAKED_LIGHT true"),
      ("case GI_MODE_DYNAMIC: {", "Dynamic: USE_BAKED_LIGHT false, USE_DYNAMIC_GI true")]),
    ("static-light-cull", CULL, "if (instance_light->bake_mode == RSE::LIGHT_BAKE_STATIC && cull_data.cull->sdfgi.region_cascade[j] <= instance_light->max_sdfgi_cascade) {", "RendererSceneCull::_scene_cull (static lights)",
     "A visible light whose bake mode is Static and whose box overlaps a region joins the static list for that region's cascade, if the cascade number is at most the light's max_sdfgi_cascade (default 2, so cascade 3 gets no static positional light). Dynamic lights are not culled by region.",
     [("cull_result.sdfgi_cascade_lights[sdfgi_last_light_cascade].push_back(instance_light->instance);", "stored under the cascade number"),
      ("if (sdfgi_last_light_index != i || sdfgi_last_light_cascade != cull_data.cull->sdfgi.region_cascade[j]) {", "one entry per (light, cascade)")]),
    ("static-lights-index", CULL, "for (uint32_t i = 0; i < cull.sdfgi.cascade_light_count; i++) {", "RendererSceneCull::_render_scene (static light check)",
     "The static lists are stored by cascade number (cull.cpp, the line above in static-light-cull) but read by position in the list of dirty cascades, here and in render_static_lights: they agree only when the dirty cascades are 0..n-1. With only cascade 2 dirty, position 0 is empty and the light is dropped for that frame (finding F2, read from the code).",
     [("if (scene_cull_result.sdfgi_cascade_lights[i].size()) {", "read by position i"),
      ("sdfgi_update_data.static_positional_lights = scene_cull_result.sdfgi_cascade_lights;", "the arrays are handed on as they are")]),
    ("light-bake-route", CULL, "if (RSG::light_storage->light_get_type(p_instance->base) != RSE::LIGHT_DIRECTIONAL && bake_mode != light->bake_mode) {", "RendererSceneCull::_update_instance",
     "A non-directional light with bake mode Dynamic (the default) is kept in scenario->dynamic_lights, which the per-frame dynamic pass reads; a Static light is in no list and reaches SDFGI only by region culling. A directional light never gets a bake mode here, so it is always dynamic.",
     [("p_instance->scenario->dynamic_lights.erase(light->instance);", "a light that leaves Dynamic is removed from the list"),
      ("p_instance->scenario->dynamic_lights.push_back(light->instance);", "Dynamic lights are listed"),
      ("light->max_sdfgi_cascade = max_sdfgi_cascade; //should most likely make sdfgi dirty in scenario", "changing it does not re-bake")]),

    # ---- 4. Voxelization ---------------------------------------------------------------------------------------------
    ("render-sdfgi", RFC, "void RenderForwardClustered::_render_sdfgi(", "RenderForwardClustered::_render_sdfgi",
     "Renders a region three times, once per axis i = 0, 1, 2: an orthographic camera on the +axis face of the region's box looking along -axis, the projection exactly the box, into an empty framebuffer with one pixel per voxel. The pass mode is PASS_MODE_SDF with the double-sided cull variant, so both faces draw; nothing is written to a colour target.",
     [("PassMode pass_mode = PASS_MODE_SDF;", "the voxelizer's pass mode"),
      ("scene_data.cam_projection.set_orthogonal(-h_size, h_size, -v_size, v_size, 0, d_size);", "an orthographic projection of exactly the region box"),
      ("fb_size.x = p_size[right_axis];", "one pixel per voxel across the other two axes"),
      ("RD::get_singleton()->framebuffer_create_empty(fb_size);", "an empty framebuffer: no colour or depth target")]),
    ("sdf-double-sided", RFC, "if constexpr (p_pass_mode == PASS_MODE_DEPTH_MATERIAL || p_pass_mode == PASS_MODE_SDF) {", "RenderForwardClustered::_render_list_template (cull variant)",
     "For PASS_MODE_SDF the cull variant is double-sided whatever the material's cull mode, so back faces voxelize too.",
     [("if constexpr (p_pass_mode == PASS_MODE_DEPTH_MATERIAL || p_pass_mode == PASS_MODE_SDF) { >>> cull_variant = SceneShaderForwardClustered::ShaderData::CULL_VARIANT_DOUBLE_SIDED;", "double-sided")]),
    ("sdf-fragment", SCENE, "vec3 local_pos = (implementation_data.sdf_to_bounds * vec4(vertex, 1.0)).xyz;", "main (MODE_RENDER_SDF)",
     "The voxelizer's fragment shader writes to the voxel the fragment's 3D position falls in, not to its pixel: local_pos is the position in the region's box scaled to [0, 1], and grid_pos = sdf_offset + ivec3(local_pos * sdf_size), with no clamp. Whichever fragment writes last wins the albedo.",
     [("ivec3 grid_pos = implementation_data.sdf_offset + ivec3(local_pos * vec3(implementation_data.sdf_size));", "the voxel from the 3D position"),
      ("imageStore(albedo_volume_grid, grid_pos, uvec4(albedo16));", "a plain store: the last write wins")]),
    ("albedo-pack", SCENE, "uint albedo16 = 0x1; //solid flag", "main (MODE_RENDER_SDF, albedo)",
     "The albedo voxel is 16 bits (R16_UINT): bit 0 is the solid flag; blue is bits 1-5, green 6-10, red 11-15, each clamp(uint(c * 31), 0, 31), truncated, not rounded.",
     [("albedo16 |= clamp(uint(albedo.r * 31.0), 0, 31) << 11;", "red, bits 11-15"),
      ("albedo16 |= clamp(uint(albedo.g * 31.0), 0, 31) << 6;", "green, bits 6-10"),
      ("albedo16 |= clamp(uint(albedo.b * 31.0), 0, 31) << 1;", "blue, bits 1-5")]),
    ("facing-mask", SCENE, "vec3 cam_normal = mat3(inv_view_matrix) * normalize(normal_interp);", "main (MODE_RENDER_SDF, facing)",
     "The facing mask has one bit per fragment: of the six directions +X, +Y, +Z, -X, -Y, -Z (bits 0-5) the one with the largest dot against the interpolated vertex normal, rotated to world space, not flipped for back faces, no normal map. Fragments are combined with imageAtomicOr, so a voxel keeps every direction any fragment chose.",
     [("if (d > closest_dist) {", "the direction with the largest dot wins"),
      ("facing_bits = (1 << i);", "one bit, not an accumulation"),
      ("imageAtomicOr(geom_facing_grid, grid_pos, facing_bits); //store facing bits", "OR across fragments")]),
    ("emission-pack", SCENE, "if (length(emission) > 0.001) {", "main (MODE_RENDER_SDF, emission)",
     "Emission is stored only above 0.001. Per direction the strength is max(0, dot(normal, dir)); render_emission_aniso keeps each direction's share of the largest channel as 5 bits (bits 0-29, in the facing order) and render_emission keeps the total light as RGBE 8985: red the top 8 of 9 bits at 0-7, green 9 bits at 8-16, blue the top 8 of 9 bits at 17-24, exponent 5 bits at 25-29; bits 30-31 are left for neighbour bits.",
     [("float strength = max(0.0, dot(cam_normal, aniso_dir[i]));", "the strength per direction"),
      ("light_aniso |= min(31, uint((lumas[i] / luma_total) * 31.0)) << (i * 5);", "six 5-bit shares"),
      ("//store as 8985 to have 2 extra neighbor bits", "RGBE 8985"),
      ("imageStore(emission_grid, grid_pos, uvec4(light_rgbe));", "the light word")]),
    ("render-textures", GI, "render_albedo = create_clear_texture(tf_render, \"SDFGI Render Albedo\");", "GI::SDFGI::create (scratch textures)",
     "The voxelizer's scratch textures are 128^3, shared by all cascades: render_albedo R16_UINT, render_emission and render_emission_aniso R32_UINT, render_geom_facing R32_UINT; in stable that one has no atomic usage bit although the shader writes it with imageAtomicOr. Then the occlusion and jump-flood textures.",
     [("tf_render.format = RD::DATA_FORMAT_R16_UINT;", "the albedo voxel format"),
      ("render_emission = create_clear_texture(tf_render, \"SDFGI Render Emission\");", "emission, R32_UINT"),
      ("render_geom_facing = create_clear_texture(tf_render, \"SDFGI Render Geometry Facing\");", "facing, R32_UINT, no atomic usage bit"),
      ("tf_render.format = RD::DATA_FORMAT_R8G8B8A8_UINT;", "the jump-flood seed textures follow, RGBA8_UINT")]),
    ("render-region", GI, "void GI::SDFGI::render_region(", "GI::SDFGI::render_region",
     "Voxelizes one region and, after the last region of a cascade, runs the preprocess: the scratch textures are cleared at a cascade's first region, the scroll (if not DIRTY_ALL), jump flood, occlusion and STORE follow, then the cascade's light textures are cleared. It also marks the cascade's dynamic lights dirty.",
     [("if (cascade_prev != cascade) {", "the first region of a cascade clears the scratch textures"),
      ("if (cascade_next != cascade) {", "the last region runs the preprocess"),
      ("cascades[cascade].all_dynamic_lights_dirty = true;", "so the next light update refreshes every cell")]),

    # ---- 5. The distance field -----------------------------------------------------------------------------------------

    # ---- 5. The distance field ----------------------------------------------------------------------------------------
    ("jump-flood-init", PRE, "#ifdef MODE_INITIALIZE_JUMP_FLOOD_HALF", "MODE_INITIALIZE_JUMP_FLOOD_HALF",
     "Each half-size cell (64^3) starts with a solid voxel of its 2 x 2 x 2 block as its seed: it collects the solid ones and picks closest[index % count], where index is the cell's own x, y, z parity bits, so neighbours pick different voxels. A block with none gets (0, 0, 0, 0). A seed is the voxel's full-resolution coordinates in xyz with w = 255 for valid.",
     [("int index = (indexv.x | indexv.y | indexv.z) % closest_count;", "the parity picks the voxel"),
      ("uvec4 v = uvec4(uvec3(src_pos), 255);", "a seed: the voxel's own position, valid"),
      ("imageStore(dst_positions, pos, uvec4(0));", "no solid voxel: unused")]),
    ("jump-flood", PRE, "//regular jumpflood, efficient for large steps, inefficient for small steps", "MODE_JUMPFLOOD",
     "One jump-flood pass: each cell looks at its 26 neighbours at the step distance, skips those whose seed has w = 0, and keeps the seed nearest to its own centre (pos * 2 + 0.5 at half size). MODE_JUMPFLOOD_OPTIMIZED does the same in 8^3 groups, each on a strided sub-lattice with its neighbourhood in shared memory.",
     [("//regular jumpflood, efficient for large steps, inefficient for small steps >>> posf = posf * 2.0 + 0.5;", "the half-size cell's centre, in full-resolution voxels"),
      ("p_dist = distance(posf, vec3(p.xyz));", "the distance to the seed the cell holds"),
      ("ivec3 group_offset = ivec3(gl_WorkGroupID.xyz) % params.step_size;", "the optimized variant: one strided sub-lattice per group")]),
    ("jump-flood-upscale", PRE, "v = imageLoad(src_positions_half, pos >> 1);", "MODE_UPSCALE_JUMP_FLOOD",
     "Back to full size: a solid voxel gets (pos, 255); any other takes the half seed at pos >> 1 and tests the 8 voxels of the 2 x 2 x 2 block that seed lies in, keeping a nearer solid one.",
     [("//search around if there is a better candidate from the same block", "the 8 voxels of the block the half seed lies in"),
      ("if (d2 < d) { //check valid distance before test so we avoid a read", "a nearer voxel of the same block replaces it, if solid")]),
    ("jump-flood-schedule", GI, "bool half_size = true; //much faster, very little difference", "GI::SDFGI::render_region (jump flood)",
     "half_size is hard-coded true, so the full-size branch is never used. For a 128 cascade: one init at 64^3, six half-size passes with steps 32, 16 (regular) and 8, 4, 2, 1 (optimized; the regular passes end after the one where 64 / (step / 2) reaches 8, at step 16), one upscale to 128^3, and one full-size optimized pass with step 1 to fix half-size artefacts: 9 dispatches.",
     [("if (cascade_half_size / (s / 2) >= optimized_jf_group_size) {", "the regular passes stop when the grid per half step reaches 8"),
      ("RENDER_TIMESTAMP(\"SDFGI Jump Flood Optimized (Half-Size)\");", "the optimized half-size passes"),
      ("//run one pass of fullsize jumpflood to fix up half size artifacts", "the final full-size pass")]),
    ("sdf-store", PRE, "d = 1.0 + length(vec3(p.xyz) - vec3(pos));", "MODE_STORE (distance)",
     "STORE turns seeds into the distance field: a cell that is its own seed (solid) stores 0; any other stores 1 + the distance in cells to its seed, then everything is divided by 255 into an R8_UNORM texture. Nothing is signed, and the largest distance is 254 cells; a face neighbour of a solid cell stores 2/255, and 1/255 is never written.",
     [("//solid block", "a solid voxel stores 0"),
      ("d /= 255.0;", "to the unorm range"),
      ("imageStore(dst_sdf, pos, vec4(d));", "into the cascade's sdf_tex, R8_UNORM")]),
    ("sdf-decode", INT, "float distance = texture(sampler3D(sdf_cascades[j], linear_sampler), uvw).r * 255.0 - 1.0;", "main (MODE_PROCESS, distance)",
     "The marches undo STORE's \"1 + d\" by subtracting an offset after sampling: the probe march here subtracts 1.0, the shadow march (shadow-march) 1.0, the screen pass's reflection march (specular-trace) 1.1, and the debug view 1.7; the gradient reads subtract nothing. A solid texel therefore reads -1 (or -1.1), and linear filtering between solid and empty puts the zero crossing midway. The field itself is unsigned.",
     [("if (distance < 0.05) {", "the probe march's hit rule")]),
    ("solid-cell-store", PRE, "store_positions[index].position = uint(pos.x | (pos.y << 7) | (pos.z << 14));", "MODE_STORE (solid cells)",
     "Each solid voxel is written to the cascade's list as four words: position (x, y, z at 7 bits each, bits 0-20), albedo (RGB555 in bits 0-14, six facing bits 15-20), light (the voxelizer's 30-bit RGBE 8985) and light_aniso (6 x 5 bits). The 26 neighbour bits, one for each neighbour whose nearest seed is this cell, are split over the top bits: position 21-31 holds neighbours 11-21, albedo 21-31 holds 0-10, light 30-31 holds 22-23, light_aniso 30-31 holds 24-25.",
     [("neighbour_bits |= (1 << bit_index);", "bit n is set when the voxel at offset n has this cell as its nearest seed"),
      ("store_positions[index].albedo |= (facing & 0x3F) << 15;", "six facing bits at 15-20"),
      ("store_positions[index].albedo |= neighbour_bits << 21;", "neighbour bits 0-10 in the albedo word"),
      ("store_positions[index].light |= (neighbour_bits >> 22) << 30;", "neighbour bits 22-23 in the light word")]),
    ("solid-cell-struct", GIH, "struct SolidCell { // this struct is unused, but remains as reference for size", "GI::SDFGI::Cascade::SolidCell",
     "The solid-cell record as the C++ side declares it: four uint32 (position, albedo, static_light, static_light_aniso), 16 bytes. The struct is unused and only sets the size of solid_cell_buffer, which holds 25 % of 128^3 = 524,288 cells per cascade (the shaders call the last two words light and light_aniso).",
     [("uint32_t static_light_aniso;", "the fourth word"),
      ("float solid_cell_ratio", "the ratio of cells the buffer holds")]),

    # ---- 6. Occlusion -------------------------------------------------------------------------------------------------
    ("occlusion-dispatch", GI, "Vector3i offset(i & 1, (i >> 1) & 1, (i >> 2) & 1);", "GI::SDFGI::render_region (occlusion)",
     "Eight dispatches, one per parity class i of the probe lattice (offset = the bits of i, flipped on an axis where the cascade's global probe index is odd), each with (9, 9, 9) minus the offset groups. There is no test of use_occlusion anywhere around this block: the occlusion is computed every time a cascade is rebuilt, whether or not it is read (finding F14, read from the code).",
     [("if ((probe_global_pos.x & 1) != 0) {", "an odd global probe index flips the class on that axis"),
      ("push_constant.occlusion_index = i;", "the class picks the render_occlusion[i] texture"),
      ("Vector3i groups = Vector3i(probe_size + 1, probe_size + 1, probe_size + 1) - offset;", "9 - offset groups per axis")]),
    ("occlusion-shells", PRE, "#define OCC_STEPS (OCCLUSION_SIZE * 3 - 2)", "MODE_OCCLUSION",
     "Each group owns a 16^3 region around one probe (OCCLUSION_SIZE = 8 per side) and processes it in 22 steps, shells of equal |x| + |y| + |z| growing from the probe and then shrinking, in all eight octants with a barrier after each. A solid voxel (facing != 0) gets 0, the shell next to the probe gets 1, and any other free voxel gets the average of its up to three neighbours one step closer to the probe; a solid neighbour adds 0 but still counts in the average, except on the coordinate planes.",
     [("if (facing != 0) { //solid", "solid voxels get 0"),
      ("} else if (step == 0) {", "the first shell is 1"),
      ("} else if (step == 0) { >>> occ /= avg;", "the average of the neighbours one step closer"),
      ("//bias solid voxels away", "phase 3: a solid voxel takes the occlusion of its free neighbours on the probe side")]),
    ("occlusion-store", PRE, "occlusion |= uint(clamp(occ * 15.0, 0.0, 15.0)) << occlusion_shift[i];", "MODE_STORE (occlusion)",
     "STORE packs the eight classes' values of a voxel into one word, 4 bits each (occ * 15, truncated), class i at shift {12, 8, 4, 0, 28, 24, 20, 16}[i]; the low 16 bits go to texel (x, y, z + cascade * 128) and the high 16 bits to (x + 128, y, z + cascade * 128). Viewed as R4G4B4A4, R is bits 12-15 (class 0 or 4), G bits 8-11, B bits 4-7, A bits 0-3.",
     [("// STORE OCCLUSION >>> const uint occlusion_shift[8] = uint[](12, 8, 4, 0, 28, 24, 20, 16);", "the shifts: class 0 at bit 12 ... class 7 at bit 16"),
      ("imageStore(dst_occlusion, occ_pos, uvec4(occlusion & 0xFFFF));", "the low half"),
      ("occ_pos.x += params.grid_size;", "the high half goes 128 texels along x"),
      ("imageStore(dst_occlusion, occ_pos, uvec4(occlusion >> 16));", "the high half")]),
    ("occlusion-read", GIG, "if (sdfgi.use_occlusion) {", "sdfvoxel_gi_process (occlusion)",
     "The screen pass reads occlusion only when use_occlusion is on (default off): the class is the parity of (probe_world_offset + probe) per axis, x bit 1, y bit 2, z bit 4; the point is clamped to within occlusion_clamp = 0.9375 of the probe, scaled by 1/16 and renormalized; z bit 4 moves to the second half of x; the weight is multiplied by max(occlusion, 0.01).",
     [("vec3 occ_pos = clamp(cascade_pos, probe_pos - sdfgi.occlusion_clamp, probe_pos + sdfgi.occlusion_clamp) * sdfgi.probe_to_uvw;", "clamped near the probe"),
      ("if (occ_indexv.z != 0) { //z bit is on, means index is >=4, so make it switch to the other half of textures", "the z bit selects the second half of x"),
      ("weight *= max(occlusion, 0.01);", "the probe weight")]),
    ("occlusion-texture", GI, "tf_occlusion.depth *= cascades.size(); //use depth for occlusion slices", "GI::SDFGI::create (occlusion_data)",
     "occlusion_data is R16_UINT, 256 x 128 x (128 x cascades): the width doubles to hold the second 16 bits, the depth stacks the cascades; it is read through an R4G4B4A4_UNORM view.",
     [("tf_occlusion.width *= 2; //use width for the other half", "the other 16 bits go to x + 128"),
      ("tv.format_override = RD::DATA_FORMAT_R4G4B4A4_UNORM_PACK16;", "the four-bit view")]),

    # ---- 7. Light in the voxels --------------------------------------------------------------------------------------
    ("update-light", GI, "void GI::SDFGI::update_light() {", "GI::SDFGI::update_light",
     "The dynamic light pass: for every cascade, copy the solid-cell counters into the indirect-dispatch buffer, push bounce_feedback and use_occlusion, and dispatch one thread per solid cell; unless all of the cascade's lights are dirty, a frame processes only every Nth cell (process_offset and process_increment, N from frames_to_update_light: 1, 2, 4, 8, or 16). Every cascade dispatches even with no light, because the pass also adds emission and the stored static light and feeds the bounce back.",
     [("push_constant.bounce_feedback = bounce_feedback;", "the bounce setting reaches the shader here"),
      ("push_constant.light_count = cascade_dynamic_light_count[i];", "the lights counted for this cascade by pre_process_gi"),
      ("RD::get_singleton()->compute_list_dispatch_indirect(compute_list, cascade.solid_cell_dispatch_buffer_call, 0);", "one thread per solid cell, from the counted list")]),
    ("light-schedule", GI, "static const uint32_t frames_to_update_table[RSE::ENV_SDFGI_UPDATE_LIGHT_MAX] = {", "GI::SDFGI::update_light (schedule)",
     "Cells are refreshed in turn: with frames_to_update_lights at its default index 2 the table gives N = 4, process_increment = 4 and process_offset = frame number % 4, the same offset for every cascade in a frame, so each solid cell is lit every fourth frame. A cascade that was just rebuilt (all_dynamic_lights_dirty) is lit entirely that frame.",
     [("if (cascades[i].all_dynamic_lights_dirty || gi->sdfgi_frames_to_update_light == RSE::ENV_SDFGI_UPDATE_LIGHT_IN_1_FRAME) {", "a rebuilt cascade, or the 1-frame setting, processes every cell"),
      ("push_constant.process_offset = RSG::rasterizer->get_frame_number() % frames_to_update;", "the offset rotates with the frame number"),
      ("push_constant.process_increment = frames_to_update;", "every Nth cell")]),
    ("light-thread", DL, "voxel_index *= params.process_increment;", "main (voxel_index)",
     "Thread t handles solid cell t * N + offset and returns if that is at or above the cell count; the dispatch is not shrunk, so the other threads exit at once. A cell not processed this frame keeps its old texel, so a moving light lags by up to N frames.",
     [("if (voxel_index >= dispatch_data.total_count) {", "the early exit past the list"),
      ("vec3 position = vec3(positioni) + vec3(0.5);", "the cell centre, before it is turned into world space")]),
    ("dynamic-lights", GI, "lights[idx].type = RSE::LIGHT_DIRECTIONAL;", "GI::SDFGI::pre_process_gi (lights)",
     "Per cascade, pre_process_gi fills up to 128 lights: first every visible directional light that is not sky-only (no bake-mode, max_sdfgi_cascade or box test; direction.y times y_mult, then normalized), then the Dynamic positional lights whose cascade number is at most max_sdfgi_cascade and whose box meets the cascade's box. That box is built in y-squashed space and the light's in real space (finding F9, read from the code). A positional light's direction is not y-stretched, which matters for spot lights (finding F20).",
     [("if (RSG::light_storage->light_directional_get_sky_mode(light) == RSE::LIGHT_DIRECTIONAL_SKY_MODE_SKY_ONLY) {", "sky-only directional lights are skipped"),
      ("cascade_aabb.position = Vector3((Vector3i(1, 1, 1) * -int32_t(cascade_size >> 1) + cascade.position)) * cascade.cell_size;", "the cascade's box has no 1 / y_mult"),
      ("if (i > max_sdfgi_cascade) {", "positional lights stop at max_sdfgi_cascade"),
      ("//dir.y *= y_mult;", "a positional light's direction is left unstretched (it matters for spot lights)")]),
    ("omni-attenuation", DL, "float get_omni_attenuation(float distance, float inv_range, float decay) {", "get_omni_attenuation",
     "A positional light's falloff at distance d with range r (Light3D default 5) and decay = the light's attenuation (default 1, not 2): (1 - (d / r)^4)^2 * d^-decay, the bracket clamped at 0 and d floored at 0.0001. A cell with attenuation below 0.001 skips the shadow march.",
     [("nd *= nd; // nd^4", "(d / r) to the fourth"),
      ("return nd * pow(max(distance, 0.0001), -decay);", "times d to the power -decay"),
      ("case LIGHT_TYPE_OMNI: { >>> attenuation = get_omni_attenuation(light_distance, 1.0 / lights.data[i].radius, lights.data[i].attenuation);", "radius is the light's range, decay its attenuation"),
      ("if (attenuation < 0.001) {", "below this, no shadow march")]),
    ("shadow-march", DL, "ray_pos += sign(direction) * cell_size * 0.48; // go almost to the box edge but remain inside", "main (shadow ray)",
     "The shadow ray starts 0.48 cell along each axis in the light's direction, plus 0.4 cell along the ray, then sphere-traces the distance field (step = sample * 255 - 1 cells) from the cell's own cascade outward; a sample below 0.001 cell is a hit, tested before the step. Leaving the last cascade means lit. Shadows are binary: has_shadow is copied but never read, so every light casts one, and the march's occlusion value is never used (finding F18, read from the code).",
     [("ray_pos += ray_dir * 0.4 * cell_size; //apply a small bias from there", "the second part of the start bias"),
      ("float distance = texture(sampler3D(sdf_cascades[j], linear_sampler), uvw).r * 255.0 - 1.0;", "the step, decoded from the byte"),
      ("if (distance < 0.001) {", "the hit rule"),
      ("light_distance -= max_advance / cascades.data[j].to_cell;", "the remaining light distance after leaving a cascade")]),
    ("aniso-pack", DL, "aniso0.r = lumas[0] / luma_total;", "main (storing the light)",
     "A cell's light is kept per face: for each of the six faces the cell exposes, light_accum[j] gets max(0, dot(face, direction)) times the light (no 1/pi). It is then packed: the total as E5B9G9R9 (r | g << 9 | b << 18 | e << 27) into light_data, and each face's share of the largest channel as UNORM, +X +Y +Z -X into one RGBA8 texture and -Y -Z into an RG8 texture; the same three values are copied into each of the cell's neighbours. The shares are relative weights and do not sum to 1.",
     [("light_accum[j] += max(0.0, dot(aniso_dir[j], direction)) * light;", "the face weights, per face the cell exposes"),
      ("light_total_rgbe = (uint(sRed) & 0x1FF) | ((uint(sGreen) & 0x1FF) << 9) | ((uint(sBlue) & 0x1FF) << 18) | ((uint(exps) & 0x1F) << 27);", "the dynamic pass's E5B9G9R9 word"),
      ("imageStore(dst_aniso1, positioni, vec4(aniso1, 0.0, 0.0));", "the second texture holds -Y and -Z"),
      ("imageStore(dst_light, neighbour_pos, uvec4(light_total_rgbe));", "the same light in each neighbour")]),
    ("static-pass", GI, "dl_push_constant.bounce_feedback = 0.0; // this is static light, do not multibounce yet", "GI::SDFGI::render_static_lights",
     "The static pass runs only on a frame where a cascade was rebuilt and a static light was culled into it: per dirty cascade it fills up to 1024 lights (skipping any whose cascade number is above its max_sdfgi_cascade or whose box misses the cascade), then dispatches MODE_PROCESS_STATIC over every solid cell with bounce 0, process_offset 0 and process_increment 1, skipped when there are no lights. It reads the lights at the dirty list's position (finding F2).",
     [("if (idx == SDFGI::MAX_STATIC_LIGHTS) {", "at most 1024 static lights"),
      ("void GI::SDFGI::render_static_lights( >>> if (!cascade_aabb.intersects(light_aabb)) {", "the box test"),
      ("RID light_instance = p_positional_light_cull_result[i][j];", "the lights are read at position i of the dirty list, not by cascade number"),
      ("dl_push_constant.process_increment = 1;", "every cell, once")]),
    ("static-write", DL, "uint light = process_voxels.data[voxel_index].light & (3 << 30);", "main (MODE_PROCESS_STATIC)",
     "The static pass writes no texture: it writes back into the solid cell, keeping the two neighbour bits and replacing the rest of the light word with the new RGBE 8985 total, and rebuilding light_aniso. Since the sum started from the cell's current light, the cell ends with its old light plus each light culled this time. RGBE 8985 drops the low bit of red and blue (finding F24).",
     [("uint rgbe = process_voxels.data[voxel_index].light;", "the sum starts from the cell's current light"),
      ("light_total_rgbe = ((uint(sRed) & 0x1FF) >> 1) | ((uint(sGreen) & 0x1FF) << 8) | (((uint(sBlue) & 0x1FF) >> 1) << 17) | ((uint(exps) & 0x1F) << 25);", "RGBE 8985: the low bit of red and blue is shifted out"),
      ("light |= light_total_rgbe;", "the word is replaced")]),
    ("scroll-reinject", PRE, "uint light = src_process_voxels.data[index].light & 0x3fffffff; //30 bits of RGBE8985", "MODE_SCROLL (light)",
     "SCROLL copies each kept cell's stored light word (its emission plus any earlier static light) into the cleared render_emission texture at its new position, and STORE reads render_emission back into the list. The static pass then adds every culled light again onto cells that were kept, so a static light that overlaps old content and the new slab is counted again at each scroll; DIRTY_ALL does not (finding F1, read from the code).",
     [("imageStore(dst_light, write_pos, uvec4(light));", "written into the emission texture"),
      ("store_positions[index].light = imageLoad(src_light, pos).r;", "STORE reads it back into the cell record")]),
    ("bounce-feedback", DL, "vec3 feedback = (params.bounce_feedback < 1.0) ? (albedo * params.bounce_feedback) : mix(albedo, vec3(1.0), params.bounce_feedback - 1.0);", "main (MODE_PROCESS_DYNAMIC, bounce)",
     "Dynamic pass only, when bounce_feedback is above 0.001: the feedback colour is albedo * bounce below 1 and mix(albedo, 1, bounce - 1) above. For each of the 8 probes around the cell and each face it exposes, the weight is trilinear * max(0, dot(face, direction to the probe)) (times occlusion if on); the face's light is the weighted mean of the probes' irradiance in that face's direction, times the feedback. It reads the maps stored at the end of the previous frame, so each bounce takes a frame.",
     [("vec3 pos = (vec3(positioni) + vec3(0.5)) * float(params.probe_axis_size - 1) / params.grid_size;", "the cell's position in probe units, times 16 / 128"),
      ("float weight = trilinear.x * trilinear.y * trilinear.z * max(0, dot(n, probe_dir));", "the weight of one probe for one face"),
      ("vec3 indirect_light = textureLod(sampler2DArray(lightprobe_texture, linear_sampler), pos_uvw, 0.0).rgb;", "the irradiance map read"),
      ("light_accum[k] *= feedback;", "the damped result")]),

    # ---- 8. Probes trace the field ---------------------------------------------------------------------------------------
    ("update-probes", GI, "void GI::SDFGI::update_probes(RID p_env, SkyRD::Sky *p_sky) {", "GI::SDFGI::update_probes",
     "Per cascade, one thread per probe (17 x 17 columns by 17 rows, the z slices side by side) traces its rays serially. history_index = render_pass % history_size, and render_pass is incremented once per call, so it counts probe updates, not frames. ray_bias is the probe_bias setting (1.1 cells) and the ray count comes from the table; the sky is read only when read_sky_light is on.",
     [("void GI::SDFGI::update_probes( >>> push_constant.history_index = render_pass % history_size;", "the ring slot of this update"),
      ("void GI::SDFGI::update_probes( >>> push_constant.ray_bias = probe_bias;", "the start bias, in cells"),
      ("if (reads_sky && p_env.is_valid()) {", "the sky is read only if enabled"),
      ("void GI::SDFGI::update_probes( >>> compute_list_dispatch_threads(compute_list, probe_axis_count * probe_axis_count, probe_axis_count, 1);", "one thread per probe: 289 x 17")]),
    ("probe-position", INT, "float probe_cell_size = float(params.grid_size.x / float(params.probe_axis_size - 1)) / cascades.data[params.cascade].to_cell;", "main (MODE_PROCESS, probe cell)",
     "A thread's probe: probe_cell = (pos.x % 17, pos.y, pos.x / 17), and its position is the cascade's offset plus probe_cell times probe_cell_size, which is 128 / 16 = 8 cells, in y-stretched space; probes lie at cell coordinates 0, 8, ..., 128.",
     [("float probe_cell_size = float(params.grid_size.x / float(params.probe_axis_size - 1)) / cascades.data[params.cascade].to_cell; >>> probe_cell.x = pos.x % int(params.probe_axis_size);", "x and the z slice share one image axis"),
      ("float probe_cell_size = float(params.grid_size.x / float(params.probe_axis_size - 1)) / cascades.data[params.cascade].to_cell; >>> probe_cell.z = pos.x / int(params.probe_axis_size);", "the z slice"),
      ("float probe_cell_size = float(params.grid_size.x / float(params.probe_axis_size - 1)) / cascades.data[params.cascade].to_cell; >>> vec3 probe_pos = cascades.data[params.cascade].offset + vec3(probe_cell) * probe_cell_size;", "the probe's position in y-stretched space")]),
    ("vogel-rays", INT, "vec3 vogel_hemisphere(uint p_index, uint p_count, float p_offset) {", "vogel_hemisphere",
     "The probe's rays are a Vogel spiral: index idx = history_index + i * history_size of ray_total = history_size * ray_count (240 by default), r = sqrt(idx + 0.5) / sqrt(ray_total), theta = idx * golden angle + a per-probe offset hashed from its world probe index (no time input). Over the 30 frames the indices cover 0..239 once. The sign of z is the parity of idx, so with an even history_size every ray of one frame points into the same z hemisphere: even frames -z, odd frames +z (finding F22, read from the code). The ray's y is then multiplied by y_mult and normalized.",
     [("return vec3(l * cos(theta), l * sin(theta), y * (float(p_index & 1) * 2.0 - 1.0));", "z takes the sign of the index's parity"),
      ("vec3 ray_dir = vogel_hemisphere(ray_offset + i * ray_mult, ray_total, offset);", "the index of ray i in frame slot history_index"),
      ("float offset = hashf3(vec3(h3 & uvec3(0xFFFFF)));", "the per-probe rotation"),
      ("ray_dir.y *= params.y_mult;", "y-stretched space")]),
    ("probe-march", INT, "ray_pos += ray_dir * 1.0 / max(abs_ray_dir.x, max(abs_ray_dir.y, abs_ray_dir.z)) * bias / cascades.data[params.cascade].to_cell;", "main (MODE_PROCESS, march)",
     "The ray starts ray_bias = 1.1 cells from the probe along its dominant axis (a cube, not a sphere), then marches from the probe's own cascade outward: in a cascade it steps by the stored distance (sample * 255 - 1 cells) until a step is below 0.05 cell (a hit) or the ray leaves the box, where it continues from the exit point in the next cascade. There is no iteration cap; leaving the last cascade is a miss.",
     [("if (distance < 0.05) {", "the hit rule, in cells"),
      ("advance += distance;", "the sphere-tracing step"),
      ("hit_cascade = j;", "the cascade the ray hit in"),
      ("ray_pos = pos;", "the exit point becomes the origin in the next cascade")]),
    ("probe-hit", INT, "vec3 hit_normal = normalize(vec3(", "main (MODE_PROCESS, hit)",
     "A hit reads only the cascade it hit in: the normal is the central-difference gradient of the stored distance (epsilon 0.001 in uvw), the light is the voxel light texture, and the six face weights are +X +Y +Z in aniso0.rgb and -X -Y -Z in (aniso0.a, aniso1.rg). The ray's radiance is hit_light times the sum of max(0, n * w) over the positive faces and max(0, -n * w) over the negative ones: no BRDF, cosine or distance falloff, since albedo is already in the stored light.",
     [("const float EPSILON = 0.001;", "the gradient step"),
      ("vec3 hit_light = texture(sampler3D(light_cascades[hit_cascade], linear_sampler), uvw).rgb;", "the stored light"),
      ("vec3 hit_aniso1 = vec3(aniso0.a, texture(sampler3D(aniso1_cascades[hit_cascade], linear_sampler), uvw).rg);", "the negative faces' weights"),
      ("light.rgb = hit_light * (dot(max(vec3(0.0), (hit_normal * hit_aniso0)), vec3(1.0)) + dot(max(vec3(0.0), (-hit_normal * hit_aniso1)), vec3(1.0)));", "the radiance of the ray")]),
    ("probe-sky", INT, "} else if (bool(params.sky_flags & SKY_FLAGS_MODE_SKY)) {", "main (MODE_PROCESS, miss)",
     "A miss reads the sky: with a sky, the ray rotated by the inverse sky orientation reads the radiance map at mip level 2 (\"because we don't usually throw a lot of rays\") times sky_energy; with a colour background, that colour times sky_energy; otherwise black. In 4.7.2-stable the probe shader is compiled with a texture2D sky binding although, with the default texture_array_reflections on, the sky's radiance is a 2D array, so what a missing ray reads is not defined by the code (finding F25, read from the code; see sky-define and sky-radiance-array).",
     [("layout(set = 1, binding = 0) uniform texture2D sky_irradiance;", "the declaration that is compiled (the array one needs USE_RADIANCE_OCTMAP_ARRAY)"),
      ("light.rgb = textureLod(sampler2D(sky_irradiance, linear_sampler_mipmaps), vec3_to_oct_with_border(sky_dir, params.sky_irradiance_border_size), 2.0).rgb;", "the mip-2 read the compiled path takes"),
      ("light.rgb = params.sky_color_or_orientation;", "a colour background")]),
    ("sky-define", GI, "defines += \"\\n#define USE_OCTMAP_ARRAY\\n\";", "GI::init (integrate defines)",
     "The probe shader is built with USE_OCTMAP_ARRAY when the sky uses octahedral arrays, but sdfgi_integrate.glsl tests USE_RADIANCE_OCTMAP_ARRAY (the name the forward, mobile and fog shaders are given), so the array declaration is never compiled in. Read from the code (finding F25); commit d9ea5c26 in this repository's history (after 4.7.2-stable) changes it to USE_RADIANCE_OCTMAP_ARRAY.",
     [("void GI::init(SkyRD *p_sky) { >>> if (p_sky->sky_use_octmap_array) {", "the condition that adds the define")]),
    ("sky-radiance-array", SKY, "tf.texture_type = RD::TEXTURE_TYPE_2D_ARRAY;", "SkyRD::update_dirty_skys (radiance)",
     "With sky_use_octmap_array (rendering/reflections/sky_reflections/texture_array_reflections, on by default on desktop) the sky's radiance is a TEXTURE_TYPE_2D_ARRAY; update_probes binds that texture at binding 0 of the probe shader's set 1 (gi.cpp), where the compiled shader declares a texture2D.",
     [("void SkyRD::update_dirty_skys() { >>> if (sky_use_octmap_array) {", "the array path")]),
    ("sh-accum", INT, "#define SH_ACCUM(m_idx, m_value)", "main (MODE_PROCESS, SH_ACCUM)",
     "Each ray adds light * Y_m(direction) to 16 real spherical-harmonic coefficients, l = 0 to 3, per colour channel, in a per-thread shared array (order: 0; y, z, x; five l = 2 terms; seven l = 3 terms; z is the m = 0 axis). The SH_SIZE != 16 branches are dead.",
     [("SH_ACCUM(0, 0.282095); //l0", "the constant term"),
      ("SH_ACCUM(3, 0.488603 * ray_dir.x); //l1p1", "the first-band terms follow y, z, x"),
      ("SH_ACCUM(15, 0.590043 * ray_dir.x * (ray_dir2.x - 3.0f * ray_dir2.y));", "the last of the 16"),
      ("shared SH sh_accum[64]; //8x8", "one SH per thread of the 8 x 8 group")]),
    ("history-ring", INT, "vec4 value = vec4(sh_accum[probe_index].c[i * 3 + 0], sh_accum[probe_index].c[i * 3 + 1], sh_accum[probe_index].c[i * 3 + 2], 1.0) * 4.0 / float(params.ray_count);", "main (MODE_PROCESS, history)",
     "Each coefficient is stored as fixed point: value = (rgb, 1) * 4 / ray_count, times 1024 (HISTORY_BITS 10), clamped to 16 bits. The history texture keeps the last history_size updates as layers; the average texture is the exact integer sum of the ring: average -= the old layer, += the new value. So the mean is a sliding-window box filter, not an exponential average: after k < 30 frames from zero it is about k / 30 of the converged value.",
     [("ivec4 ivalue = clamp(ivec4(value * float(1 << HISTORY_BITS)), -32768, 32767);", "to 16-bit fixed point, scale 1024"),
      ("average -= prev_value;", "subtract the oldest slot"),
      ("average += ivalue;", "add the newest"),
      ("#define HISTORY_BITS 10", "the fixed-point scale")]),
    ("history-textures", GI, "tf_probe_history.format = RD::DATA_FORMAT_R16G16B16A16_SINT; //signed integer because SH are signed", "GI::SDFGI::create (probe history)",
     "The probe history is R16G16B16A16_SINT, 289 x 272 texels (x + 17 * z columns, y * 16 + coefficient rows) with history_size array layers per cascade, and the running average is R32G32B32A32_SINT of the same size; both are cleared at creation so the average works.",
     [("tf_probe_history.array_layers = history_size;", "one layer per history slot"),
      ("tf_probe_average.format = RD::DATA_FORMAT_R32G32B32A32_SINT; //signed integer because SH are signed", "the sum needs 32 bits"),
      ("tf_probes.height = probe_axis_count * SDFGI::SH_SIZE;", "17 * 16 = 272 rows")]),
    ("ambient-texture", INT, "ambient_light *= 0.88622; // SHL0", "main (MODE_PROCESS, ambient)",
     "Only when volumetric fog is on, coefficient 0 of the average is also written to an RGBA16F ambient texture, (average / history_size / 1024) * 0.88622, one texel per probe and a layer per cascade; the fog reads this non-directional value.",
     [("if (params.store_ambient_texture && i == 0) {", "only for coefficient 0, and only with fog")]),
    ("sh-gain", INT, "vec3 m = sh.rgb * c[i] * 4.0;", "main (MODE_STORE, SH gain)",
     "The probe estimator stores (4 / N) * sum(L * Y) and STORE multiplies by 4 and by l_mult = {1, 2/3 x 3, 1/4 x 5, 0 x 7}, with no 1/pi or 4*pi anywhere on the chain; for a uniform sky of radiance L the irradiance texel is (4 / pi) L = 1.273 L, where Godot's own sky ambient is 1.0 L (finding F4, read from the code; whether it is a deliberate gain is not stated).",
     [("* 4.0 / float(params.ray_count);", "the estimator's 4 / N"),
      ("const float l_mult[SH_SIZE] = float[](", "the convolution weights"),
      ("irradiance += m * l_mult[i];", "irradiance weights each term by l_mult"),
      ("radiance += m;", "radiance does not")]),

    # ---- 9. From coefficients to maps ----------------------------------------------------------------------------------
    ("store-probes", GI, "void GI::SDFGI::store_probes() {", "GI::SDFGI::store_probes",
     "Every frame GI is used, whether or not the probes were updated, one thread per octahedral texel (289 * 6 by 17 * 6 = 1734 x 102 per cascade) re-encodes the current averages into the octahedral maps; image_size is multiplied by LIGHTPROBE_OCT_SIZE = 6 for this.",
     [("void GI::SDFGI::store_probes() { >>> push_constant.image_size[0] *= SDFGI::LIGHTPROBE_OCT_SIZE;", "the map is 6 x 6 per probe"),
      ("void GI::SDFGI::store_probes() { >>> RD::get_singleton()->compute_list_dispatch_threads(compute_list, probe_axis_count * probe_axis_count * SDFGI::LIGHTPROBE_OCT_SIZE, probe_axis_count * SDFGI::LIGHTPROBE_OCT_SIZE, 1);", "one thread per texel")]),
    ("oct-store", INT, "vec3 normal = octahedron_encode(vec2(local_pos) / float(OCT_SIZE));", "main (MODE_STORE)",
     "Each texel's direction is octahedron_encode(local_pos / 6), with no half-texel added: texel k holds the direction for u = k / 6 although its centre is at k + 0.5 (finding F5, read from the code). Its irradiance and radiance come from the same 16 coefficients, summed with l_mult for irradiance and without it for radiance, each encoded as E5B9G9R9 (r | g << 9 | b << 18 | e << 27, channels clamped to [0, 65408]) into two layers: irradiance at the cascade's layer, radiance at cascade + max_cascades.",
     [("vec3 m = sh.rgb * c[i] * 4.0;", "the SH sum"),
      ("uint irradiance_rgbe = rgbe_encode(irradiance);", "the irradiance word"),
      ("float cRed = clamp(color.r, 0.0, 65408.0);", "negative ringing becomes 0, the top is 65408"),
      ("imageStore(lightprobe_texture_data, copy_to[i] + ivec3(0, 0, int(params.max_cascades)), uvec4(radiance_rgbe));", "the radiance layer")]),
    ("oct-border", INT, "copy_to[0] = texture_pos + ivec3(local_pos, 0);", "main (MODE_STORE, border)",
     "Each 6 x 6 map sits in an 8 x 8 block (oct_pos = probe * 8 + 1) with a one-texel border. A texel writes itself, and an edge texel also writes its mirror across the octahedral fold into the border (y = 0 to (5 - x, -1), x = 0 to (-1, 5 - y), and so on); a corner texel writes the three other corners. The border exists for the bilinear taps just outside the interior.",
     [("ivec2 oct_pos = (pos / OCT_SIZE) * (OCT_SIZE + 2) + ivec2(1);", "an 8 x 8 block per probe, the map starts at +1"),
      ("} else if (local_pos.y == 0) {", "an edge texel's mirror (y = 0 shown)"),
      ("} else if (local_pos == ivec2(OCT_SIZE - 1, OCT_SIZE - 1)) {", "the corners copy to three border corners")]),
    ("lightprobe-textures", GI, "tf_octprobes.array_layers = cascades.size() * 2;", "GI::SDFGI::create (lightprobe_data)",
     "lightprobe_data is R32_UINT, a 2D array of 2312 x 136 texels (17 * 17 * 8 by 17 * 8: each probe an 8 x 8 block) with 2 layers per cascade, read through an E5B9G9R9_UFLOAT view: layer c is cascade c's irradiance, layer cascades + c its radiance.",
     [("tf_octprobes.width = probe_axis_count * probe_axis_count * (SDFGI::LIGHTPROBE_OCT_SIZE + 2);", "17 * 17 * 8 = 2312"),
      ("lightprobe_texture = RD::get_singleton()->texture_create_shared(tv, lightprobe_data);", "the E5B9G9R9 view the shaders sample")]),
    ("oct-read", GIG, "vec3 diffuse_posf = (vec3(tex_pos) + vec3(octahedron_encode(cam_normal) * float(SDFGI_OCT_SIZE), 0.0)) * sdfgi.lightprobe_tex_pixel_size;", "sdfvoxel_gi_process (octahedral read)",
     "The reader locates a probe's block (x + z * 17 columns, times 8, plus 1), encodes the normal with the same octahedral map, and samples at block + uv * 6 with hardware bilinear filtering: a texel corner. Because the writer put the direction for u = k / 6 in the texel centred at k + 0.5, a lookup returns about a 50/50 blend of two texels, the map shifted by half a texel (1/12 of the map); the border covers the extra reach (finding F5, read from the code).",
     [("tex_pos.x += probe_base_pos.z * sdfgi.probe_axis_size;", "the z slice goes along x"),
      ("tex_pos.xy = tex_pos.xy * (SDFGI_OCT_SIZE + 2) + ivec2(1);", "8 x 8 blocks, the map starts at +1"),
      ("n.xy = n.xy * 0.5 + 0.5;", "uv in [0, 1], the octahedral coordinate"),
      ("diffuse = textureLod(sampler2DArray(lightprobe_texture, linear_sampler), pos_uvw, 0.0).rgb;", "bilinear, in the cascade's irradiance layer")]),
    ("probe-scroll", INT, "ivec3 read_probe = probe_cell - params.scroll;", "main (MODE_SCROLL)",
     "When a cascade scrolls, each destination probe reads the probe at probe_cell - scroll (scroll is the dirty amount / 8, one probe per step); if that is inside 0..16 the whole history (all layers, 16 rows each) and the average are copied to the scratch textures, to be copied back by SCROLL_STORE. The ring position is unchanged, so each layer's ray set stays aligned. With DIRTY_ALL nothing scrolls and the probes re-converge from what they hold.",
     [("if (all(greaterThanEqual(read_probe, ivec3(0))) && all(lessThan(read_probe, ivec3(params.probe_axis_size)))) {", "the probe existed before the scroll"),
      ("ivec3 read_probe = probe_cell - params.scroll; >>> imageStore(lightprobe_history_scroll_texture, dst_pos, value);", "copied to the scratch texture"),
      ("#ifdef MODE_SCROLL_STORE", "the copy back")]),
    ("probe-scroll-dispatch", GI, "ipush_constant.scroll[0] = dirty.x / probe_divisor;", "GI::SDFGI::render_region (probe scroll)",
     "Inside render_region, when the cascade is not DIRTY_ALL, the probe scroll runs: INTEGRATE_MODE_SCROLL, a barrier, INTEGRATE_MODE_SCROLL_STORE, a barrier, and, if bounce_feedback is above 0, one INTEGRATE_MODE_STORE so the light pass can read the moved probes. The scroll amount is the dirty amount / 8, one probe per step.",
     [("if (bounce_feedback > 0.0) {", "multibounce needs the maps stored"),
      ("ipush_constant.history_index = 0;", "no ray work in these dispatches")]),
    ("stale-cascade-ubo", RFC, "sdfgi->update_cascades();", "RenderForwardClustered::_render_scene (update_cascades)",
     "The cascade UBO is rewritten here, after _update_sdfgi (rfc.cpp, frame-order) has already run render_region and, with it, the probe SCROLL shader that reads cascades.data[].offset: in a frame where a cascade scrolls, that shader sees the previous frame's offsets, so newly exposed probes sample their parent about one step off. It is only a starting guess that re-converges (finding F10, read from the code).",
     [("_update_sdfgi(p_render_data);", "render_region, with the probe scroll, runs inside this call, earlier")]),
    ("parent-seed", INT, "float probe_cell_size_next = cell_to_probe / cascades.data[params.cascade + 1].to_cell;", "main (MODE_SCROLL, parent)",
     "A probe that was not scrolled in (new at the edge) is seeded from the next coarser cascade: its world position is turned into parent probe coordinates and the 8 surrounding parent probes' averages (each divided to a mean) are blended trilinearly, requantized, written into every history layer, and the average set to the value times history_size. A new probe starts as a coarse estimate, not black. In the last cascade it copies what is there.",
     [("float weight = trilinear.x * trilinear.y * trilinear.z;", "trilinear over the 8 parent probes"),
      ("average_light[j] += value * weight;", "the parents' means"),
      ("ivalue *= int(params.history_size); //average needs to have all history added up", "the average holds the whole ring"),
      ("//scroll at the edge of the highest cascade, just copy what is there,", "the last cascade has no parent")]),

    # ---- 10. Shading a pixel -------------------------------------------------------------------------------------------
    ("process-gi", GIG, "void process_gi(ivec2 pos, vec3 vertex, inout vec4 ambient_light, inout vec4 reflection_light) {", "process_gi (gi.glsl)",
     "Per pixel: the view-space position is rebuilt from depth, the normal and roughness are read from the normal-roughness buffer (normal xyz * 2 - 1; roughness folded back if above 0.5 and divided by 127/255), and position, view and normal are rotated by cam_transform to a world orientation with the camera at the origin; the reflection is reflect(-view, normal). sdfgi_process then multiplies y of position, normal and reflection by y_mult and renormalizes. The test normal.length() > 0.5 is always true, as length() of a vec3 is 3 in GLSL, so sky pixels are processed too (read from the code).",
     [("normal_roughness.xyz = normalize(normal_roughness.xyz * 2.0 - 1.0);", "the normal decode"),
      ("roughness /= (127.0 / 255.0);", "the roughness decode"),
      ("if (normal.length() > 0.5) {", "always true: length() of a vec3 is 3"),
      ("vertex.y *= sdfgi.y_mult;", "y_mult applied inside sdfgi_process")]),
    ("process-gi-dispatch", GI, "void GI::process_gi(Ref<RenderSceneBuffersRD> p_render_buffers,", "GI::process_gi",
     "The screen GI pass: at most two views; it makes the ambient and reflection buffers (R16G16B16A16_SFLOAT, at the render size or, with use_half_resolution, half size per axis) and dispatches the compute shader over them. It is called only when SDFGI or VoxelGI is in use, and SDFGI counts as in use only with SDFGI enabled and a viewport debug mode other than Unshaded.",
     [("ERR_FAIL_COND_MSG(p_view_count > 2, \"Maximum of 2 views supported for Processing GI.\");", "two views at most"),
      ("p_render_buffers->create_texture(RB_SCOPE_GI, RB_TEX_AMBIENT, RD::DATA_FORMAT_R16G16B16A16_SFLOAT, usage_bits, RD::TEXTURE_SAMPLES_1, size);", "the ambient buffer"),
      ("RD::get_singleton()->compute_list_dispatch_threads(compute_list, internal_size.x >> 1, internal_size.y >> 1, 1);", "half-resolution dispatch")]),
    ("cascade-select", GIG, "cascade_pos = (cam_pos - sdfgi.cascades[i].position) * sdfgi.cascades[i].to_probe;", "sdfgi_process (cascade selection)",
     "The finest cascade that holds the pixel: cascade_pos is the point relative to the cascade's corner in probe units; the first cascade i in which every component is in [0, 16) is chosen. If none contains it, ambient and reflection are vec4(0), alpha 0, and the opaque shader keeps its sky ambient.",
     [("if (any(lessThan(cascade_pos, vec3(0.0))) || any(greaterThanEqual(cascade_pos, sdfgi.cascade_probe_size))) {", "outside [0, 16): try the next cascade"),
      ("ambient_light = vec4(0);", "no cascade holds it: alpha 0")]),
    ("cascade-blend", GIG, "float blend_from = (float(sdfgi.probe_axis_size - 1) / 2.0) - 2.5;", "sdfgi_process (blend)",
     "Near the edge a pixel blends toward the next cascade: blend_from = 16 / 2 - 2.5 = 5.5 and blend_to = 7.5 probe units, len is the largest component of the camera-relative position in probe units, and blend = smoothstep(5.5, 7.5, len). The finest cascade's result and the next one's, each fully computed (16 probe fetches), are mixed; in the last cascade the alpha becomes 1 - blend, fading to the sky.",
     [("float blend_to = blend_from + 2.0;", "the band is 2 probe units wide"),
      ("len *= max(inner_pos.x, max(inner_pos.y, inner_pos.z));", "the largest component (the L-infinity norm)"),
      ("blend = smoothstep(blend_from, blend_to, len);", "the blend weight"),
      ("ambient_light.a = 1.0 - blend;", "the last cascade fades to the sky")]),
    ("probe-weights", GIG, "float weight = trilinear.x * trilinear.y * trilinear.z * max(0.005, dot(cam_normal, probe_dir));", "sdfvoxel_gi_process (weights)",
     "The point is first moved off the surface by normal_bias (1.1 cells, 0.1375 probe units) and the lattice cell chosen; each of the 8 probes then gets trilinear weight times max(0.005, dot(normal, direction to the probe)), so probes behind the surface count about 200 times less, times max(occlusion, 0.01) if enabled. The irradiance of each probe is read in the normal's direction (times exposure_normalization), and the sum is divided by the total weight.",
     [("cascade_pos += cam_normal * sdfgi.normal_bias;", "the normal bias"),
      ("vec3 trilinear = vec3(1.0) - abs(probe_to_pos);", "the tent weight per axis"),
      ("diffuse_accum += vec4(diffuse * weight * sdfgi.cascades[cascade].exposure_normalization, weight);", "the weighted sum"),
      ("diffuse_accum.rgb /= diffuse_accum.a;", "divided by the total weight")]),
    ("specular-probes", GIG, "specular = mix(specular, textureLod(sampler2DArray(lightprobe_texture, linear_sampler), pos_uvw, 0.0).rgb, (roughness - 0.2) * 1.25);", "sdfvoxel_gi_process (specular)",
     "Specular from the probes: the radiance layer (cascade + max_cascades) in the reflection direction when roughness < 0.99, and above roughness 0.2 the irradiance layer in the same direction is mixed in by (roughness - 0.2) * 1.25, reaching 1 at roughness 1.0.",
     [("if (roughness < 0.99) {", "the radiance read"),
      ("if (roughness > 0.2) {", "the irradiance mix starts at 0.2")]),
    ("specular-trace", GIG, "float softness = 0.2 + min(1.0, roughness * 5.0) * 4.0; //approximation to roughness so it does not seem like a hard fade", "sdfgi_process (reflection march)",
     "For roughness below 0.2 the screen pass also marches the reflection through the cascades' distance and light volumes (radius 60 cells per cascade, start bias (1 + base_blend) * 1.1 plus the normal times 1.4). Each pass takes one step of sample * 255 - 1.1 cells; a sample below softness (0.2 + min(1, 5 r) * 4 cells) adds light * 0.5 * alpha with alpha = 1 - d / softness. The result is mixed with the probe radiance: reflection alpha = alpha * (1 - b) + b with b = min(1, 5 r). There is no iteration counter, and the step can be negative once, as the last.",
     [("float fdistance = textureLod(sampler3D(sdf_cascades[i], linear_sampler), pos, 0.0).r * 255.0 - 1.1;", "the step, with offset 1.1"),
      ("hit_light.rgb *= 0.5; //approximation given value read is actually meant for anisotropy", "the light is halved"),
      ("ray_pos += ray_dir * fdistance;", "the step is unconditional"),
      ("reflection_light.a = alpha * sa + b;", "the mix with the probes")]),
    ("gi-energy", GIG, "ambient_light.rgb *= sdfgi.energy;", "sdfgi_process (energy)",
     "The environment's SDFGI energy scales the ambient and the reflection at the end of the screen pass, which only opaque surfaces read; the forward path for transparents never reads it (finding F11, read from the code).",
     [("reflection_light.rgb *= sdfgi.energy;", "the same for the reflection")]),
    ("gi-compose", SCENE, "ambient_light = mix(ambient_light, buffer_ambient.rgb, buffer_ambient.a);", "main (GI buffers)",
     "The opaque shader mixes the screen pass's buffers into its own sky ambient and reflection by their alpha, so where SDFGI has no cascade (alpha 0) the sky stays. Then reflection probes accumulate, the ambient is multiplied by the ambient occlusion (SSAO included), by albedo, and by 1 - metallic, and added to emission and direct light; no pi or 1/pi appears on this chain.",
     [("indirect_specular_light = mix(indirect_specular_light, buffer_reflection.rgb, buffer_reflection.a);", "the reflection, by its alpha"),
      ("ambient_light *= ao;", "ambient occlusion"),
      ("ambient_light *= albedo.rgb;", "albedo"),
      ("ambient_light *= 1.0 - metallic;", "metallic surfaces take no diffuse")]),
    ("forward-gi", FWDGI, "void sdfgi_process(uint cascade, vec3 cascade_pos, vec3 cam_pos, vec3 cam_normal, vec3 cam_specular_normal, bool use_specular, float roughness, out vec3 diffuse_light, out vec3 specular_light, out float blend) {", "sdfgi_process (scene_forward_gi_inc.glsl)",
     "Transparent surfaces that use SDFGI repeat the lookup in their own fragment shader with the same 8-probe weights, normal bias and blend band, but never trace: use_specular is always true and specular is always the probe maps, even at roughness 0, with different thresholds (radiance below 0.99; irradiance mixed in only above 0.5, by (roughness - 0.5) * 2). The environment's SDFGI energy is never applied here (finding F11, read from the code: the field is not read in this file or in the forward shader).",
     [("if (roughness > 0.5) {", "irradiance only above 0.5 (the screen pass uses 0.2)"),
      ("float weight = trilinear.x * trilinear.y * trilinear.z * max(0.005, dot(cam_normal, probe_dir));", "the same weights"),
      ("blend = smoothstep(blend_from, blend_to, len);", "the same blend band")]),
    ("forward-gi-scene", SCENE, "bool use_specular = true;", "main (forward SDFGI)",
     "In the forward path, use_specular is hard-coded true, so no reflection march is ever made, and at the last cascade the shader itself mixes toward the sky ambient (diffuse = mix(diffuse, ambient_light, blend)); outside every cascade nothing is written and the sky ambient stays. It also applies y_mult to position, normal and reflection like the screen pass.",
     [("cam_pos.y *= sdfgi.y_mult;", "y_mult, as in the screen pass"),
      ("diffuse = mix(diffuse, ambient_light, blend);", "the last cascade fades to the sky ambient")]),
    ("fog-sdfgi", FOGG, "vec3 cascade_pos = (world_pos - sdfgi.cascades[i].position) * sdfgi.cascades[i].to_probe;", "main (volumetric fog, SDFGI)",
     "Volumetric fog reads the non-directional ambient texture, not the maps: per froxel, the finest cascade holding the world position, 8 probes with trilinear weights (times occlusion when use_occlusion is on; no normal term or bias, no cascade blend), fetched with texelFetch, the first cascade used; total_light += ambient * gi_inject. y_mult is declared in the struct (line 132) and never read, while the UBO's cascade positions are y-stretched, so a froxel's height above the camera is looked up as dy where the cascade expects 1.5 dy at the default y scale (finding F6, read from the code).",
     [("vec3 world_pos = mat3(params.cam_rotation) * view_pos;", "no y_mult is applied to the world position"),
      ("float weight = trilinear.x * trilinear.y * trilinear.z;", "trilinear only"),
      ("total_light += ambient_total * params.gi_inject;", "the fog's contribution"),
      ("float y_mult;", "declared in the fog shader's copy of SDFGIData, never read (the next lines of the struct are padding)")]),
    ("fog-sdfgi-enable", FOGC, "bool using_sdfgi = RendererSceneRenderRD::get_singleton()->environment_get_volumetric_fog_gi_inject(p_settings.env) > 0.0001", "Fog::volumetric_fog_update (SDFGI)",
     "The fog uses SDFGI when volumetric_fog_gi_inject is above 0.0001, SDFGI is enabled and the viewport has an SDFGI object; it binds the SDFGIData UBO, the ambient texture and the occlusion texture.",
     [("u.append_id(p_settings.sdfgi->ambient_texture);", "the ambient texture"),
      ("u.append_id(p_settings.sdfgi->occlusion_texture);", "the occlusion texture")]),

    # ---- records and the other claims the outline cites ----------------------------------------------------------------
    ("cascade-ubo", GIH, "struct UBO {", "GI::SDFGI::Cascade::UBO",
     "The per-cascade UBO is 48 bytes: float offset[3] (bytes 0-11), float to_cell (12-15), int32 probe_offset[3] (16-27), uint pad (28-31) and float pad2[4] (32-47); 8 of them, 384 bytes, are uploaded.",
     [("float pad2[4];", "the last 16 bytes")]),
    ("update-cascades", GI, "void GI::SDFGI::update_cascades() {", "GI::SDFGI::update_cascades",
     "Fills the cascade UBO: offset = (cascade position - 64) cells times cell size, in y-squashed space; to_cell = 1 / cell_size; probe_offset = position / 8. The array of 8 is a stack local that is not initialised, so slots at or above the cascade count hold garbage.",
     [("cascade_data[i].to_cell = 1.0 / cascades[i].cell_size;", "cells per metre"),
      ("cascade_data[i].probe_offset[0] = cascades[i].position.x / probe_divisor;", "the position in probes"),
      ("RD::get_singleton()->buffer_update(cascades_ubo, 0, sizeof(SDFGI::Cascade::UBO) * SDFGI::MAX_CASCADES, cascade_data);", "all 8 slots are uploaded")]),
    ("sdfgi-data-ubo", GIH, "struct SDFGIData {", "GI::SDFGIData",
     "The screen pass's UBO, 496 bytes: a 112-byte header (grid_size[3] and max_cascades; use_occlusion, probe_axis_size, probe_to_uvw, normal_bias; lightprobe_tex_pixel_size[3] and energy; lightprobe_uv_offset[3] and y_mult; occlusion_clamp[3], occlusion_renormalize[3], cascade_probe_size[3], each padded to 16 bytes) then 8 ProbeCascadeData of 48 bytes each: position[3] and to_probe, probe_world_offset[3] and to_cell, pad[3] and exposure_normalization.",
     [("struct SDFGIData { >>> float exposure_normalization;", "the last word of each cascade's 48 bytes"),
      ("ProbeCascadeData cascades[SDFGI::MAX_CASCADES];", "8 cascades from byte 112")]),
    ("sdfgi-data-fill", GI, "sdfgi_data.normal_bias = (normal_bias / csize) * sdfgi_data.cascade_probe_size[0];", "GI::SDFGI::pre_process_gi (SDFGIData)",
     "pre_process_gi writes the SDFGIData UBO each frame: grid_size 128, probe_axis_size 17, cascade_probe_size 16, probe_to_uvw 1/16, normal_bias = 1.1 / 128 * 16 = 0.1375 (in probe units), occlusion_clamp (8 - 0.5) / 8 = 0.9375, and per cascade a position relative to the camera, to_probe = 1 / (8 cells), probe_world_offset = position / 8 and exposure_normalization = current / baked. The probe_bias is not in this UBO; it is a push constant.",
     [("float occlusion_clamp = (cascade_voxel_size - 0.5) / cascade_voxel_size;", "the clamp, from the voxel count per probe interval"),
      ("pos -= cam_origin; //make pos local to camera, to reduce numerical error", "positions are relative to the camera"),
      ("c.to_probe = 1.0 / (float(cascade_size) * cascades[i].cell_size / float(probe_axis_count - 1));", "1 / (8 cells) in metres"),
      ("c.exposure_normalization = exposure_normalization / cascades[i].baked_exposure_normalization;", "current over baked exposure")]),
    ("can-sdfgi", RFC, "if (ginstance->voxel_gi_instances[0].is_null() && (ginstance->data->use_baked_light || ginstance->data->use_dynamic_gi)) {", "RenderForwardClustered::_geometry_instance_update",
     "Receiving is separate from voxelizing: a geometry instance can receive SDFGI when it has no lightmap, no VoxelGI and either the baked-light or the dynamic-GI flag, so both Static and Dynamic meshes receive it and Disabled does not; lightmapped or VoxelGI geometry is still voxelized.",
     [("ginstance->can_sdfgi = true;", "the instance receives SDFGI"),
      ("if (p_using_sdfgi && inst->can_sdfgi) {", "and the draw-time flag needs SDFGI to be in use")]),
    ("cull-sdfgi-call", CULL, "scene_render->sdfgi_update(p_render_buffers, p_environment, camera_position);", "RendererSceneCull::_render_scene (sdfgi_update)",
     "The culling step asks the renderer to update SDFGI first, with the camera position, for every render but a reflection probe's; the regions it then reports are turned into cull.sdfgi, and each instance is tested against them (sdfgi-cull).",
     [("cull.sdfgi.region_aabb[i] = scene_render->sdfgi_get_pending_region_bounds(p_render_buffers, i);", "the region boxes come from the renderer"),
      ("cull.sdfgi.cascade_light_index[cull.sdfgi.cascade_light_count] = region_cascade;", "the distinct dirty cascades, in order")]),

    ("process-voxel", DL, "struct ProcessVoxel {", "ProcessVoxel (sdfgi_direct_light.glsl)",
     "The solid cell as the light pass reads it: four uint (position, albedo, light, light_aniso), 16 bytes, with the 26 neighbour bits spread over the top bits of all four. Its albedo comment says the facing bits are 16-21, but the code that writes and reads them uses bits 15-20 (6 bits at shift 15).",
     [("uint albedo; // rgb bits 0-15 albedo, bits 16-21 are normal bits (set if geometry exists toward that side), extra 11 bits for neighbors.", "the comment on the albedo word"),
      ("uint valid_aniso = (voxel_albedo >> 15) & 0x3F;", "the code: facing at bits 15-20")]),
]

CHAPTERS = [
    (1, "Where SDFGI runs",
     ["frame-order", "cull-sdfgi-call", "sdfgi-update", "settings", "project-settings", "mobile-stub", "gles3-stub", "prepass-gate", "prepass-call", "ray-count-table", "ray-count-hint"]),
    (2, "Cascades that follow the camera",
     ["cascade-create", "cell-size-double", "y-mult-table", "probe-axis-count", "probe-position", "scroll-update", "dirty-all-rule", "update-cascades", "cascade-ubo"]),
    (3, "What is voxelized, and which lights",
     ["pending-regions", "region-bounds", "sdfgi-cull", "gi-mode", "can-sdfgi", "light-bake-route", "static-light-cull", "static-lights-index"]),
    (4, "Voxelization",
     ["render-sdfgi", "sdf-double-sided", "sdf-fragment", "albedo-pack", "facing-mask", "emission-pack", "render-textures", "render-region"]),
    (5, "The distance field",
     ["jump-flood-init", "jump-flood", "jump-flood-schedule", "jump-flood-upscale", "sdf-store", "sdf-decode", "solid-cell-store", "solid-cell-struct", "process-voxel"]),
    (6, "Occlusion",
     ["occlusion-dispatch", "occlusion-shells", "occlusion-store", "occlusion-texture", "occlusion-read"]),
    (7, "Light in the voxels",
     ["update-light", "light-schedule", "light-thread", "dynamic-lights", "omni-attenuation", "shadow-march", "aniso-pack", "static-pass", "static-write", "scroll-reinject", "bounce-feedback"]),
    (8, "Probes trace the field",
     ["update-probes", "vogel-rays", "probe-march", "probe-hit", "probe-sky", "sky-define", "sky-radiance-array", "sh-accum", "history-ring", "history-textures"]),
    (9, "From coefficients to maps",
     ["store-probes", "oct-store", "sh-gain", "oct-border", "lightprobe-textures", "oct-read", "probe-scroll", "probe-scroll-dispatch", "stale-cascade-ubo", "parent-seed"]),
    (10, "Shading a pixel",
     ["process-gi-dispatch", "process-gi", "cascade-select", "cascade-blend", "probe-weights", "specular-probes", "specular-trace", "gi-energy", "gi-compose", "forward-gi", "forward-gi-scene", "fog-sdfgi", "fog-sdfgi-enable", "ambient-texture", "sdfgi-data-ubo", "sdfgi-data-fill"]),
    (11, "The frame again",
     ["frame-order"]),
]

# Demonstrations do not exist yet: demo id -> the entries it reads.
DEMOS: dict[str, list[str]] = {}

LAB_LAYOUT = "The same bits, packed by the lab's CPU model (godot/sdfgi)."

# What the lab's records stand for in Godot, with both layouts.
MAPPINGS = [
    {"lab": "SdfgiAlbedo", "engineShort": "render_albedo", "engineName": "The albedo voxel (render_albedo, R16_UINT)",
     "relationship": "exact", "sources": ["albedo-pack", "sdf-fragment", "render-textures"],
     "exact": "R16_UINT, one texel per voxel: bit 0 the solid flag; bits 1-5 blue, 6-10 green, 11-15 red, each clamp(uint(c * 31), 0, 31) (truncated).",
     "labLayout": LAB_LAYOUT},
    {"lab": "SdfgiEmission", "engineShort": "render_emission + render_emission_aniso", "engineName": "The emission voxel (render_emission, R32_UINT, RGBE 8985) and its face shares (render_emission_aniso)",
     "relationship": "exact", "sources": ["emission-pack", "render-textures"],
     "exact": "render_emission: red the top 8 of 9 bits at bits 0-7, green 9 bits at 8-16, blue the top 8 of 9 bits at 17-24, exponent 5 bits at 25-29, bits 30-31 free. render_emission_aniso: six 5-bit shares, +X at bits 0-4, +Y 5-9, +Z 10-14, -X 15-19, -Y 20-24, -Z 25-29, bits 30-31 free.",
     "labLayout": LAB_LAYOUT},
    {"lab": "SdfgiSolidCell", "engineShort": "ProcessVoxel / Cascade::SolidCell", "engineName": "A solid cell of a cascade's list (ProcessVoxel in the shaders, Cascade::SolidCell in gi.h)",
     "relationship": "exact", "sources": ["solid-cell-store", "solid-cell-struct", "process-voxel"],
     "exact": "Four uint32, 16 bytes. position: x bits 0-6, y 7-13, z 14-20, neighbour bits 11-21 in bits 21-31. albedo: blue bits 0-4, green 5-9, red 10-14 (RGB555), facing bits 15-20, neighbour bits 0-10 in bits 21-31. light: RGBE 8985 in bits 0-29, neighbour bits 22-23 in bits 30-31. light_aniso: six 5-bit shares in bits 0-29, neighbour bits 24-25 in bits 30-31. Neighbour bit n (0 to 25, in the order of the offsets (-1,-1,-1) to (1,1,1) without the centre) is set when the voxel at that offset has this cell as its nearest seed.",
     "labLayout": LAB_LAYOUT},
    {"lab": "SdfgiCascade", "engineShort": "Cascade::UBO", "engineName": "The cascade UBO entry (Cascade::UBO, written by update_cascades)",
     "relationship": "exact", "sources": ["cascade-ubo", "update-cascades"],
     "exact": "48 bytes: float offset[3] at 0-11 (the cascade's min corner, (position - 64) cells times cell size, y-squashed), float to_cell at 12-15, int32 probe_offset[3] at 16-27 (position / 8), uint pad at 28-31, float pad2[4] at 32-47. Eight entries, 384 bytes.",
     "labLayout": LAB_LAYOUT},
    {"lab": "SdfgiData", "engineShort": "SDFGIData", "engineName": "The screen pass's UBO (SDFGIData, written by pre_process_gi)",
     "relationship": "exact", "sources": ["sdfgi-data-ubo", "sdfgi-data-fill"],
     "exact": "496 bytes. Header: grid_size[3] at 0-11, max_cascades 12; use_occlusion 16, probe_axis_size 20, probe_to_uvw 24, normal_bias 28; lightprobe_tex_pixel_size[3] 32-43, energy 44; lightprobe_uv_offset[3] 48-59, y_mult 60; occlusion_clamp[3] 64-75, pad 76; occlusion_renormalize[3] 80-91, pad 92; cascade_probe_size[3] 96-107, pad 108. Then 8 ProbeCascadeData from byte 112, 48 bytes each: position[3] 0-11, to_probe 12-15, probe_world_offset[3] 16-27, to_cell 28-31, pad[3] 32-43, exposure_normalization 44-47.",
     "labLayout": LAB_LAYOUT},
    {"lab": "SdfgiHistory", "engineShort": "lightprobe_history_tex", "engineName": "The probe SH history texel (lightprobe_history_tex) and its running sum (lightprobe_average_tex)",
     "relationship": "exact", "sources": ["history-ring", "history-textures", "sh-accum"],
     "exact": "R16G16B16A16_SINT, 289 x 272 texels per layer, history_size layers per cascade (30 by default): column x + 17 * z of the probe, row y * 16 + the coefficient index (0-15), layer = the update's slot; rgb = 1024 * 4 / ray_count * the coefficient's light sum, a = 1024 * 4 / ray_count (512 at 8 rays), each clamped to -32768..32767. The running sum is R32G32B32A32_SINT, 289 x 272, the exact sum of the layers.",
     "labLayout": LAB_LAYOUT},
    {"lab": "SdfgiLightprobe", "engineShort": "lightprobe_data", "engineName": "The octahedral lightprobe texel (lightprobe_data, R32_UINT read as E5B9G9R9_UFLOAT)",
     "relationship": "exact", "sources": ["oct-store", "lightprobe-textures", "oct-border", "oct-read"],
     "exact": "One uint per texel read as E5B9G9R9: red mantissa bits 0-8, green 9-17, blue 18-26, shared 5-bit exponent 27-31 (channels clamped to [0, 65408] before encoding). The array is 2312 x 136 with 2 layers per cascade: layer c irradiance, layer cascades + c radiance; probe p owns the 8 x 8 block at ((p.x + 17 * p.z) * 8, p.y * 8), the 6 x 6 map starting at +1 inside a one-texel border.",
     "labLayout": LAB_LAYOUT},
    {"lab": "SdfgiOcclusion", "engineShort": "occlusion_data", "engineName": "The occlusion word (occlusion_data, R16_UINT, read as R4G4B4A4)",
     "relationship": "exact", "sources": ["occlusion-store", "occlusion-texture", "occlusion-read"],
     "exact": "Eight 4-bit values (occ * 15, truncated) in a 32-bit word: class 0 at bits 12-15, 1 at 8-11, 2 at 4-7, 3 at 0-3, 4 at 28-31, 5 at 24-27, 6 at 20-23, 7 at 16-19. The low 16 bits are the texel (x, y, z + cascade * 128) of a 256 x 128 x (128 * cascades) R16_UINT texture, the high 16 bits the texel (x + 128, y, z + cascade * 128); as R4G4B4A4, R is bits 12-15, G 8-11, B 4-7, A 0-3.",
     "labLayout": LAB_LAYOUT},
]


# The frame graph's nodes (lab/src/graph/; shading.toml) and the engine passes they mirror: the graph panel names each
# node's pass and its sources. A pass with a RENDER_TIMESTAMP is named by it; the others by their function and mode.
PASSES = [
    {"lab": "sdfgiVoxelize", "engine": "Render SDFGI (RenderForwardClustered::_render_sdfgi, MODE_RENDER_SDF)", "sources": ["render-sdfgi", "sdf-fragment"]},
    {"lab": "sdfgiScroll", "engine": "SDFGI Scroll SDF (sdfgi_preprocess.glsl MODE_SCROLL)", "sources": ["scroll-reinject", "render-region"]},
    {"lab": "sdfgiScrollOcclusion", "engine": "SDFGI Scroll SDF (sdfgi_preprocess.glsl MODE_SCROLL_OCCLUSION)", "sources": ["render-region"]},
    {"lab": "sdfgiProbeScroll", "engine": "GI::SDFGI::render_region (sdfgi_integrate.glsl MODE_SCROLL)", "sources": ["probe-scroll", "probe-scroll-dispatch", "parent-seed"]},
    {"lab": "sdfgiProbeScrollStore", "engine": "GI::SDFGI::render_region (sdfgi_integrate.glsl MODE_SCROLL_STORE)", "sources": ["probe-scroll-dispatch"]},
    {"lab": "sdfgiProbeStoreScroll", "engine": "GI::SDFGI::render_region (sdfgi_integrate.glsl MODE_STORE, after a scroll)", "sources": ["probe-scroll-dispatch", "oct-store"]},
    {"lab": "sdfgiJumpFloodInit", "engine": "sdfgi_preprocess.glsl MODE_INITIALIZE_JUMP_FLOOD_HALF", "sources": ["jump-flood-init", "jump-flood-schedule"]},
    {"lab": "sdfgiJumpFlood", "engine": "SDFGI Jump Flood (Half-Size) (sdfgi_preprocess.glsl MODE_JUMPFLOOD)", "sources": ["jump-flood", "jump-flood-schedule"]},
    {"lab": "sdfgiJumpFloodShared", "engine": "SDFGI Jump Flood Optimized (Half-Size) (sdfgi_preprocess.glsl MODE_JUMPFLOOD_OPTIMIZED)", "sources": ["jump-flood", "jump-flood-schedule"]},
    {"lab": "sdfgiUpscale", "engine": "sdfgi_preprocess.glsl MODE_UPSCALE_JUMP_FLOOD", "sources": ["jump-flood-upscale"]},
    {"lab": "sdfgiJumpFloodFull", "engine": "sdfgi_preprocess.glsl MODE_JUMPFLOOD_OPTIMIZED (full size, step 1)", "sources": ["jump-flood-schedule"]},
    {"lab": "sdfgiOcclusion", "engine": "SDFGI Occlusion (sdfgi_preprocess.glsl MODE_OCCLUSION)", "sources": ["occlusion-dispatch", "occlusion-shells"]},
    {"lab": "sdfgiStore", "engine": "SDFGI Store (sdfgi_preprocess.glsl MODE_STORE)", "sources": ["sdf-store", "solid-cell-store", "occlusion-store"]},
    {"lab": "sdfgiStaticLight", "engine": "GI::SDFGI::render_static_lights (sdfgi_direct_light.glsl MODE_PROCESS_STATIC)", "sources": ["static-pass", "static-write"]},
    {"lab": "sdfgiDynamicLight", "engine": "GI::SDFGI::update_light (sdfgi_direct_light.glsl MODE_PROCESS_DYNAMIC)", "sources": ["update-light", "light-schedule", "bounce-feedback"]},
    {"lab": "sdfgiPrepassFS", "engine": "Render Depth Pre-Pass", "sources": ["prepass-gate"]},
    {"lab": "sdfgiProbeProcess", "engine": "GI::SDFGI::update_probes (sdfgi_integrate.glsl MODE_PROCESS)", "sources": ["update-probes", "sh-accum"]},
    {"lab": "sdfgiProbeStore", "engine": "Average SDFGI Probes (GI::SDFGI::store_probes, sdfgi_integrate.glsl MODE_STORE)", "sources": ["store-probes", "oct-store"]},
    {"lab": "sdfgiGatherCS", "engine": "Render GI (GI::process_gi, gi.glsl)", "sources": ["process-gi", "process-gi-dispatch"]},
    {"lab": "sdfgiForwardFS", "engine": "Render Opaque Pass", "sources": ["frame-order", "forward-gi-scene"]},
]


def line_hash(text: str) -> str:
    return hashlib.sha256(text.strip().encode()).hexdigest()[:16]


def find(path: str, token: str) -> int:
    lines = (CHECKOUT / path).read_text(encoding="utf-8", errors="replace").splitlines()
    if " >>> " in token:
        # `anchor >>> text`: the first line holding `text` after the anchor's own line.
        anchor, text = token.split(" >>> ", 1)
        start = find(path, anchor)
        after = [i + 1 for i, l in enumerate(lines) if i + 1 > start and text in l]
        if not after:
            raise SystemExit(f"{path}: `{text}` not found after `{anchor}`")
        return after[0]
    # A plain token must occur exactly once, so its line is never a guess.
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
        # Each entry's line and its hash (to notice a drifted checkout), with its details' lines.
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
    # Every key a chapter, a demonstration, a mapping, or a pass cites must be an entry.
    for _, _, keys in CHAPTERS:
        for k in keys:
            assert k in entries, k
    for keys in DEMOS.values():
        for k in keys:
            assert k in entries, k
    for m in MAPPINGS:
        for k in m["sources"]:
            assert k in entries, k
    for p in PASSES:
        for k in p["sources"]:
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
    out = LAB / "packages/godot_sdfgi/source-index.json"
    out.write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {out}: {len(entries)} entries at {commit}")


main()
