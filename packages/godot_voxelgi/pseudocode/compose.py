# title-text: listing-compose-title
# source: gi-buffer-mix
# demo: compose
def shade(pixel, material):
    gi = ambient_buffer[pixel]                        # @read
    ambient = mix(environment_ambient, gi.rgb, gi.a)  # @mix
    spec = reflection_buffer[pixel]
    specular = mix(sky_reflection, spec.rgb, spec.a)  # @mix
    ambient *= ao * material.albedo                   # @albedo
    specular *= env_brdf(material)                    # @brdf
    return direct + ambient + specular                # @sum
