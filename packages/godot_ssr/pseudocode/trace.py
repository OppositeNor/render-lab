# title-text: listing-trace-title
# source: loop
# demo: trace
# dispatch-text: listing-trace-dispatch
def trace(p):
    pos = view_pos(p) + geom_normal * bias               # @bias
    ray = reflect(view_dir, normal)                      # @reflect
    ray_s = (screen(pos + ray) - screen(pos)) / abs_dz   # @screen
    t = start_cell_edge(ray_s)                           # @start
    level = 0                                            # @start
    while level >= 0 and steps > 0 and t < t_max:        # @loop
        cell = floor(at(t).xy * size(level))             # @cell
        depth_t = (hiz[level][cell] - start.z) * dz      # @depth
        edge_t = t_at_cell_edge(cell, level)             # @edge
        hit = depth_t <= edge_t                          # @hit
        if level == 0 and behind_by(cell) > tolerance:   # @tolerance
            hit = None                                   # @tolerance
        t = max(t, depth_t) if hit else edge_t           # @advance
        level += -1 if hit else (0 if hit is None else 1)  # @level
        steps -= 1                                       # @level
