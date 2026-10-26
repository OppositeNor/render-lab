# title: Decals in the tile lists | tile 列表中的 decal
# source: decal-cull
# demo: decals
def cull_decals(tile, decals, box):   # group 0        # @cull
    for i, d in enumerate(decals):                   # @cull
        c = world_to_view(d.pos)                     # @cull
        if d.max_draw_distance > 0 and \
           dot(c, c) > d.max_draw_distance ** 2: continue  # @cull
        r2 = dot(d.half_size, d.half_size)  # a sphere  # @cull
        if not sphere_meets_box(c, r2, box): continue  # @cull
        run = bits32(c.z - sqrt(r2), c.z + sqrt(r2))  # @cull
        bins = bins_of(tile.mask & run, tile.bins)   # @cull
        if bins: shared.append(i << 16 | bins)       # @cull
    # SortDecals: a bitonic sort across the threads  # @sort
    shared.sort(key=lambda e: (sort_key(e >> 16), e >> 16))  # @sort
    tile.list += shared + [END_OF_GROUP]             # @sort
def forward_pixel(s, tile, bin):                     # @forward
    for i in remapped(tile, bin, group=0):           # @forward
        apply_decal(decals[i], s)    # lowest key first  # @forward
    lit = initialize_lighting_data(s)  # then lights  # @forward
