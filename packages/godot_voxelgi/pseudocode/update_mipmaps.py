# title: Filling the coarser levels | 填充更粗的层级
# source: update-mipmaps
# demo: mips
def update_mipmaps(cell):
    light, count = 0, 0
    for child in cell.children:                       # @children
        if child != EMPTY:
            light += outputs[child]                   # @children
            count += 1                                # @children
    divisor = mix(8, count, propagation)              # @divisor
    outputs[cell] = light / divisor                   # @divisor

def write_texture(cell):
    texel = outputs[cell] / dynamic_range             # @write
    texture[level_mip(cell)][cell.position] = (texel, cell.alpha)  # @write
