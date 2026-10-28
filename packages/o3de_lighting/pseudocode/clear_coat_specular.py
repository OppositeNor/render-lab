# title: Clear coat: its specular | Clear coat：涂层的高光
# source: clear-coat-specular
# demo: clearcoat
def clear_coat_specular(l, v, coat, base):              # @specular
    h = normalize(l + v)                                # @specular
    fc = fresnel_schlick(dot(h, l), 0.04) * coat.factor  # @specular
    a = max(coat.roughness ** 2, 0.0005)                # @specular
    d = normal_distribution_ggx(dot(coat.n, h), a * a)  # @specular
    g = 0.25 / max(dot(h, l) ** 2, 1e-4)   # Kelemen    # @specular
    coat_spec = d * g * fc * dot(coat.n, l)             # @specular
    return base * (1 - fc) + coat_spec                  # @specular

def forward_ibl_specular(s, lit, v):   # clear coat on  # @ibl
    spec = global_ibl(s, lit) * lit.multi_scatter      # @ibl
    n_dot_v = max(saturate(dot(s.coat.n, v)), 0.01)     # @ibl
    coat = global_ibl(s.coat.n, 0.04, s.coat.roughness)  # @ibl
    fc = fresnel_schlick_with_roughness(                # @ibl
        n_dot_v, 0.04, s.coat.roughness) * s.coat.factor  # @ibl
    return spec * (1 - fc) ** 2 + coat * s.coat.factor  # @ibl
