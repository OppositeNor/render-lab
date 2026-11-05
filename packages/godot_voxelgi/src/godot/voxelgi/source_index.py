"""Write packages/godot_voxelgi/source-index.json from the Godot checkout.

Every entry names a file and a token; the line is the token's only
occurrence in the file, so no line number is typed by hand. A token
`anchor >>> text` is the first line holding `text` after the anchor's only
line, for lines that repeat. Run it from anywhere after changing an entry or
moving to another Godot commit:

    python3 packages/godot_voxelgi/src/godot/voxelgi/source_index.py

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
GI = RD + "environment/gi.cpp"
GIH = RD + "environment/gi.h"
FOGC = RD + "environment/fog.cpp"
RFC = RD + "forward_clustered/render_forward_clustered.cpp"
CULL = "servers/rendering/renderer_scene_cull.cpp"
RS = "servers/rendering/rendering_server.cpp"
MOBILE = RD + "forward_mobile/render_forward_mobile.h"
GLES = "drivers/gles3/rasterizer_scene_gles3.cpp"
VGI = "scene/3d/voxel_gi.cpp"
VGIH = "scene/3d/voxel_gi.h"
VOX = "scene/3d/voxelizer.cpp"
VOXH = "scene/3d/voxelizer.h"
G3D = "core/math/geometry_3d.h"
COLOR = "core/math/color.h"
VI3D = "scene/3d/visual_instance_3d.cpp"
DOC = "doc/classes/VoxelGI.xml"
PS = "doc/classes/ProjectSettings.xml"
SH = RD + "shaders/environment/"
VG = SH + "voxel_gi.glsl"
VGD = SH + "voxel_gi_debug.glsl"
GIG = SH + "gi.glsl"
FOGG = SH + "volumetric_fog_process.glsl"
SCENE = RD + "shaders/forward_clustered/scene_forward_clustered.glsl"
FWDGI = RD + "shaders/scene_forward_gi_inc.glsl"
AREA = RD + "shaders/area_lights_inc.glsl"

Entry = tuple[str, str, str, str, str, list[tuple[str, str]]]

ENTRIES: list[Entry] = [
    # ---- 1. Where VoxelGI runs ------------------------------------------------------------------------------------------
    ("frame-order", RFC, "gi.setup_voxel_gi_instances(", "RenderForwardClustered::_render_scene (VoxelGI steps)",
     "In the Forward+ frame, the visible VoxelGIs' per-instance data is written first; the depth pre-pass then writes normal, roughness, and each pixel's VoxelGI tag; _pre_opaque_render runs the screen pass (process_gi) before the opaque pass reads its buffers.",
     [("depth_pass_mode = PASS_MODE_DEPTH_NORMAL_ROUGHNESS_VOXEL_GI;", "with a VoxelGI in view, the pre-pass also writes the tag"),
      ("_pre_opaque_render(p_render_data, using_ssao, using_ssil, using_ssr, using_sdfgi || using_voxelgi", "the screen pass runs here, before the opaque pass")]),
    ("voxelgi-gate", RFC, "using_voxelgi = true;", "RenderForwardClustered::_render_scene",
     "VoxelGI's per-pixel work runs when the frame's cull found at least one VoxelGI instance in view; there is no project setting that turns it on or off.",
     [("bool using_voxelgi = false;", "off unless an instance is in view")]),
    ("relight-gate", GI, "return voxel_gi->last_probe_version != voxel_gi_get_version(voxel_gi->probe);", "GI::voxel_gi_needs_update",
     "The light passes run only when the VoxelGI's version changed (new data, or dynamic range, propagation, or two bounces set) or the light cache found a changed light; with dynamic objects present, an update re-writes the texture from the float buffer and plots them; otherwise it does nothing.",
     [("void GI::voxel_gi_set_dynamic_range(RID p_voxel_gi, float p_range) {", "a setter that bumps the version")]),
    ("light-cache", CULL, "Render VoxelGI >>> cache->has_shadow != RSG::light_storage->light_has_shadow(instance->base) ||", "RendererSceneCull::render_probes (VoxelGI light cache)",
     "Each frame the culler compares every paired light (and every visible directional light) with its cached parameters: shadow, type, transform, colour, energy, intensity, indirect energy, range, attenuation, spot angle and attenuation, and the area light's size, normalization, and texture (a directional light's sky mode); a difference marks the lights dirty and relights the probe.",
     []),
    ("mobile-stub", MOBILE, "virtual void pair_voxel_gi_instances(const RID *p_voxel_gi_instances, uint32_t p_voxel_gi_instance_count) override {}", "RenderForwardMobile::GeometryInstanceForwardMobile::pair_voxel_gi_instances",
     "The Mobile renderer pairs no VoxelGI with its geometry (an empty override), so VoxelGI has no effect there.",
     []),
    ("gles3-stub", GLES, "RID RasterizerSceneGLES3::voxel_gi_instance_create(RID p_voxel_gi) {", "RasterizerSceneGLES3::voxel_gi_instance_create",
     "The Compatibility (GLES3) renderer's VoxelGI entry points are stubs: creating an instance returns an empty RID.",
     []),
    ("voxelgi-doc", DOC, "[VoxelGI] is only supported in the Forward+ rendering method", "VoxelGI (class reference)",
     "The class reference states it: VoxelGI is only supported in the Forward+ rendering method, not Mobile or Compatibility.",
     []),
    ("node-defaults", VGIH, "Subdiv subdiv = SUBDIV_128;", "VoxelGI (node properties)",
     "The node's defaults: subdivision 128 and a size of 20 x 20 x 20 m around the node.",
     [("Vector3 size = Vector3(20, 20, 20);", "the box's size")]),
    ("data-properties", VGIH, "float dynamic_range = 2.0;", "VoxelGIData (properties)",
     "VoxelGIData's defaults: dynamic range 2, energy 1, bias 1.5 cells, normal bias 0, propagation 0.5, interior off, two bounces on.",
     [("float bias = 1.5;", "the cone's start distance, in cells"),
      ("float propagation = 0.5;", "the mip pass's divisor blend"),
      ("bool use_two_bounces = true;", "the second bounce")]),
    ("bias-mismatch", GIH, "float bias = 1.4;", "GI::VoxelGI (server defaults)",
     "The rendering server's own default bias is 1.4, while VoxelGIData's is 1.5; a resource that never calls set_bias runs with 1.4 (read from the code, not tested).",
     []),
    ("quality-setting", RS, "rendering/global_illumination/voxel_gi/quality", "rendering_server.cpp (project settings)",
     "The project setting rendering/global_illumination/voxel_gi/quality: Low, 4 cones (the default, 0), or High, 6 cones; it reaches the screen pass as a push-constant flag.",
     [("GLOBAL_DEF(\"rendering/global_illumination/gi/use_half_resolution\", false);", "the GI buffers at full resolution by default")]),
    # ---- 2. The bake --------------------------------------------------------------------------------------------------------
    ("bake-entry", VGI, "void VoxelGI::bake(Node *p_from_node, bool p_create_visual_debug) {", "VoxelGI::bake",
     "The bake runs once, usually from the editor (VoxelGI.bake is also callable at run time): the subdivision becomes an octree depth of 6 to 9, the voxelizer starts on the box AABB(-size / 2, size), every mesh found is plotted, the bake ends, and the distance field is made before the data is allocated.",
     [("baker.begin_bake(subdiv_value[subdiv], AABB(-size / 2, size), exposure_normalization);", "the box, centred on the node")]),
    ("find-meshes", VGI, "if (geometry != nullptr && geometry->get_gi_mode() != GeometryInstance3D::GI_MODE_STATIC) {", "VoxelGI::is_node_voxel_bakeable",
     "The walk starts at the VoxelGI's parent (or the node given); a node is skipped when it is hidden, and a GeometryInstance3D also when its GI mode is not Static (the default). MeshInstance3D meshes and the meshes of nodes with get_meshes (MultiMesh, CSG, GridMap) are plotted when their bounds touch the box.",
     []),
    ("begin-bake", VOX, "void Voxelizer::begin_bake(int p_subdiv, const AABB &p_bounds, float p_exposure_normalization) {", "Voxelizer::begin_bake",
     "The box's longest axis gets 1 << subdiv cells; each shorter axis halves its cell count while half the longest side still fits it; every axis is then given the longest side, so the octree covers a cube with cells of one size.",
     [("while (axis_size / 2.0 >= po2_bounds.size[i]) {", "the halving"),
      ("cell_size = po2_bounds.size[longest_axis] / axis_cell_size[longest_axis];", "one cell size for every axis")]),
    ("plot-mesh", VOX, "int facecount = index.size() / 3; >>> _plot_face(0, 0, 0, 0, 0, vtxs, normal, uvs, material, po2_bounds);", "Voxelizer::plot_mesh",
     "Every triangle of a triangle surface that touches the original box is sent down the octree from the root cell; triangles are culled, not clipped.",
     []),
    ("plot-face-recurse", VOX, "int half = (1 << cell_subdiv) >> (p_level + 1);", "Voxelizer::_plot_face (going down)",
     "Above the leaves, the triangle is tested against each of the cell's eight octants; a child cell is created only for an octant the triangle overlaps, and the triangle goes down into it.",
     [("if (!Geometry3D::triangle_box_overlap(test_aabb.position + qsize, qsize, p_vtx)) {", "the overlap test"),
      ("uint32_t child_idx = bake_cells.size();", "a new cell at the end of the array")]),
    ("tri-box", G3D, "_FORCE_INLINE_ bool triangle_box_overlap(const Vector3 &boxcenter, const Vector3 boxhalfsize, const Vector3 *triverts) {", "Geometry3D::triangle_box_overlap",
     "A separating-axis test: nine axes from the triangle's edges crossed with the box's axes, the box's three axes, and the triangle's plane; the first axis that separates them returns false.",
     [("return planeBoxOverlap(normal, d, boxhalfsize); /* if true, box and triangle overlaps */", "the plane test, last")]),
    ("leaf-scan", VOX, "//map to a grid average in the best axis for this face", "Voxelizer::_plot_face (a leaf)",
     "At a leaf, the triangle's plane picks its dominant axis; the cell is cut into 4 x 4 columns along it, each column the triangle crosses gives a sample of albedo, emission, and the interpolated normal, and the sums are scaled by 1/16, so alpha is the fraction of columns hit.",
     [("float accdiv = 1.0 / (color_scan_cell_width * color_scan_cell_width);", "one sixteenth"),
      ("//could not in any way get texture information.. so use closest point to center", "no column hit: one sample at the closest point, weight 1/16")]),
    ("material-cache", VOX, "Voxelizer::MaterialCache Voxelizer::_get_material_cache(Ref<Material> p_material) {", "Voxelizer::_get_material_cache",
     "A BaseMaterial3D becomes two 128 x 128 tables: albedo (its texture times its colour, converted to linear) and emission (colour times energy times the bake's exposure); other materials bake white with no emission.",
     []),
    ("sort-cells", VOX, "// it is important that level has more priority (for compute), and that Z has the least,", "Voxelizer::_sort",
     "The cells are sorted by level first, then position, so each level is a contiguous run of the array with the root first; children indices are remapped to the new order.",
     [("ERR_FAIL_COND(sorted_cells[0].level != 0);", "the root first")]),
    ("fixup-leaf", VOX, "//too much fight over normal, zero it", "Voxelizer::_fixup_plot (a leaf)",
     "A leaf's sums are divided by its alpha, an average over every sample it received; a normal shorter than 0.01 (samples that cancel out) becomes zero, and the leaf's alpha becomes 1.",
     [("bake_cells.write[p_idx].alpha = 1.0;", "every leaf fully covered")]),
    ("fixup-parent", VOX, "bake_cells.write[p_idx].alpha = alpha_average / 8.0;", "Voxelizer::_fixup_plot (a parent)",
     "A parent's albedo, emission, and normal are set to zero; only its alpha is kept, the sum of its children's alpha over eight. Its light is made on the GPU, by the mip pass.",
     []),
    # ---- 3. Storage -------------------------------------------------------------------------------------------------------
    ("export-octree", VOX, "children_cells[i * 8 + j] = cells[i].children[j];", "Voxelizer::get_voxel_gi_octree_cells",
     "The octree record: eight 32-bit child indices per cell, 32 bytes; 0xFFFFFFFF marks an empty octant.",
     []),
    ("export-data", VOX, "position |= z << 21;", "Voxelizer::get_voxel_gi_data_cells",
     "The data record, 16 bytes: the position as x | y << 11 | z << 21 in the cell's own level, albedo and alpha as RGBA8, emission as RGB9E5, and the normal as three signed bytes.",
     [("dataptr[i * 4 + 2] = emission.to_rgbe9995();", "emission in RGB9E5"),
      ("uint32_t normal = uint32_t(uint8_t(int8_t(CLAMP(n.x * 127.0, -128, 127))));", "the normal as snorm bytes")]),
    ("rgbe9995", COLOR, "_FORCE_INLINE_ uint32_t to_rgbe9995() const {", "Color::to_rgbe9995",
     "RGB9E5: three 9-bit mantissas sharing one 5-bit exponent, the channels clamped to [0, 0x1FF << 7]; the shaders decode it as m x 2^(e - 15 - 9).",
     []),
    ("level-counts", VOX, "level_count.resize(cell_subdiv + 1); //remember, always x+1 levels for x subdivisions", "Voxelizer::get_voxel_gi_level_cell_count",
     "The cells per level, depth + 1 levels; the renderer uses them to find each level's run in the sorted array.",
     []),
    ("sdf-edt", VOX, "Voxelizer::BakeResult Voxelizer::get_sdf_3d_image(Vector<uint8_t> &r_image, BakeStepFunc p_bake_step_function) const {", "Voxelizer::get_sdf_3d_image",
     "The distance field: every leaf is a seed at squared distance 0, every other texel 1e20; one-dimensional distance transforms run along z, then y, then x, which gives the exact Euclidean distance to the nearest leaf.",
     [("//xy->z", "the first pass, along z"),
      ("//yz->x", "the last pass, along x")]),
    ("edt-1d", VOX, "static void edt(float *f, int stride, int n) {", "edt",
     "The one-dimensional transform (after prideout.net's distance-field article): the lower envelope of parabolas through the squared distances.",
     []),
    ("sdf-encode", VOX, "w[i] = MIN(d, 254u) + 1;", "Voxelizer::get_sdf_3d_image (encode)",
     "One byte per texel: 0 for a solid leaf, else the distance in cells, truncated, at most 254, plus one.",
     []),
    ("allocate-data", GI, "void GI::voxel_gi_allocate_data(", "GI::voxel_gi_allocate_data",
     "The upload: the octree records and the data records become two storage buffers, the distance field a 3D R8_UNORM texture of the octree's size; a GPU version of the field is compiled out (#if 0).",
     [("s.push_back(p_distance_field);", "the CPU field, uploaded")]),
    ("sdf-gpu-disabled", GI, "void GI::voxel_gi_allocate_data( >>> #if 0", "GI::voxel_gi_allocate_data (#if 0)",
     "The GPU distance-field pass (voxel_gi_sdf.glsl) is under #if 0: it never runs.",
     []),
    ("instance-texture", GI, "void GI::VoxelGIInstance::update( >>> tf.format = RD::DATA_FORMAT_R8G8B8A8_UNORM;", "GI::VoxelGIInstance::update (resources)",
     "Each instance owns an RGBA8 3D texture of the octree's size with one mip per level, cleared to zero, and a float buffer of one vec4 per cell.",
     [("mipmap.level = levels.size() - i - 1;", "mip 0 the leaves, the last mip the root")]),
    # ---- 4. The light passes ---------------------------------------------------------------------------------------------
    ("max-lights", GIH, "uint32_t voxel_gi_max_lights = 32;", "GI::voxel_gi_max_lights",
     "At most 32 lights reach the light passes; the shader is compiled with MAX_LIGHTS 32.",
     []),
    ("light-upload", GI, "light_count = MIN(gi->voxel_gi_max_lights, (uint32_t)p_light_instances.size());", "GI::VoxelGIInstance::update (lights)",
     "The lights are written into the light buffer in the probe's cell space: energy times indirect energy, colour to linear, range in cells; a sky-only directional light is skipped with a decrement that leaves a stale slot and drops the last light.",
     [("light_count--;", "the sky-only skip")]),
    ("pass-order", GI, "passes = gi->voxel_gi_is_using_two_bounces(probe) ? 2 : 1;", "GI::VoxelGIInstance::update (passes)",
     "With the lights changed, pass 0 lights the leaves, fills the mips, and writes the texture; with two bounces, pass 1 adds the bounce at the leaves, fills the mips again, and writes the texture again. Without a change, only the write runs.",
     [("passes = 1; //only re-blitting is necessary", "nothing changed: write only")]),
    ("compute-light", VG, "outputs.data[cell_index] = vec4(accum + emission, 0.0);", "voxel_gi.glsl MODE_COMPUTE_LIGHT",
     "Per leaf cell, each light's contribution times the cell's albedo, with the cosine to the cell's normal when it has one, summed, plus the cell's emission, goes into the float buffer.",
     [("#ifdef MODE_COMPUTE_LIGHT >>> accum += max(0.0, dot(normal, -light_dir)) * light;", "the cosine")]),
    ("light-at-pos", VG, "bool compute_light_at_pos(uint index, vec3 pos, vec3 normal, inout vec3 light, inout vec3 light_dir) {", "compute_light_at_pos",
     "A punctual light: its attenuation and direction; a cell facing away or an attenuation below 0.01 gets nothing; with shadows on, a march through the distance field toward the light scales it.",
     [("return false; //not facing the light, or attenuation is near zero", "the back-face test")]),
    ("omni-atten", VG, "float get_omni_attenuation(float distance, float inv_range, float decay) {", "get_omni_attenuation",
     "The windowed inverse-power falloff: (1 - (d / range)^4)^2 x d^-decay, with distances in metres.",
     []),
    ("raymarch", VG, "float raymarch(float distance, float distance_adv, vec3 from, vec3 direction) {", "raymarch",
     "The shadow march: from the light's end toward the cell, it reads the distance field (byte x 255 - 1 cells); a negative value is solid and returns 0; otherwise it steps by that distance, rounded down to whole steps of one cell along the dominant axis, at least one.",
     [("occlusion = min(advance, occlusion);", "a distance below one cell darkens partly")]),
    ("area-light", VG, "bool compute_area_light(uint index, vec3 pos, vec3 normal, inout vec3 light) {", "compute_area_light",
     "An area light: the closest point of its rectangle sets the range window, and the linearly transformed cosine integral over the rectangle gives the light; there is no shadow march.",
     []),
    ("ltc", AREA, "integral = I; // no division by (2.0 * M_PI) for GI calculations", "ltc_evaluate_diff",
     "The diffuse LTC integral, not divided by 2 pi for GI calculations.",
     []),
    ("write-texture", VG, "imageStore(color_tex, ivec3(posu), vec4(outputs.data[cell_index].rgb / params.dynamic_range, albedo.a));", "voxel_gi.glsl MODE_WRITE_TEXTURE",
     "Every cell writes its light divided by the dynamic range, and its coverage, into its texel of its level's mip; RGBA8 keeps 8 bits, so light up to the dynamic range survives.",
     []),
    ("unused-push", VG, "float aniso_strength;", "voxel_gi.glsl Params",
     "The push constant still carries an anisotropy strength (and an emission scale and stack size) that no shader line reads: this version stores no directional light.",
     []),
    # ---- 5. Bounce and mips ---------------------------------------------------------------------------------------------
    ("second-bounce", VG, "vec3 accum = outputs.data[cell_index].rgb;", "voxel_gi.glsl MODE_SECOND_BOUNCE",
     "Each leaf with a normal keeps its direct light and traces six cones in its tangent frame through the texture the first pass wrote.",
     [("float tan_half_angle = 0.577;", "a half-angle of about 30 degrees")]),
    ("bounce-cones", VG, "float cone_weights[MAX_CONE_DIRS] = float[](0.25, 0.15, 0.15, 0.15, 0.15, 0.15);", "voxel_gi.glsl MODE_SECOND_BOUNCE (cones)",
     "The six cones: one along the normal with weight 0.25, five at 60 degrees from it with 0.15 each; the weights sum to 1.",
     []),
    ("bounce-march", VG, "while (dist < max_distance && color.a < 0.95) {", "voxel_gi.glsl MODE_SECOND_BOUNCE (march)",
     "A cone starts 1.5 cells out; its diameter is 2 x tan x distance, at least a cell; it samples the mip log2(diameter), composites front to back, and steps half its diameter, until its alpha reaches 0.95.",
     []),
    ("bounce-restore", VG, "color *= cone_weights[i] * vec4(albedo.rgb, 1.0) * params.dynamic_range; //restore range", "voxel_gi.glsl MODE_SECOND_BOUNCE (result)",
     "Each cone's light times its weight, the cell's albedo, and the dynamic range (undoing the texture's division) is added to the cell's light.",
     []),
    ("update-mipmaps", VG, "float divisor = mix(8.0, count, params.propagation);", "voxel_gi.glsl MODE_UPDATE_MIPMAPS",
     "A parent's light is its children's sum divided by mix(8, count, propagation): at 0 empty octants count as black, at 1 only the occupied ones are averaged.",
     []),
    # ---- 6. Dynamic objects --------------------------------------------------------------------------------------------
    ("gi-mode-flags", VI3D, "RS::get_singleton()->instance_geometry_set_flag(get_instance(), RSE::INSTANCE_FLAG_USE_DYNAMIC_GI, true);", "GeometryInstance3D::set_gi_mode (Dynamic)",
     "GI mode Dynamic sets the dynamic-GI flag: the object is not baked, and a VoxelGI it overlaps lists it among its dynamic geometries.",
     []),
    ("dynamic-list", CULL, "for (Instance *E : probe->dynamic_geometries) {", "RendererSceneCull::render_probes (dynamic objects)",
     "Every frame the VoxelGI is visible, each of its visible dynamic geometries is handed to its update, whether it moved or not.",
     []),
    ("dynamic-oversample", GI, "int multiplier = dynamic_maps[0].size / MAX(MAX(octree_size.x, octree_size.y), octree_size.z);", "GI::VoxelGIInstance::update (dynamic objects)",
     "The first dynamic map is 16 times the octree's largest axis, so an object's box is drawn at 16 texels per cell; the chain then has the 8x, 4x, and 2x maps and one per mip.",
     []),
    ("dynamic-views", GI, "static const Vector3 render_z[6] = {", "GI::VoxelGIInstance::update (six views)",
     "Each dynamic object is drawn six times, orthographic views of its box along +x, +y, +z, -x, -y, -z, with its own material.",
     []),
    ("material-outputs", SCENE, "albedo_output_buffer.rgb = albedo;", "scene_forward_clustered.glsl MODE_RENDER_MATERIAL",
     "The material pass writes albedo with its alpha, the normal, ambient occlusion with roughness and metallic, emission, and depth: the views the dynamic pass lights.",
     []),
    ("dynamic-lighting", VG, "float z = params.z_base + imageLoad(depth, uv_xy).x * params.z_sign;", "voxel_gi.glsl MODE_DYNAMIC_LIGHTING",
     "Each texel's depth gives its voxel along the view; it is lit with the same light functions as the cells (shadows from the baked field), plus its emission. Nothing bounces off it.",
     []),
    ("shrink", VG, "accum_z += z * 0.5; //shrink half too", "voxel_gi.glsl MODE_DYNAMIC_SHRINK",
     "Each shrink sums the non-empty texels of a 2 x 2 block and halves the depth (averaged over those texels); before the first mip it divides the sum by 4, on a mip it divides the light by mix(8, count, propagation) and the alpha by 8.",
     []),
    ("plot-blend", VG, "result.rgb = (color.rgb * color.a * sa + color_blit.rgb * color_blit.a) / result.a;", "voxel_gi.glsl MODE_DYNAMIC_SHRINK_PLOT",
     "The plot splats each texel into two voxels along the view, weighted by its depth's fraction, blended over the texel already there.",
     []),
    ("dynamic-clear", GI, "RD::get_singleton()->texture_clear(texture, Color(0, 0, 0, 0), 0, mipmaps.size(), 0, 1);", "GI::VoxelGIInstance::update (clear)",
     "While dynamic data exists, each update clears the whole texture and writes the static light again from the float buffer before the objects are plotted.",
     []),
    ("dynamic-light-count", GI, "push_constant.light_count = p_light_instances.size();", "GI::VoxelGIInstance::update (dynamic lights)",
     "The dynamic pass's light count is the lights' number, not clamped to the buffer's 32 entries.",
     []),
    # ---- 7. The read side -------------------------------------------------------------------------------------------------
    ("pairing-two", RFC, "void RenderForwardClustered::GeometryInstanceForwardClustered::pair_voxel_gi_instances(", "GeometryInstanceForwardClustered::pair_voxel_gi_instances",
     "A geometry keeps the first two VoxelGIs it overlaps, in the order the culler's set gives them.",
     []),
    ("instance-ubo", GI, "void GI::setup_voxel_gi_instances(", "GI::setup_voxel_gi_instances",
     "Up to 8 visible VoxelGIs per frame: each gets its transform to cells, its bounds, dynamic range x energy, bias, normal bias, blend_ambient = !interior, and exposure.",
     [("gipd.dynamic_range = voxel_gi_get_dynamic_range(base_probe) * voxel_gi_get_energy(base_probe);", "energy folds into the range"),
      ("gipd.blend_ambient = !voxel_gi_is_interior(base_probe);", "interior turns off the environment's share")]),
    ("tag-format", RFC, "return RD::DATA_FORMAT_R8G8_UINT;", "RenderBufferDataForwardClustered::get_voxelgi_format",
     "The tag buffer is R8G8_UINT: two 8-bit instance indices per pixel.",
     []),
    ("tag-write", SCENE, "voxel_gi_buffer.x = index1 & 0xFFu;", "scene_forward_clustered.glsl MODE_RENDER_VOXEL_GI",
     "The pre-pass writes, per pixel, the render indices of the object's two VoxelGIs, or 0xFF for none.",
     []),
    ("process-gi", GI, "void GI::process_gi(", "GI::process_gi",
     "The screen pass: a compute shader over the screen that writes the ambient and reflection buffers, both RGBA16F, at full resolution unless half resolution is set.",
     [("p_render_buffers->create_texture(RB_SCOPE_GI, RB_TEX_AMBIENT, RD::DATA_FORMAT_R16G16B16A16_SFLOAT, usage_bits, RD::TEXTURE_SAMPLES_1, size);", "the ambient buffer")]),
    ("normal-length-quirk", GIG, "if (normal.length() > 0.5) {", "process_gi (gi.glsl)",
     "In GLSL, .length() on a vector is its component count, 3, so this 'valid normal' test is always true.",
     []),
    ("instance-loop", GIG, "if (any(equal(uvec2(i), voxel_gi_tex))) {", "process_gi (gi.glsl, VoxelGI)",
     "For each VoxelGI whose index is in the pixel's tag, voxel_gi_compute adds its light; the sums are then divided by the sum of the edge fades.",
     [("amb_accum /= blend_accum;", "divided by the fades"),
      ("ambient_light = blend_color(amb_accum, ambient_light);", "over SDFGI, by its alpha")]),
    ("voxel-gi-compute", GIG, "void voxel_gi_compute(uint index, vec3 position, vec3 normal, vec3 ref_vec, mat3 normal_xform, float roughness, inout vec4 out_spec, inout vec4 out_diff, inout float out_blend) {", "voxel_gi_compute",
     "The pixel's position, normal, and reflection move into the box's cells; the normal bias pushes it out; outside the box it adds nothing.",
     []),
    ("edge-blend", GIG, "float blend = clamp(1.0 - max(blendv.x, max(blendv.y, blendv.z)), 0.0, 1.0);", "voxel_gi_compute (edge fade)",
     "The edge fade: 1 at the box's centre, 0 at its faces; it weights overlapping VoxelGIs against each other.",
     []),
    ("cones-low", GIG, "light += cone_weights[i] * voxel_cone_trace_45_degrees(", "voxel_gi_compute (Low)",
     "Low quality, the default: four cones 45 degrees from the normal, a quarter each, traced by the 45-degree march.",
     []),
    ("cones-high", GIG, "if (params.high_quality_vct) {", "voxel_gi_compute (High)",
     "High quality: the bounce's six cones, tan 0.577, traced by the general march.",
     []),
    ("cone-trace", GIG, "vec4 voxel_cone_trace(texture3D probe, vec3 cell_size, vec3 pos, vec3 direction, float tan_half_angle, float max_distance, float p_bias) {", "voxel_cone_trace",
     "The general march: from the bias, diameter 2 x tan x distance (at least a cell), mip log2(diameter), front-to-back compositing, half a diameter per step, until alpha 1, the box's diagonal, or the cone leaves the box.",
     []),
    ("cone-trace-45", GIG, "vec4 voxel_cone_trace_45_degrees(texture3D probe, vec3 cell_size, vec3 pos, vec3 direction, float max_distance, float p_bias) {", "voxel_cone_trace_45_degrees",
     "The 45-degree march: the radius equals the distance, so each step doubles the distance and climbs one mip.",
     []),
    ("spec-cone", GIG, "vec4 irr_light = voxel_cone_trace(", "voxel_gi_compute (reflection)",
     "One more cone along the reflection, its half-angle's tangent tan(roughness x 0.5 x pi x 0.99), the roughness already squared.",
     [("roughness *= roughness;", "squared first")]),
    ("range-restore", GIG, "float cone_weights[cone_dir_count] = float[](0.25, 0.25, 0.25, 0.25); >>> light.rgb *= voxel_gi_instances.data[index].dynamic_range * voxel_gi_instances.data[index].exposure_normalization;", "voxel_gi_compute (range)",
     "The cones' light is multiplied back by dynamic range x energy and the exposure; with interior set, the alpha is forced to 1.",
     []),
    # ---- 8. The composite ---------------------------------------------------------------------------------------------------
    ("gi-buffer-mix", SCENE, "ambient_light = mix(ambient_light, buffer_ambient.rgb, buffer_ambient.a);", "scene_forward_clustered.glsl (GI buffers)",
     "The opaque pass mixes the environment's ambient toward the buffer's by the buffer's alpha, and the sky's reflection the same way; then ambient occlusion, albedo, SSIL, SSR, and the environment BRDF follow.",
     [("indirect_specular_light = mix(indirect_specular_light, buffer_reflection.rgb, buffer_reflection.a);", "the reflection")]),
    ("forward-gi-transparent", RFC, "surf->sort.uses_forward_gi = 1;", "RenderForwardClustered::_fill_render_list",
     "Only surfaces of the transparent list trace VoxelGI per pixel in the forward shader; opaque ones read the buffers.",
     [("surf->sort.uses_forward_gi = 0;", "reset for every surface")]),
    ("forward-cones", FWDGI, "float cone_angle_tan = 0.98269;", "scene_forward_gi_inc.glsl voxel_gi_compute",
     "The forward version always traces four cones (tan 0.98269), stops at alpha 0.95, and mixes the environment in per cone.",
     []),
    ("fog-inject", FOGG, "vec4 slight = textureLod(sampler3D(voxel_gi_textures[i], linear_sampler_with_mipmaps), position, float(j));", "volumetric_fog_process.glsl",
     "Volumetric fog reads every mip of the texture at the froxel's point and composites them front to back; it traces no cone.",
     []),
    ("debug-noop", VGD, "color_interp.xyz *params.dynamic_range;", "voxel_gi_debug.glsl (lighting view)",
     "The debug lighting view's multiplication by the dynamic range has no assignment, so it shows the stored texel, darker by the dynamic range.",
     []),
    # Chapter 8: how it is kept fast (research.md section 9).
    ("update-gate", GI, "if (has_dynamic_object_data || p_update_light_instances || p_dynamic_objects.size()) {", "GI::VoxelGIInstance::update (the gate)",
     "The light passes, the mips, and the texture writes run only when the lights changed, or dynamic objects are or were plotted; a static scene with unchanged lights does no light-pass work on the GPU at all.",
     []),
    ("level-dispatch", GI, "passes = gi->voxel_gi_is_using_two_bounces(probe) ? 2 : 1; >>> push_constant.cell_offset = mipmaps[i].cell_offset;", "GI::VoxelGIInstance::update (one dispatch per level)",
     "Each level is one dispatch over its run of the sorted cells: its first cell and count go in the push constant, (count + 63) / 64 workgroups; the leaves first, then each coarser level after a barrier.",
     [("passes = gi->voxel_gi_is_using_two_bounces(probe) ? 2 : 1; >>> RD::get_singleton()->compute_list_add_barrier(compute_list);", "a barrier between levels")]),
    ("local-size", VG, "layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;", "voxel_gi.glsl",
     "The light passes' workgroup: 64 cells in a row; a thread past the level's count returns.",
     []),
    ("half-res-setting", GI, "half_resolution = GLOBAL_GET(\"rendering/global_illumination/gi/use_half_resolution\");", "GI::init",
     "The project setting rendering/global_illumination/gi/use_half_resolution, off by default and read at start-up, for VoxelGI's and SDFGI's screen pass.",
     []),
    ("half-res-buffers", GI, "size.x >>= 1;", "GI::process_gi (buffers)",
     "At half resolution the ambient and reflection buffers (RGBA16F) are half the width and half the height.",
     []),
    ("half-res-dispatch", GI, "compute_list_dispatch_threads(compute_list, internal_size.x >> 1, internal_size.y >> 1, 1);", "GI::process_gi (dispatch)",
     "The screen pass then runs on a quarter of the threads.",
     []),
    ("half-res-shift", GIG, "pos <<= 1;", "gi.glsl main (half resolution)",
     "Each thread reads the top-left pixel of its 2 x 2 block (its position doubled), traces it, and stores the result at its own, halved, texel: one pixel of four is traced.",
     [("pos >>= 1;", "back to the buffer's texel")]),
    ("gi-buffer-fetch", SCENE, "vec4 buffer_ambient = textureLod(sampler2D(ambient_buffer, SAMPLER_LINEAR_CLAMP), coord, 0.0);", "scene_forward_clustered.glsl (GI buffers)",
     "The opaque pass reads the buffers at the pixel's centre with a plain bilinear sampler: at half resolution this is the whole upsample, with no depth weights; only with MSAA does it first move to the neighbour whose normal is closest.",
     [("if (implementation_data.gi_upscale_for_msaa) {", "the normal-aware pick, with MSAA only")]),
    ("half-res-doc", PS, "<member name=\"rendering/global_illumination/gi/use_half_resolution\"", "ProjectSettings.xml",
     "The documentation: it improves performance significantly, at the cost of artifacts that may be visible on polygon edges, less noticeable as the resolution grows.",
     []),
    ("quality-doc", PS, "<member name=\"rendering/global_illumination/voxel_gi/quality\"", "ProjectSettings.xml",
     "The documentation says High gives more precise lighting and better reflections; in gi.glsl only the diffuse cones change, and the reflection cone is the same at both qualities.",
     []),
]

CHAPTERS = [
    (1, "Where VoxelGI runs",
     ["frame-order", "voxelgi-gate", "relight-gate", "voxelgi-doc", "mobile-stub", "gles3-stub", "node-defaults", "data-properties", "quality-setting"]),
    (2, "Baking the octree",
     ["bake-entry", "find-meshes", "begin-bake", "plot-mesh", "plot-face-recurse", "tri-box", "leaf-scan", "material-cache"]),
    (3, "What the bake stores",
     ["fixup-leaf", "fixup-parent", "sort-cells", "level-counts", "export-octree", "export-data", "rgbe9995", "sdf-edt", "edt-1d", "sdf-encode", "allocate-data", "sdf-gpu-disabled", "instance-texture"]),
    (4, "Lighting the cells",
     ["relight-gate", "light-cache", "max-lights", "light-upload", "compute-light", "light-at-pos", "omni-atten", "raymarch", "area-light", "ltc", "write-texture", "unused-push"]),
    (5, "The second bounce and the mips",
     ["update-mipmaps", "pass-order", "second-bounce", "bounce-cones", "bounce-march", "bounce-restore"]),
    (6, "Moving objects",
     ["gi-mode-flags", "dynamic-list", "dynamic-oversample", "dynamic-views", "material-outputs", "dynamic-lighting", "shrink", "plot-blend", "dynamic-clear", "dynamic-light-count"]),
    (7, "Reading it per pixel",
     ["pairing-two", "instance-ubo", "tag-format", "tag-write", "process-gi", "normal-length-quirk", "instance-loop", "voxel-gi-compute", "edge-blend", "cones-low", "cone-trace-45", "cones-high", "cone-trace", "spec-cone", "range-restore"]),
    (8, "Keeping it fast",
     ["relight-gate", "update-gate", "light-cache", "sort-cells", "level-dispatch", "local-size", "pass-order", "raymarch", "light-at-pos", "cones-low", "cones-high", "quality-doc", "spec-cone", "half-res-setting", "half-res-buffers", "half-res-shift", "half-res-dispatch", "gi-buffer-fetch", "half-res-doc"]),
    (9, "Into the final image",
     ["gi-buffer-mix", "forward-gi-transparent", "forward-cones", "fog-inject", "debug-noop", "bias-mismatch", "frame-order"]),
]

DEMOS: dict[str, list[str]] = {
    "room": ["frame-order", "data-properties"],
    "bounds": ["begin-bake"],
    "octree": ["plot-face-recurse", "tri-box", "leaf-scan"],
    "cells": ["export-data", "fixup-leaf", "fixup-parent", "sort-cells"],
    "field": ["sdf-edt", "sdf-encode"],
    "light": ["compute-light", "raymarch", "area-light"],
    "bounce": ["second-bounce", "bounce-march"],
    "mips": ["update-mipmaps", "write-texture"],
    "dynamic": ["dynamic-views", "shrink", "plot-blend"],
    "pixel": ["voxel-gi-compute", "cone-trace", "cone-trace-45"],
    "cost": ["update-gate", "level-dispatch", "cones-low", "half-res-dispatch", "gi-buffer-fetch"],
    "compose": ["gi-buffer-mix"],
}

LAB_LAYOUT = "The same bits, packed by the lab's CPU model (godot/voxelgi, storage.rs)."

# The frame graph's nodes (lab/src/graph/) and the engine passes they mirror:
# the graph panel names each node's pass and its sources.
PASSES = [
    {"lab": "voxelgiLightCS", "engine": "voxel_gi.glsl MODE_COMPUTE_LIGHT", "sources": ["compute-light"]},
    {"lab": "voxelgiBounceCS", "engine": "voxel_gi.glsl MODE_SECOND_BOUNCE", "sources": ["second-bounce"]},
    {"lab": "voxelgiMipmapsCS", "engine": "voxel_gi.glsl MODE_UPDATE_MIPMAPS", "sources": ["update-mipmaps"]},
    {"lab": "voxelgiWriteCS", "engine": "voxel_gi.glsl MODE_WRITE_TEXTURE", "sources": ["write-texture"]},
    {"lab": "voxelgiPrepassFS", "engine": "Render Depth Pre-Pass", "sources": ["frame-order"]},
    {"lab": "voxelgiForwardFS", "engine": "Render Opaque Pass", "sources": ["frame-order", "material-outputs"]},
    {"lab": "voxelgiGiCS", "engine": "process_gi (gi.glsl)", "sources": ["instance-loop", "voxel-gi-compute"]},
    {"lab": "voxelgiComposeCS", "engine": "scene_forward_clustered.glsl (GI buffers)", "sources": ["gi-buffer-fetch", "gi-buffer-mix"]},
]

MAPPINGS = [
    {"lab": "VoxelgiCellChildren", "engineShort": "CellChildren", "engineName": "The octree record (CellChildren, voxel_gi.glsl; get_voxel_gi_octree_cells)",
     "relationship": "exact", "sources": ["export-octree"],
     "exact": "uint children[8], 32 bytes per cell, the cell's index in the sorted array times 32; child j at byte 4 j is the octant (j & 1 for x, j & 2 for y, j & 4 for z), 0xFFFFFFFF when empty.",
     "labLayout": LAB_LAYOUT},
    {"lab": "VoxelgiCellData", "engineShort": "CellData", "engineName": "The data record (CellData, voxel_gi.glsl; get_voxel_gi_data_cells)",
     "relationship": "exact", "sources": ["export-data", "rgbe9995"],
     "exact": "Four uint32, 16 bytes: position x bits 0-10, y 11-20, z 21-31 (in the cell's level); albedo R bits 0-7, G 8-15, B 16-23, alpha 24-31, each uint(clamp(v * 255, 0, 255)) (truncated); emission RGB9E5 (R 0-8, G 9-17, B 18-26, exponent 27-31, bias 15); normal three int8 (n * 127) at bits 0-7, 8-15, 16-23, bits 24-31 zero.",
     "labLayout": LAB_LAYOUT},
    {"lab": "VoxelgiTexel", "engineShort": "VoxelGIInstance::texture", "engineName": "A texel of the light texture (R8G8B8A8_UNORM 3D, one mip per level)",
     "relationship": "exact", "sources": ["instance-texture", "write-texture"],
     "exact": "RGBA8 UNORM; mip m is the octree's size >> m (at least 1), holding level depth - m; rgb = the cell's light / dynamic_range, a = its coverage (1 for a leaf, the children's sum / 8 above); texels without a cell are 0.",
     "labLayout": "The same texels, in an RGBA8 3D texture the lab's light passes write on the GPU (update.rs); storage.rs's LightTexture is their CPU mirror."},
    {"lab": "VoxelgiInstance", "engineShort": "VoxelGIData (UBO)", "engineName": "The screen pass's per-instance UBO entry (GI::VoxelGIData, setup_voxel_gi_instances)",
     "relationship": "conceptual", "sources": ["instance-ubo"],
     "exact": "112 bytes, std140: float xform[16] (to cells, from the camera-relative position), float bounds[3], float dynamic_range (x energy), float bias, float normal_bias, uint blend_ambient, uint mipmaps, float exposure_normalization, pad. Up to 8 entries.",
     "labLayout": "The lab's shading uniforms (toCellOrigin, toCellScale, bounds, dynamicRange, bias, normalBias, mipCount), for its one VoxelGI, from world positions."},
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
    out = LAB / "packages/godot_voxelgi/source-index.json"
    out.write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {out}: {len(entries)} entries at {commit}")


main()
