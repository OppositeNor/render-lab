# title: Decals: ApplyDecal | 贴花：ApplyDecal 函数
# source: decal-apply
# demo: decals
def apply_decal(d, s):    # before InitializeLightingData   # @box
    uvw = transpose(rotation(d.quat)) @ (s.pos - d.pos)  # @box
    uvw /= d.half_size                               # @box
    if any(abs(uvw) > 1): return     # outside its box  # @box
    uv = (uvw.x, -uvw.y) * 0.5 + 0.5                 # @box
    base, nmap = d.diffuse(uv), d.normal_map(uv).rg  # @box
    up = rotation(d.quat).z          # projection axis  # @angle
    d_n = clamp(dot(s.vertex_normal, up), 0.55, 1)   # @angle
    t = smoothstep(0.55, 1, d_n)                     # @angle
    att = lerp(1, t, d.angle_attenuation)            # @angle
    a = base.a * d.opacity * att                     # @albedo
    b = d.color_factor * d.color * base.rgb          # @albedo
    s.albedo = lerp(s.albedo, b, a)                  # @albedo
    s.f0 = lerp(s.f0, b, a * s.metallic)             # @albedo
    a_n = base.a * d.normal_map_opacity * att        # @normal
    n = tangent_space_normal(nmap)   # swap rg, rebuild z  # @normal
    n = normalize(to_world(n, rotation(d.quat)))     # @normal
    s.normal = normalize(lerp(s.normal, n, a_n))     # @normal
