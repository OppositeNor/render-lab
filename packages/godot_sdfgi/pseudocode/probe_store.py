# title: STORE: coefficients to octahedral maps | STORE：系数变成八面体贴图
# source: oct-store
# demo: maps
def store_probe(probe, texel):            # 6 x 6 texels
    n = octahedron_decode(texel / 6)      # no + 0.5   # @direction
    sh = average / history_size / 1024                  # @mean
    for i in range(16):
        m = sh[i] * sh_basis(n)[i] * 4                  # @gain
        irradiance += m * L_MULT[i]   # 1, 2/3, 1/4, 0  # @irradiance
        radiance += m                                   # @radiance
    maps[probe, texel] = e5b9g9r9(irradiance)           # @store
    maps[probe, mirror(texel)] = e5b9g9r9(irradiance)   # @border
