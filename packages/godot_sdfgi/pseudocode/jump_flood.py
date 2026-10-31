# title: The jump flood | 跳跃洪泛
# source: jump-flood-schedule
# demo: field
def jump_flood(solid):
    seeds = initialize_half(solid)        # 64^3       # @init
    s = 64
    while s > 1:                                       # @regular
        s //= 2                                        # @regular
        seeds = flood(seeds, s, half=True)             # @regular
        if 64 // (s // 2) >= 8:                        # @regular
            break
    while s > 1:                                       # @optimized
        s //= 2                                        # @optimized
        seeds = flood_optimized(seeds, s, half=True)   # @optimized
    seeds = upscale(solid, seeds)         # 128^3      # @upscale
    return flood_optimized(seeds, 1, half=False)       # @final

def flood(seeds, step, half):
    for p in cells(seeds):
        for o in OFFSETS_26:                           # @neighbours
            q = seeds[p + o * step]
            if q.valid and dist(p, q) < dist(p, seeds[p]):  # @nearest
                seeds[p] = q
