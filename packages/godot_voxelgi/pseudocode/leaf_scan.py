# title: A leaf samples its triangle | 叶子对三角形采样
# source: leaf-scan
# demo: octree
def sample_leaf(cell, tri, box):
    axis = dominant_axis(tri.plane.normal)            # @axis
    for i in range(4):                                # @columns
        for j in range(4):                            # @columns
            column = box.column(axis, i, j)           # @columns
            if not triangle_box_overlap(column, tri):
                continue
            p = tri.closest_point(ray_hit(column))    # @hit
            albedo += material.albedo                 # @accum
            normal += tri.normal_at(p)                # @accum
            alpha += 1                                # @accum
    cell.add(albedo / 16, normal / 16, alpha / 16)    # @add
