# title: Clear coat: the base layer beneath it | Clear coat：涂层下的底层
# source: clear-coat-f0
# demo: clearcoat
COAT_F0 = 0.04   # IOR 1.5

def apply_clear_coat_to_specular_f0(s):                 # @f0
    f0c = (1 - 5 * sqrt(s.f0)) / (5 - sqrt(s.f0))       # @f0
    s.f0 = lerp(s.f0, f0c * f0c, s.coat.factor)         # @f0

def init_lighting_data(s, lit):                         # @response
    fr = fresnel_schlick_with_roughness(                # @response
        lit.n_dot_v, COAT_F0, s.coat.roughness)         # @response
    lit.diffuse_response *= 1 - fr * s.coat.factor      # @response

def diffuse(s, lit, l, v):   # per light                # @diffuse
    d = diffuse_lambertian(s, l, lit.diffuse_response)  # @diffuse
    h_dot_v = saturate(dot(normalize(l + v), v))        # @diffuse
    f = fresnel_schlick(h_dot_v, COAT_F0)               # @diffuse
    return d * (1 - f * s.coat.factor)   # again        # @diffuse
