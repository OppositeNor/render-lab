# title: The shadow march | 阴影步进
# source: raymarch
# demo: light
def raymarch(distance, step, origin, direction):
    occlusion = 1.0
    while distance > 0.5:                             # @loop
        advance = sdf(origin) * 255 - 1               # @read
        if advance < 0:                               # @solid
            return 0.0                                # @solid
        occlusion = min(advance, occlusion)           # @partial
        advance = max(step, advance - advance % step)  # @step
        origin += direction * advance                 # @step
        distance -= advance                           # @step
    return occlusion
