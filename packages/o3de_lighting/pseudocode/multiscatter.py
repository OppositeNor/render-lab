# title: Multiscatter compensation | 多次散射补偿
# source: multiscatter
# demo: multiscatter
def lighting_data_init(s, lit):
    uv = (lit.n_dot_v, 1 - s.roughness_linear)         # @lookup
    lit.brdf = brdf_texture.sample(uv)   # (A, B)      # @lookup

# enabled: StandardPBR's material property, off unless set
def get_multiscatter_compensation(f0, brdf, enabled):
    if not enabled:
        return 1
    single = brdf.x + brdf.y   # energy with F = 1     # @energy
    return 1 + f0 * (1 / single - 1)                   # @compensate

# SpecularGGX: D * G * NdotL * F * compensation      # @apply
