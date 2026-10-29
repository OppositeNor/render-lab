# title: Regions, geometry, and static lights | 区域、几何体与静态光源
# source: static-light-cull
# demo: regions
def cull_for_sdfgi(scenario, regions):
    for inst in scenario.instances:   # no frustum test        # @all
        for j, region in enumerate(regions):
            if not inst.aabb.intersects(region.aabb):
                continue
            if inst.is_light:
                if inst.bake_mode == STATIC and \
                   region.cascade <= inst.max_sdfgi_cascade:      # @static
                    lights[region.cascade].append(inst)          # @stored
            elif inst.flags & USES_BAKED_LIGHT:  # gi_mode Static  # @geometry
                geometry[j].append(inst)                         # @geometry

def render_static_lights(cascade_index, lights):
    for i, cascade in enumerate(cascade_index):
        for light in lights[i]:   # by position, not cascade      # @read
            ...
