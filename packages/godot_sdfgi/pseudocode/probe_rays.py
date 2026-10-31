# title: A probe's rays and its history | 探针的光线与历史
# source: probe-march
# demo: probes
def update_probe(probe, history_index):
    rotation = hashf3(hash3(world_offset + probe))      # @rotation
    total = history_size * ray_count     # 30 * 8 = 240 # @rays
    for i in range(ray_count):
        dir = vogel(history_index + i * history_size,   # @rays
                    total, rotation)                    # @rays
        pos = probe + dir / max(abs(dir)) * bias        # @bias
        while advance < exit:
            d = sdf(pos + dir * advance) * 255 - 1      # @march
            if d < 0.05:                                # @hit
                light = light_tex(hit) * aniso(hit, normal)  # @hit
                break
            advance += d                                # @march
        else:
            light = sky_radiance(dir, mip=2)            # @sky
        sh += light * sh_basis(dir)     # 16 terms      # @sh
    value = ivec4(sh * 4 / ray_count * 1024)            # @fixed
    average += value - history[history_index]           # @ring
    history[history_index] = value                      # @ring
