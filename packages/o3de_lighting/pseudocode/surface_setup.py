# title: Surface: the metallic workflow | 表面：金属度工作流
# source: set-albedo-f0
# demo: surface
MAX_DIELECTRIC_SPECULAR_F0 = 0.08
MIN_ROUGHNESS_LINEAR = 0.024
def set_albedo_and_specular_f0(s, base, factor, metallic):
    s.base_color = base
    s.metallic = metallic
    s.albedo = base * (1 - metallic)                   # @albedo
    dielectric = MAX_DIELECTRIC_SPECULAR_F0 * factor   # @f0
    s.specular_f0 = lerp(dielectric, base, metallic)   # @f0

def calculate_roughness_values(s, roughness):
    r = max(roughness, MIN_ROUGHNESS_LINEAR)           # @roughness
    s.roughness_linear = r                             # @roughness
    s.roughness_a = r * r                              # @roughness
    s.roughness_a2 = s.roughness_a ** 2                # @roughness
    if o_apply_specular_aa:   # on by default       # @aa
        k = specular_aa_kernel(s.normal)   # <= 0.18  # @aa
        s.roughness_a2 = saturate(s.roughness_a2 + k)  # @aa
        s.roughness_a = sqrt(s.roughness_a2)       # @aa
        s.roughness_linear = sqrt(s.roughness_a)   # @aa
