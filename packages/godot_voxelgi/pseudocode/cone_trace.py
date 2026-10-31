# title: Tracing a cone | 追踪一个锥体
# source: cone-trace
# demo: pixel
def voxel_cone_trace(pos, direction, tan_half_angle, bias):
    dist, color = bias, 0                             # @start
    while dist < max_distance and color.a < 1.0:      # @loop
        diameter = max(1, 2 * tan_half_angle * dist)  # @diameter
        if outside_box(pos + dist * direction):       # @exit
            break                                     # @exit
        sample = texture.lod(pos + dist * direction, log2(diameter))  # @sample
        color += (1 - color.a) * sample               # @accum
        dist += diameter / 2                          # @step
    return color
