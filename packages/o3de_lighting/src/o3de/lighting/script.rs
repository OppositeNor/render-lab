//! What the investigation's scripts (script/*.lam, lab/src/script/runtime.rs) read from its code: natives over the
//! frame `build_frame` computed (`Frame`, `FrameContent::script`), so the BRDF stays the CPU mirror's (brdf.rs)
//! and the overlay script only places what it draws.
//!
//! | Native | Returns |
//! |---|---|
//! | `inspected` | the inspected pixel: its point, normal, and the directions to the light and the viewer, H, R |
//! | `brdfTerms dirs` | the specular and diffuse terms at the inspected pixel for each outgoing direction |
//! | `ggxDirections count` | the first `count` of the 256 directions the specular prefilter samples along R |
//! | `pointLight` | SimplePointLightUtil at the inspected pixel: distance, falloff, attenuation, illuminance, channels |
//! | `falloffCurve distances` | the illuminance on the floor along +x from below the light, inverse-square and attenuated |

use super::brdf;
use glam::Vec3;
use lab::script::lamlet::embed::{Error, NativeRegistry};
use lab::script::runtime::{V3, frame_data};

/// What `build_frame` leaves for the natives in each frame.
pub struct Frame {
    /// The inspected pixel, as the CPU mirror shades it.
    pub pixel: brdf::Pixel,
    /// Its normal as the shot gives it (normalised).
    pub normal: Vec3,
    pub light_on: bool,
    /// The simple point light.
    pub light: brdf::PointLight,
    /// The inspected surface's lighting channels.
    pub surface_channels: u32,
    /// The inspected point's 16 x 16 tile: its top-left pixel and the viewport's size, for the tile view's clicks.
    pub tile_origin: glam::Vec2,
    pub viewport: glam::Vec2,
}

lab::lamlet_record! {
    /// A place on the viewport, 0..1 with y down.
    pub struct ViewportUv {
        pub u: f64 = "u",
        pub v: f64 = "v",
    }
}

lab::lamlet_record! {
    /// The inspected pixel, as the overlay draws it.
    pub struct Inspected {
        /// The inspected point (the shot's `inspect`).
        pub point: V3 = "point",
        /// The surface point the pixel shades.
        pub surface: V3 = "surface",
        pub normal: V3 = "normal",
        pub to_light: V3 = "toLight",
        pub to_view: V3 = "toView",
        pub half: V3 = "half",
        /// The IBL's reflection direction.
        pub reflect: V3 = "reflect",
        pub light_on: bool = "lightOn",
        pub light_at: V3 = "lightAt",
        /// The light's attenuation radius (SimplePointLight's).
        pub light_radius: f64 = "lightRadius",
    }
}

lab::lamlet_record! {
    /// SimplePointLightUtil at the inspected pixel (the readout's rows).
    pub struct PointLightAt {
        /// The distance to the light, d.
        pub distance: f64 = "distance",
        /// The attenuation radius r, and m_invAttenuationRadiusSquared.
        pub radius: f64 = "radius",
        pub inv_radius_squared: f64 = "invRadiusSquared",
        /// d² / r², and (1 − falloff²)².
        pub falloff: f64 = "falloff",
        pub radius_attenuation: f64 = "radiusAttenuation",
        /// I / max(d², 1e-6), and the light's illuminance (lux).
        pub inverse_square: V3 = "inverseSquare",
        pub intensity: V3 = "intensity",
        /// The light's and the surface's lighting channels.
        pub light_channels: i64 = "lightChannels",
        pub surface_channels: i64 = "surfaceChannels",
    }
}

lab::lamlet_record! {
    /// The illuminance on the floor at distances along +x from below the light, each over the inverse-square value
    /// one light height away (so the curves share one scale).
    pub struct Falloff {
        /// Inverse-square alone.
        pub plain: Vec<f64> = "plain",
        /// With SimplePointLight's radius attenuation (0 past the radius).
        pub attenuated: Vec<f64> = "attenuated",
    }
}

lab::lamlet_record! {
    /// The BRDF's terms in a list of outgoing directions.
    pub struct Terms {
        /// The light the specular term reflects into each direction (the term times N·V), averaged over RGB.
        pub specular: Vec<f64> = "specular",
        /// The diffuse term seen from each direction, averaged over RGB.
        pub diffuse: Vec<f64> = "diffuse",
    }
}

/// The diffuse and specular terms at the inspected pixel for an outgoing direction `w`, as radii of its lobes. The
/// diffuse lobe is the term itself, the radiance seen from that direction, which Lambert keeps constant until
/// the response flattens it. The specular lobe is the light the term reflects into that direction, the term times
/// N·V: its radiance grows toward the horizon, where G's division by N·V outpaces it, and would stretch the lobe
/// flat along the surface instead of around the mirror direction.
fn terms(frame: &Frame, w: Vec3) -> (f32, f32) {
    let px = &frame.pixel;
    let s = &px.surface;
    let (n, l) = (frame.normal, px.light.dir_to_light);
    let n_dot_v = w.dot(n).clamp(0.0, 1.0);
    let spec = brdf::specular_ggx(
        w,
        l,
        n,
        s.specular_f0,
        n_dot_v,
        s.roughness_a2,
        px.lighting.multi_scatter_compensation,
    );
    let response = Vec3::ONE
        - brdf::fresnel_schlick_with_roughness(n_dot_v, s.specular_f0, s.roughness_linear);
    let diff = brdf::diffuse_lambertian(s.albedo, n, l, response);
    (
        spec.dot(Vec3::splat(1.0 / 3.0)) * n_dot_v,
        diff.dot(Vec3::splat(1.0 / 3.0)),
    )
}

/// Registers the natives (`Investigation::script_natives`).
pub fn register(natives: &mut NativeRegistry) -> Result<(), Error> {
    natives
        .function("o3de_lighting/inspected", |_, ()| {
            let f = frame_data::<Frame>()?;
            let px = &f.pixel;
            let point = lab::script::runtime::state(|s| {
                Vec3::new(s.p("inspectX"), s.p("inspectY"), s.p("inspectZ"))
            })?;
            Ok(Inspected {
                point: V3(point),
                surface: V3(px.surface.position),
                normal: V3(f.normal),
                to_light: V3(px.light.dir_to_light),
                to_view: V3(px.lighting.dir_to_camera),
                half: V3(px.terms.half_vector),
                reflect: V3(px.ibl.reflect_dir),
                light_on: f.light_on,
                light_at: V3(f.light.position),
                light_radius: f.light.attenuation_radius() as f64,
            })
        })
        .doc("Fn([], Inspected). The inspected pixel as the CPU mirror shades it in this frame.")
        .register()?;
    register_picks(natives)?;
    natives
        .function("o3de_lighting/brdf-terms", |_, (dirs,): (Vec<V3>,)| {
            let f = frame_data::<Frame>()?;
            let (specular, diffuse) = dirs.iter().map(|w| terms(&f, w.0)).map(|(s, d)| (s as f64, d as f64)).unzip();
            Ok(Terms { specular, diffuse })
        })
        .doc("Fn([List(Vec3)], {diffuse: List(Float), specular: List(Float)}). The BRDF's terms at the inspected pixel for each outgoing direction, as its lobes' radii.")
        .register()?;
    natives
        .function("o3de_lighting/point-light", |_, ()| {
            let f = frame_data::<Frame>()?;
            let (light, sample) = (&f.light, &f.pixel.light);
            let d2 = sample.distance_squared;
            Ok(PointLightAt {
                distance: d2.sqrt() as f64,
                radius: light.attenuation_radius() as f64,
                inv_radius_squared: light.inv_attenuation_radius_squared as f64,
                falloff: sample.falloff as f64,
                radius_attenuation: sample.radius_attenuation as f64,
                inverse_square: V3(light.rgb_intensity_candelas / d2.max(1e-6)),
                intensity: V3(sample.intensity),
                light_channels: light.lighting_channel_mask as i64,
                surface_channels: f.surface_channels as i64,
            })
        })
        .doc("Fn([], PointLightAt). SimplePointLightUtil at the inspected pixel: its distance, the attenuation, the illuminance, and the lighting channels.")
        .register()?;
    natives
        .function("o3de_lighting/ggx-directions", |_, (count,): (i64,)| {
            let f = frame_data::<Frame>()?;
            let (r, a) = (f.pixel.ibl.reflect_dir, f.pixel.surface.roughness_a);
            let count = count.clamp(0, 256) as u32;
            Ok((0..count)
                .map(|i| {
                    let h = brdf::importance_sample_ggx(brdf::hammersley(i, 256), a, r);
                    V3(2.0 * r.dot(h) * h - r)
                })
                .collect::<Vec<_>>())
        })
        .doc("Fn([Int], List(Vec3)). The first `count` (up to 256) of the directions the specular prefilter samples for a texel along R (N = V = R), at the inspected surface's roughness.")
        .register()?;
    natives
        .function("o3de_lighting/falloff-curve", |_, (distances,): (Vec<f64>,)| {
            let f = frame_data::<Frame>()?;
            let light = &f.light;
            let foot = Vec3::new(light.position.x, 0.0, light.position.z);
            let candelas = light.rgb_intensity_candelas.y;
            let unit = candelas / (light.position.y * light.position.y);
            let (plain, attenuated) = distances
                .iter()
                .map(|x| {
                    let s = brdf::simple_point_light(light, foot + Vec3::X * *x as f32);
                    ((candelas / s.distance_squared / unit) as f64, (if s.falloff < 1.0 {
                        s.intensity.y
                    } else {
                        0.0
                    } / unit) as f64)
                })
                .unzip();
            Ok(Falloff { plain, attenuated })
        })
        .doc("Fn([List(Float)], {attenuated: List(Float), plain: List(Float)}). The illuminance on the floor at each distance along +x from below the light: inverse-square alone, and with the radius attenuation, over the inverse-square value one light height away.")
        .register()?;
    Ok(())
}

/// Registers the natives the click handlers use (script/picks.lam): the tile view's cell and the atlas's map.
fn register_picks(natives: &mut NativeRegistry) -> Result<(), Error> {
    natives
        .function("o3de_lighting/tile-cell", |_, (u, v): (f64, f64)| {
            let f = frame_data::<Frame>()?;
            // The cell of the zoomed tile view under (u, v), and the centre of its pixel on the viewport.
            let tile = super::culling::TILE_DIM as f32;
            let cell = (glam::Vec2::new(u as f32, v as f32) * tile).floor().clamp(glam::Vec2::ZERO, glam::Vec2::splat(tile - 1.0));
            let at = (f.tile_origin + cell + 0.5) / f.viewport;
            Ok(ViewportUv { u: at.x as f64, v: at.y as f64 })
        })
        .doc("Fn([Float, Float], {u: Float, v: Float}). The viewport place of the inspected tile's pixel under a click on the zoomed tile view.")
        .register()?;
    natives
        .function("o3de_lighting/atlas-map-at", |_, (x, y): (f64, f64)| {
            Ok(super::demos::illustrations::atlas_map_at(x as f32, y as f32) as f64)
        })
        .doc("Fn([Float, Float], Float). The illustrated atlas's map under the back wall's point (x, y), or -1.")
        .register()?;
    Ok(())
}
