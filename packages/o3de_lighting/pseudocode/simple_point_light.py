# title: The simple point light | 简易点光源
# source: point-light-apply
# demo: pointlight
def apply_simple_point_light(light, s, lit):
    if not light.channels & s.channels: return  # 5 bits  # @channel
    to_light = light.position - s.position
    d2 = dot(to_light, to_light)                       # @falloff
    falloff = d2 * light.inv_attenuation_radius_sq     # @falloff
    if falloff >= 1:
        return            # beyond the radius: skipped   # @skip
    l = normalize(to_light)
    window = (1 - falloff * falloff) ** 2              # @window
    candelas = light.rgb_intensity_candelas            # @intensity
    intensity = candelas / max(d2, 1e-6) * window      # @intensity
    lit_ratio = 1         # no shadows                 # @shadow
    diffuse = get_diffuse_lighting(s, lit, intensity, l)    # @shade
    specular = get_specular_lighting(s, lit, intensity, l)  # @shade
    lit.diffuse_lighting += diffuse * lit_ratio        # @shade
    lit.specular_lighting += specular * lit_ratio      # @shade
