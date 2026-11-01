# title: The shadow march | 阴影步进
# source: shadow-march
# demo: light
# dispatch: in each solid cell's thread, per light | 在每个实心单元格的线程中，逐个光源
def shadow_march(p, light):
    ray = p + sign(dir) * 0.48 * cell + dir * 0.4 * cell  # @bias @@math
    while advance < min(light_distance, exit):          # @march
        d = sdf(ray + dir * advance) * 255 - 1          # @march @@read
        if d < 0.001:                                   # @hit
            return True
        advance += d                                    # @march @@math
    return False   # past the last cascade: lit         # @lit
