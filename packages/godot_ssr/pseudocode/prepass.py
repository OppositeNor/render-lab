# title-text: listing-prepass-title
# source: nr-write
# demo: inputs
# dispatch-text: listing-prepass-dispatch
def encode24(v):                                         # @encode
    m = max(abs(v.x), abs(v.y), abs(v.z))                # @cube
    scale = best_fit_table(collapsed_cube_uv(v))         # @table
    return v / m * scale                                 # @fit
def prepass(normal, roughness, dynamic):
    rgb = encode24(normal) * 0.5 + 0.5                   # @normal
    a = roughness * 127.0 / 255.0                        # @roughness
    if dynamic:                                          # @dynamic
        a = 1.0 - a                                      # @dynamic
    return rgba(rgb, a)                                  # @store
