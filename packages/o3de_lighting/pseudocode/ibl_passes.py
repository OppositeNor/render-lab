# title: The global IBL passes | 全局 IBL 渲染阶段
# source: reflection-global
# demo: ibl
def forward_targets(s, lit):   # 8-bit targets        # @targets
    a = s.albedo * lit.diffuse_response * lit.diffuse_ao  # @targets
    albedo = unorm8(a)                                # @targets
    f0 = unorm8(s.specular_f0)                        # @targets
    return albedo, f0, unorm8(s.roughness_linear)     # @targets

def diffuse_global(albedo, n):     # added to diffuse  # @diffuse
    irradiance = diffuse_env.sample(n)   # E / pi     # @diffuse
    return albedo * irradiance * 2 ** ibl_exposure    # @diffuse

def reflection_global(f0, roughness, n, v, weight):  # @reflect
    r = rotate(reflect(-v, n), ibl_orientation)       # @reflect
    radiance = specular_env.sample_level(r, roughness * 5)  # @reflect
    n_dot_v = max(saturate(dot(n, v)), 0.01)          # @split
    a, b = brdf_texture.sample((n_dot_v, 1 - roughness))  # @split
    comp = multiscatter_compensation(f0, (a, b))      # @split
    spec = weight * radiance * comp * (f0 * a + b)    # @split
    spec *= 2 ** ibl_exposure * specular_occlusion    # @split
    return spec   # its own target; a composite adds it  # @split
