# title: Occlusion maps | 遮蔽贴图
# source: occlusion-input
# demo: surface
def get_occlusion_input(tex, uv, factor, use_texture):  # @input
    if not use_texture: return 1.0                   # @input
    return tex.sample(uv).r ** factor   # factor 1  # @input
def evaluate_surface(s, m, uv):                      # @input
    s.diffuse_ao = get_occlusion_input(              # @input
        m.diffuse_map, uv, m.diffuse_factor, m.diffuse_on)  # @input
    s.specular_occlusion = get_occlusion_input(      # @input
        m.cavity_map, uv, m.specular_factor, m.cavity_on)  # @input
def finalize_lighting(s, lit):   # forward pass, last  # @finalize
    lit.specular *= s.specular_occlusion             # @finalize
    lit.diffuse *= s.diffuse_ao                      # @finalize
def forward_targets(s, lit):                         # @targets
    albedo = s.albedo * lit.diffuse_response * s.diffuse_ao  # @targets
    return albedo, s.specular_occlusion   # rgb, alpha  # @targets
# The diffuse IBL pass reads that albedo; the global
# reflection pass multiplies by its alpha; the probes do not.
