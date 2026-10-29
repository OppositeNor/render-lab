// Open 3D Engine (Atom) StandardPBR direct lighting, ported to WGSL for the
// lab's scene pass. Composed after the scene module (src/lab/shaders.rs,
// `SceneShading` in src/lab/investigation.rs), so SceneVaryings, SceneOutput,
// gFrame, gObjects, and common.wgsl are in scope.
//
// Ported from the Open 3D Engine, Copyright Contributors to the Open 3D Engine
// Project, used under the MIT License (THIRD_PARTY.md). Each function names
// the O3DE function it follows; src/o3de/lighting/source-index.json gives
// their files and lines. src/o3de/lighting/brdf.rs mirrors every function on
// the CPU, and `selftest` compares the two through the kernels at the end.
//
// The lab keeps O3DE's arithmetic in f32 (O3DE's `real` may be half precision
// on some platforms).

// gShading: the investigation's values (the `///` tags; the lab packs them
// from the shot, lab/src/shader_data.rs, and `render-lab values` lists them)
// and what src/o3de/lighting/demos.rs derives from them. The roughness of
// object i is gObjects[i].albedoRoughness.w.
struct O3deShading {
    /// The simple point light's m_position, in metres.
    /// {@ key=lightPosition components=lightX,lightY,lightZ default=0.55,1.85,0.95}
    /// {@ range=-2.8,2.8,0.1,2.9,-2.8,2.8 unit=m label=light}
    lightPosition: vec3<f32>,
    /// Its m_invAttenuationRadiusSquared, from attenuationRadius.
    /// {@ derived}
    invAttenuationRadiusSquared: f32,
    /// Its m_rgbIntensityCandelas: the light's colour times lightIntensity.
    /// {@ derived}
    rgbIntensityCandelas: vec3<f32>,
    /// The point light on or off.
    /// {@ key=lightOn toggle default=true label=toggles text=controls.light_on}
    lightOn: f32,
    /// What the scene pass shows (VIEW_*; brdf::ViewMode).
    /// {@ key=viewMode choice=ViewMode default=final}
    viewMode: u32,
    /// o_specularF0_enableMultiScatterCompensation.
    /// {@ key=multiscatter toggle default=true label=toggles text=controls.multiscatter}
    multiscatter: f32,
    /// 0 DiffuseLambertian (StandardPBR), 1 NormalizedDisneyDiffuse (Skin;
    /// EnhancedPBR with subsurface scattering).
    /// {@ key=diffuseModel choice=DiffuseModel default=lambertian}
    diffuseModel: f32,
    /// The object whose point the readout inspects (inspectObject, or the
    /// demonstration's subject).
    /// {@ derived}
    inspectedObject: f32,
    /// The full-screen IBL passes (global skylight) on or off.
    /// {@ key=ibl toggle default=true label=toggles}
    ibl: f32,
    /// The skylight's exposure, in stops (m_iblExposure).
    /// {@ key=iblExposure default=0 range=-4,4 label=light text=controls.ibl_exposure}
    iblExposure: f32,
    /// MSAAResolveSpecularPass's custom resolve (MSAAResolveCustom: the
    /// neighbour clamp and the log-domain mean) instead of the lab's plain
    /// average of the specular samples.
    /// {@ key=specularResolve toggle default=false label=toggles text=controls.specular_resolve}
    specularResolve: f32,
    /// Specular anti-aliasing (the applySpecularAA material property, true by default).
    /// {@ key=specularAA toggle default=true label=toggles}
    specularAA: f32,
    /// Object i: metallic, specularF0Factor, and the diffuse and specular
    /// occlusion factors when its occlusion maps are assigned (else -1).
    /// {@ derived}
    materials: array<vec4<f32>, 12>,
}
/// {@ shader_data}
@group(1) @binding(0) var<uniform> gShading: O3deShading;
// gShadingData (ibl::shading_data): the BRDF texture (BRDFTextureCS),
// BRDF_MAP_SIZE^2 texels row-major from the top row, (A, B) in xy; then the
// specular cubemap's SPECULAR_MIPS mips and the diffuse cubemap, each six
// faces of row-major texels, radiance in xyz; then the occlusion map
// (maps.rs), OCCLUSION_WIDTH x OCCLUSION_HEIGHT texels in x.
@group(1) @binding(1) var<storage, read> gShadingData: array<vec4<f32>>;
// gShadingFrame (demos.rs), per frame: [0..3] the inverse of worldToClip, by
// column; [FRAME_MATERIALS + i] object i's clear coat factor (-1 without
// clearCoat.enable), its roughness, its lighting-channel mask, unused;
// [FRAME_LIGHTS] the number of simple point lights, then two vectors per
// light, as SimplePointLight holds it: (m_position,
// m_invAttenuationRadiusSquared), (m_rgbIntensityCandelas,
// m_lightingChannelMask). Light 0 is the one the readout and views describe.
@group(1) @binding(2) var<storage, read> gShadingFrame: array<vec4<f32>>;
const FRAME_MATERIALS: u32 = 4u;
const FRAME_LIGHTS: u32 = 16u;

// The VIEW_* constants are ViewMode's (src/o3de/lighting/brdf.rs), generated into
// the package's build/generated/view_mode.wgsl by the lab, and composed before this file.
const MAX_MATERIALS: u32 = 12u;

// Atom/RPI/Math.azsli.
const O3DE_PI: f32 = 3.14159265;
const INV_PI: f32 = 0.31830988;
// Ggx.azsli: "a smaller epsilon is required for avoiding artifacts in the generated BRDF texture".
const GGX_EPSILON: f32 = 1e-8;
// MaterialUtils.azsli.
const MAX_DIELECTRIC_SPECULAR_F0: f32 = 0.08;
const MIN_ROUGHNESS_LINEAR: f32 = 0.024;
// Textures/BRDFTexture.attimage: 256 x 256.
const BRDF_MAP_SIZE: u32 = 256u;
// BRDFTextureCS: NumSamples.
const BRDF_MAP_SAMPLES: u32 = 1024u;
// IBLSpecular.preset and IBLDiffuse.preset (the lab's diffuse size; see ibl.rs).
const SPECULAR_SIZE: u32 = 256u;
const SPECULAR_MIPS: u32 = 6u;
const DIFFUSE_SIZE: u32 = 32u;

// ---------------------------------------------------------------------------
// The surface (Materials/StandardPBR/StandardPBR_SurfaceData.azsli,
// PBR/MaterialUtils.azsli).

struct O3deSurface {
    position: vec3<f32>,
    normal: vec3<f32>,
    baseColor: vec3<f32>,
    metallic: f32,
    roughnessLinear: f32,
    roughnessA: f32,
    roughnessA2: f32,
    albedo: vec3<f32>,
    specularF0: vec3<f32>,
    // diffuseAmbientOcclusion and specularOcclusion: 1 without occlusion maps.
    diffuseAO: f32,
    specularOcclusion: f32,
    // ClearCoatSurfaceData, and o_clearCoat_feature_enabled (the material's clearCoat.enable).
    clearCoat: bool,
    clearCoatFactor: f32,
    clearCoatRoughness: f32,
    clearCoatNormal: vec3<f32>,
    // GetVertexNormal(), and TransmissionSurfaceData with o_transmission_mode
    // (EnhancedPBR; 0 None, 1 ThickObject, 2 ThinObject).
    vertexNormal: vec3<f32>,
    transmissionMode: u32,
    transmissionTint: vec3<f32>,
    transmissionThickness: f32,
    transmissionParams: vec4<f32>,
    scatterDistance: vec3<f32>,
}

// SpecularAA.azsli: CalculateSpecularAA, from Tokuyoshi and Kaplanyan's
// "Improved Geometric Specular Antialiasing" (2019). dndx and dndy are the
// normal's screen-space derivatives (ddx_fine, ddy_fine).
fn calculateSpecularAA(dndx: vec3<f32>, dndy: vec3<f32>, roughnessA2: f32) -> f32 {
    let screenVariance = 0.25;
    let varianceThresh = 0.18;
    let variance = screenVariance * (dot(dndx, dndx) + dot(dndy, dndy));
    let kernelRoughnessA2 = min(2.0 * variance, varianceThresh);
    return saturate(roughnessA2 + kernelRoughnessA2);
}

// SurfaceData_StandardPBR::CalculateRoughnessA -> CalculateRoughnessValues
// (specular AA when on, as the applySpecularAA property is by default), then
// SetAlbedoAndSpecularF0 -> GetAlbedo and CalculateSpecularF0 (the metallic
// workflow).
fn o3deSurface(position: vec3<f32>, normal: vec3<f32>, baseColor: vec3<f32>, metallic: f32, specularF0Factor: f32, roughness: f32, dndx: vec3<f32>, dndy: vec3<f32>) -> O3deSurface {
    var s: O3deSurface;
    s.position = position;
    s.normal = normal;
    s.roughnessLinear = max(roughness, MIN_ROUGHNESS_LINEAR);
    s.roughnessA = s.roughnessLinear * s.roughnessLinear;
    s.roughnessA2 = s.roughnessA * s.roughnessA;
    if (gShading.specularAA > 0.5) {
        s.roughnessA2 = calculateSpecularAA(dndx, dndy, s.roughnessA2);
        // "Apply backwards so we get consistent behavior".
        s.roughnessA = sqrt(s.roughnessA2);
        s.roughnessLinear = sqrt(s.roughnessA);
    }
    s.baseColor = baseColor;
    s.metallic = metallic;
    let dielectricSpecularF0 = vec3<f32>(MAX_DIELECTRIC_SPECULAR_F0 * specularF0Factor);
    s.albedo = baseColor * (1.0 - metallic);
    s.specularF0 = mix(dielectricSpecularF0, baseColor, metallic);
    s.diffuseAO = 1.0;
    s.specularOcclusion = 1.0;
    s.clearCoat = false;
    s.clearCoatFactor = 0.0;
    s.clearCoatRoughness = 0.0;
    s.clearCoatNormal = normal;
    s.vertexNormal = normal;
    s.transmissionMode = 0u;
    s.transmissionTint = vec3<f32>(0.0);
    s.transmissionThickness = 0.0;
    s.transmissionParams = vec4<f32>(0.0);
    s.scatterDistance = vec3<f32>(0.0);
    return s;
}

// MaterialUtils.azsli: ApplyClearCoatToSpecularF0 (Filament's base-layer change).
fn applyClearCoatToSpecularF0(specularF0: vec3<f32>, clearCoatFactor: f32) -> vec3<f32> {
    let f0 = (1.0 - 5.0 * sqrt(specularF0)) / (5.0 - sqrt(specularF0));
    return mix(specularF0, f0 * f0, clearCoatFactor);
}

// The clear coat of the material on object i (gShadingFrame[FRAME_MATERIALS + i]):
// GetClearCoatInputs with no maps (factor, max(roughness, 1e-3), the vertex
// normal), then ApplyClearCoatToSpecularF0, as EvaluateSurface_StandardPBR does.
fn withClearCoat(surface: O3deSurface, objectIndex: u32) -> O3deSurface {
    var s = surface;
    let coat = gShadingFrame[FRAME_MATERIALS + min(objectIndex, MAX_MATERIALS - 1u)];
    if (coat.x < 0.0) {
        return s;
    }
    s.clearCoat = true;
    s.clearCoatFactor = coat.x;
    s.clearCoatRoughness = max(coat.y, 1e-3);
    s.clearCoatNormal = s.normal;
    s.specularF0 = applyClearCoatToSpecularF0(s.specularF0, s.clearCoatFactor);
    return s;
}

// The occlusion map (maps.rs), read as the material sampler does: bilinear,
// wrapping in both directions (mip 0 only).
const OCCLUSION_WIDTH: u32 = 256u;
const OCCLUSION_HEIGHT: u32 = 128u;

fn occlusionTexel(x: i32, y: i32) -> f32 {
    let w = i32(OCCLUSION_WIDTH);
    let h = i32(OCCLUSION_HEIGHT);
    let wrapped = vec2<i32>(((x % w) + w) % w, ((y % h) + h) % h);
    let offset = specularMipOffset(SPECULAR_MIPS) + 6u * DIFFUSE_SIZE * DIFFUSE_SIZE;
    return gShadingData[offset + u32(wrapped.y * w + wrapped.x)].x;
}

fn sampleOcclusionMap(uv: vec2<f32>) -> f32 {
    let p = uv * vec2<f32>(f32(OCCLUSION_WIDTH), f32(OCCLUSION_HEIGHT)) - 0.5;
    let base = floor(p);
    let t = p - base;
    let x = i32(base.x);
    let y = i32(base.y);
    let top = mix(occlusionTexel(x, y), occlusionTexel(x + 1, y), t.x);
    let bottom = mix(occlusionTexel(x, y + 1), occlusionTexel(x + 1, y + 1), t.x);
    return mix(top, bottom, t.y);
}

// The sphere's UV (maps.rs): longitude across, latitude from the top pole down.
fn sphereUv(localDirection: vec3<f32>) -> vec2<f32> {
    let d = normalize(localDirection);
    return vec2<f32>(atan2(d.z, d.x) / (2.0 * O3DE_PI) + 0.5, acos(clamp(d.y, -1.0, 1.0)) / O3DE_PI);
}

// GetOcclusionInput (OcclusionInput.azsli): 1 without a texture (factor < 0
// here), otherwise the texture's red channel raised to the factor.
fn occlusionInput(uv: vec2<f32>, factor: f32) -> f32 {
    if (factor < 0.0) {
        return 1.0;
    }
    return pow(sampleOcclusionMap(uv), factor);
}

// ---------------------------------------------------------------------------
// Decals (PBR/Decals.azsli; Decals/DecalStructures.azsli), applied to the
// surface before InitializeLightingData. gShadingFrame from FRAME_DECALS
// (decals.rs): the count, two vectors with the decals in SortDecals' order,
// then five vectors per decal as Decal holds it: (m_position, m_opacity),
// m_quaternion, (m_halfSize, m_angleAttenuation), (m_normalMapOpacity,
// m_sortKeyPacked, the texture, m_maxDrawDistance), (m_decalColor,
// m_decalColorFactor).
const FRAME_DECALS: u32 = 128u;
const DECAL_RECORD: u32 = 5u;
// The decal textures (maps.rs) after the occlusion map: per texture, its base
// map (RGB, alpha) and then its normal map (two signed channels), each
// DECAL_SIZE^2 texels.
const DECAL_SIZE: u32 = 128u;

struct O3deDecal {
    position: vec3<f32>,
    opacity: f32,
    quaternion: vec4<f32>,
    halfSize: vec3<f32>,
    angleAttenuation: f32,
    normalMapOpacity: f32,
    sortKey: u32,
    texture: u32,
    maxDrawDistance: f32,
    decalColor: vec3<f32>,
    decalColorFactor: f32,
}

fn decalCount() -> u32 {
    return u32(gShadingFrame[FRAME_DECALS].x);
}

fn decalAt(i: u32) -> O3deDecal {
    let base = FRAME_DECALS + 3u + DECAL_RECORD * i;
    let a = gShadingFrame[base];
    let c = gShadingFrame[base + 2u];
    let d = gShadingFrame[base + 3u];
    let e = gShadingFrame[base + 4u];
    return O3deDecal(a.xyz, a.w, gShadingFrame[base + 1u], c.xyz, c.w, d.x, u32(d.y), u32(d.z), d.w, e.xyz, e.w);
}

// The k-th decal in sort order.
fn sortedDecal(k: u32) -> u32 {
    return u32(gShadingFrame[FRAME_DECALS + 1u + k / 4u][k % 4u]);
}

// MatrixUtility.azsli: MatrixFromQuaternion (HLSL's rows, here columns of the transpose).
fn matrixFromQuaternion(q: vec4<f32>) -> mat3x3<f32> {
    let t = q.xyz * 2.0;
    let tw = q.w * t;
    let txy = q.x * t.y;
    let txx = q.x * t.x;
    let txz = q.x * t.z;
    let tyy = q.y * t.y;
    let tyz = q.y * t.z;
    let tzz = q.z * t.z;
    // The rows of O3DE's m, so that m * v here is mul(m, v) there.
    return transpose(mat3x3<f32>(
        vec3<f32>(1.0 - tyy - tzz, txy - tw.z, txz + tw.y),
        vec3<f32>(txy + tw.z, 1.0 - txx - tzz, tyz - tw.x),
        vec3<f32>(txz - tw.y, tyz + tw.x, 1.0 - txx - tyy)));
}

fn decalTexel(offset: u32, x: i32, y: i32) -> vec4<f32> {
    let size = i32(DECAL_SIZE);
    let c = clamp(vec2<i32>(x, y), vec2<i32>(0), vec2<i32>(size - 1));
    return gShadingData[offset + u32(c.y * size + c.x)];
}

// A decal texture read as PassSrg::LinearSampler does: bilinear, clamped
// (mip 0 only).
fn sampleDecalMap(texture: u32, normalMap: bool, uv: vec2<f32>) -> vec4<f32> {
    let start = specularMipOffset(SPECULAR_MIPS) + 6u * DIFFUSE_SIZE * DIFFUSE_SIZE + OCCLUSION_WIDTH * OCCLUSION_HEIGHT;
    let offset = start + (2u * texture + select(0u, 1u, normalMap)) * DECAL_SIZE * DECAL_SIZE;
    let p = uv * f32(DECAL_SIZE) - 0.5;
    let base = floor(p);
    let t = p - base;
    let x = i32(base.x);
    let y = i32(base.y);
    let top = mix(decalTexel(offset, x, y), decalTexel(offset, x + 1, y), t.x);
    let bottom = mix(decalTexel(offset, x, y + 1), decalTexel(offset, x + 1, y + 1), t.x);
    return mix(top, bottom, t.y);
}

// GetDecalAttenuation.
fn getDecalAttenuation(surfNormal: vec3<f32>, decalUp: vec3<f32>, decalAngleAttenuation: f32) -> f32 {
    let decalClampAngle = 0.55;
    var dotAtten = dot(surfNormal, decalUp);
    dotAtten = clamp(dotAtten, decalClampAngle, 1.0);
    dotAtten = smoothstep(decalClampAngle, 1.0, dotAtten);
    return mix(1.0, dotAtten, decalAngleAttenuation);
}

// GetTangentSpaceNormal_Unnormalized (RPI/TangentSpace.azsli), strength 1:
// the image builder's swapped channels swapped back, z rebuilt.
fn tangentSpaceNormalUnnormalized(normalMapSample: vec2<f32>) -> vec3<f32> {
    var surfaceNormal: vec3<f32>;
    surfaceNormal = vec3<f32>(normalMapSample.yx, 0.0);
    // max(…, 0): HLSL's max turns the NaN of a negative root into 0.01; WGSL's need not.
    surfaceNormal.z = sqrt(max(1.0 - dot(surfaceNormal.xy, surfaceNormal.xy), 0.0));
    surfaceNormal.z = max(surfaceNormal.z, 0.01);
    return surfaceNormal;
}

// ApplyDecal: inside the decal's box, the base map times the decal colour
// (BlendBaseColor, Multiply) replaces the albedo by its opacity, and
// specularF0 by that opacity times metallic; the normal map bends the
// normal. vertexNormal is surface.GetVertexNormal().
fn applyDecal(i: u32, surface: O3deSurface, vertexNormal: vec3<f32>) -> O3deSurface {
    var s = surface;
    let decal = decalAt(i);
    // decalRot = transpose(MatrixFromQuaternion): its rows are the decal's axes.
    let m = matrixFromQuaternion(decal.quaternion);
    let decalRot = transpose(m);
    let localPos = decalRot * (s.position - decal.position);
    var decalUVW = localPos / decal.halfSize;
    if (all(decalUVW >= vec3<f32>(-1.0)) && all(decalUVW <= vec3<f32>(1.0))) {
        decalUVW.y *= -1.0;
        let decalUV = decalUVW.xy * 0.5 + 0.5;
        let baseMap = sampleDecalMap(decal.texture, false, decalUV);
        let normalMap = sampleDecalMap(decal.texture, true, decalUV).rg;
        // decalRot[0..2], the rows, are m's columns.
        let decalAttenuation = getDecalAttenuation(vertexNormal, m[2], decal.angleAttenuation);
        let albedoOpacity = baseMap.a * decal.opacity * decalAttenuation;
        let blendedBaseColor = decal.decalColorFactor * decal.decalColor * baseMap.rgb;
        s.albedo = mix(s.albedo, blendedBaseColor, albedoOpacity);
        s.specularF0 = mix(s.specularF0, blendedBaseColor, albedoOpacity * s.metallic);
        let normalOpacity = baseMap.a * decal.normalMapOpacity * decalAttenuation;
        // GetWorldSpaceNormal(normalMap, decalRot[2], decalRot[0], decalRot[1]).
        let ts = tangentSpaceNormalUnnormalized(normalMap);
        let normalMapWS = normalize(ts.x * m[0] + ts.y * m[1] + ts.z * m[2]);
        s.normal = normalize(mix(s.normal, normalMapWS, normalOpacity));
    }
    return s;
}

// ---------------------------------------------------------------------------
// Fresnel (Microfacet/Fresnel.azsli).

fn fresnelSchlickF90(HdotV: f32, F0: vec3<f32>, F90: f32) -> vec3<f32> {
    return F0 + (F90 - F0) * pow(1.0 - HdotV, 5.0);
}

fn fresnelSchlick(HdotV: f32, F0: vec3<f32>) -> vec3<f32> {
    return fresnelSchlickF90(HdotV, F0, 1.0);
}

// FresnelSchlickWithRoughness: the grazing reflectance is held at
// max(1 - roughness, F0), for light coming from the whole environment.
fn fresnelSchlickWithRoughness(NdotV: f32, specularF0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let smoothness = 1.0 - roughness;
    return specularF0 + (max(vec3<f32>(smoothness), specularF0) - specularF0) * pow(1.0 - saturate(NdotV), 5.0);
}

// ---------------------------------------------------------------------------
// GGX (Microfacet/Ggx.azsli).

// NormalDistributionGGX: Walter et al. 2007, equation 33.
fn normalDistributionGGX(NdotH: f32, roughnessA2: f32) -> f32 {
    let b = (NdotH * roughnessA2 - NdotH) * NdotH + 1.0;
    return roughnessA2 / (O3DE_PI * b * b);
}

// GeometricShadowingMaskingGGXCorrelated: height-correlated Smith, with the
// BRDF's 1 / (4 NdotL NdotV) already folded in (a visibility term).
fn geometricShadowingMaskingGGXCorrelated(NdotV: f32, NdotL: f32, roughnessA2: f32) -> f32 {
    let ggxV = NdotL * sqrt((-NdotV * roughnessA2 + NdotV) * NdotV + roughnessA2);
    let ggxL = NdotV * sqrt((-NdotL * roughnessA2 + NdotL) * NdotL + roughnessA2);
    return 0.5 / max(ggxV + ggxL, GGX_EPSILON);
}

// ImportanceSampleGGX: a microfacet normal for a 2D point, Walter 2007 (35, 36).
fn importanceSampleGGX(xi: vec2<f32>, roughnessA: f32, normalWS: vec3<f32>) -> vec3<f32> {
    let roughnessA2 = roughnessA * roughnessA;
    let phi = 2.0 * O3DE_PI * xi.x;
    let cosTheta = sqrt((1.0 - xi.y) / (1.0 + (roughnessA2 - 1.0) * xi.y));
    let sinTheta = sqrt(1.0 - cosTheta * cosTheta);
    let micronormalTS = vec3<f32>(sinTheta * cos(phi), sinTheta * sin(phi), cosTheta);
    let upVector = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(normalWS.z) < 0.999);
    let tangentWS = normalize(cross(upVector, normalWS));
    let bitangentWS = cross(normalWS, tangentWS);
    return tangentWS * micronormalTS.x + bitangentWS * micronormalTS.y + normalWS * micronormalTS.z;
}

// importanceSampleGGX with sinTheta's argument kept from going negative. Only
// the selftest uses it: on the GPU, cosTheta can round above 1 for a nearly
// mirror-like lobe, and O3DE's unguarded square root then drops the sample.
fn importanceSampleGGXGuarded(xi: vec2<f32>, roughnessA: f32, normalWS: vec3<f32>) -> vec3<f32> {
    let roughnessA2 = roughnessA * roughnessA;
    let phi = 2.0 * O3DE_PI * xi.x;
    let cosTheta = sqrt((1.0 - xi.y) / (1.0 + (roughnessA2 - 1.0) * xi.y));
    let sinTheta = sqrt(max(0.0, 1.0 - cosTheta * cosTheta));
    let micronormalTS = vec3<f32>(sinTheta * cos(phi), sinTheta * sin(phi), cosTheta);
    let upVector = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(normalWS.z) < 0.999);
    let tangentWS = normalize(cross(upVector, normalWS));
    let bitangentWS = cross(normalWS, tangentWS);
    return tangentWS * micronormalTS.x + bitangentWS * micronormalTS.y + normalWS * micronormalTS.z;
}

// Hammersley.azsli: GetHammersleyPoint (no half-sample offset, unlike the lab's hammersley).
fn o3deHammersley(pointIndex: u32, totalPointCount: u32) -> vec2<f32> {
    return vec2<f32>(f32(pointIndex) / f32(totalPointCount), f32(reverseBits(pointIndex)) * 2.3283064365386963e-10);
}

// ---------------------------------------------------------------------------
// The BRDF terms (Microfacet/Brdf.azsli).

fn diffuseLambertian(albedo: vec3<f32>, normal: vec3<f32>, dirToLight: vec3<f32>, diffuseResponse: vec3<f32>) -> vec3<f32> {
    let NdotL = saturate(dot(normal, dirToLight));
    return albedo * NdotL * INV_PI * diffuseResponse;
}

// NormalizedDisneyDiffuse: not called by StandardPBR (EnhancedPBR calls it with
// subsurface scattering on, Skin always); it has no diffuse response.
fn normalizedDisneyDiffuse(albedo: vec3<f32>, normal: vec3<f32>, dirToCamera: vec3<f32>, dirToLight: vec3<f32>, roughnessLinear: f32) -> vec3<f32> {
    let halfVector = normalize(dirToLight + dirToCamera);
    let NdotV = saturate(dot(normal, dirToCamera));
    let NdotL = saturate(dot(normal, dirToLight));
    let HdotV = saturate(dot(halfVector, dirToCamera));
    let energyBias = mix(0.0, 0.5, roughnessLinear);
    let energyFactor = mix(1.0, 1.0 / 1.51, roughnessLinear);
    let fd90 = energyBias + 2.0 * HdotV * HdotV * roughnessLinear;
    let f0 = vec3<f32>(1.0);
    let lightScatter = fresnelSchlickF90(NdotL, f0, fd90).r;
    let viewScatter = fresnelSchlickF90(NdotV, f0, fd90).r;
    return albedo * lightScatter * viewScatter * energyFactor * NdotL * INV_PI;
}

// The three microfacet terms SpecularGGX multiplies, for display and tests.
struct GgxTerms {
    D: f32,
    G: f32,
    F: vec3<f32>,
    NdotL: f32,
}

fn ggxTerms(dirToCamera: vec3<f32>, dirToLight: vec3<f32>, normal: vec3<f32>, specularF0: vec3<f32>, NdotV: f32, roughnessA2: f32) -> GgxTerms {
    let halfVector = normalize(dirToLight + dirToCamera);
    let NdotH = saturate(dot(normal, halfVector));
    let NdotL = saturate(dot(normal, dirToLight));
    let HdotV = saturate(dot(halfVector, dirToCamera));
    let D = max(0.0, normalDistributionGGX(NdotH, roughnessA2));
    let G = max(0.0, geometricShadowingMaskingGGXCorrelated(NdotV, NdotL, roughnessA2));
    return GgxTerms(D, G, fresnelSchlick(HdotV, specularF0), NdotL);
}

// SpecularGGX: D * G * F * NdotL (G already holds 1 / (4 NdotL NdotV)),
// times the multiscatter compensation.
fn specularGGX(dirToCamera: vec3<f32>, dirToLight: vec3<f32>, normal: vec3<f32>, specularF0: vec3<f32>, NdotV: f32, roughnessA2: f32, multiScatterCompensation: vec3<f32>) -> vec3<f32> {
    let t = ggxTerms(dirToCamera, dirToLight, normal, specularF0, NdotV, roughnessA2);
    return ((t.D * t.G) * t.NdotL * t.F) * multiScatterCompensation;
}

// ---------------------------------------------------------------------------
// The lighting data (Materials/BasePBR/BasePBR_LightingData.azsli and
// BasePBR_LightingEval.azsli; StandardPBR uses LightingData_BasePBR).

// SampleBrdfMap: a bilinear sample with clamped addressing, like
// PassSrg::LinearSampler on the BRDF texture.
fn sampleBrdfMap(uv: vec2<f32>) -> vec2<f32> {
    let size = f32(BRDF_MAP_SIZE);
    let p = clamp(uv * size - 0.5, vec2<f32>(0.0), vec2<f32>(size - 1.0));
    let i0 = vec2<u32>(floor(p));
    let i1 = min(i0 + vec2<u32>(1u), vec2<u32>(BRDF_MAP_SIZE - 1u));
    let w = p - floor(p);
    let a =gShadingData[i0.y * BRDF_MAP_SIZE + i0.x].xy;
    let b = gShadingData[i0.y * BRDF_MAP_SIZE + i1.x].xy;
    let c = gShadingData[i1.y * BRDF_MAP_SIZE + i0.x].xy;
    let d = gShadingData[i1.y * BRDF_MAP_SIZE + i1.x].xy;
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

// GetMultiScatterCompensation (Turquin 2019): 1 + F0 (1 / (A + B) - 1).
fn getMultiScatterCompensation(specularF0: vec3<f32>, brdf: vec2<f32>, enabled: bool) -> vec3<f32> {
    if (!enabled) {
        return vec3<f32>(1.0);
    }
    let compensation = specularF0 * ((1.0 / (brdf.x + brdf.y)) - 1.0);
    return compensation + 1.0;
}

struct O3deLightingData {
    dirToCamera: vec3<f32>,
    NdotV: f32,
    brdf: vec2<f32>,
    diffuseResponse: vec3<f32>,
    multiScatterCompensation: vec3<f32>,
    diffuseLighting: vec3<f32>,
    specularLighting: vec3<f32>,
    translucentBackLighting: vec3<f32>,
}

// LightingData_BasePBR::Init and InitializeLightingData_BasePBR: NdotV, the BRDF
// texture at (NdotV, smoothness), the diffuse response 1 - F_roughness, and
// the multiscatter compensation.
fn o3deLightingData(s: O3deSurface, viewPosition: vec3<f32>, multiscatter: bool) -> O3deLightingData {
    var l: O3deLightingData;
    l.dirToCamera = normalize(viewPosition - s.position);
    l.NdotV = saturate(dot(s.normal, l.dirToCamera));
    l.brdf = sampleBrdfMap(vec2<f32>(l.NdotV, 1.0 - s.roughnessLinear));
    let specularResponse = fresnelSchlickWithRoughness(l.NdotV, s.specularF0, s.roughnessLinear);
    l.diffuseResponse = 1.0 - specularResponse;
    l.multiScatterCompensation = getMultiScatterCompensation(s.specularF0, l.brdf, multiscatter);
    // InitializeLightingData_StandardPBR: the coat (IOR 1.5, so F0 0.04) takes its share.
    if (s.clearCoat) {
        l.diffuseResponse *= 1.0 - fresnelSchlickWithRoughness(l.NdotV, vec3<f32>(0.04), s.clearCoatRoughness) * s.clearCoatFactor;
    }
    l.diffuseLighting = vec3<f32>(0.0);
    l.specularLighting = vec3<f32>(0.0);
    l.translucentBackLighting = vec3<f32>(0.0);
    return l;
}

// ---------------------------------------------------------------------------
// The simple point light (Lights/SimplePointLight.azsli,
// Pipeline/Forward/ForwardPassSimplePointLights.azsli). It casts no shadows.

struct PointLightSample {
    dirToLight: vec3<f32>,
    falloff: f32,
    radiusAttenuation: f32,
    // The light's illuminance at the surface: rgb candelas / d^2 * radiusAttenuation.
    intensity: vec3<f32>,
}

// SimplePointLightUtil::Init and the first half of Apply.
fn simplePointLight(lightPosition: vec3<f32>, invAttenuationRadiusSquared: f32, rgbIntensityCandelas: vec3<f32>, surfacePosition: vec3<f32>) -> PointLightSample {
    var out: PointLightSample;
    let posToLight = lightPosition - surfacePosition;
    let distanceSquared = dot(posToLight, posToLight);
    out.falloff = distanceSquared * invAttenuationRadiusSquared;
    out.dirToLight = normalize(posToLight);
    var radiusAttenuation = 1.0 - out.falloff * out.falloff;
    radiusAttenuation = radiusAttenuation * radiusAttenuation;
    out.radiusAttenuation = radiusAttenuation;
    out.intensity = rgbIntensityCandelas / max(0.001 * 0.001, distanceSquared) * radiusAttenuation;
    return out;
}

// Brdf.azsli: ClearCoatGGX, with Kelemen's visibility 0.25 / HdotL^2 (Kelemen
// and Szirmay-Kalos 2001); its specular AA is commented out in O3DE.
fn clearCoatGGX(NdotH: f32, HdotL: f32, NdotL: f32, roughnessA: f32, F: vec3<f32>) -> vec3<f32> {
    let roughnessA2 = roughnessA * roughnessA;
    let D = max(0.0, normalDistributionGGX(NdotH, roughnessA2));
    let G = max(0.0, 0.25 / max(HdotL * HdotL, 1e-4));
    return D * G * F * NdotL;
}

// Brdf.azsli: ClearCoatSpecular: the base specular through (1 - Fc), plus the coat's.
fn clearCoatSpecular(dirToLight: vec3<f32>, dirToCamera: vec3<f32>, normal: vec3<f32>, factor: f32, roughness: f32, specular: vec3<f32>) -> vec3<f32> {
    let halfVector = normalize(dirToLight + dirToCamera);
    let NdotH = saturate(dot(normal, halfVector));
    let NdotL = saturate(dot(normal, dirToLight));
    let HdotL = saturate(dot(halfVector, dirToLight));
    let clearCoatF = fresnelSchlick(HdotL, vec3<f32>(0.04)) * factor;
    let clearCoatRoughness = max(roughness * roughness, 0.0005);
    let coat = clearCoatGGX(NdotH, HdotL, NdotL, clearCoatRoughness, clearCoatF);
    return specular * (1.0 - clearCoatF) + coat;
}

// GetDiffuseLighting_StandardPBR and GetSpecularLighting_StandardPBR
// (StandardPBR_LightingBrdf.azsli), clear coat included.
fn o3deApplyLight(s: O3deSurface, l: ptr<function, O3deLightingData>, intensity: vec3<f32>, dirToLight: vec3<f32>, diffuseModel: u32) {
    (*l).diffuseLighting += o3deDiffuse(s, *l, intensity, dirToLight, diffuseModel);
    (*l).specularLighting += o3deSpecular(s, *l, intensity, dirToLight);
}

// GetDiffuseLighting_StandardPBR (and _EnhancedPBR without subsurface scattering).
fn o3deDiffuse(s: O3deSurface, l: O3deLightingData, intensity: vec3<f32>, dirToLight: vec3<f32>, diffuseModel: u32) -> vec3<f32> {
    var diffuse = diffuseLambertian(s.albedo, s.normal, dirToLight, l.diffuseResponse);
    if (diffuseModel == 1u) {
        diffuse = normalizedDisneyDiffuse(s.albedo, s.normal, l.dirToCamera, dirToLight, s.roughnessLinear);
    } else if (s.clearCoat) {
        // The coat's Fresnel at the light's half vector, again.
        let HdotV = saturate(dot(normalize(dirToLight + l.dirToCamera), l.dirToCamera));
        diffuse *= 1.0 - fresnelSchlick(HdotV, vec3<f32>(0.04)) * s.clearCoatFactor;
    }
    return diffuse * intensity;
}

// GetSpecularLighting_StandardPBR (and _EnhancedPBR without anisotropy).
fn o3deSpecular(s: O3deSurface, l: O3deLightingData, intensity: vec3<f32>, dirToLight: vec3<f32>) -> vec3<f32> {
    var specular = specularGGX(l.dirToCamera, dirToLight, s.normal, s.specularF0, l.NdotV, s.roughnessA2, l.multiScatterCompensation);
    if (s.clearCoat) {
        specular = clearCoatSpecular(dirToLight, l.dirToCamera, s.clearCoatNormal, s.clearCoatFactor, s.clearCoatRoughness, specular);
    }
    return specular * intensity;
}

// ---------------------------------------------------------------------------
// Transmission (PBR/BackLighting.azsli; EnhancedPBR) and the point light it
// needs: simple point lights do no back lighting, so the lab adds one O3DE
// point light, the sphere area light (Lights/PointLight.azsli,
// Pipeline/Forward/ForwardPassPointLights.azsli), without shadows.
//
// gShadingFrame from FRAME_TRANSMISSION: (o_transmission_mode, thickness,
// the object, unused), transmissionParams, (tint, unused), (scatterDistance,
// unused); from FRAME_SPHERE_LIGHTS: the count, then three vectors per light:
// (m_position, m_invAttenuationRadiusSquared), (m_rgbIntensityCandelas,
// m_lightingChannelMask), (m_bulbRadius, unused).
const FRAME_TRANSMISSION: u32 = 176u;
const FRAME_SPHERE_LIGHTS: u32 = 184u;
const TRANSMISSION_NONE: u32 = 0u;
const TRANSMISSION_THICK: u32 = 1u;
const TRANSMISSION_THIN: u32 = 2u;

// EvaluateSurface_EnhancedPBR's transmission inputs for object i, without a thickness map.
fn withTransmission(surface: O3deSurface, objectIndex: u32) -> O3deSurface {
    var s = surface;
    let a = gShadingFrame[FRAME_TRANSMISSION];
    if (u32(a.z) != objectIndex || u32(a.x) == TRANSMISSION_NONE) {
        return s;
    }
    s.transmissionMode = u32(a.x);
    s.transmissionThickness = a.y;
    s.transmissionParams = gShadingFrame[FRAME_TRANSMISSION + 1u];
    s.transmissionTint = gShadingFrame[FRAME_TRANSMISSION + 2u].xyz;
    s.scatterDistance = gShadingFrame[FRAME_TRANSMISSION + 3u].xyz;
    return s;
}

// TransmissionKernel: the diffusion profile's approximate integral.
fn transmissionKernel(t: f32, s: vec3<f32>) -> vec3<f32> {
    let exponent = s * t;
    return 0.25 * (1.0 / exp(exponent) + 3.0 / exp(exponent / 3.0));
}

// GetBackLighting_Default (USE_HUMAN_SKIN_PROFILE off).
fn getBackLighting(s: O3deSurface, l: O3deLightingData, lightIntensity: vec3<f32>, dirToLight: vec3<f32>, transmissionDistance: f32, attenuationDistance: f32) -> vec3<f32> {
    var result = vec3<f32>(0.0);
    let p = s.transmissionParams;
    if (s.transmissionMode == TRANSMISSION_THICK) {
        // Barré-Brisebois and Bouchard 2011: (attenuation coefficient, power, distortion, scale).
        let thickness = max(transmissionDistance, s.transmissionThickness);
        let transmittance = pow(saturate(dot(l.dirToCamera, -normalize(dirToLight + s.vertexNormal * p.z))), p.y) * p.w;
        let lamberAttenuation = exp(-thickness * p.x) * saturate(1.0 - thickness);
        result = transmittance * lamberAttenuation * lightIntensity;
    } else if (s.transmissionMode == TRANSMISSION_THIN) {
        // Jimenez et al. 2010: (shrink factor, NdotL bias, distance attenuation, scale).
        if (transmissionDistance >= 0.0) {
            let E = s.albedo * saturate(p.y + dot(-s.vertexNormal, dirToLight));
            let C = 300.0;
            let t = transmissionDistance * C * s.transmissionThickness;
            let invScattering = 1.0 / max(s.scatterDistance, vec3<f32>(0.00001));
            result = transmissionKernel(t, invScattering) * lightIntensity * E * p.w;
            result /= max(1.0, attenuationDistance * attenuationDistance * p.z);
        }
    }
    return result;
}

// LightTypesCommon.azsli: GetIntensityAdjustedByRadiusAndRoughness.
fn intensityAdjustedByRadiusAndRoughness(roughnessA: f32, radius: f32, distance2: f32) -> f32 {
    let roughnessAdjusted = saturate(roughnessA + (radius / (3.0 * sqrt(distance2))));
    let intensityNormalization = max(roughnessA, 0.001) / roughnessAdjusted;
    return intensityNormalization * intensityNormalization;
}

fn sphereLightCount() -> u32 {
    return u32(gShadingFrame[FRAME_SPHERE_LIGHTS].x);
}

// ApplyPointLight: lighting channels, the radius, then PointLightUtil_PBR::Apply
// with litRatio 1. Shadows are on as a shader option (o_enableShadows and
// o_enableSphereLightShadows default to true, and per-mesh flags are off by
// default), so the shadow branch runs; for a light without shadows
// GetVisibility gives 1 and GetThickness 0, so the transmission distance is 0.
fn applySphereLight(i: u32, objectIndex: u32, s: O3deSurface, l: ptr<function, O3deLightingData>) {
    let a = gShadingFrame[FRAME_SPHERE_LIGHTS + 1u + 3u * i];
    let b = gShadingFrame[FRAME_SPHERE_LIGHTS + 2u + 3u * i];
    let bulbRadius = gShadingFrame[FRAME_SPHERE_LIGHTS + 3u + 3u * i].x;
    let surfaceChannels = u32(gShadingFrame[FRAME_MATERIALS + min(objectIndex, MAX_MATERIALS - 1u)].z);
    if ((u32(b.w) & surfaceChannels) == 0u) {
        return;
    }
    // PointLightUtil_PBR::Init.
    let surfaceToLight = a.xyz - s.position;
    let surfaceToLightDirection = normalize(surfaceToLight);
    var lightDistanceSquared = dot(surfaceToLight, surfaceToLight);
    let falloff = lightDistanceSquared * a.w;
    if (falloff >= 1.0) {
        return;
    }
    let transmissionDistance = 0.0;
    // Apply.
    var radiusAttenuation = 1.0 - falloff * falloff;
    radiusAttenuation = radiusAttenuation * radiusAttenuation;
    lightDistanceSquared = max(0.001 * 0.001, lightDistanceSquared);
    let lightIntensity = b.xyz / lightDistanceSquared * radiusAttenuation;
    (*l).diffuseLighting += o3deDiffuse(s, *l, lightIntensity, surfaceToLightDirection, u32(gShading.diffuseModel));
    let posToLightDist = length(surfaceToLight);
    (*l).translucentBackLighting += getBackLighting(s, *l, lightIntensity, surfaceToLightDirection, transmissionDistance, posToLightDist);
    // The bulb's size: the direction to the closest point of the sphere to the reflection ray, and an energy normalization.
    let reflectionDir = reflect(-(*l).dirToCamera, s.normal);
    let reflectionPosToLight = surfaceToLight - dot(surfaceToLight, reflectionDir) * reflectionDir;
    let surfaceToLightAdjusted = surfaceToLight - reflectionPosToLight * saturate(bulbRadius / length(reflectionPosToLight));
    let sphereIntensityNormalization = intensityAdjustedByRadiusAndRoughness(s.roughnessA, bulbRadius, lightDistanceSquared);
    (*l).specularLighting += sphereIntensityNormalization * o3deSpecular(s, *l, lightIntensity, normalize(surfaceToLightAdjusted));
}

// Ibl.azsli: ApplyIBL's specular in the forward pass, which clear coat
// switches on (StandardPBR_ClearCoatEnableFeature.lua sets
// o_materialUseForwardPassIBLSpecular): GetIblSpecular from the global
// cubemap with the multiscatter compensation, then the coat's own IBL and the
// base layer through (1 - Fc)^2, times 2^m_iblExposure.
// GetIblSpecular: the global cubemap, blended toward the mesh's reflection
// probe (ObjectSrg m_reflectionProbeData; -1 without one) by the inner/outer
// weight, parallax-corrected, as ENABLE_REFLECTIONPROBE_PARALLAXCORRECTION and
// ENABLE_REFLECTIONPROBE_BLENDING (both 1) compile it.
fn getIblSpecular(position: vec3<f32>, normal: vec3<f32>, specularF0: vec3<f32>, roughnessLinear: f32, dirToCamera: vec3<f32>, brdf: vec2<f32>, probeIndex: i32) -> vec3<f32> {
    let reflectDir = reflect(-dirToCamera, normal);
    var outSpecular = sampleSpecularEnv(reflectDir, roughnessLinear * 5.0) * (specularF0 * brdf.x + brdf.y);
    if (probeIndex >= 0) {
        let probe = probeAt(u32(probeIndex));
        var dir = reflectDir;
        if (probe.parallax) {
            dir = parallaxCorrection(probe, position, reflectDir);
        }
        let probeSpecular = sampleProbeEnv(probe.cubemap, dir, roughnessLinear * 5.0) * (specularF0 * brdf.x + brdf.y);
        // Inside the inner box the lerp is 1 (O3DE's normalize of a zero vector falls through to 1).
        let blendAmount = select(lerpBetweenInnerOuter(probe, position), 1.0, obbContainsPoint(probe, probe.innerHalf, position));
        outSpecular = mix(outSpecular, probeSpecular, blendAmount);
    }
    return outSpecular;
}

// The mesh's forward-pass reflection probe: the smallest whose outer box overlaps its bounds.
fn forwardProbe(objectIndex: u32) -> i32 {
    return i32(gShadingFrame[FRAME_MATERIALS + min(objectIndex, MAX_MATERIALS - 1u)].w);
}

fn forwardIblSpecular(s: O3deSurface, l: O3deLightingData, probeIndex: i32) -> vec3<f32> {
    var iblSpecular = getIblSpecular(s.position, s.normal, s.specularF0, s.roughnessLinear, l.dirToCamera, l.brdf, probeIndex);
    iblSpecular *= l.multiScatterCompensation;
    if (s.clearCoat && s.clearCoatFactor > 0.0) {
        let clearCoatNdotV = max(saturate(dot(s.clearCoatNormal, l.dirToCamera)), 0.01);
        let clearCoatBrdf = sampleBrdfMap(vec2<f32>(clearCoatNdotV, 1.0 - s.clearCoatRoughness));
        var coatSpecular = getIblSpecular(s.position, s.clearCoatNormal, vec3<f32>(0.04), s.clearCoatRoughness, l.dirToCamera, clearCoatBrdf, probeIndex);
        coatSpecular *= s.clearCoatFactor;
        let clearCoatResponse = fresnelSchlickWithRoughness(clearCoatNdotV, vec3<f32>(0.04), s.clearCoatRoughness) * s.clearCoatFactor;
        iblSpecular = iblSpecular * (1.0 - clearCoatResponse) * (1.0 - clearCoatResponse) + coatSpecular;
    }
    // A probe's exposure replaces the skylight's.
    let exposure = select(pow(2.0, gShading.iblExposure), pow(2.0, probeAt(u32(max(probeIndex, 0))).exposure), probeIndex >= 0);
    return iblSpecular * exposure;
}

// ---------------------------------------------------------------------------
// Image-based lighting: the cubemaps' lookups, and the two full-screen passes
// that light opaque surfaces with them (Ibl.azsli's ApplyIBL runs in the
// forward pass only for transparent ones).

// CubeMapGen's face table (sgFace2DMapping): u direction, v direction, axis.
fn cubeFace(face: u32) -> mat3x3<f32> {
    switch face {
        case 0u: { return mat3x3<f32>(vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0)); }
        case 1u: { return mat3x3<f32>(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0)); }
        case 2u: { return mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0)); }
        case 3u: { return mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, -1.0, 0.0)); }
        case 4u: { return mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0)); }
        default: { return mat3x3<f32>(vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, 0.0, -1.0)); }
    }
}

// A cubemap lookup: the face of the largest component, then bilinear
// between texel centres, clamped at the face's edge (no seamless filtering).
fn cubeSample(offset: u32, size: u32, dir: vec3<f32>) -> vec3<f32> {
    let a = abs(dir);
    var face = 4u;
    var major = a.z;
    if (a.x >= a.y && a.x >= a.z) {
        face = select(1u, 0u, dir.x >= 0.0);
        major = a.x;
    } else if (a.y >= a.x && a.y >= a.z) {
        face = select(3u, 2u, dir.y >= 0.0);
        major = a.y;
    } else {
        face = select(5u, 4u, dir.z >= 0.0);
    }
    let onFace = dir / major;
    let axes = cubeFace(face);
    let s = f32(size);
    let p = clamp((vec2<f32>(dot(axes[0], onFace), dot(axes[1], onFace)) + 1.0) * 0.5 * s - 0.5, vec2<f32>(0.0), vec2<f32>(s - 1.0));
    let i0 = vec2<u32>(floor(p));
    let i1 = min(i0 + vec2<u32>(1u), vec2<u32>(size - 1u));
    let w = p - floor(p);
    let base = offset + face * size * size;
    let t00 = gShadingData[base + i0.y * size + i0.x].xyz;
    let t10 = gShadingData[base + i0.y * size + i1.x].xyz;
    let t01 = gShadingData[base + i1.y * size + i0.x].xyz;
    let t11 = gShadingData[base + i1.y * size + i1.x].xyz;
    return mix(mix(t00, t10, w.x), mix(t01, t11, w.x), w.y);
}

fn specularMipOffset(mip: u32) -> u32 {
    var offset = BRDF_MAP_SIZE * BRDF_MAP_SIZE;
    for (var m = 0u; m < mip; m++) {
        let size = SPECULAR_SIZE >> m;
        offset += 6u * size * size;
    }
    return offset;
}

// m_specularEnvMap.SampleLevel(..., mip): trilinear between two mips.
fn sampleSpecularEnv(dir: vec3<f32>, mip: f32) -> vec3<f32> {
    let m = clamp(mip, 0.0, f32(SPECULAR_MIPS - 1u));
    let m0 = u32(floor(m));
    let m1 = min(m0 + 1u, SPECULAR_MIPS - 1u);
    let a = cubeSample(specularMipOffset(m0), SPECULAR_SIZE >> m0, dir);
    let b = cubeSample(specularMipOffset(m1), SPECULAR_SIZE >> m1, dir);
    return mix(a, b, m - floor(m));
}

fn sampleDiffuseEnv(dir: vec3<f32>) -> vec3<f32> {
    return cubeSample(specularMipOffset(SPECULAR_MIPS), DIFFUSE_SIZE, dir);
}

// What an R8G8B8A8_UNORM target stores (round() takes ties to even).
fn unorm8(x: vec3<f32>) -> vec3<f32> {
    return round(saturate(x) * 255.0) / 255.0;
}

// ---------------------------------------------------------------------------
// Reflection probes (Reflections.pass: ReflectionProbeStencil,
// ReflectionProbeBlendWeight, ReflectionGlobalFullscreen,
// ReflectionProbeRenderOuter, ReflectionProbeRenderInner, then
// ReflectionComposite). gShadingFrame from FRAME_PROBES (reflections.rs): the
// count, then four vectors per probe in the feature processor's order
// (descending inner volume): (position, m_exposure), the rotation quaternion,
// (outer half lengths, m_useParallaxCorrection), (inner half lengths, the
// cubemap). Each probe's cubemap is in gShadingData after the decal
// textures, six mips like the skylight's specular cubemap.
const FRAME_PROBES: u32 = 200u;
const PROBE_RECORD: u32 = 4u;
const DECAL_TEXTURES: u32 = 2u;
// The reflection target's clear value (ReflectionGlobalFullscreen.pass).
const REFLECTION_CLEAR: vec3<f32> = vec3<f32>(0.4, 0.4, 0.4);

struct O3deProbe {
    position: vec3<f32>,
    exposure: f32,
    rotation: vec4<f32>,
    outerHalf: vec3<f32>,
    parallax: bool,
    innerHalf: vec3<f32>,
    cubemap: u32,
}

fn probeCount() -> u32 {
    return u32(gShadingFrame[FRAME_PROBES].x);
}

fn probeAt(i: u32) -> O3deProbe {
    let base = FRAME_PROBES + 1u + PROBE_RECORD * i;
    let a = gShadingFrame[base];
    let c = gShadingFrame[base + 2u];
    let d = gShadingFrame[base + 3u];
    return O3deProbe(a.xyz, a.w, gShadingFrame[base + 1u], c.xyz, c.w > 0.5, d.xyz, u32(d.w));
}

// m_modelToWorldInverse (without the extents) applied to a point, and its rotation to a vector.
fn probeLocalPoint(probe: O3deProbe, p: vec3<f32>) -> vec3<f32> {
    return transpose(matrixFromQuaternion(probe.rotation)) * (p - probe.position);
}

fn probeLocalVector(probe: O3deProbe, v: vec3<f32>) -> vec3<f32> {
    return transpose(matrixFromQuaternion(probe.rotation)) * v;
}

// Math.azsli: ObbContainsPoint, through AabbContainsPoint's
// step(min, p) - step(max, p) on |p|: inside when |p| < halfExtents.
fn obbContainsPoint(probe: O3deProbe, halfExtents: vec3<f32>, p: vec3<f32>) -> bool {
    let q = abs(probeLocalPoint(probe, p));
    let d = step(-halfExtents, q) - step(halfExtents, q);
    return d.x * d.y * d.z > 0.0;
}

// Math.azsli: ComputeLerpBetweenInnerOuterOBBs and ComputeLerpBetweenInnerOuterAABBs.
fn lerpBetweenInnerOuter(probe: O3deProbe, position: vec3<f32>) -> f32 {
    let p = abs(probeLocalPoint(probe, position));
    let closestPointOnInner = clamp(p, -probe.innerHalf, probe.innerHalf);
    let dirToPoint = normalize(p - closestPointOnInner);
    let distance = abs(probe.outerHalf - p) / dirToPoint;
    let shortestDistance = min(min(distance.x, distance.y), distance.z);
    let outerIntersect = p + dirToPoint * shortestDistance;
    let totalDistance = length(outerIntersect - closestPointOnInner);
    return select(1.0, saturate(shortestDistance / totalDistance), totalDistance > 0.0);
}

// LightingUtils.azsli: ApplyParallaxCorrectionOBB (ApplyParallaxCorrectionAABB in the probe's frame).
fn parallaxCorrection(probe: O3deProbe, positionWS: vec3<f32>, reflectDir: vec3<f32>) -> vec3<f32> {
    let p = probeLocalPoint(probe, positionWS);
    let r = probeLocalVector(probe, reflectDir);
    let rcpReflectDir = 1.0 / r;
    let intersectA = (probe.outerHalf - p) * rcpReflectDir;
    let intersectB = (-probe.outerHalf - p) * rcpReflectDir;
    let furthest = max(intersectA, intersectB);
    let distance = min(min(furthest.x, furthest.y), furthest.z);
    let local = r * distance + p;
    return normalize(matrixFromQuaternion(probe.rotation) * local);
}

fn probeMipOffset(cubemap: u32, mip: u32) -> u32 {
    let start = specularMipOffset(SPECULAR_MIPS) + 6u * DIFFUSE_SIZE * DIFFUSE_SIZE + OCCLUSION_WIDTH * OCCLUSION_HEIGHT + DECAL_TEXTURES * 2u * DECAL_SIZE * DECAL_SIZE;
    let stride = specularMipOffset(SPECULAR_MIPS) - BRDF_MAP_SIZE * BRDF_MAP_SIZE;
    return start + cubemap * stride + specularMipOffset(mip) - BRDF_MAP_SIZE * BRDF_MAP_SIZE;
}

fn sampleProbeEnv(cubemap: u32, dir: vec3<f32>, mip: f32) -> vec3<f32> {
    let m = clamp(mip, 0.0, f32(SPECULAR_MIPS - 1u));
    let m0 = u32(floor(m));
    let m1 = min(m0 + 1u, SPECULAR_MIPS - 1u);
    let a = cubeSample(probeMipOffset(cubemap, m0), SPECULAR_SIZE >> m0, dir);
    let b = cubeSample(probeMipOffset(cubemap, m1), SPECULAR_SIZE >> m1, dir);
    return mix(a, b, m - floor(m));
}

// What the reflection passes read at one sample: the forward pass's targets.
struct ReflectionInputs {
    position: vec3<f32>,
    normal: vec3<f32>,
    enableIBL: bool,
    multiscatter: bool,
    specularF0: vec3<f32>,
    roughness: f32,
    specularOcclusion: f32,
    // The stencil the forward pass left (0x83, 0x80 for forward-IBL materials, 0 with no surface).
    stencil: u32,
}

// ReflectionProbeRenderCommon.azsli: ComputeProbeSpecular; false where it discards.
fn computeProbeSpecular(probe: O3deProbe, halfExtents: vec3<f32>, input: ReflectionInputs, specular: ptr<function, vec3<f32>>) -> bool {
    if (!obbContainsPoint(probe, halfExtents, input.position) || !input.enableIBL) {
        return false;
    }
    let dirToCamera = normalize(gFrame.camera.position - input.position);
    let NdotV = max(dot(input.normal, dirToCamera), 0.01);
    let brdf = sampleBrdfMap(vec2<f32>(saturate(NdotV), 1.0 - input.roughness));
    let reflectDir = reflect(-dirToCamera, input.normal);
    var localReflectDir = reflectDir;
    if (probe.parallax) {
        localReflectDir = parallaxCorrection(probe, input.position, reflectDir);
    }
    let probeSpecular = sampleProbeEnv(probe.cubemap, localReflectDir, input.roughness * 5.0);
    let compensation = getMultiScatterCompensation(input.specularF0, brdf, input.multiscatter);
    *specular = probeSpecular * compensation * (input.specularF0 * brdf.x + brdf.y);
    return true;
}

// The probe blend weight at a point inside the outer volume: 1 inside the inner one.
fn probeBlendWeight(probe: O3deProbe, position: vec3<f32>) -> f32 {
    if (obbContainsPoint(probe, probe.innerHalf, position)) {
        return 1.0;
    }
    return lerpBetweenInnerOuter(probe, position);
}

// One sample through the reflection passes, in their order.
struct ReflectionSample {
    // ReflectionProbeStencil's stencil, and ReflectionProbeBlendWeight's sum.
    stencil: u32,
    blendWeight: f32,
    // The reflection target after ReflectionProbeRenderInner.
    reflection: vec3<f32>,
    // What ReflectionComposite adds onto the specular target.
    composited: vec3<f32>,
}

fn reflectionSample(input: ReflectionInputs) -> ReflectionSample {
    var out: ReflectionSample;
    // ReflectionProbeStencil: StencilRef None, Less on 0x3F, so only drawn
    // surfaces; each slightly reduced inner volume adds one where the surface
    // is inside it (back faces IncrementSaturate, front faces
    // DecrementSaturate on depth fail).
    var stencil = input.stencil;
    if ((stencil & STENCIL_REFLECTION_MASK) > 0u) {
        for (var i = 0u; i < probeCount(); i++) {
            let probe = probeAt(i);
            if (obbContainsPoint(probe, probe.innerHalf - vec3<f32>(0.05), input.position)) {
                stencil = (stencil & ~STENCIL_REFLECTION_MASK) | (min(stencil + 1u, 255u) & STENCIL_REFLECTION_MASK);
            }
        }
    }
    let masked = stencil & STENCIL_REFLECTION_MASK;
    out.stencil = stencil;
    // ReflectionProbeBlendWeight: Equal 3 (outside every inner volume), additive into R16_FLOAT, cleared to 0.
    var total = 0.0;
    if (masked == STENCIL_USE_IBL_SPECULAR_PASS) {
        for (var i = 0u; i < probeCount(); i++) {
            let probe = probeAt(i);
            if (obbContainsPoint(probe, probe.outerHalf, input.position)) {
                total += probeBlendWeight(probe, input.position);
            }
        }
    }
    out.blendWeight = total;
    // The reflection target, cleared to (0.4, 0.4, 0.4, 0).
    var reflection = REFLECTION_CLEAR;
    // ReflectionGlobalFullscreen: Equal 3; the weight the probes leave.
    if (masked == STENCIL_USE_IBL_SPECULAR_PASS) {
        reflection = vec3<f32>(0.0);
        if (input.enableIBL) {
            let dirToCamera = normalize(gFrame.camera.position - input.position);
            let reflectDir = reflect(-dirToCamera, input.normal);
            let globalSpecular = sampleSpecularEnv(reflectDir, input.roughness * 5.0);
            let NdotV = max(saturate(dot(input.normal, dirToCamera)), 0.01);
            let brdf = sampleBrdfMap(vec2<f32>(NdotV, 1.0 - input.roughness));
            let compensation = getMultiScatterCompensation(input.specularF0, brdf, input.multiscatter);
            let weight = 1.0 - min(total, 1.0);
            reflection = weight * globalSpecular * compensation * (input.specularF0 * brdf.x + brdf.y);
            reflection *= pow(2.0, gShading.iblExposure);
            reflection *= input.specularOcclusion;
        }
    }
    // ReflectionProbeRenderOuter: Equal 3, additive; each probe's weight over max(1, the sum).
    if (masked == STENCIL_USE_IBL_SPECULAR_PASS) {
        for (var i = 0u; i < probeCount(); i++) {
            let probe = probeAt(i);
            var specular = vec3<f32>(0.0);
            if (computeProbeSpecular(probe, probe.outerHalf, input, &specular)) {
                let weight = probeBlendWeight(probe, input.position) / max(1.0, total);
                reflection += specular * pow(2.0, probe.exposure) * weight;
            }
        }
    }
    // ReflectionProbeRenderInner: LessEqual 3, no blending; later (smaller) probes overwrite.
    if (masked >= STENCIL_USE_IBL_SPECULAR_PASS) {
        for (var i = 0u; i < probeCount(); i++) {
            let probe = probeAt(i);
            var specular = vec3<f32>(0.0);
            if (computeProbeSpecular(probe, probe.innerHalf, input, &specular)) {
                reflection = specular * pow(2.0, probe.exposure);
            }
        }
    }
    out.reflection = reflection;
    // ReflectionComposite: StencilRef 1, LessEqual on 0x3F, One/One.
    out.composited = select(vec3<f32>(0.0), reflection, masked >= 1u);
    return out;
}

struct O3deIbl {
    // The forward pass's outputs, as the full-screen passes read them back.
    albedo: vec3<f32>,
    specularF0: vec3<f32>,
    roughness: f32,
    // DiffuseGlobalFullscreen.
    irradiance: vec3<f32>,
    diffuse: vec3<f32>,
    // ReflectionGlobalFullscreen.
    reflectDir: vec3<f32>,
    mip: f32,
    prefiltered: vec3<f32>,
    NdotV: f32,
    brdf: vec2<f32>,
    compensation: vec3<f32>,
    // What ReflectionComposite adds (the global pass and the probes), the
    // stencil, and the probes' blend weight.
    specular: vec3<f32>,
    stencil: u32,
    blendWeight: f32,
}

// The forward pass writes albedo * diffuseResponse (times AO, 1 here),
// specularF0, and roughnessLinear to 8-bit targets; DiffuseGlobalFullscreen
// adds albedo * the diffuse cubemap at the normal, and ReflectionGlobalFullscreen
// the specular cubemap along the reflection at mip roughness * 5, times
// F0 * A + B and the multiscatter compensation, both times 2^m_iblExposure.
// The lab's world is +Y up, so the cubemaps are indexed by world directions
// (O3DE's GetCubemapCoords turns its +Z-up directions to the cubemap's).
fn o3deIbl(s: O3deSurface, l: O3deLightingData, eye: vec3<f32>, multiscatter: bool) -> O3deIbl {
    var out: O3deIbl;
    out.albedo = s.albedo * l.diffuseResponse * s.diffuseAO;
    out.specularF0 = s.specularF0;
    out.roughness = s.roughnessLinear;
    // R8G8B8A8_UNORM targets.
    out.albedo = unorm8(out.albedo);
    out.specularF0 = unorm8(out.specularF0);
    out.roughness = round(saturate(out.roughness) * 255.0) / 255.0;
    let exposure = pow(2.0, gShading.iblExposure);
    out.irradiance = sampleDiffuseEnv(s.normal);
    out.diffuse = out.albedo * out.irradiance * exposure;
    let dirToCamera = normalize(eye - s.position);
    out.reflectDir = reflect(-dirToCamera, s.normal);
    out.mip = out.roughness * 5.0;
    out.prefiltered = sampleSpecularEnv(out.reflectDir, out.mip);
    out.NdotV = max(saturate(dot(s.normal, dirToCamera)), 0.01);
    out.brdf = sampleBrdfMap(vec2<f32>(saturate(out.NdotV), 1.0 - out.roughness));
    out.compensation = getMultiScatterCompensation(out.specularF0, out.brdf, multiscatter);
    // The reflection passes at this point, with specular occlusion from the
    // albedo target's 8-bit alpha: what ReflectionComposite adds.
    let input = ReflectionInputs(s.position, s.normal, gShading.ibl > 0.5, multiscatter, out.specularF0, out.roughness, round(saturate(s.specularOcclusion) * 255.0) / 255.0, select(STENCIL_USE_IBL_SPECULAR_PASS, 0u, s.clearCoat) | STENCIL_USE_DIFFUSE_GI_PASS);
    let r = reflectionSample(input);
    out.specular = r.composited;
    out.stencil = r.stencil;
    out.blendWeight = r.blendWeight;
    return out;
}

// ---------------------------------------------------------------------------
// The pixel.

// The inverse of tonemap() in common.wgsl, so a false-colour view shows its
// colours unchanged on screen.
fn untonemap(display: vec3<f32>) -> vec3<f32> {
    let y = clamp(srgbToLinear(display), vec3<f32>(0.0), vec3<f32>(0.98));
    let a = 2.43 * y - 2.51;
    let b = 0.59 * y - 0.03;
    let c = 0.14 * y;
    let x = (-b - sqrt(max(b * b - 4.0 * a * c, vec3<f32>(0.0)))) / (2.0 * a);
    return x / gFrame.overlayParams.x;
}

// A value from 0 to 1 as a dark-blue to yellow ramp.
fn heat(v: f32) -> vec3<f32> {
    let t = saturate(v);
    let c0 = vec3<f32>(0.05, 0.08, 0.30);
    let c1 = vec3<f32>(0.10, 0.55, 0.65);
    let c2 = vec3<f32>(0.98, 0.90, 0.35);
    return select(mix(c1, c2, (t - 0.5) * 2.0), mix(c0, c1, t * 2.0), t < 0.5);
}

struct O3dePixel {
    color: vec3<f32>,
    // The surface after decals, as the forward pass writes it to its targets.
    surface: O3deSurface,
    clearCoat: bool,
    diffuseAO: f32,
    specularOcclusion: f32,
    diffuse: vec3<f32>,
    specular: vec3<f32>,
    terms: GgxTerms,
    lighting: O3deLightingData,
    light: PointLightSample,
    ibl: O3deIbl,
}

// One pixel: the forward pass (EvaluateLighting_ForwardPass) with the simple
// point light's diffuse and specular, the full-screen IBL passes added to
// them, emission, and DiffuseSpecularMerge's sum.
// The surface and lighting data of a pixel, before any light.
struct O3dePixelStart {
    s: O3deSurface,
    l: O3deLightingData,
}

// The surface (EvaluateSurface), before decals.
fn o3dePixelSurface(p: vec3<f32>, n: vec3<f32>, objectIndex: u32, dndx: vec3<f32>, dndy: vec3<f32>) -> O3deSurface {
    let o = gObjects[objectIndex];
    let material = gShading.materials[min(objectIndex, MAX_MATERIALS - 1u)];
    var s = o3deSurface(p, n, o.albedoRoughness.rgb, material.x, material.y, o.albedoRoughness.w, dndx, dndy);
    // The occlusion maps, on the sphere's UVs.
    if (material.z >= 0.0 || material.w >= 0.0) {
        let uv = sphereUv((o.worldToLocal * vec4<f32>(p, 1.0)).xyz - o.localCenter);
        s.diffuseAO = occlusionInput(uv, material.z);
        s.specularOcclusion = occlusionInput(uv, material.w);
    }
    return withTransmission(withClearCoat(s, objectIndex), objectIndex);
}

// InitializeLightingData, after the decals.
fn o3dePixelStart(s: O3deSurface, eye: vec3<f32>) -> O3dePixelStart {
    return O3dePixelStart(s, o3deLightingData(s, eye, gShading.multiscatter > 0.5));
}

fn simplePointLightCount() -> u32 {
    return u32(gShadingFrame[FRAME_LIGHTS].x);
}

// Light i of gShadingFrame at the surface.
fn simplePointLightAt(i: u32, surfacePosition: vec3<f32>) -> PointLightSample {
    let a = gShadingFrame[FRAME_LIGHTS + 1u + 2u * i];
    let b = gShadingFrame[FRAME_LIGHTS + 2u + 2u * i];
    return simplePointLight(a.xyz, a.w, b.xyz, surfacePosition);
}

// ApplySimplePointLight (ForwardPassSimplePointLights.azsli): lights of other
// lighting channels and lights at or beyond their radius are skipped.
fn applySimplePointLight(i: u32, objectIndex: u32, s: O3deSurface, l: ptr<function, O3deLightingData>) {
    let lightChannels = u32(gShadingFrame[FRAME_LIGHTS + 2u + 2u * i].w);
    let surfaceChannels = u32(gShadingFrame[FRAME_MATERIALS + min(objectIndex, MAX_MATERIALS - 1u)].z);
    // LightTypesCommon.azsli: IsSameLightChannel.
    if ((lightChannels & surfaceChannels) == 0u) {
        return;
    }
    let light = simplePointLightAt(i, s.position);
    if (light.falloff < 1.0) {
        o3deApplyLight(s, l, light.intensity, light.dirToLight, u32(gShading.diffuseModel));
    }
}

// The pixel with every decal applied in sort order and every light in index
// order: what the tile lists give, since a culled decal or light cannot reach
// the pixel (the kernels and the CPU mirror have no tiles).
fn o3dePixel(p: vec3<f32>, n: vec3<f32>, eye: vec3<f32>, objectIndex: u32, dndx: vec3<f32>, dndy: vec3<f32>) -> O3dePixel {
    var s = o3dePixelSurface(p, n, objectIndex, dndx, dndy);
    for (var k = 0u; k < decalCount(); k++) {
        s = applyDecal(sortedDecal(k), s, n);
    }
    var start = o3dePixelStart(s, eye);
    // A plain local for the lights to add to (naga's SPIR-V writer fails on a
    // pointer to a struct member passed in a loop).
    var lighting = start.l;
    for (var i = 0u; i < simplePointLightCount(); i++) {
        applySimplePointLight(i, objectIndex, start.s, &lighting);
    }
    for (var i = 0u; i < sphereLightCount(); i++) {
        applySphereLight(i, objectIndex, start.s, &lighting);
    }
    start.l = lighting;
    return o3dePixelFinish(start, eye, objectIndex, 0u);
}

// The rest of the forward pass after its lights, and the full-screen passes'
// view of the pixel. `lightsInBin` feeds the light-culling view.
fn o3dePixelFinish(start: O3dePixelStart, eye: vec3<f32>, objectIndex: u32, lightsInBin: u32) -> O3dePixel {
    let s = start.s;
    var l = start.l;
    let o = gObjects[objectIndex];
    let p = s.position;
    let light = simplePointLightAt(0u, p);
    // Image-based lighting in the forward pass, for clear coat.
    let ibl = select(0.0, 1.0, gShading.ibl > 0.5);
    if (s.clearCoat) {
        l.specularLighting += forwardIblSpecular(s, l, forwardProbe(objectIndex)) * ibl;
    }
    // LightingData_BasePBR::FinalizeLighting.
    l.specularLighting *= s.specularOcclusion;
    l.diffuseLighting *= s.diffuseAO;
    // Transmitted light, after the occlusion.
    if (s.transmissionMode != TRANSMISSION_NONE) {
        l.diffuseLighting += l.translucentBackLighting * s.transmissionTint;
    }
    var out: O3dePixel;
    out.diffuseAO = s.diffuseAO;
    out.specularOcclusion = s.specularOcclusion;
    out.terms = ggxTerms(l.dirToCamera, light.dirToLight, s.normal, s.specularF0, l.NdotV, s.roughnessA2);
    out.lighting = l;
    out.light = light;
    out.diffuse = l.diffuseLighting;
    out.specular = l.specularLighting;
    // A forward-IBL material's stencil (0x80) keeps it out of the reflection passes.
    out.ibl = o3deIbl(s, l, eye, gShading.multiscatter > 0.5);
    out.clearCoat = s.clearCoat;
    out.surface = s;
    let emissive = o.emissive.rgb * o.emissive.w;
    let view = gShading.viewMode;
    // The reflection passes test the IBL flag themselves.
    var color = out.diffuse + out.specular + out.ibl.diffuse * ibl + out.ibl.specular + emissive;
    if (view == VIEW_DIFFUSE) {
        color = out.diffuse;
    } else if (view == VIEW_SPECULAR) {
        color = out.specular;
    } else if (view == VIEW_D) {
        color = untonemap(heat(out.terms.D / (out.terms.D + 1.0)));
    } else if (view == VIEW_G) {
        // The Smith masking-shadowing G itself: the visibility term times 4 NdotL NdotV.
        color = untonemap(heat(out.terms.G * 4.0 * out.terms.NdotL * l.NdotV));
    } else if (view == VIEW_F) {
        color = untonemap(clamp(out.terms.F, vec3<f32>(0.0), vec3<f32>(1.0)));
    } else if (view == VIEW_MULTISCATTER) {
        color = untonemap(heat((l.multiScatterCompensation.g - 1.0) / 0.6));
    } else if (view == VIEW_DIFFUSE_RESPONSE) {
        color = untonemap(clamp(l.diffuseResponse, vec3<f32>(0.0), vec3<f32>(1.0)));
    } else if (view == VIEW_NDOTL) {
        color = untonemap(heat(out.terms.NdotL));
    } else if (view == VIEW_ATTENUATION) {
        color = untonemap(heat(select(0.0, light.radiusAttenuation, light.falloff < 1.0)));
    } else if (view == VIEW_IBL_DIFFUSE) {
        color = out.ibl.diffuse;
    } else if (view == VIEW_IBL_SPECULAR) {
        color = out.ibl.specular;
    } else if (view == VIEW_LIGHT_COUNT) {
        // The lights in this pixel's tile and depth bin, 0 to 8 on the ramp.
        color = untonemap(heat(f32(lightsInBin) / 8.0));
    } else if (view == VIEW_PROBE_STENCIL) {
        color = untonemap(stencilColor(out.ibl.stencil & STENCIL_REFLECTION_MASK));
    } else if (view == VIEW_PROBE_WEIGHT) {
        color = untonemap(heat(out.ibl.blendWeight / 2.0));
    }
    out.color = color;
    return out;
}

// ---------------------------------------------------------------------------
// The pass chain (SceneShading in mod.rs), in the main pipeline's order: the
// forward pass writes O3DE's targets, then DiffuseGlobalFullscreen,
// ReflectionGlobalFullscreen (into its own reflection target),
// ReflectionComposite, and DiffuseSpecularMerge. The forward targets stay
// multisampled; each later pass works per sample, as O3DE's do, and writes
// the mean of its samples (where O3DE resolves after the composite).


// Math.azsli.
const UNORM_2BIT_FLAG1ONLY: f32 = 1.0 / 3.0;
const UNORM_2BIT_FLAG2ONLY: f32 = 2.0 / 3.0;
const UNORM_2BIT_BOTHFLAGS: f32 = 3.0 / 3.0;
const UNORM_2BIT_EPSILON: f32 = 10e-3;

// Math.azsli: EncodeNormalSignedOctahedron. Its z is saturate(z * FLOAT_32_MAX),
// written here as the sign test it amounts to.
fn encodeNormalSignedOctahedron(normal: vec3<f32>) -> vec3<f32> {
    let n = normal / (abs(normal.x) + abs(normal.y) + abs(normal.z));
    var e: vec3<f32>;
    e.y = n.y * 0.5 + 0.5;
    e.x = n.x * 0.5 + e.y;
    e.y = n.x * -0.5 + e.y;
    e.z = select(0.0, 1.0, n.z > 0.0);
    return e;
}

// Math.azsli: DecodeNormalSignedOctahedron.
fn decodeNormalSignedOctahedron(e: vec3<f32>) -> vec3<f32> {
    var n: vec3<f32>;
    n.x = e.x - e.y;
    n.y = (e.x + e.y) - 1.0;
    n.z = e.z * 2.0 - 1.0;
    n.z = n.z * (1.0 - abs(n.x) - abs(n.y));
    return normalize(n);
}

// Math.azsli: EncodeUnorm2BitFlags, DecodeUnorm2BitFlag1 and 2.
fn encodeUnorm2BitFlags(flag1: bool, flag2: bool) -> f32 {
    return (f32(flag1) + f32(flag2) * 2.0) / 3.0;
}
fn decodeUnorm2BitFlag1(v: f32) -> bool {
    return abs(v - UNORM_2BIT_FLAG1ONLY) < UNORM_2BIT_EPSILON || v == UNORM_2BIT_BOTHFLAGS;
}
fn decodeUnorm2BitFlag2(v: f32) -> bool {
    return abs(v - UNORM_2BIT_FLAG2ONLY) < UNORM_2BIT_EPSILON || v == UNORM_2BIT_BOTHFLAGS;
}

// ---------------------------------------------------------------------------
// Tiled light culling (LightCullingParent.pass: LightCullingTilePrepare,
// LightCulling, LightCullingRemap, before the forward pass), and the forward
// pass's walk of its lists (LightCullingTileIterator.azsli). Depth is reversed
// in both (near 1, the sky 0); O3DE's view space looks down -z, and the lab
// works in positive view distances, where O3DE's RH_COORD_SYSTEM_REVERSE
// products cancel. With no transparent objects in the room, the transparent
// depth range adds nothing.

// LightCullingShared.azsli and NVLC.azsli.
const TILE_DIM: u32 = 16u;
const NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN: u32 = 256u;
const NVLC_MAX_BINS: u32 = 8u;
const LOG_MAX_BINS: u32 = 3u;
const NVLC_ALL_BIN_BITS: u32 = 255u;
const NVLC_BINS_MASK: u32 = 7u;
const NVLC_END_OF_LIST: u32 = 0xFFFFu;
const NVLC_END_OF_GROUP: u32 = 0xFFFEu;
// The group order of LightCulling MainCS: decals, simple point, simple spot,
// point, disk, capsule, and quad lights.
const LIGHT_GROUPS: u32 = 7u;
const LIGHT_GROUP_DECALS: u32 = 0u;
const LIGHT_GROUP_SIMPLE_POINT: u32 = 1u;
const LIGHT_GROUP_POINT: u32 = 3u;

// The chain's buffers (mod.rs): per tile, TileLightData's four words (O3DE
// keeps them in an R32G32B32A32_UINT image), 256 list entries, and 8 x 256
// remapped entries (O3DE's R32_UINT LightListRemapped).
@group(2) @binding(24) var<storage, read_write> bTileData: array<u32>;
@group(2) @binding(25) var<storage, read_write> bLightList: array<u32>;
@group(2) @binding(26) var<storage, read_write> bLightListRemapped: array<u32>;

fn tileGridWidth() -> u32 {
    return (u32(gFrame.camera.viewportSize.x) + TILE_DIM - 1u) / TILE_DIM;
}

// NVLC.azsli: Tile_UnitValueToBit, saturate(x) * asfloat(asuint(32.0) - 1).
fn tileUnitValueToBit(x: f32) -> u32 {
    return u32(saturate(x) * bitcast<f32>(bitcast<u32>(32.0) - 1u));
}

// The view distance of a depth sample.
fn viewDistance(pixel: vec2<u32>, depth: f32) -> f32 {
    let world = reconstructWorldPosition(pixel, depth);
    return -(gFrame.camera.worldToView * vec4<f32>(world, 1.0)).z;
}

struct TileLightData {
    zNear: f32,
    zFar: f32,
    mask: u32,
    logMaxBins: u32,
    overflow: bool,
}

// NVLC.azsli: Tile_UnpackData; the lost bits are added back to zFar to stay conservative.
fn tileUnpackData(tile: u32) -> TileLightData {
    var data: TileLightData;
    data.zNear = bitcast<f32>(bTileData[4u * tile]);
    data.zFar = bitcast<f32>(bTileData[4u * tile + 1u] | NVLC_BINS_MASK);
    data.mask = bTileData[4u * tile + 2u];
    data.logMaxBins = bTileData[4u * tile + 1u] & NVLC_BINS_MASK;
    data.overflow = (bTileData[4u * tile + 3u] >> 31u) != 0u;
    return data;
}

var<workgroup> wDepthNear: atomic<u32>;
var<workgroup> wDepthFar: atomic<u32>;
var<workgroup> wOpaqueMask: atomic<u32>;

// LightCullingTilePrepare MainCS (its 4x MSAA path; the lab renders 4x): the
// tile's nearest sample and its farthest one short of the sky (the sky is
// moved to the near plane for the far bound), then one bit of a 32-slice
// mask per sample between them. One workgroup per 16 x 16 tile.
@compute @workgroup_size(16, 16)
fn o3deLightCullingTilePrepareCS(@builtin(global_invocation_id) id: vec3<u32>, @builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) index: u32) {
    if (index == 0u) {
        atomicStore(&wDepthNear, bitcast<u32>(3.4e38));
        atomicStore(&wDepthFar, 0u);
        atomicStore(&wOpaqueMask, 0u);
    }
    workgroupBarrier();
    let onScreen = all(id.xy < textureDimensions(tDepth));
    var z = array<f32, 4>(0.0, 0.0, 0.0, 0.0);
    var sky = array<bool, 4>(true, true, true, true);
    if (onScreen) {
        var nearest = 3.4e38;
        var farthest = gFrame.camera.nearPlane;
        for (var i = 0u; i < 4u; i++) {
            let d = textureLoad(tDepth, id.xy, i);
            // DepthLessThanInf: the sky is at depth 0.
            sky[i] = d <= 0.0;
            z[i] = viewDistance(id.xy, d);
            nearest = min(nearest, z[i]);
            // InfDepthToNearPlane: the sky does not stretch the far bound.
            farthest = max(farthest, select(z[i], gFrame.camera.nearPlane, sky[i]));
        }
        // Distances are positive, so their bits order as the floats do.
        atomicMin(&wDepthNear, bitcast<u32>(nearest));
        atomicMax(&wDepthFar, bitcast<u32>(farthest));
    }
    workgroupBarrier();
    let zNear = bitcast<f32>(atomicLoad(&wDepthNear));
    // IncrementULP.
    let zFar = bitcast<f32>(atomicLoad(&wDepthFar) + 1u);
    if (onScreen) {
        // DepthSamplesToBinMask4x: one bit per sample, none for the sky.
        var mask = 0u;
        for (var i = 0u; i < 4u; i++) {
            if (!sky[i]) {
                mask |= 1u << tileUnitValueToBit((z[i] - zNear) / (zFar - zNear));
            }
        }
        atomicOr(&wOpaqueMask, mask);
    }
    workgroupBarrier();
    // WriteTileLightDataToMainMemory: 1, 2, 4, or 8 bins from the tile's depth span in metres.
    if (index == 0u) {
        let tile = group.y * tileGridWidth() + group.x;
        let logMaxBins = clamp(log2(abs(zFar - zNear) / 1.0), 0.0, f32(LOG_MAX_BINS));
        bTileData[4u * tile] = bitcast<u32>(zNear);
        bTileData[4u * tile + 1u] = (bitcast<u32>(zFar) & ~NVLC_BINS_MASK) | u32(logMaxBins + 0.5);
        bTileData[4u * tile + 2u] = atomicLoad(&wOpaqueMask);
        bTileData[4u * tile + 3u] = 0u;
    }
}

// NVLC.azsli: g_binMask0 and g_binMask1, the 32 slices folded into 8 bins.
fn binMask(logMaxBins: u32, bin: u32) -> u32 {
    switch logMaxBins {
        case 0u: { return select(0u, 0xFFFFFFFFu, bin == 0u); }
        case 1u: { return select(0u, 0xFFFFu << (16u * bin), bin < 2u); }
        case 2u: { return select(0u, 0xFFu << (8u * bin), bin < 4u); }
        default: { return 0xFu << (4u * bin); }
    }
}

// NVLC.azsli: IsObjectInsideTile, for an object's view-distance range; the
// bins it touches are added to `bins`.
fn isObjectInsideTile(data: TileLightData, objectMinMax: vec2<f32>, bins: ptr<function, u32>) -> bool {
    let normalized = (objectMinMax - data.zNear) / (data.zFar - data.zNear);
    let extents = vec2<u32>(1u << tileUnitValueToBit(normalized.x), 1u << tileUnitValueToBit(normalized.y));
    let objectMask = ((extents.y - 1u) | extents.y) & ~(extents.x - 1u);
    var overlap = data.mask & objectMask;
    if (!(normalized.x <= 1.0 && normalized.y >= 0.0)) {
        overlap = 0u;
    }
    for (var bin = 0u; bin < NVLC_MAX_BINS; bin++) {
        if ((overlap & binMask(data.logMaxBins, bin)) != 0u) {
            *bins |= 1u << bin;
        }
    }
    return overlap != 0u;
}

// LightCulling BuildAabb: the view-space box through the tile's corners at its near and far distances.
struct TileAabb {
    center: vec3<f32>,
    halfExtents: vec3<f32>,
}

fn viewSpacePosition(uv: vec2<f32>, distance: f32) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let projection = gFrame.camera.viewToClip;
    return vec3<f32>(ndc.x * distance / projection[0][0], ndc.y * distance / projection[1][1], -distance);
}

fn buildAabb(data: TileLightData, tile: vec2<u32>) -> TileAabb {
    let grid = vec2<f32>(f32(TILE_DIM)) / gFrame.camera.viewportSize;
    let uvMin = vec2<f32>(tile) * grid;
    let uvMax = uvMin + grid;
    let p0 = viewSpacePosition(uvMin, data.zNear);
    let p1 = viewSpacePosition(uvMin, data.zFar);
    let p2 = viewSpacePosition(uvMax, data.zNear);
    let p3 = viewSpacePosition(uvMax, data.zFar);
    let lo = min(min(p0, p1), min(p2, p3));
    let hi = max(max(p0, p1), max(p2, p3));
    let center = (lo + hi) * 0.5;
    return TileAabb(center, center - lo);
}

// LightCulling: TestSphereVsAabb.
fn testSphereVsAabb(sphereCenter: vec3<f32>, sphereRadiusSq: f32, box: TileAabb) -> bool {
    let delta = max(vec3<f32>(0.0), abs(box.center - sphereCenter) - box.halfExtents);
    return dot(delta, delta) < sphereRadiusSq;
}

// LightCulling AreDecalsOutofOrder, on packed entries: by sort key, then index.
fn areDecalsOutOfOrder(left: u32, right: u32) -> bool {
    let leftIndex = left >> 16u;
    let rightIndex = right >> 16u;
    let leftKey = decalAt(leftIndex).sortKey & 0xFFu;
    let rightKey = decalAt(rightIndex).sortKey & 0xFFu;
    if (leftKey == rightKey) {
        return leftIndex > rightIndex;
    }
    return leftKey > rightKey;
}

// LightCulling: TestSphereVsAabbInvSqrt.
fn testSphereVsAabbInvSqrt(sphereCenter: vec3<f32>, invSphereRadiusSq: f32, box: TileAabb) -> bool {
    let delta = max(vec3<f32>(0.0), abs(box.center - sphereCenter) - box.halfExtents);
    return dot(delta, delta) * invSphereRadiusSq < 1.0;
}

// LightCulling MainCS for one tile: each light group in order, every light
// that meets the tile's box and its depth mask packed as index << 16 | bins,
// each group closed by END_OF_GROUP; the count is clamped to 255 slots. O3DE
// appends a group's lights in the order its threads finish (InterlockedAdd);
// the lab appends them in light-index order, so frames stay deterministic.
@compute @workgroup_size(16, 16)
fn o3deLightCullingCS(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) index: u32) {
    if (index != 0u) {
        return;
    }
    let tile = group.y * tileGridWidth() + group.x;
    let data = tileUnpackData(tile);
    let box = buildAabb(data, group.xy);
    let base = tile * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN;
    var count = 0u;
    for (var g = 0u; g < LIGHT_GROUPS; g++) {
        if (g == LIGHT_GROUP_DECALS) {
            for (var i = 0u; i < decalCount(); i++) {
                let decal = decalAt(i);
                // CullDecals: the draw distance (0: none), then a sphere around the box.
                let center = (gFrame.camera.worldToView * vec4<f32>(decal.position, 1.0)).xyz;
                if (decal.maxDrawDistance > 0.0 && dot(center, center) > decal.maxDrawDistance * decal.maxDrawDistance) {
                    continue;
                }
                let radiusSq = dot(decal.halfSize, decal.halfSize);
                if (testSphereVsAabb(center, radiusSq, box)) {
                    let radius = sqrt(radiusSq);
                    var bins = 0u;
                    if (isObjectInsideTile(data, vec2<f32>(-center.z - radius, -center.z + radius), &bins)) {
                        bLightList[base + min(count, NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN - 1u)] = (i << 16u) | bins;
                        count++;
                    }
                }
            }
            // SortDecals: O3DE's bitonic sort across the group's threads; an
            // insertion sort here, with the same order (keys and indices
            // leave no ties).
            let n = min(count, NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN - 1u);
            for (var a = 1u; a < n; a++) {
                let entry = bLightList[base + a];
                var b = a;
                while (b > 0u && areDecalsOutOfOrder(bLightList[base + b - 1u], entry)) {
                    bLightList[base + b] = bLightList[base + b - 1u];
                    b--;
                }
                bLightList[base + b] = entry;
            }
        }
        if (g == LIGHT_GROUP_SIMPLE_POINT) {
            for (var i = 0u; i < simplePointLightCount(); i++) {
                let a = gShadingFrame[FRAME_LIGHTS + 1u + 2u * i];
                // CullPointLight: the light's view position, the sphere test, then its depth range.
                let center = (gFrame.camera.worldToView * vec4<f32>(a.xyz, 1.0)).xyz;
                if (testSphereVsAabbInvSqrt(center, a.w, box)) {
                    let radius = inverseSqrt(a.w);
                    var bins = 0u;
                    if (isObjectInsideTile(data, vec2<f32>(-center.z - radius, -center.z + radius), &bins)) {
                        bLightList[base + min(count, NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN - 1u)] = (i << 16u) | bins;
                        count++;
                    }
                }
            }
        }
        if (g == LIGHT_GROUP_POINT) {
            for (var i = 0u; i < sphereLightCount(); i++) {
                let a = gShadingFrame[FRAME_SPHERE_LIGHTS + 1u + 3u * i];
                // CullPointLights: CullPointLight, as for simple point lights.
                let center = (gFrame.camera.worldToView * vec4<f32>(a.xyz, 1.0)).xyz;
                if (testSphereVsAabbInvSqrt(center, a.w, box)) {
                    let radius = inverseSqrt(a.w);
                    var bins = 0u;
                    if (isObjectInsideTile(data, vec2<f32>(-center.z - radius, -center.z + radius), &bins)) {
                        bLightList[base + min(count, NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN - 1u)] = (i << 16u) | bins;
                        count++;
                    }
                }
            }
        }
        // WriteEndOfGroup.
        bLightList[base + min(count, NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN - 1u)] = (NVLC_END_OF_GROUP << 16u) | NVLC_ALL_BIN_BITS;
        count++;
    }
    // LightCount (O3DE's R32_UINT image), kept in the tile data's last word until the remap.
    bTileData[4u * tile + 3u] = count;
}

// LightCullingRemap MainCS for one tile: for each depth bin, the indices of
// the entries that touch it, in list order, ending with END_OF_LIST; the
// tile's last word then holds the worst bin's count, with the overflow flag
// in its top bit.
@compute @workgroup_size(16, 16)
fn o3deLightCullingRemapCS(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) index: u32) {
    if (index != 0u) {
        return;
    }
    let tile = group.y * tileGridWidth() + group.x;
    var total = bTileData[4u * tile + 3u];
    let overflow = total > NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN - 1u;
    total = min(total, NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN - 1u);
    let list = tile * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN;
    let baseBin = tile * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN * NVLC_MAX_BINS;
    var worst = 0u;
    for (var bin = 0u; bin < NVLC_MAX_BINS; bin++) {
        var write = baseBin + bin * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN;
        let start = write;
        for (var k = 0u; k < total; k++) {
            let bins = bLightList[list + k];
            if ((bins & (1u << bin)) != 0u) {
                bLightListRemapped[write] = bins >> 16u;
                write++;
            }
        }
        worst = max(worst, write - start);
        bLightListRemapped[write] = NVLC_END_OF_LIST;
    }
    bTileData[4u * tile + 3u] = worst | select(0u, 1u << 31u, overflow);
}

// LightCullingTileIterator.azsli.
struct TileIterator {
    readIndex: u32,
    value: u32,
}

fn lightCullingTileIterator(clip: vec4<f32>) -> TileIterator {
    let tileId = vec2<u32>(clip.xy) >> vec2<u32>(4u);
    let data = tileUnpackData(tileId.y * tileGridWidth() + tileId.x);
    // NVLC_GetBin, with clip w the view distance.
    let f = saturate((abs(clip.w) - data.zNear) / (data.zFar - data.zNear));
    let bin = u32(min(f, 0.999999) * f32(1u << data.logMaxBins));
    let tile = tileId.y * tileGridWidth() + tileId.x;
    return TileIterator((tile * NVLC_MAX_BINS + bin) * NVLC_MAX_POSSIBLE_LIGHTS_PER_BIN, 0u);
}

fn loadAdvance(it: ptr<function, TileIterator>) {
    if ((*it).value != NVLC_END_OF_LIST) {
        (*it).value = bLightListRemapped[(*it).readIndex];
        (*it).readIndex++;
    }
}

fn isDone(it: TileIterator) -> bool {
    return !((it).value < NVLC_END_OF_GROUP);
}

// The fading objects' coverage, as the forward pass draws it.
fn presenceCoverage(objectIndex: u32, pixel: vec2<f32>) -> u32 {
    let presence = objectPresence(objectIndex);
    if (presence < 1.0) {
        return (1u << u32(clamp(floor(presence * 4.0 + interleavedGradientNoise(pixel)), 0.0, 4.0))) - 1u;
    }
    return 0xffffffffu;
}

// The depth pre-pass (MainPipeline.pass DepthPrePass): depth only, with the
// forward pass's coverage, so light culling sees the frame's depth first.
@fragment
fn o3dePrepassFS(input: SceneVaryings) -> @builtin(sample_mask) u32 {
    return presenceCoverage(input.objectIndex, input.clip.xy);
}

// ForwardPassOutput (ForwardPassOutput.azsli) and the lab's false-colour view.
struct O3deForwardOutput {
    @location(0) diffuse: vec4<f32>,
    @location(1) specular: vec4<f32>,
    @location(2) albedo: vec4<f32>,
    @location(3) specularF0: vec4<f32>,
    @location(4) normal: vec4<f32>,
    @location(5) debug: vec4<f32>,
    // The stencil reference the mesh draws with (MeshFeatureProcessor), / 255.
    @location(6) stencil: f32,
    @builtin(sample_mask) coverage: u32,
}

// RenderCommon.h: StencilRefs.
const STENCIL_USE_IBL_SPECULAR_PASS: u32 = 0x3u;
const STENCIL_USE_DIFFUSE_GI_PASS: u32 = 0x80u;
const STENCIL_REFLECTION_MASK: u32 = 0x3Fu;

// The forward pass's PixelShader (ForwardPassVertexAndPixel.azsli): the
// lighting into the diffuse and specular targets, emission added to
// specular, albedo times the diffuse response and diffuse AO with specular
// occlusion in alpha, specularF0 with roughnessLinear in alpha, and the
// normal with the full-screen IBL and multiscatter flags.
@fragment
fn o3deForwardFS(input: SceneVaryings) -> O3deForwardOutput {
    var n = normalize(input.worldNormal);
    let eye = gFrame.camera.position;
    if (dot(n, eye - input.worldPosition) < 0.0) {
        n = -n;
    }
    // CalculateSpecularAA's ddx_fine and ddy_fine of the normal.
    let dndx = dpdxFine(n);
    let dndy = dpdyFine(n);
    // EvaluateLighting_ForwardPass: the lights from this pixel's tile and depth bin.
    var surface = o3dePixelSurface(input.worldPosition, n, input.objectIndex, dndx, dndy);
    var tile = lightCullingTileIterator(input.clip);
    // ApplyDecals_ForwardPass: the first group.
    loadAdvance(&tile);
    while (!isDone(tile)) {
        let index = tile.value;
        loadAdvance(&tile);
        surface = applyDecal(index, surface, n);
    }
    var start = o3dePixelStart(surface, eye);
    // ApplySimplePointLights: the second group.
    var lightsInBin = 0u;
    var lighting = start.l;
    loadAdvance(&tile);
    while (!isDone(tile)) {
        let index = tile.value;
        loadAdvance(&tile);
        applySimplePointLight(index, input.objectIndex, start.s, &lighting);
        lightsInBin++;
    }
    // ApplySimpleSpotLights (none in the room), then ApplyPointLights: the third and fourth groups.
    loadAdvance(&tile);
    while (!isDone(tile)) {
        loadAdvance(&tile);
    }
    loadAdvance(&tile);
    while (!isDone(tile)) {
        let index = tile.value;
        loadAdvance(&tile);
        applySphereLight(index, input.objectIndex, start.s, &lighting);
        lightsInBin++;
    }
    start.l = lighting;
    let px = o3dePixelFinish(start, eye, input.objectIndex, lightsInBin);
    let o = gObjects[input.objectIndex];
    // GetPbrLightingOutput: the surface after decals (with clear coat's specularF0).
    let s = px.surface;
    var out: O3deForwardOutput;
    out.diffuse = vec4<f32>(px.diffuse, -1.0);
    out.specular = vec4<f32>(px.specular + o.emissive.rgb * o.emissive.w, 1.0);
    out.albedo = vec4<f32>(s.albedo * px.lighting.diffuseResponse * px.diffuseAO, px.specularOcclusion);
    out.specularF0 = vec4<f32>(s.specularF0, s.roughnessLinear);
    out.normal = vec4<f32>(encodeNormalSignedOctahedron(s.normal), encodeUnorm2BitFlags(gShading.ibl > 0.5, gShading.multiscatter > 0.5));
    let view = gShading.viewMode;
    out.debug = vec4<f32>(px.color, select(0.0, 1.0, (view >= VIEW_D && view <= VIEW_ATTENUATION) || view == VIEW_LIGHT_COUNT));
    // UseIBLSpecularPass unless the material needs forward-pass IBL specular (clear coat), and always UseDiffuseGIPass.
    out.stencil = f32(select(STENCIL_USE_IBL_SPECULAR_PASS, 0u, px.clearCoat) | STENCIL_USE_DIFFUSE_GI_PASS) / 255.0;
    out.coverage = presenceCoverage(input.objectIndex, input.clip.xy);
    return out;
}

@group(2) @binding(0) var tDiffuse: texture_multisampled_2d<f32>;
@group(2) @binding(2) var tSpecular: texture_multisampled_2d<f32>;
@group(2) @binding(4) var tAlbedo: texture_multisampled_2d<f32>;
@group(2) @binding(6) var tSpecularF0: texture_multisampled_2d<f32>;
@group(2) @binding(8) var tNormal: texture_multisampled_2d<f32>;
@group(2) @binding(10) var tDebug: texture_multisampled_2d<f32>;
@group(2) @binding(12) var tStencil: texture_multisampled_2d<f32>;
@group(2) @binding(14) var tDiffuseLit: texture_2d<f32>;
@group(2) @binding(15) var oDiffuseLit: texture_storage_2d<rgba16float, write>;
@group(2) @binding(16) var tReflection: texture_2d<f32>;
@group(2) @binding(17) var oReflection: texture_storage_2d<rgba16float, write>;
@group(2) @binding(18) var tSpecularLit: texture_2d<f32>;
@group(2) @binding(19) var oSpecularLit: texture_storage_2d<rgba16float, write>;
@group(2) @binding(20) var tProbeView: texture_2d<f32>;
@group(2) @binding(21) var oProbeView: texture_storage_2d<rgba16float, write>;
@group(2) @binding(22) var tDepth: texture_depth_multisampled_2d;
@group(2) @binding(23) var oHdr: texture_storage_2d<rgba16float, write>;

// The stencil value of a sample: 0 where nothing was drawn.
fn stencilAt(pixel: vec2<u32>, i: u32) -> u32 {
    return u32(round(textureLoad(tStencil, pixel, i).r * 255.0));
}

// ReflectionCommon.azsli: ReconstructWorldPositionFromDepth, with the lab's
// inverse view-projection (O3DE multiplies its inverse projection and view).
fn reconstructWorldPosition(pixel: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = saturate((vec2<f32>(pixel) + 0.5) / vec2<f32>(textureDimensions(tDepth)));
    let clip = vec4<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0, depth, 1.0);
    let inverse = mat4x4<f32>(gShadingFrame[0], gShadingFrame[1], gShadingFrame[2], gShadingFrame[3]);
    let world = inverse * clip;
    return world.xyz / world.w;
}

// Where no surface was drawn (depth 0, the far plane of the reversed depth),
// the merge pass draws the sky.
fn covered(depth: f32) -> bool {
    return depth > 0.0;
}

// DiffuseGlobalFullscreen MainPS: the albedo target times the diffuse
// cubemap at the normal, times 2^m_iblExposure, added onto the diffuse target
// (O3DE blends One/One; the lab writes the sum to a new target).
@compute @workgroup_size(8, 8)
fn o3deDiffuseGlobalCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = id.xy;
    if (any(pixel >= textureDimensions(oDiffuseLit))) {
        return;
    }
    let samples = textureNumSamples(tDiffuse);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < samples; i++) {
        var diffuse = textureLoad(tDiffuse, pixel, i).rgb;
        let encoded = textureLoad(tNormal, pixel, i);
        // DiffuseGlobalFullscreen.shader: stencil Equal, ReadMask 0x80, StencilRef 128.
        // Its MainPS reads no IBL flag; the lab's IBL toggle stands for a scene
        // without the global skylight.
        if ((stencilAt(pixel, i) & STENCIL_USE_DIFFUSE_GI_PASS) == STENCIL_USE_DIFFUSE_GI_PASS && gShading.ibl > 0.5) {
            let normal = decodeNormalSignedOctahedron(encoded.rgb);
            let irradiance = sampleDiffuseEnv(normal);
            diffuse += textureLoad(tAlbedo, pixel, i).rgb * irradiance * pow(2.0, gShading.iblExposure);
        }
        sum += diffuse;
    }
    textureStore(oDiffuseLit, pixel, vec4<f32>(sum / f32(samples), 1.0));
}

// The reflection passes, per sample and in their order (reflectionSample):
// the probe stencil, the blend weight, ReflectionGlobalFullscreen, the outer
// and inner probe passes, and ReflectionComposite's stencil test. Each reads
// only its own sample and the earlier passes' results there, so one compute
// pass gives what O3DE's six give. The reflection target holds the mean of
// what the composite adds; probeView the stencil and blend-weight views.
@compute @workgroup_size(8, 8)
fn o3deReflectionsCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = id.xy;
    if (any(pixel >= textureDimensions(oReflection))) {
        return;
    }
    let samples = textureNumSamples(tNormal);
    var sum = vec3<f32>(0.0);
    var stencilView = vec3<f32>(0.0);
    var weightView = 0.0;
    for (var i = 0u; i < samples; i++) {
        let encoded = textureLoad(tNormal, pixel, i);
        let depth = textureLoad(tDepth, pixel, i);
        let f0r = textureLoad(tSpecularF0, pixel, i);
        var input: ReflectionInputs;
        input.position = reconstructWorldPosition(pixel, depth);
        input.normal = decodeNormalSignedOctahedron(encoded.rgb);
        input.enableIBL = decodeUnorm2BitFlag1(encoded.a);
        input.multiscatter = decodeUnorm2BitFlag2(encoded.a);
        input.specularF0 = f0r.rgb;
        input.roughness = f0r.a;
        input.specularOcclusion = textureLoad(tAlbedo, pixel, i).a;
        input.stencil = stencilAt(pixel, i);
        let r = reflectionSample(input);
        sum += r.composited;
        stencilView += stencilColor(r.stencil & STENCIL_REFLECTION_MASK);
        weightView += r.blendWeight;
    }
    let n = f32(samples);
    textureStore(oReflection, pixel, vec4<f32>(sum / n, 1.0));
    textureStore(oProbeView, pixel, vec4<f32>(stencilView / n, weightView / n));
}

// The probe stencil view's colours: none, outside every inner volume (3), inside one, two, or more.
fn stencilColor(masked: u32) -> vec3<f32> {
    if (masked < STENCIL_USE_IBL_SPECULAR_PASS) {
        return vec3<f32>(0.02);
    }
    return heat(f32(masked - STENCIL_USE_IBL_SPECULAR_PASS) / 3.0);
}

// ReflectionComposite: the reflection target added onto the specular
// target (One/One, where the stencil marks a surface).
@compute @workgroup_size(8, 8)
fn o3deReflectionCompositeCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = id.xy;
    if (any(pixel >= textureDimensions(oSpecularLit))) {
        return;
    }
    let samples = textureNumSamples(tSpecular);
    // ReflectionComposite.shader: stencil LessEqual, ReadMask 0x3F, StencilRef 1,
    // so it adds the reflection target where 1 <= stencil & 0x3F;
    // o3deReflectionsCS applied that test per sample before taking the mean,
    // so adding the mean is exact.
    var specular = vec3<f32>(0.0);
    for (var i = 0u; i < samples; i++) {
        specular += textureLoad(tSpecular, pixel, i).rgb;
    }
    specular = specular / f32(samples) + textureLoad(tReflection, pixel, 0).rgb;
    let view = gShading.viewMode;
    if (gShading.specularResolve > 0.5 || view == VIEW_RESOLVE_DIFFERENCE) {
        let resolved = msaaResolveSpecular(vec2<i32>(pixel), samples);
        if (view == VIEW_RESOLVE_DIFFERENCE) {
            // How much the custom resolve changes the pixel, relative to a
            // plain mean of the same composited samples (the sky where nothing
            // was drawn), on the heat ramp: 0 to 10 percent.
            var mean = vec3<f32>(0.0);
            for (var i = 0u; i < samples; i++) {
                mean += compositedSpecular(vec2<i32>(pixel), i);
            }
            mean /= f32(samples);
            let relative = resolveLuminance(abs(resolved - mean)) / max(resolveLuminance(mean), 1e-3);
            specular = untonemap(heat(saturate(relative * 10.0)));
        } else {
            specular = resolved;
        }
    }
    textureStore(oSpecularLit, pixel, vec4<f32>(specular, 1.0));
}

// MSAAResolveCustom.azsl, as MSAAResolveSpecularPass runs it (OpaqueParent.pass:
// enableNeighborClamping 1, maxNeighborContrast 1.5; TONEMAPPING_STRATEGY
// LOG, CLAMPING_STRATEGY CLAMP_NEIGHBOR_MAX_LUMINANCE). O3DE resolves after
// ReflectionComposite has added the reflections onto the specular target,
// and the sky box has written the sky where nothing was drawn, so the lab
// resolves those composited samples (compositedSpecular); it renders four
// samples where O3DE renders two.
const RESOLVE_MAX_NEIGHBOR_CONTRAST: f32 = 1.5;

// CalcLuminance.
fn resolveLuminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

// Sample i of the specular target at pixel, after the composite; zero off the
// target, as Texture2DMS.Load gives out of bounds.
fn compositedSpecular(pixel: vec2<i32>, i: u32) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(tSpecular));
    if (any(pixel < vec2<i32>(0)) || any(pixel >= size)) {
        return vec3<f32>(0.0);
    }
    let p = vec2<u32>(pixel);
    if (!covered(textureLoad(tDepth, p, i))) {
        return skyRadiance(viewRay((vec2<f32>(p) + 0.5) / vec2<f32>(textureDimensions(tDepth))));
    }
    return textureLoad(tSpecular, p, i).rgb + textureLoad(tReflection, p, 0).rgb;
}

// MainPS: the brightest luminance of every sample of the four diagonal
// neighbours, then each of the pixel's samples scaled down to at most 1.5
// times it, and their mean taken in the log domain (a geometric mean).
fn msaaResolveSpecular(pixel: vec2<i32>, samples: u32) -> vec3<f32> {
    var brightest = 0.0;
    for (var i = 0u; i < samples; i++) {
        brightest = max(brightest, resolveLuminance(compositedSpecular(pixel + vec2<i32>(1, 1), i)));
        brightest = max(brightest, resolveLuminance(compositedSpecular(pixel + vec2<i32>(1, -1), i)));
        brightest = max(brightest, resolveLuminance(compositedSpecular(pixel + vec2<i32>(-1, 1), i)));
        brightest = max(brightest, resolveLuminance(compositedSpecular(pixel + vec2<i32>(-1, -1), i)));
    }
    var color = vec3<f32>(0.0);
    for (var i = 0u; i < samples; i++) {
        color += resolveSample(compositedSpecular(pixel, i), brightest, RESOLVE_MAX_NEIGHBOR_CONTRAST);
    }
    return exp(color / f32(samples));
}

// One sample: clamped against the brightest neighbour, then ConvertLighting (log).
fn resolveSample(sampleColor: vec3<f32>, brightest: f32, contrast: f32) -> vec3<f32> {
    let lumFactor = contrast * brightest / resolveLuminance(sampleColor);
    return log(sampleColor * min(1.0, lumFactor));
}

// ---------------------------------------------------------------------------
// Ambient occlusion (AoParent.pass -> SsaoParent.pass, after the reflection
// composite and before DiffuseSpecularMerge): DepthToLinearDepth on the
// resolved depth, DepthDownsample, SsaoCompute, the FastDepthAwareBlur
// passes (vertical, then horizontal), DepthUpsample, then ModulateWithSsao
// multiplies the diffuse target (in o3deMergeCS here). Each stage writes a
// buffer (mod.rs), one float per pixel at full or half resolution; the
// R8_UNORM ones are rounded to 8 bits. O3DE's compute shaders share samples
// through groupshared memory; each output depends only on the pixel's
// position in its group, so the lab computes every pixel on its own.
// gShadingFrame from FRAME_SSAO (demos.rs): (strength, sampling radius, blur
// on, unused), (constant falloff, depth falloff threshold, depth falloff
// strength, unused).
const FRAME_SSAO: u32 = 220u;
const SSAO_LOCAL_SAMPLE_COUNT: u32 = 12u;
const SSAO_INNER_SAMPLE_COUNT: u32 = 4u;
const SSAO_OUTER_SAMPLE_LOOPS: u32 = 10u;
const SSAO_THREADS_PER_SUBGROUP: u32 = 16u;
const SSAO_SUBGROUP_COUNT: u32 = 16u;
const SSAO_OUTER_SAMPLE_ANGLE: f32 = 2.0 * O3DE_PI / 10.0;
const SSAO_EPSILON: f32 = 10e-5;
const BLUR_RADIUS: i32 = 8;
const BLUR_OUTPUT_PIXEL_PER_THREAD: i32 = 3;

@group(2) @binding(27) var<storage, read_write> bLinearDepth: array<u32>;
@group(2) @binding(28) var<storage, read_write> bHalfDepth: array<u32>;
@group(2) @binding(29) var<storage, read_write> bSsao: array<u32>;
@group(2) @binding(30) var<storage, read_write> bSsaoBlurV: array<u32>;
@group(2) @binding(31) var<storage, read_write> bSsaoBlur: array<u32>;
@group(2) @binding(32) var<storage, read_write> bSsaoFull: array<u32>;

fn fullSize() -> vec2<u32> {
    return textureDimensions(tDepth);
}

fn halfSize() -> vec2<u32> {
    return (fullSize() + 1u) / 2u;
}

fn clampedIndex(p: vec2<i32>, size: vec2<u32>) -> u32 {
    let c = clamp(p, vec2<i32>(0), vec2<i32>(size) - 1);
    return u32(c.y) * size.x + u32(c.x);
}

// An R8_UNORM store.
fn unorm8Store(x: f32) -> f32 {
    return round(saturate(x) * 255.0) / 255.0;
}

fn linearDepthAt(p: vec2<i32>) -> f32 {
    return bitcast<f32>(bLinearDepth[clampedIndex(p, fullSize())]);
}

fn halfDepthAt(p: vec2<i32>) -> f32 {
    return bitcast<f32>(bHalfDepth[clampedIndex(p, halfSize())]);
}

// DepthToLinearDepth MainPS on MSAAResolveDepth's output (sample 0): the view distance.
@compute @workgroup_size(16, 16)
fn o3deLinearDepthCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= fullSize())) {
        return;
    }
    let depth = textureLoad(tDepth, id.xy, 0);
    bLinearDepth[id.y * fullSize().x + id.x] = bitcast<u32>(viewDistance(id.xy, depth));
}

// DepthDownsample MainCS: the upper-left pixel of each 2 x 2.
@compute @workgroup_size(16, 16)
fn o3deDepthDownsampleCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= halfSize())) {
        return;
    }
    bHalfDepth[id.y * halfSize().x + id.x] = bLinearDepth[clampedIndex(vec2<i32>(id.xy * 2u), fullSize())];
}

// ViewSrg::GetViewSpacePosition (Z forward, Y up) with m_unprojectionConstants
// (2 tanX, -2 tanY, -tanX, tanY).
fn ssaoViewSpacePosition(uv: vec2<f32>, linearDepth: f32) -> vec3<f32> {
    let projection = gFrame.camera.viewToClip;
    let tanX = 1.0 / projection[0][0];
    let tanY = 1.0 / projection[1][1];
    let screenRay = uv * vec2<f32>(2.0 * tanX, -2.0 * tanY) + vec2<f32>(-tanX, tanY);
    return vec3<f32>(screenRay * linearDepth, linearDepth);
}

// GetPositionFromDepthLDS: half-res texel t (clamped like the gather), at its centre's UV.
fn ssaoPositionAt(t: vec2<i32>) -> vec3<f32> {
    let pixelSize = 1.0 / vec2<f32>(halfSize());
    return ssaoViewSpacePosition((vec2<f32>(t) + 0.5) * pixelSize, halfDepthAt(t));
}

// SsaoCompute: AccumulateOcclusion, option 4.
fn accumulateOcclusion(position: vec3<f32>, normal: vec3<f32>, occluder: vec3<f32>, totalOcclusion: ptr<function, f32>, totalWeight: ptr<function, f32>) {
    let diff = occluder - position;
    let distanceSq = dot(diff, diff);
    let invDistance = inverseSqrt(distanceSq);
    let direction = diff * invDistance;
    let cosineFalloff = dot(direction, normal);
    let minDistanceSq = 0.07;
    var weight = inverseSqrt(distanceSq + minDistanceSq);
    weight *= weight;
    *totalOcclusion += saturate(cosineFalloff) * weight;
    *totalWeight += weight;
}

const SSAO_LOCAL_SAMPLES = array<vec2<i32>, 12>(
    vec2<i32>(0, -4), vec2<i32>(-4, 0), vec2<i32>(4, 0), vec2<i32>(0, 4),
    vec2<i32>(-4, -4), vec2<i32>(4, -4), vec2<i32>(-4, 4), vec2<i32>(4, 4),
    vec2<i32>(0, -8), vec2<i32>(-8, 0), vec2<i32>(8, 0), vec2<i32>(0, 8));

// Phase II's sample for thread `linearId` of a 16 x 16 group in loop `i`.
fn ssaoOuterSample(linearId: u32, i: u32, subgroupOffsetPixel: vec2<f32>) -> vec3<f32> {
    let size = vec2<f32>(halfSize());
    let pixelSize = 1.0 / size;
    let subgroupAngleOffset = f32((linearId >> 4u) & 0x0Fu) * (SSAO_OUTER_SAMPLE_ANGLE / f32(SSAO_SUBGROUP_COUNT)) + f32(i) * SSAO_OUTER_SAMPLE_ANGLE;
    let sampleFactor = (f32(linearId & 0x0Fu) + 1.0) / f32(SSAO_THREADS_PER_SUBGROUP);
    let sampleRadius = sqrt(sampleFactor) * (size.y * gShadingFrame[FRAME_SSAO].y);
    let sampleAngle = sampleFactor * (SSAO_OUTER_SAMPLE_ANGLE / f32(SSAO_THREADS_PER_SUBGROUP)) + subgroupAngleOffset;
    let samplePixel = floor(vec2<f32>(cos(sampleAngle), sin(sampleAngle)) * sampleRadius + subgroupOffsetPixel);
    let sampleUV = samplePixel * pixelSize + 0.5 * pixelSize;
    // SampleLevel with the point sampler, clamped.
    let occluderDepth = halfDepthAt(vec2<i32>(floor(sampleUV * size)));
    return ssaoViewSpacePosition(sampleUV, occluderDepth);
}

// SsaoCompute MainCS for one half-res pixel.
@compute @workgroup_size(16, 16)
fn o3deSsaoCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= halfSize())) {
        return;
    }
    let group = id.xy / 16u;
    let local = id.xy % 16u;
    // The thread whose reshuffled position (newThreadID) is this pixel.
    let subgroup = local / 4u;
    let threadSub = local % 4u;
    let linearId = (threadSub.x & 1u) | ((threadSub.y & 1u) << 1u) | ((threadSub.x >> 1u) << 2u) | ((threadSub.y >> 1u) << 3u) | (subgroup.x << 4u) | (subgroup.y << 6u);
    let pixel = vec2<i32>(id.xy);
    // Position and normal (CalculateNormalFromLDS: the smaller depth step each way).
    let position = ssaoPositionAt(pixel);
    let left = ssaoPositionAt(pixel - vec2<i32>(1, 0));
    let right = ssaoPositionAt(pixel + vec2<i32>(1, 0));
    let up = ssaoPositionAt(pixel - vec2<i32>(0, 1));
    let down = ssaoPositionAt(pixel + vec2<i32>(0, 1));
    let diffLeft = position - left;
    let diffRight = right - position;
    let diffX = select(diffRight, diffLeft, abs(diffLeft.z) < abs(diffRight.z));
    let diffUp = position - up;
    let diffDown = down - position;
    let diffY = select(diffDown, diffUp, abs(diffUp.z) < abs(diffDown.z));
    let normal = normalize(cross(diffX, diffY));
    var totalOcclusion = 0.0;
    var totalWeight = 0.0;
    // Phase I: 12 local samples.
    for (var k = 0u; k < SSAO_LOCAL_SAMPLE_COUNT; k++) {
        accumulateOcclusion(position, normal, ssaoPositionAt(pixel + SSAO_LOCAL_SAMPLES[k]), &totalOcclusion, &totalWeight);
    }
    // Phase II: ten rotations of this thread's sample and three of its 4 x 4 subgroup's.
    let groupOffsetPixel = vec2<f32>(group) * 16.0 + 2.0;
    let subgroupOffsetPixel = vec2<f32>(subgroup) * 4.0 + groupOffsetPixel;
    for (var i = 0u; i < SSAO_OUTER_SAMPLE_LOOPS; i++) {
        var other = linearId;
        accumulateOcclusion(position, normal, ssaoOuterSample(other, i, subgroupOffsetPixel), &totalOcclusion, &totalWeight);
        for (var j = 1u; j < SSAO_INNER_SAMPLE_COUNT; j++) {
            // GetNextLinearIndex.
            other = (other & 0xF0u) + ((other + SSAO_THREADS_PER_SUBGROUP / SSAO_INNER_SAMPLE_COUNT) & 0x0Fu);
            accumulateOcclusion(position, normal, ssaoOuterSample(other, i, subgroupOffsetPixel), &totalOcclusion, &totalWeight);
        }
    }
    totalWeight = max(totalWeight, SSAO_EPSILON);
    let finalAO = totalOcclusion / totalWeight;
    var output = 1.0 - finalAO * gShadingFrame[FRAME_SSAO].x;
    // Gamma compensation.
    output = pow(output, 2.2);
    bSsao[id.y * halfSize().x + id.x] = bitcast<u32>(unorm8Store(output));
}

fn blurValueAt(p: vec2<i32>, horizontal: bool) -> f32 {
    let i = clampedIndex(p, halfSize());
    return bitcast<f32>(select(bSsao[i], bSsaoBlurV[i], horizontal));
}

// FastDepthAwareBlur CalculateDepthFalloff.
fn blurDepthFalloff(previousDepthSlope: f32, currentDepthSlope: f32) -> f32 {
    let c = gShadingFrame[FRAME_SSAO + 1u];
    let difference = abs(previousDepthSlope - currentDepthSlope) - c.y;
    return saturate(1.0 - difference * c.z);
}

// BlurDirection's chain from `start` along `direction`, up to the pixel `steps` past the two first samples.
fn blurChain(start: vec2<i32>, direction: vec2<i32>, steps: i32, horizontal: bool) -> f32 {
    let constFalloff = gShadingFrame[FRAME_SSAO + 1u].x;
    var position = start;
    var previousDepth = halfDepthAt(position);
    var previousValue = blurValueAt(position, horizontal);
    position += direction;
    var currentDepth = halfDepthAt(position);
    var currentValue = blurValueAt(position, horizontal);
    var previousDepthSlope = currentDepth - previousDepth;
    var currentDepthSlope = previousDepthSlope;
    currentValue = mix(previousValue, currentValue, constFalloff);
    for (var i = 0; i < steps; i++) {
        // BlurStep.
        previousValue = currentValue;
        previousDepth = currentDepth;
        previousDepthSlope = currentDepthSlope;
        position += direction;
        currentDepth = halfDepthAt(position);
        currentValue = blurValueAt(position, horizontal);
        currentDepthSlope = currentDepth - previousDepth;
        let falloff = blurDepthFalloff(previousDepthSlope, currentDepthSlope) * constFalloff;
        currentValue = mix(currentValue, previousValue, falloff);
    }
    return currentValue;
}

// FastDepthAwareBlurVer / Hor MainCS for one pixel: each thread blurs three
// consecutive pixels, from BLUR_RADIUS before the first one forwards and
// from BLUR_RADIUS after the last one backwards, half of each.
fn depthAwareBlur(pixel: vec2<i32>, horizontal: bool) -> f32 {
    let axis = select(vec2<i32>(0, 1), vec2<i32>(1, 0), horizontal);
    let coordinate = select(pixel.y, pixel.x, horizontal);
    let k = coordinate % BLUR_OUTPUT_PIXEL_PER_THREAD;
    let base = pixel - axis * k;
    let loopCount = BLUR_RADIUS - 2;
    let forwards = blurChain(base - axis * BLUR_RADIUS, axis, loopCount + k + 1, horizontal);
    let backwards = blurChain(base - axis * BLUR_RADIUS + axis * (BLUR_RADIUS * 2 + BLUR_OUTPUT_PIXEL_PER_THREAD - 1), -axis, loopCount + (BLUR_OUTPUT_PIXEL_PER_THREAD - 1 - k) + 1, horizontal);
    return forwards * 0.5 + backwards * 0.5;
}

@compute @workgroup_size(16, 16)
fn o3deSsaoBlurVerticalCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= halfSize())) {
        return;
    }
    let i = id.y * halfSize().x + id.x;
    // A disabled blur pass forwards its input.
    var value = bitcast<f32>(bSsao[i]);
    if (gShadingFrame[FRAME_SSAO].z > 0.5) {
        value = unorm8Store(depthAwareBlur(vec2<i32>(id.xy), false));
    }
    bSsaoBlurV[i] = bitcast<u32>(value);
}

@compute @workgroup_size(16, 16)
fn o3deSsaoBlurHorizontalCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= halfSize())) {
        return;
    }
    let i = id.y * halfSize().x + id.x;
    var value = bitcast<f32>(bSsaoBlurV[i]);
    if (gShadingFrame[FRAME_SSAO].z > 0.5) {
        value = unorm8Store(depthAwareBlur(vec2<i32>(id.xy), true));
    }
    bSsaoBlur[i] = bitcast<u32>(value);
}

// DepthUpsample GetDepthFactor.
fn upsampleDepthFactor(depth1: f32, depth2: f32) -> f32 {
    let distance = abs(depth1 - depth2) + 0.00001;
    return 1.0 / (distance * distance);
}

// DepthUpsample MainCS for one full-res pixel: its four nearest half-res
// pixels, weighted 3/4 near and 1/4 far on each axis and by depth.
@compute @workgroup_size(16, 16)
fn o3deSsaoUpsampleCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= fullSize())) {
        return;
    }
    let f = vec2<i32>(id.xy);
    let near = f / 2;
    let far = select(near - 1, near + 1, (f & vec2<i32>(1)) == vec2<i32>(1));
    let fullDepth = linearDepthAt(f);
    let taps = array<vec2<i32>, 4>(near, vec2<i32>(far.x, near.y), vec2<i32>(near.x, far.y), far);
    let weights = array<f32, 4>(0.75 * 0.75, 0.25 * 0.75, 0.75 * 0.25, 0.25 * 0.25);
    var value = 0.0;
    var totalWeight = 0.0;
    for (var t = 0; t < 4; t++) {
        let weight = weights[t] * upsampleDepthFactor(fullDepth, halfDepthAt(taps[t]));
        value += bitcast<f32>(bSsaoBlur[clampedIndex(taps[t], halfSize())]) * weight;
        totalWeight += weight;
    }
    bSsaoFull[id.y * fullSize().x + id.x] = bitcast<u32>(unorm8Store(value / totalWeight));
}

// DiffuseSpecularMerge MainPS: diffuse plus specular (its 2 x 2 filter runs
// only for subsurface-scattering pixels, which StandardPBR has none of). The
// dock's views show one part instead, or the forward pass's false colours.
@compute @workgroup_size(8, 8)
fn o3deMergeCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = id.xy;
    if (any(pixel >= textureDimensions(oHdr))) {
        return;
    }
    let samples = textureNumSamples(tDiffuse);
    var coverage = 0.0;
    var diffuse = vec3<f32>(0.0);
    var specular = vec3<f32>(0.0);
    var debug = vec3<f32>(0.0);
    for (var i = 0u; i < samples; i++) {
        coverage += select(0.0, 1.0, covered(textureLoad(tDepth, pixel, i)));
        diffuse += textureLoad(tDiffuse, pixel, i).rgb;
        specular += textureLoad(tSpecular, pixel, i).rgb;
        debug += textureLoad(tDebug, pixel, i).rgb;
    }
    let n = f32(samples);
    // ModulateWithSsao: the diffuse target times the upsampled AO.
    let ao = bitcast<f32>(bSsaoFull[pixel.y * fullSize().x + pixel.x]);
    let diffuseLit = textureLoad(tDiffuseLit, pixel, 0).rgb * ao;
    let specularLit = textureLoad(tSpecularLit, pixel, 0).rgb;
    var color = diffuseLit + specularLit;
    let view = gShading.viewMode;
    // The custom resolve already holds the sky where nothing was drawn.
    let skyResolved = gShading.specularResolve > 0.5;
    if (view == VIEW_DIFFUSE) {
        color = diffuse / n;
    } else if (view == VIEW_SPECULAR) {
        color = specular / n;
    } else if ((view >= VIEW_D && view <= VIEW_ATTENUATION) || view == VIEW_LIGHT_COUNT) {
        color = debug / n;
    } else if (view == VIEW_IBL_DIFFUSE) {
        color = textureLoad(tDiffuseLit, pixel, 0).rgb - diffuse / n;
    } else if (view == VIEW_IBL_SPECULAR) {
        color = textureLoad(tReflection, pixel, 0).rgb;
    } else if (view == VIEW_PROBE_STENCIL) {
        color = untonemap(textureLoad(tProbeView, pixel, 0).rgb);
    } else if (view == VIEW_PROBE_WEIGHT) {
        color = untonemap(heat(textureLoad(tProbeView, pixel, 0).a / 2.0));
    } else if (view == VIEW_SSAO) {
        color = untonemap(vec3<f32>(ao));
    } else if (view == VIEW_RESOLVE_DIFFERENCE) {
        color = specularLit;
    }
    // Where samples hit nothing, the sky box: the environment the skylight's
    // cubemaps were baked from, unfiltered and without m_iblExposure (the lab
    // draws it here instead of in a pass of its own). The false-colour views
    // keep a dark background.
    var background = vec3<f32>(0.004, 0.009, 0.016);
    if (view == VIEW_FINAL) {
        background = skyRadiance(viewRay((vec2<f32>(pixel) + 0.5) / vec2<f32>(textureDimensions(tDepth))));
    }
    if (!(skyResolved && view == VIEW_FINAL)) {
        color += background * (1.0 - coverage / n);
    }
    color = probeCubemapViews(pixel, color);
    textureStore(oHdr, pixel, vec4<f32>(color, 1.0));
}

// ---------------------------------------------------------------------------
// The lab's views of a probe's baked cubemap (not O3DE passes). gShadingFrame
// from FRAME_CUBEMAP_VIEW (reflections.rs): (the unfolded cross's opacity,
// its cubemap, its mip, how many faces are baked), (the mirror ball's
// opacity, its radius, its cubemap, its mip), (the ball's centre, unused).
const FRAME_CUBEMAP_VIEW: u32 = 224u;
// The cross's cells as a fraction of the viewport, at most.
const CROSS_FILL: f32 = 0.94;

// The direction through a point of the horizontal cross (four faces across,
// three down, -Z at the centre seen from inside), with uv in -1..1 (y down)
// on the cell; w is the face (cubeSample's numbering) or -1 off the cross.
fn crossDirection(cell: vec2<i32>, uv: vec2<f32>) -> vec4<f32> {
    let u = uv.x;
    let v = uv.y;
    if (cell.y == 0 && cell.x == 1) {
        return vec4<f32>(u, 1.0, v, 2.0);
    }
    if (cell.y == 2 && cell.x == 1) {
        return vec4<f32>(u, -1.0, -v, 3.0);
    }
    if (cell.y == 1) {
        switch cell.x {
            case 0: { return vec4<f32>(-1.0, -v, u, 1.0); }
            case 1: { return vec4<f32>(u, -v, -1.0, 5.0); }
            case 2: { return vec4<f32>(1.0, -v, -u, 0.0); }
            case 3: { return vec4<f32>(-u, -v, 1.0, 4.0); }
            default: {}
        }
    }
    return vec4<f32>(0.0, 0.0, 0.0, -1.0);
}

fn probeCubemapViews(pixel: vec2<u32>, sceneColor: vec3<f32>) -> vec3<f32> {
    var color = sceneColor;
    let size = vec2<f32>(textureDimensions(tDepth));
    let ball = gShadingFrame[FRAME_CUBEMAP_VIEW + 1u];
    if (ball.x > 0.0) {
        let center = gShadingFrame[FRAME_CUBEMAP_VIEW + 2u].xyz;
        let eye = gFrame.camera.position;
        let dir = viewRay((vec2<f32>(pixel) + 0.5) / size);
        let toCenter = center - eye;
        let along = dot(toCenter, dir);
        let miss = length(toCenter - dir * along);
        // About one pixel of edge at the ball's distance.
        let pixelSize = along * 2.0 / (gFrame.camera.viewToClip[1][1] * size.y);
        let edge = saturate((ball.y - miss) / max(pixelSize, 1e-5) + 0.5);
        if (edge > 0.0 && along > 0.0) {
            let t = along - sqrt(max(ball.y * ball.y - miss * miss, 0.0));
            let depth = textureLoad(tDepth, pixel, 0);
            let sceneDistance = select(1e9, length(reconstructWorldPosition(pixel, depth) - eye), covered(depth));
            if (t < sceneDistance) {
                let normal = normalize(eye + dir * t - center);
                let mirrored = sampleProbeEnv(u32(ball.z), reflect(dir, normal), ball.w);
                color = mix(color, mirrored, edge * ball.x);
            }
        }
    }
    let view = gShadingFrame[FRAME_CUBEMAP_VIEW];
    if (view.x > 0.0) {
        let cellSize = min(size.x / 4.0, size.y / 3.0) * CROSS_FILL;
        let origin = (size - vec2<f32>(4.0, 3.0) * cellSize) * 0.5;
        let q = (vec2<f32>(pixel) + 0.5 - origin) / cellSize;
        var shown = vec3<f32>(0.004, 0.009, 0.016);
        if (all(q >= vec2<f32>(0.0)) && all(q < vec2<f32>(4.0, 3.0))) {
            let cell = vec2<i32>(floor(q));
            let d = crossDirection(cell, (q - floor(q)) * 2.0 - 1.0);
            if (d.w >= 0.0) {
                // A face not baked yet is the target's clear colour.
                shown = select(vec3<f32>(0.0), sampleProbeEnv(u32(view.y), normalize(d.xyz), view.z), d.w < view.w);
                // A thin seam between faces.
                let f = (q - floor(q)) * cellSize;
                if (any(f < vec2<f32>(1.0)) || any(f > vec2<f32>(cellSize - 1.0))) {
                    shown = mix(shown, vec3<f32>(0.03), 0.6);
                }
            }
        }
        color = mix(color, shown, view.x);
    }
    return color;
}

// ---------------------------------------------------------------------------
// Verification kernels (src/o3de/lighting/selftest.rs), one workgroup per test.

@group(2) @binding(0) var<storage, read_write> gO3deOut: array<u32>;
@group(2) @binding(1) var<storage, read> gO3deTests: array<vec4<f32>>;

var<private> o3deCursor: u32;

fn o3dePut(v: f32) {
    gO3deOut[o3deCursor] = bitcast<u32>(v);
    o3deCursor += 1u;
}
fn o3dePut2(v: vec2<f32>) { o3dePut(v.x); o3dePut(v.y); }
fn o3dePut3(v: vec3<f32>) { o3dePut(v.x); o3dePut(v.y); o3dePut(v.z); }

// Input, six vectors per test: (baseColor, metallic), (normal, roughness),
// (dirToCamera, specularF0Factor), (dirToLight, multiscatter on), and the
// normal's screen derivatives (dndx, unused), (dndy, unused).
// Output, 32 words: D, G, F, SpecularGGX, DiffuseLambertian,
// NormalizedDisneyDiffuse, diffuseResponse, multiScatterCompensation, brdf,
// roughnessA2, albedo, specularF0.
@compute @workgroup_size(1)
fn o3deBrdfTests(@builtin(workgroup_id) id: vec3<u32>) {
    let a = gO3deTests[id.x * 6u];
    let b = gO3deTests[id.x * 6u + 1u];
    let c = gO3deTests[id.x * 6u + 2u];
    let d = gO3deTests[id.x * 6u + 3u];
    let e = gO3deTests[id.x * 6u + 4u];
    let g = gO3deTests[id.x * 6u + 5u];
    o3deCursor = id.x * 32u;
    let s = o3deSurface(vec3<f32>(0.0), b.xyz, a.xyz, a.w, c.w, b.w, e.xyz, g.xyz);
    let l = o3deLightingData(s, c.xyz, d.w > 0.5);
    let t = ggxTerms(l.dirToCamera, d.xyz, s.normal, s.specularF0, l.NdotV, s.roughnessA2);
    o3dePut(t.D);
    o3dePut(t.G);
    o3dePut3(t.F);
    o3dePut3(specularGGX(l.dirToCamera, d.xyz, s.normal, s.specularF0, l.NdotV, s.roughnessA2, l.multiScatterCompensation));
    o3dePut3(diffuseLambertian(s.albedo, s.normal, d.xyz, l.diffuseResponse));
    o3dePut3(normalizedDisneyDiffuse(s.albedo, s.normal, l.dirToCamera, d.xyz, s.roughnessLinear));
    o3dePut3(l.diffuseResponse);
    o3dePut3(l.multiScatterCompensation);
    o3dePut2(l.brdf);
    o3dePut(s.roughnessA2);
    o3dePut3(s.albedo);
    o3dePut3(s.specularF0);
    o3dePut(0.0);
}

// Input, two vectors per test: (surface position, object index), (normal, unused).
// Output, 8 words: the pixel's colour, its diffuse and specular lighting
// summed, the light's falloff and radius attenuation.
@compute @workgroup_size(1)
fn o3dePixelTests(@builtin(workgroup_id) id: vec3<u32>) {
    let a = gO3deTests[id.x * 2u];
    let b = gO3deTests[id.x * 2u + 1u];
    o3deCursor = id.x * 8u;
    let px = o3dePixel(a.xyz, b.xyz, gFrame.camera.position, u32(a.w), vec3<f32>(0.0), vec3<f32>(0.0));
    o3dePut3(px.color);
    o3dePut(dot(px.diffuse, vec3<f32>(1.0)));
    o3dePut(dot(px.specular, vec3<f32>(1.0)));
    o3dePut(px.light.falloff);
    o3dePut(px.light.radiusAttenuation);
    o3dePut(0.0);
}

// The cubemap lookups: input (direction, specular mip), output 8 words: the
// specular cubemap at that mip, the diffuse cubemap, two unused.
@compute @workgroup_size(1)
fn o3deIblTests(@builtin(workgroup_id) id: vec3<u32>) {
    let input = gO3deTests[id.x];
    o3deCursor = id.x * 8u;
    o3dePut3(sampleSpecularEnv(input.xyz, input.w));
    o3dePut3(sampleDiffuseEnv(input.xyz));
    o3dePut(0.0);
    o3dePut(0.0);
}

// MSAAResolveCustom for one pixel: input (four samples, unused), (brightest
// neighbour luminance, contrast, sample count, unused); output 4 words: the
// resolved colour, unused.
@compute @workgroup_size(1)
fn o3deResolveTests(@builtin(workgroup_id) id: vec3<u32>) {
    let p = gO3deTests[id.x * 5u + 4u];
    o3deCursor = id.x * 4u;
    let count = u32(p.z);
    var color = vec3<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        color += resolveSample(gO3deTests[id.x * 5u + i].xyz, p.x, p.y);
    }
    o3dePut3(exp(color / f32(count)));
    o3dePut(0.0);
}

// BRDFTextureCS for one texel: input (x, y, texture size, guarded square
// root (1) or O3DE's (0)), output (A, B).
@compute @workgroup_size(1)
fn o3deBrdfMapTests(@builtin(workgroup_id) id: vec3<u32>) {
    let input = gO3deTests[id.x];
    o3deCursor = id.x * 2u;
    o3dePut2(brdfTexel(u32(input.x), u32(input.y), u32(input.z), input.w > 0.5));
}

// BRDFTextureCS::MainCS for dispatch (x, y): the split-sum scale A and bias B
// of the GGX BRDF at NdotV = (x + 0.5) / size, roughness = (y + 0.5) / size.
// MainCS stores it at row size - 1 - y.
fn brdfTexel(x: u32, y: u32, textureSize: u32, guarded: bool) -> vec2<f32> {
    let roughness = (f32(y) + 0.5) / f32(textureSize);
    let roughnessA = roughness * roughness;
    let roughnessA2 = roughnessA * roughnessA;
    let NdotV = (f32(x) + 0.5) / f32(textureSize);
    let V = vec3<f32>(sqrt(1.0 - NdotV * NdotV), 0.0, NdotV);
    let N = vec3<f32>(0.0, 0.0, 1.0);
    var A = 0.0;
    var B = 0.0;
    for (var i = 0u; i < BRDF_MAP_SAMPLES; i++) {
        let Xi = o3deHammersley(i, BRDF_MAP_SAMPLES);
        var H = importanceSampleGGX(Xi, roughnessA, N);
        if (guarded) {
            H = importanceSampleGGXGuarded(Xi, roughnessA, N);
        }
        let L = normalize(2.0 * dot(V, H) * H - V);
        let NdotL = max(L.z, 0.0);
        let NdotH = max(H.z, 0.0);
        let VdotH = max(dot(V, H), 0.0);
        if (NdotL > 0.0) {
            let G = geometricShadowingMaskingGGXCorrelated(NdotV, NdotL, roughnessA2);
            let G_Vis = (G * 4.0 * VdotH * NdotL) / NdotH;
            let Fc = pow(1.0 - VdotH, 5.0);
            A += (1.0 - Fc) * G_Vis;
            B += Fc * G_Vis;
        }
    }
    return vec2<f32>(A, B) / f32(BRDF_MAP_SAMPLES);
}
