# title: A pixel's cascade | 像素的级联
# source: cascade-select
# demo: cascades
def shadow_cascade(p, normal, size=1024):
    q = light.space(p + normal * 2.5 * 2 / size)  # @offset
    for i in range(4):                            # @select
        x, y = cascade[i].texel(q)                # @select
        if 1.5 <= min(x, y) and max(x, y) <= size - 1.5:  # @select
            return i                              # @select
    return None  # outside every cascade: lit     # @none

def directional_light(s, lit, light, pixel):
    v = fullscreen_shadow[pixel]  # R8, blurred   # @fullscreen
    l = -light.direction  # angular size unused   # @angular
    lit.diffuse += diffuse(s, l) * light.lux * v
    lit.specular += specular(s, l) * light.lux * v
