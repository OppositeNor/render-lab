# title: The box becomes cells | 盒子变成单元格
# source: begin-bake
# demo: bounds
def begin_bake(subdiv, bounds):
    longest = bounds.longest_axis()                  # @longest
    cells[longest] = 1 << subdiv                     # @longest
    for i in other_axes(longest):
        cells[i] = cells[longest]
        side = bounds.size[longest]
        while side / 2 >= bounds.size[i]:            # @shrink
            side /= 2                                # @shrink
            cells[i] >>= 1                           # @shrink
        po2.size[i] = bounds.size[longest]           # @cube
    cell_size = po2.size[longest] / cells[longest]   # @cell
