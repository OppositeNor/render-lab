# title-text: listing-frame-title
# source: alg5
# demo: room
# dispatch-text: listing-frame-dispatch
def reservoir_reuse(prev_frame_reservoirs):
    reservoirs = [None] * image_size
    for q in image:  # @candidates
        reservoirs[q] = ris(q)  # @@write
    for q in image:  # @visibility
        if shadowed(reservoirs[q].y):  # @@read
            reservoirs[q].W = 0  # @@write
    for q in image:  # @temporal
        q_prev = pick_temporal_neighbor(q)
        reservoirs[q] = combine_reservoirs(q, reservoirs[q], prev_frame_reservoirs[q_prev])  # @@read
    for i in range(n):  # @spatial
        for q in image:
            neighbors = pick_spatial_neighbors(q)
            r = [reservoirs[n] for n in neighbors]  # @@read
            reservoirs[q] = combine_reservoirs(q, reservoirs[q], r)  # @@write
    for q in image:  # @shade
        image[q] = shade_pixel(reservoirs[q], q)  # @@math
    return reservoirs
