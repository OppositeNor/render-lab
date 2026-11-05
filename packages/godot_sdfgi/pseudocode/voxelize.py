# title-text: listing-voxelize-title
# source: sdf-fragment
# demo: voxels
# dispatch-text: listing-voxelize-dispatch
def render_sdf_fragment(vertex, normal, albedo, emission):
    local = sdf_to_bounds * vertex                       # @position @@math
    cell = sdf_offset + ivec3(local * sdf_size)          # @position @@math
    a = 1                              # solid           # @albedo @@math
    a |= min(uint(albedo.r * 31), 31) << 11              # @albedo @@math
    a |= min(uint(albedo.g * 31), 31) << 6               # @albedo @@math
    a |= min(uint(albedo.b * 31), 31) << 1               # @albedo @@math
    albedo_grid[cell] = a                                # @albedo @@write
    k = argmax(dot(normal, d) for d in ANISO_DIR)        # @facing @@math
    atomic_or(facing_grid[cell], 1 << k)                 # @facing @@write
    if length(emission) > 0.001:                         # @emission
        emission_grid[cell] = rgbe8985(emission)         # @emission @@write
