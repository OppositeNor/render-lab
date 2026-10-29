# title: The shadow march | 阴影步进
# source: shadow-march
# demo: light
def shadow_march(p, light):
    ray = p + sign(dir) * 0.48 * cell + dir * 0.4 * cell  # @bias
    while advance < min(light_distance, exit):          # @march
        d = sdf(ray + dir * advance) * 255 - 1          # @march
        if d < 0.001:                                   # @hit
            return True
        advance += d                                    # @march
    return False   # past the last cascade: lit         # @lit
