# title: Finishing the cells | 收尾单元格
# source: fixup-leaf
# demo: cells
def fixup_plot(cell, level):
    if level == cell_subdiv:
        cell.albedo /= cell.alpha                     # @leaf
        cell.normal /= cell.alpha                     # @leaf
        if length(cell.normal) < 0.01:                # @fight
            cell.normal = 0                           # @fight
        cell.alpha = 1.0                              # @leaf
        return
    cell.albedo = cell.emission = cell.normal = 0     # @parent
    alpha = 0
    for child in cell.children:
        fixup_plot(child, level + 1)
        alpha += child.alpha                          # @alpha
    cell.alpha = alpha / 8                            # @alpha
