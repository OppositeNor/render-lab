# title: SDFGI::update: following the camera | SDFGI::update：跟随相机
# source: scroll-update
# demo: cascades
def update(cascades, camera):
    drag_margin = (CASCADE_SIZE // PROBE_DIVISOR) // 2  # @margin
    for c in cascades:
        c.dirty = ivec3(0)
        p = vec3(camera.x, camera.y * y_mult, camera.z)   # @squash
        cell = ivec3(p / c.cell_size)  # truncates           # @cell
        for j in range(3):
            while cell[j] > c.position[j] + drag_margin:  # @jump
                c.position[j] += drag_margin * 2          # @jump
                c.dirty[j] -= drag_margin * 2             # @dirty
            while cell[j] < c.position[j] - drag_margin:  # @jump
                c.position[j] -= drag_margin * 2          # @jump
                c.dirty[j] += drag_margin * 2             # @dirty
        kept = prod(CASCADE_SIZE - abs(c.dirty))          # @all
        if CASCADE_SIZE**3 - kept > kept // 2:            # @all
            c.dirty = DIRTY_ALL                           # @all
