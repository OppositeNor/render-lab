# title: Validity and colour | 有效度与颜色
# source: validity
# demo: hit
# dispatch: after the loop, the same thread | 循环之后，同一线程
def validate(t, ray_len):
    v = 0.0 if left_screen(t) or sky_at(t) else 1.0      # @edge
    if within_two_pixels(t) and facing_away(t):          # @back
        v = 0.0                                          # @back
    c = 1.0 - smoothstep(0.0, tolerance, gap(t))         # @confidence
    v *= c * c                                           # @confidence
    v *= margin_blend(reprojected(t))                    # @margin
    v *= ray_len ** fade_in * (1.0 - ray_len) ** fade_out  # @fade
    color = last_frame(reprojected(t)) * v               # @color
    color.rgb /= 1.0 + luminance(color.rgb)              # @compress
    return vec4(color.rgb, v)                            # @store
