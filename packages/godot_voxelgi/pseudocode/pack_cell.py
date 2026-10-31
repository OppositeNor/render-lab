# title: Packing a cell | 打包一个单元格
# source: export-data
# demo: cells
def pack_cell(cell):
    children = [c for c in cell.children]             # @children
    position = x | y << 11 | z << 21                  # @position
    albedo = rgba8(cell.albedo, cell.alpha)           # @albedo
    emission = cell.emission.to_rgbe9995()            # @emission
    normal = snorm8(normalize(cell.normal))           # @normal
    return children, (position, albedo, emission, normal)
