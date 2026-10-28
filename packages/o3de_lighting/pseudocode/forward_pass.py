# title: The forward pass, one pixel | Forward pass 的一个像素
# source: forward-pixel
# demo: room
def pixel_shader(material, pixel, views):
    s = evaluate_surface(material, pixel)            # @surface
    lit = LightingData()
    initialize_lighting_data(s, views, lit)          # @init
    for light in simple_point_lights(pixel):          # @lights
        apply_simple_point_light(light, s, lit)       # @lights
    if transparent:   # opaque: the full-screen passes  # @ibl
        apply_ibl(s, lit)                             # @ibl
    finalize_lighting_data(s, lit)                    # @finalize
    diffuse = lit.diffuse_lighting                    # @targets
    specular = lit.specular_lighting + lit.emissive   # @targets
    write_targets(s, lit)  # albedo, F0, roughness    # @targets
    return diffuse, specular

def after_forward(diffuse, specular, targets):
    diffuse += diffuse_global_fullscreen(targets)     # @ibl
    specular += reflection_global_fullscreen(targets)  # @ibl
    return diffuse + specular   # DiffuseSpecularMerge  # @merge
