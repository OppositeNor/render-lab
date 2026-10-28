# title: Transmission: GetBackLighting | 透射：GetBackLighting
# source: back-lighting
# demo: transmission
def apply_point_light(light, s, lit):   # a sphere light  # @light
    if light.falloff(s) >= 1: return                 # @light
    t_dist = shadow_thickness(light, s)  # 0: no shadow  # @light
    e = light.candelas / d2 * radius_attenuation     # @light
    lit.diffuse += diffuse(s, lit, e, l)             # @light
    lit.back += back_lighting(s, lit, e, l, t_dist)  # @light
    lit.specular += bulb_specular(s, lit, e, l)      # @light
def back_lighting(s, lit, e, l, t_dist):             # @thick
    if s.mode == THICK_OBJECT:                       # @thick
        t = max(t_dist, s.thick)                 # @thick
        h = -normalize(l + s.n_v * s.distortion)  # @thick
        tr = saturate(dot(lit.v, h)) ** s.power * s.scale  # @thick
        a = exp(-t * s.attenuation) * saturate(1 - t)  # @thick
        return tr * a * e                            # @thick
    if s.mode == THIN_OBJECT and t_dist >= 0:        # @thin
        irr = s.albedo * saturate(s.bias - dot(s.n_v, l))  # @thin
        k = kernel(t_dist * 300 * s.thick, 1 / s.scatter)  # @thin
        return k * e * irr * s.scale / max(1, d2 * s.d_att)  # @thin
def finalize_lighting(s, lit):   # after the AO multiply  # @tint
    lit.diffuse += lit.back * s.tint                 # @tint
