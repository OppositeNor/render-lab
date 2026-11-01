# title: sdfgi_process: shading a pixel | sdfgi_process：为一个像素着色
# source: probe-weights
# demo: pixel
# dispatch: for each screen pixel, in parallel | 每个屏幕像素并行
def sdfgi_process(vertex, normal, reflection, roughness):
    vertex.y *= y_mult; normal.y *= y_mult         # @squash @@math
    c = finest_cascade_holding(vertex)             # @cascade
    pos = (vertex - cascades[c].position) * to_probe  # @cascade @@math
    pos += normal * normal_bias                    # @bias @@math
    for probe in eight_around(pos):                # @probes
        t = 1 - abs(pos - probe)                   # @weight @@math
        facing = max(0.005, dot(normal, to(probe)))  # @weight @@math
        w = t.x * t.y * t.z * facing               # @weight @@math
        diffuse += irradiance(probe, normal) * w   # @diffuse @@read
        specular += radiance(probe, reflection) * w  # @specular @@read
    diffuse /= sum(w); specular /= sum(w)          # @weight @@math
    d = max(abs(vertex * to_probe))                # @blend @@math
    blend = smoothstep(5.5, 7.5, d)                # @blend @@math
    if blend > 0:                                  # @blend
        diffuse = mix(diffuse, next_cascade(), blend)  # @blend @@read
    return diffuse * energy, specular * energy     # @energy
