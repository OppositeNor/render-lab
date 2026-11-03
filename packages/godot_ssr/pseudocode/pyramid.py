# title: Downsample and the depth pyramid | 下采样与深度金字塔
# source: hiz-max
# demo: pyramid
# dispatch: one thread per texel, a dispatch per level | 每个纹素一个线程，每级一次调度
def downsample(depth, normal_roughness, p):
    winner = nearest_of_2x2(depth, 2 * p)                # @winner
    hiz[0][p] = depth[winner]                            # @level0
    half_normal[p] = normal_roughness[winner]            # @normal
def hiz_level(m, p):
    d = hiz[m - 1][2 * p]                                # @reduce
    for o in [(1, 0), (0, 1), (1, 1)]:                   # @reduce
        d = max(d, hiz[m - 1][2 * p + o])                # @max
    if parent_is_odd:                                    # @odd
        d = max(d, third_row_and_column(m - 1, p))       # @odd
    hiz[m][p] = d                                        # @store
