# title: The directional light's cascades | 方向光的级联阴影
# source: cascade-splits
# demo: cascades
def update_cascades(cam, light, r=0.9, n=4):
    near = cam.near
    far = min(cam.far, light.shadow_far)     # @far
    for i in range(n - 1):
        t = (i + 1) / n
        uni = near + (far - near) * t        # @split
        log = near * (far / near) ** t       # @split
        border[i] = r * log + (1 - r) * uni  # @split
    border[n - 1] = far
    for i in range(n):
        c, R = slice_sphere(cam, i, border)  # @sphere
        lo = light.space(c) - R              # @box
        hi = light.space(c) + R              # @box
        lo.y = min(lo.y, caster_side())      # @box
        step = (hi - lo) / 1024
        lo = floor(lo / step) * step         # @snap
        hi = floor(hi / step) * step         # @snap
        cascade[i] = ortho(lo, hi)
