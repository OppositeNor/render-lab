# title: Prefiltering the specular cubemap | 预过滤镜面反射立方体贴图
# source: cubemap-ggx
# demo: ibl
def filter_ggx_texel(source, mip, direction, samples=256):
    roughness = mip / 5            # one mip per 0.2   # @roughness
    a2 = (roughness ** 2) ** 2                         # @roughness
    n = v = direction              # N = V = R         # @frame
    color, total = 0, 0
    for i in range(samples):
        xi = hammersley(i, samples)                    # @sample
        h = importance_sample_ggx(xi, a2, n)           # @sample
        l = 2 * dot(v, h) * h - v                      # @sample
        if dot(n, l) > 0:
            pdf = ggx_d(dot(v, h), a2) / 4             # @level
            ratio = 1 / (samples * pdf) / texel_angle  # @level
            level = 0.5 * log2(ratio) + 1              # @level
            texel = source.trilinear(l, level)         # @level
            color += texel * dot(n, l)                 # @accumulate
            total += dot(n, l)                         # @accumulate
    return color / total
