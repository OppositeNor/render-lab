# title-text: listing-cascade_update-title
# source: scroll-update
# demo: cascades
# dispatch-text: listing-cascade_update-dispatch
def update(cascades, camera):
    drag_margin = (CASCADE_SIZE // PROBE_DIVISOR) // 2  # @margin @@math
    for c in cascades:
        c.dirty = ivec3(0)
        p = vec3(camera.x, camera.y * y_mult, camera.z)   # @squash @@math
        cell = ivec3(p / c.cell_size)  # truncates           # @cell @@math
        for j in range(3):
            while cell[j] > c.position[j] + drag_margin:  # @jump
                c.position[j] += drag_margin * 2          # @jump @@math
                c.dirty[j] -= drag_margin * 2             # @dirty @@math
            while cell[j] < c.position[j] - drag_margin:  # @jump
                c.position[j] -= drag_margin * 2          # @jump @@math
                c.dirty[j] += drag_margin * 2             # @dirty @@math
        kept = prod(CASCADE_SIZE - abs(c.dirty))          # @all @@math
        if CASCADE_SIZE**3 - kept > kept // 2:            # @all
            c.dirty = DIRTY_ALL                           # @all
