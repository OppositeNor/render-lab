# title: The second bounce | 第二次反弹
# source: second-bounce
# demo: bounce
def second_bounce(cell, texture):
    accum = outputs[cell]                             # @direct
    frame = tangent_frame(cell.normal)                # @frame
    for direction, weight in SIX_CONES:               # @cones
        color, dist = 0, 1.5                          # @march
        while dist < max_distance and color.a < 0.95:  # @march
            diameter = max(1, 2 * 0.577 * dist)       # @lod
            sample = texture.lod(pos + dist * direction, log2(diameter))  # @lod
            color += (1 - color.a) * sample           # @accum
            dist += diameter / 2                      # @march
        accum += color * weight * cell.albedo * dynamic_range  # @restore
    outputs[cell] = accum
