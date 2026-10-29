# title: sdfgi_process: shading a pixel | sdfgi_process：为一个像素着色
# source: probe-weights
# demo: pixel
def sdfgi_process(vertex, normal, reflection, roughness):
    vertex.y *= y_mult; normal.y *= y_mult         # @squash
    c = finest_cascade_holding(vertex)             # @cascade
    pos = (vertex - cascades[c].position) * to_probe  # @cascade
    pos += normal * normal_bias                    # @bias
    for probe in eight_around(pos):                # @probes
        t = 1 - abs(pos - probe)                   # @weight
        facing = max(0.005, dot(normal, to(probe)))  # @weight
        w = t.x * t.y * t.z * facing               # @weight
        diffuse += irradiance(probe, normal) * w   # @diffuse
        specular += radiance(probe, reflection) * w  # @specular
    diffuse /= sum(w); specular /= sum(w)          # @weight
    d = max(abs(vertex * to_probe))                # @blend
    blend = smoothstep(5.5, 7.5, d)                # @blend
    if blend > 0:                                  # @blend
        diffuse = mix(diffuse, next_cascade(), blend)  # @blend
    return diffuse * energy, specular * energy     # @energy
