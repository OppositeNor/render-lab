# title: A dynamic object | 动态物体
# source: dynamic-views
# demo: dynamic
def plot_dynamic(obj, texture):
    box = obj.aabb_in_cells() * 16                    # @box
    for view in SIX_AXES:                             # @views
        maps = render_material(obj, view, box)        # @views
        lit = light_texels(maps)                      # @light
        for level in shrink_levels:                   # @shrink
            lit = shrink_2x2(lit)                     # @shrink
            if level.mip >= 0:                        # @plot
                blend_over(texture[level.mip], lit)   # @plot
