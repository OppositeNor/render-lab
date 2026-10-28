# title: SSAO | 屏幕空间环境光遮蔽
# source: ssao-compute
# demo: ssao
def ssao(depth):           # AoParent -> SsaoParent       # @half
    half = depth[0::2, 0::2]  # upper left of each 2 x 2  # @half
    ao = [occlusion(half, px) for px in half]        # @compute
    ao = blur(blur(ao, half, axis=1), half, axis=0)  # @blur
    return upsample(ao, half, depth)  # x diffuse  # @upsample
def occlusion(half, px):     # SsaoCompute              # @compute
    p, n = position(half, px), depth_normal(half, px)  # @compute
    occ = total = 0                                  # @compute
    for o in local_12(px) + outer_10x4(px):  # radius 0.05  # @compute
        d = o - p                                    # @compute
        w = 1 / (dot(d, d) + 0.07)                   # @compute
        occ += saturate(dot(normalize(d), n)) * w    # @compute
        total += w                                   # @compute
    return unorm8((1 - occ / total * s) ** 2.2)    # @compute
def blur(v, half, axis):     # FastDepthAwareBlur, IIR  # @blur
    return iir_8px_both_ways(v, half, 2 / 3, slope=50)  # @blur
def upsample(v, half, depth):  # DepthUpsample         # @upsample
    return taps_3_4_near_1_4_far(v, half, by=depth)  # @upsample
