# title-text: listing-sort_cells-title
# source: sort-cells
# demo: cells
def sort_cells(cells):
    key = lambda c: (c.level, c.x, c.y, c.z)          # @key
    order = sorted(range(len(cells)), key=key)        # @key
    assert cells[order[0]].level == 0                 # @root
    remap = {old: new for new, old in enumerate(order)}  # @remap
    for c in cells:
        c.children = [remap[ch] for ch in c.children]  # @remap
    counts = [count_level(cells, l) for l in levels]  # @runs
    offset[l] = sum(counts[:l])                       # @runs
