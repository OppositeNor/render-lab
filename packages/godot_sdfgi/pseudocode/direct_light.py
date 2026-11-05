# title-text: listing-direct_light-title
# source: update-light
# demo: light
# dispatch-text: listing-direct_light-dispatch
def process_cell(cell, lights, probes, feedback):
    p = (cell.position + 0.5) / to_cell + offset   # @position @@math
    accum = bounce(cell, probes) * feedback        # @bounce @@read
    accum += cell.light * cell.aniso               # @stored @@read
    for light in lights:
        d = length(light.position - p)  # squashed  # @distance @@math
        att = (1 - (d / light.radius)**4)**2       # @attenuation @@math
        att /= d**light.attenuation                # @attenuation @@math
        if shadow_march(p, light):                 # @shadow
            continue
        c = albedo * light.color * light.energy    # @faces @@math
        for k, face in enumerate(ANISO_DIR):       # @faces
            if cell.facing & 1 << k:               # @faces
                accum[k] += max(0, dot(face, l)) * c * att  # @faces @@math
    light_tex[cell] = e5b9g9r9(sum(accum))         # @store @@write
    aniso[cell] = shares(accum)    # six, UNORM    # @store @@write
    for n in cell.neighbours:                      # @neighbours
        light_tex[n] = light_tex[cell]             # @neighbours @@write
