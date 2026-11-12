# title-text: listing-ris-title
# source: alg3
# demo: ris
# dispatch-text: listing-ris-dispatch
def ris(q):
    r = Reservoir()
    for i in range(M):
        x = sample_light_by_power()  # @candidate @@read
        w = p_hat(q, x) / p(x)  # @weight @@math
        r.update(x, w)  # @update
    r.W = r.wsum / (p_hat(q, r.y) * r.M)  # @finish @@math
    return r


def update(r, x, w):
    r.wsum += w  # @@math
    r.M += 1
    if rand() < w / r.wsum:  # @keep
        r.y = x  # @@write


def shade_pixel(r, q):
    return f(q, r.y) * r.W  # @shade @@math
