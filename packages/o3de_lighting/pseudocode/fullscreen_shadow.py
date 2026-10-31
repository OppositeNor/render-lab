# title: The sun's shadow, per pixel | 太阳阴影，逐像素
# source: fullscreen-shadow
# demo: cascades
def fullscreen_shadow(pixel, depth):
    p = world_position(pixel, depth)               # @position
    n = face_normal(depth, pixel)  # from depth     # @normal
    q = p + n * normal_bias * 2 / map_size         # @offset
    first, last = cascade_range(q)                 # @range
    if first is None:                              # @range
        return 1.0  # outside every cascade: lit   # @range
    for c in range(first, last + 1):               # @compare
        u, v, z = shadowmap[c].project(q)          # @compare
        z -= shadow_bias / depth_range[c]          # @compare
        if z > shadowmap[c].depth(u, v):           # @compare
            return 0.0  # filter None: binary      # @compare
    return 1.0

def forward_reads(pixel):
    raw = fullscreen_shadow(pixel, depth)          # @buffer
    return depth_aware_blur(raw, radius=8)         # @blur
