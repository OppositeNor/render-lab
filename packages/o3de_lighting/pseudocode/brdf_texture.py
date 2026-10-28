# title: The BRDF texture, one texel | BRDF 贴图的一个 texel
# source: brdf-texture
# demo: multiscatter
def brdf_texel(x, y, size=256, samples=1024):
    roughness = (y + 0.5) / size                       # @inputs
    a = roughness ** 2                                 # @inputs
    n_dot_v = (x + 0.5) / size                         # @inputs
    v = (sqrt(1 - n_dot_v ** 2), 0, n_dot_v)           # @inputs
    scale, bias = 0, 0
    for i in range(samples):
        xi = hammersley(i, samples)                    # @sample
        h = importance_sample_ggx(xi, a, (0, 0, 1))    # @sample
        l = normalize(2 * dot(v, h) * h - v)           # @sample
        if l.z > 0:
            g = smith_correlated(n_dot_v, l.z, a * a)
            w = g * 4 * dot(v, h) * l.z / h.z          # @weight
            fc = (1 - dot(v, h)) ** 5                  # @weight
            scale += (1 - fc) * w                      # @accumulate
            bias += fc * w                             # @accumulate
    texel = (scale / samples, bias / samples)          # @store
    store(x, size - 1 - y, texel)                      # @store
