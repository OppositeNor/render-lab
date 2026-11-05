# title-text: listing-compose-title
# source: ssr-blend
# demo: compose
# dispatch-text: listing-compose-dispatch
def indirect_specular(n, v, roughness):
    spec = sky(bent(reflect(-v, n), roughness)) * horizon ** 2  # @sky
    spec *= specular_occlusion(ambient)                  # @occlusion
    ssr = resolved[pixel] * roughness_fade(0.6, 0.7)     # @ssr
    spec = spec * (1.0 - ssr.a) + ssr.rgb                # @blend
    return spec * env_brdf(f0, roughness, dot(n, v))     # @brdf
