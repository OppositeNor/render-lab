# title: One frame of SDFGI | SDFGI 的一帧
# source: frame-order
# demo: room
def render_scene(env, camera):
    sdfgi.update(env, camera.origin)                  # @update
    regions = sdfgi.pending_regions()                 # @cull
    for region in regions:                            # @voxelize
        sdfgi.render_region(region)                   # @voxelize
    if static_lights_culled:                          # @static
        sdfgi.render_static_lights(static_lights)     # @static
    sdfgi.update_cascades()                           # @light
    sdfgi.pre_process_gi(dynamic_lights)              # @light
    sdfgi.update_light()                              # @light
    if depth_pre_pass:                                # @probes
        render_depth_pre_pass()                       # @probes
        sdfgi.update_probes(env, sky)                 # @probes
    sdfgi.store_probes()                              # @store
    gi.process_gi(depth, normal_roughness)            # @screen
    render_opaque()                                   # @screen
