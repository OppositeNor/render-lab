# title-text: listing-combine-title
# source: alg4
# demo: spatial
# dispatch-text: listing-combine-dispatch
def combine_reservoirs(q, reservoirs):
    s = Reservoir()
    for r in reservoirs:
        s.update(r.y, p_hat(q, r.y) * r.W * r.M)  # @update @@math
    s.M = sum(r.M for r in reservoirs)  # @count
    if unbiased:  # @unbiased
        z = sum(r.M for r in reservoirs  # @z
                if p_hat(r.q, s.y) > 0)  # @@read
        s.W = s.wsum / (p_hat(q, s.y) * z)  # @@math
    else:
        s.W = s.wsum / (p_hat(q, s.y) * s.M)  # @finish @@math
    return s
