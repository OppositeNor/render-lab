# title: STORE: the distance field and the solid cells | STORE：距离场与实心单元格
# source: sdf-store
# demo: field
# dispatch: for each cell of the cascade, in parallel | 级联的每个单元格并行
def store(pos, seeds, albedo, facing, emission):
    seed = seeds[pos]  # @@read
    if seed.xyz == pos:                                 # @solid
        d = 0                                           # @solid
    else:
        d = 1 + length(seed.xyz - pos)                  # @distance @@math
    sdf[pos] = d / 255     # R8_UNORM                   # @distance @@write
    if d == 0:
        n = 0
        for bit, o in enumerate(NEIGHBOURS_26):         # @neighbours
            if seeds[pos + o].xyz == pos:               # @neighbours @@read
                n |= 1 << bit                           # @neighbours @@math
        cells.append(ProcessVoxel(                      # @cell @@write
            position=pos.x | pos.y << 7 | pos.z << 14,  # @cell
            albedo=albedo >> 1 | facing << 15,          # @cell
            light=emission, light_aniso=emission_aniso, # @cell
            neighbours=n))                              # @cell
