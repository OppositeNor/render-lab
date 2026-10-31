# title: A triangle goes down the octree | 三角形沿八叉树下行
# source: plot-face-recurse
# demo: octree
def plot_face(cell, level, tri, box):
    if level == cell_subdiv:                         # @leaf
        return sample_leaf(cell, tri, box)           # @leaf
    for octant in range(8):                          # @octants
        child = box.half(octant)                     # @octants
        if outside_grid(child):
            continue
        if not triangle_box_overlap(child, tri):     # @sat
            continue                                 # @sat
        if cell.children[octant] == EMPTY:           # @create
            cell.children[octant] = new_cell(level + 1)  # @create
        plot_face(cell.children[octant], level + 1, tri, child)  # @down
