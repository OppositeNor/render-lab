# title: Light in a solid cell | 实心单元格中的光
# source: update-light
# demo: light
def process_cell(cell, lights, probes, feedback):
    p = (cell.position + 0.5) / to_cell + offset   # @position
    accum = bounce(cell, probes) * feedback        # @bounce
    accum += cell.light * cell.aniso               # @stored
    for light in lights:
        d = length(light.position - p)  # squashed  # @distance
        att = (1 - (d / light.radius)**4)**2       # @attenuation
        att /= d**light.attenuation                # @attenuation
        if shadow_march(p, light):                 # @shadow
            continue
        c = albedo * light.color * light.energy    # @faces
        for k, face in enumerate(ANISO_DIR):       # @faces
            if cell.facing & 1 << k:               # @faces
                accum[k] += max(0, dot(face, l)) * c * att  # @faces
    light_tex[cell] = e5b9g9r9(sum(accum))         # @store
    aniso[cell] = shares(accum)    # six, UNORM    # @store
    for n in cell.neighbours:                      # @neighbours
        light_tex[n] = light_tex[cell]             # @neighbours
