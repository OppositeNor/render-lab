# title: A pixel's cones | 一个像素的锥体
# source: voxel-gi-compute
# demo: pixel
def voxel_gi_compute(probe, position, normal, reflection, roughness):
    pos = probe.to_cell(position) + normal * normal_bias  # @space
    if outside(pos, probe.bounds):                    # @space
        return
    blend = edge_fade(pos, probe.bounds)              # @blend
    if quality == HIGH:
        light = sum(w * cone_trace(pos, d, 0.577) for d, w in SIX_CONES)  # @high
    else:
        light = sum(0.25 * cone_trace_45(pos, d) for d in FOUR_CONES)  # @low
    spec = cone_trace(pos, reflection, tan(roughness * 0.99 * pi / 2))  # @spec
    light *= dynamic_range * exposure                 # @range
    if interior:
        light.a = 1.0                                 # @interior
    out_diffuse += light * blend                      # @blend
