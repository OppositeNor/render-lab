# title: VoxelGI in a frame | 一帧中的 VoxelGI
# source: frame-order
# demo: room
def render_scene(scene, camera):
    for probe in visible_voxel_gis:                   # @update
        if probe.version_changed or lights_changed:   # @update
            probe.light_passes()                      # @update
        probe.plot_dynamic_objects()                  # @dynamic
    setup_voxel_gi_instances(visible_voxel_gis)       # @setup
    depth_pre_pass(normal_roughness, voxel_gi_tag)    # @prepass
    gi.process_gi(depth, normal_roughness, tag)       # @screen
    render_opaque(ambient_buffer, reflection_buffer)  # @opaque
