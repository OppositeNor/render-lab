# title: The pre-pass's normal-roughness texel | 预渲染的法线-粗糙度纹素
# source: nr-write
# demo: inputs
# dispatch: per fragment, in the depth pre-pass | 深度预渲染中每个片元
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
