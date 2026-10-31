# title: The distance field | 距离场
# source: sdf-edt
# demo: field
def get_sdf_3d_image(cells, size):
    work = [1e20] * size.volume                       # @seed
    for leaf in leaves(cells):                        # @seed
        work[leaf.index] = 0                          # @seed
    for column in columns(size, "z"):
        edt(work, column)                             # @z
    for column in columns(size, "y"):
        edt(work, column)                             # @y
    for column in columns(size, "x"):
        edt(work, column)                             # @x
    d = int(sqrt(work))                               # @encode
    return 0 if d == 0 else min(d, 254) + 1           # @encode
