# title: SpecularGGX: D, G, and F | SpecularGGX：D、G 与 F
# source: specular-ggx
# demo: specular
def specular_ggx(v, l, n, f0, n_dot_v, a2, compensation):
    h = normalize(l + v)                               # @angles
    n_dot_h = saturate(dot(n, h))                      # @angles
    n_dot_l = saturate(dot(n, l))                      # @angles
    h_dot_v = saturate(dot(h, v))                      # @angles
    b = (n_dot_h * a2 - n_dot_h) * n_dot_h + 1         # @d
    d = a2 / (PI * b * b)                              # @d
    gv = n_dot_l * sqrt(n_dot_v**2 * (1 - a2) + a2)    # @g
    gl = n_dot_v * sqrt(n_dot_l**2 * (1 - a2) + a2)    # @g
    g = 0.5 / max(gv + gl, 1e-8)  # / (4 NdotL NdotV)  # @g
    f = f0 + (1 - f0) * (1 - h_dot_v) ** 5             # @f
    return d * g * n_dot_l * f * compensation          # @product

def get_specular_lighting(s, lit, intensity, l):
    spec = specular_ggx(lit.dir_to_camera, l, s.normal,
                        s.specular_f0, lit.n_dot_v,
                        s.roughness_a2, lit.compensation)
    return spec * intensity                            # @light
