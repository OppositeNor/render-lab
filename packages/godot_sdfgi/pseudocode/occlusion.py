# title-text: listing-occlusion-title
# source: occlusion-shells
# demo: occlusion
# dispatch-text: listing-occlusion-dispatch
def occlusion(window, probe):    # 16^3 cells around it
    for shell in range(22):      # |x| + |y| + |z|          # @shells
        for cell in window.shell(shell):
            if cell.solid:                                  # @solid
                occ[cell] = 0                               # @solid @@write
            elif shell == 0:
                occ[cell] = 1                               # @visible @@write
            else:
                closer = cell.neighbours_toward(probe)      # @average
                occ[cell] = mean(occ[c] for c in closer)    # @average @@write
    bias_solid_cells(window)                                # @bias
    dim_behind_surfaces(window)                             # @behind
    pack(uint(occ * 15))     # 4 bits per class             # @store @@write
