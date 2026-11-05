# title-text: listing-frame-title
# source: process-ssr
# demo: room
# dispatch-text: listing-frame-dispatch
def render_scene(env, camera):
    render_depth_pre_pass(normal_roughness=True)        # @prepass
    if env.ssr_enabled:                                  # @gate
        downsample(depth, normal_roughness)              # @downsample
        for level in range(1, mipmaps):                  # @hiz
            hiz[level] = max_of_four(hiz[level - 1])     # @hiz
        trace(hiz, last_frame, reprojection)             # @trace
        for level in range(1, mipmaps):                  # @filter
            ssr[level] = blur(ssr[level - 1])            # @filter
        resolve(ssr, mip_level)                          # @resolve
    render_opaque(ssr_final)                             # @opaque
    last_frame = copy(frame_colour)                      # @copy
