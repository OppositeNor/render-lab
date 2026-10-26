# title: Reflection probes | 反射 probe
# source: probe-render-outer
# demo: reflections
def reflections(px, probes):  # per sample, largest first  # @stencil
    for p in probes:   # inner box less 0.05 m          # @stencil
        if px.stencil & 0x3F and inside(p.inner, px):  # @stencil
            px.stencil += 1                          # @stencil
    s = px.stencil & 0x3F                            # @stencil
    w = sum(p.weight(px) for p in probes             # @weight
            if s == 3 and inside(p.outer, px))       # @weight
    r = (0.4, 0.4, 0.4)   # the target's clear value  # @global
    if s == 3:                                       # @global
        r = (1 - min(w, 1)) * global_ibl(px)         # @global
        for p in probes:                             # @outer
            if inside(p.outer, px) and px.ibl:       # @outer
                r += probe(p, px) * p.weight(px) / max(1,w)  # @outer
    if s >= 3:                                       # @inner
        for p in probes:   # smaller ones overwrite  # @inner
            if inside(p.inner, px) and px.ibl:       # @inner
                r = probe(p, px)                     # @inner
    return r if s >= 1 else 0   # ReflectionComposite  # @composite
