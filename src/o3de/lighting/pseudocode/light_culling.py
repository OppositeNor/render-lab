# title: Tiled light culling | 分块光源剔除
# source: cull-point-light
# demo: culling
def tile_prepare(tile):   # one group per 16 x 16 tile   # @prepare
    z = [distance(s) for s in tile.samples]          # @prepare
    tile.near, tile.far = min(z), max(z)  # sky: near   # @prepare
    tile.mask = OR(bit32(zi, tile) for zi in z)      # @prepare
    span = log2(tile.far - tile.near)                # @prepare
    tile.bins = 2 ** round(clamp(span, 0, 3))        # @prepare
def cull(tile, lights):                              # @cull
    box = view_aabb(tile)                            # @cull
    for i, light in enumerate(lights):  # in order   # @cull
        if not sphere_meets_box(light, box): continue  # @cull
        run = bits32(light.z - light.r, light.z + light.r)  # @cull
        bins = bins_of(tile.mask & run, tile.bins)   # @cull
        if bins: tile.list.append(i << 16 | bins)    # @cull
    tile.list.append(END_OF_GROUP)   # each type     # @cull
def forward_pixel(p, tile):                          # @forward
    f = (p.z - tile.near) / (tile.far - tile.near)   # @forward
    b = int(min(f, 0.999999) * tile.bins)            # @forward
    for i in remapped(tile, b):   # this bin only    # @forward
        apply_simple_point_light(i, p)               # @forward
