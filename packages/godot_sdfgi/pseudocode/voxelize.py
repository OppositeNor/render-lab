# title: The SDF pass: a fragment becomes a voxel | SDF pass：一个片元变成一个体素
# source: sdf-fragment
# demo: voxels
def render_sdf_fragment(vertex, normal, albedo, emission):
    local = sdf_to_bounds * vertex                       # @position
    cell = sdf_offset + ivec3(local * sdf_size)          # @position
    a = 1                              # solid           # @albedo
    a |= min(uint(albedo.r * 31), 31) << 11              # @albedo
    a |= min(uint(albedo.g * 31), 31) << 6               # @albedo
    a |= min(uint(albedo.b * 31), 31) << 1               # @albedo
    albedo_grid[cell] = a                                # @albedo
    k = argmax(dot(normal, d) for d in ANISO_DIR)        # @facing
    atomic_or(facing_grid[cell], 1 << k)                 # @facing
    if length(emission) > 0.001:                         # @emission
        emission_grid[cell] = rgbe8985(emission)         # @emission
