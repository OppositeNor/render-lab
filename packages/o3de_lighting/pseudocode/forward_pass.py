# title: The forward pass, one pixel | 前向渲染阶段：单个像素
# source: forward-pixel
# demo: room
def pixel_shader(material, pixel, views):
    s = evaluate_surface(material, pixel)          # @surface
    lit = initialize_lighting_data(s, views)       # @init
    tile = light_list(pixel)  # its tile and bin    # @tile
    d = apply_decals(tile, copy(s))  # by value     # @decals
    apply_direct_lights(tile, d, lit)              # @lights
    if transparent or skin:  # opaque: later       # @ibl
        apply_ibl(d, lit)                          # @ibl
    finalize_lighting_data(d, lit)                 # @finalize
    write_targets(s, lit)  # s: before decals      # @targets

def after_forward(targets):
    targets.diffuse += diffuse_ibl(targets)        # @after
    targets.specular += reflections(targets)       # @after
    targets.diffuse *= ssao(depth)                 # @after
    return targets.diffuse + targets.specular      # @merge
