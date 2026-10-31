# title: Diffuse lighting and its response | 漫反射光照与响应
# source: initialize-lighting-data
# demo: diffuse
def initialize_lighting_data(s, view_position, lit):
    v = normalize(view_position - s.position)
    lit.n_dot_v = saturate(dot(s.normal, v))           # @response
    smooth = 1 - s.roughness_linear                    # @response
    f0 = s.specular_f0                                 # @response
    grazing = max(smooth, f0)                          # @response
    k = (1 - lit.n_dot_v) ** 5                         # @response
    specular_response = f0 + (grazing - f0) * k        # @response
    lit.diffuse_response = 1 - specular_response       # @response

def diffuse_lambertian(albedo, n, l, response):
    n_dot_l = saturate(dot(n, l))                      # @lambert
    return albedo * n_dot_l / PI * response            # @lambert

def get_diffuse_lighting(s, lit, intensity, l):
    diffuse = diffuse_lambertian(s.albedo, s.normal, l,
                                 lit.diffuse_response)
    if clear_coat:
        diffuse *= 1 - fresnel(h_dot_v, 0.04) * coat
    return diffuse * intensity                         # @light
