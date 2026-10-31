# title: Direct light in a cell | 单元格中的直接光
# source: compute-light
# demo: light
def compute_light(cell, lights):
    pos = cell.position + 0.5
    accum = 0
    for light in lights:
        if light.type == AREA:
            accum += area_light(light, pos) * cell.albedo  # @area
            continue
        to_light, attenuation = light_vector(light, pos)   # @vector
        if facing_away(cell.normal, to_light):              # @cosine
            continue
        if light.has_shadow:
            attenuation *= raymarch(pos, light)             # @shadow
        light_in = light.color * attenuation * light.energy  # @atten
        accum += max(0, dot(cell.normal, to_light)) * light_in * cell.albedo  # @cosine
    outputs[cell] = accum + cell.emission                  # @result
