# title-text: listing-blur-title
# source: mip-bias
# demo: blur
# dispatch-text: listing-blur-dispatch
def blur_level(roughness, ray_len):
    angle = min(roughness, 0.999) * pi / 2               # @cone
    a = 2.0 * tan(angle) * ray_len                       # @base
    r = a * (sqrt(a * a + 4 * ray_len ** 2) - a) / (4 * ray_len)  # @circle
    mip = log2(r * max(width, height) / 16.0 + 1.0)      # @mip
    mip *= clamp(1.25 - ray_len, 0, 1) ** 0.2            # @fade
    return mip / 14.0                                    # @store
