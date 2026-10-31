# title: Occlusion around a probe | 探针周围的遮蔽
# source: occlusion-shells
# demo: occlusion
def occlusion(window, probe):    # 16^3 cells around it
    for shell in range(22):      # |x| + |y| + |z|          # @shells
        for cell in window.shell(shell):
            if cell.solid:                                  # @solid
                occ[cell] = 0                               # @solid
            elif shell == 0:
                occ[cell] = 1                               # @visible
            else:
                closer = cell.neighbours_toward(probe)      # @average
                occ[cell] = mean(occ[c] for c in closer)    # @average
    bias_solid_cells(window)                                # @bias
    dim_behind_surfaces(window)                             # @behind
    pack(uint(occ * 15))     # 4 bits per class             # @store
