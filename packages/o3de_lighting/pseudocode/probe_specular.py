# title: One probe's specular | 单个探针的镜面反射
# source: probe-specular
# demo: reflections
def probe(p, px):              # ComputeProbeSpecular  # @probe
    d = reflect(-px.v, px.n)                         # @probe
    if p.parallax:             # on by default       # @parallax
        q = to_probe(p, px.pos)                      # @parallax
        t = min(max((p.outer - q) / d, (-p.outer - q) / d))  # @parallax
        d = normalize(from_probe(p, q + d * t))      # @parallax
    c = p.cubemap(d, mip=px.roughness * 5)           # @probe
    spec = c * px.comp * (px.f0 * px.brdf.x + px.brdf.y)  # @probe
    return spec * 2 ** p.exposure  # no specular occlusion  # @probe
def weight(p, px):  # 1 in the inner box, 0 at the outer  # @weight
    if inside(p.inner, px): return 1                 # @weight
    q = abs(to_probe(p, px.pos))                     # @weight
    c = clamp(q, -p.inner, p.inner)                  # @weight
    d = normalize(q - c)                             # @weight
    t = min(abs(p.outer - q) / d)  # to the outer box  # @weight
    return saturate(t / length(q + d * t - c))       # @weight
