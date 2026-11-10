//! Each demonstration's readout: the inspected point's rows, title, and formula.

use super::*;

pub(super) fn readout(
    demo: Demo,
    state: &ShotState,
    shading: &Shading,
    px: &brdf::Pixel,
    statics: &Statics,
    objects: &[ObjectData],
    camera: &lab::layout::CameraData,
) -> Readout {
    let seen = if state.p("observer") > 0.5 {
        tr("demos-seen_from_the_observer")
    } else {
        tr("demos-seen_from_the_camera")
    };
    let object = format!(
        "{} · {seen}",
        object_name(statics.scene.objects[shading.inspected as usize].name)
    );
    let mut r = Reading {
        state,
        shading,
        px,
        objects,
        camera,
        rows: Vec::new(),
    };
    let (title, subtitle) = match demo {
        Demo::Room if state.p("readoutMode") >= 1.5 => {
            r.room_encoding();
            ("EncodeNormalSignedOctahedron", object)
        }
        Demo::Room if state.p("readoutMode") >= 0.5 => {
            r.room_targets();
            (tr("demos-forward_targets_at_the_point"), object)
        }
        Demo::Room => {
            r.room();
            (tr("demos-forward_pass_at_the_point"), object)
        }
        Demo::Culling => {
            let (rows_, _) = culling_view(state, shading, objects, camera);
            for (name, value, color) in rows_ {
                r.row(&name, value, color);
            }
            (
                tr("common-tiled_light_culling"),
                tr("demos-the_inspected_point_s_tile").to_string(),
            )
        }
        Demo::Surface => {
            r.surface();
            ("SetAlbedoAndSpecularF0", object)
        }
        Demo::Decals => {
            r.decals();
            ("ApplyDecal", object)
        }
        Demo::Specular => {
            r.specular();
            ("SpecularGGX", object)
        }
        Demo::ClearCoat => {
            r.clear_coat();
            (tr("demos-clearcoatspecular"), object)
        }
        Demo::Transmission => {
            r.transmission();
            (tr("demos-getbacklighting"), object)
        }
        Demo::Diffuse => {
            r.diffuse();
            (tr("demos-diffuse_lighting"), object)
        }
        Demo::Multiscatter => {
            r.multiscatter();
            ("GetMultiScatterCompensation", object)
        }
        Demo::Ibl => {
            r.ibl();
            (tr("demos-global_ibl_passes"), object)
        }
        Demo::Reflections => {
            r.reflections();
            (tr("common-reflection_probes"), object)
        }
        Demo::Ssao => {
            r.ssao();
            ("SsaoCompute", object)
        }
        Demo::Terrain => {
            r.terrain();
            ("TerrainPBR_ForwardPass", object)
        }
        Demo::Cascades => {
            r.cascades();
            ("DirectionalLightFeatureProcessor", object)
        }
        // Its rows are its readout script's (script/readout.lam, from `script::Frame`).
        Demo::PointLight => ("SimplePointLightUtil", object),
    };
    let mut rows = r.rows;
    rows.truncate(state.p("readoutRows").round().max(0.0) as usize);
    let (formula, formula_lit) = super::super::formulas::readout(demo, state.p("formula"));
    Readout {
        title: title.to_string(),
        subtitle,
        formula,
        formula_lit,
        rows,
    }
}

/// What a demonstration's readout reads: the shot, its shading, the inspected pixel, the objects, and the
/// camera; and the rows written so far.
pub(super) struct Reading<'a> {
    state: &'a ShotState,
    shading: &'a Shading,
    px: &'a brdf::Pixel,
    objects: &'a [ObjectData],
    camera: &'a lab::layout::CameraData,
    rows: Vec<(String, String, Color32)>,
}

impl Reading<'_> {
    /// A row: its name, value, and colour.
    fn row(&mut self, name: &str, value: String, color: Color32) {
        self.rows.push((name.to_string(), value, color));
    }

    /// The rows of Room if state.p("readoutMode") >= 1.5's readout.
    fn room_encoding(&mut self) {
        let px = self.px;
        let s = &px.surface;
        // EncodeNormalSignedOctahedron, step by step.
        let n = s.normal;
        let l1 = n.x.abs() + n.y.abs() + n.z.abs();
        let o = n / l1;
        let e = brdf::encode_normal_signed_octahedron(n);
        self.row("normal", f3(n), palette::NORMAL);
        self.row("|x| + |y| + |z|", format!("{l1:.3}"), palette::text());
        self.row("n / that (octahedron)", f3(o), palette::NORMAL);
        self.row(
            "x' = x/2 + y/2 + 1/2",
            format!("{:.3}", e.x),
            palette::accent(),
        );
        self.row(
            "y' = -x/2 + y/2 + 1/2",
            format!("{:.3}", e.y),
            palette::accent(),
        );
        self.row("z' = sign of z", format!("{:.0}", e.z), palette::accent());
        self.row(
            "RGB10A2 stores",
            format!(
                "{} · {} · {}",
                (e.x * 1023.0).round(),
                (e.y * 1023.0).round(),
                (e.z * 1023.0).round()
            ),
            palette::text(),
        );
    }

    /// The rows of Room if state.p("readoutMode") >= 0.5's readout.
    fn room_targets(&mut self) {
        let shading = self.shading;
        let px = self.px;
        let s = &px.surface;
        let l = &px.lighting;
        // The five targets' texels at the point (GetPbrLightingOutput,
        // ForwardPassVertexAndPixel.azsli): what the forward pass leaves.
        let albedo = s.albedo * l.diffuse_response * s.diffuse_ao;
        let flags = brdf::encode_unorm_2bit_flags(shading.ibl, shading.multiscatter);
        self.row(
            "DiffuseOutput",
            format!("{} · -1", f3(px.diffuse())),
            DIFFUSE,
        );
        self.row(
            "SpecularOutput",
            format!("{} · 1", f3(px.specular())),
            SPECULAR,
        );
        self.row(
            "AlbedoOutput",
            format!(
                "{} · {:.3}",
                f3(albedo.map(brdf::unorm8)),
                brdf::unorm8(s.specular_occlusion)
            ),
            palette::text(),
        );
        self.row(
            "SpecularF0Output",
            format!(
                "{} · {:.3}",
                f3(s.specular_f0.map(brdf::unorm8)),
                brdf::unorm8(s.roughness_linear)
            ),
            palette::accent(),
        );
        self.row(
            "NormalOutput",
            format!(
                "{} · {flags:.3}",
                f3(brdf::encode_normal_signed_octahedron(s.normal))
            ),
            palette::NORMAL,
        );
    }

    /// The rows of Room's readout.
    fn room(&mut self) {
        let shading = self.shading;
        let px = self.px;
        self.row(
            tr("demos-light_position"),
            f3(shading.light.position),
            LIGHT,
        );
        self.row(
            tr("demos-intensity_cd"),
            f3(shading.light.rgb_intensity_candelas),
            LIGHT,
        );
        self.row(
            tr("common-attenuation_radius"),
            format!("{:.2} m", shading.light.attenuation_radius()),
            LIGHT,
        );
        self.row("", String::new(), palette::text());
        self.row("diffuseLighting", f3(px.diffuse()), DIFFUSE);
        self.row("specularLighting", f3(px.specular()), SPECULAR);
        let ibl_on = if shading.ibl {
            1.0
        } else {
            0.0
        };
        self.row(
            tr("demos-ibl_diffuse"),
            f3(px.ibl.diffuse * ibl_on),
            DIFFUSE,
        );
        self.row(
            tr("demos-ibl_specular"),
            f3(px.ibl.specular * ibl_on),
            SPECULAR,
        );
        self.row(
            tr("demos-pixel"),
            f3(px.diffuse() + px.specular() + (px.ibl.diffuse + px.ibl.specular) * ibl_on),
            palette::text(),
        );
    }

    /// The rows of Surface's readout.
    fn surface(&mut self) {
        let state = self.state;
        let shading = self.shading;
        let px = self.px;
        let s = &px.surface;
        let material = shading.material(shading.inspected);
        self.row("baseColor", f3(s.base_color), palette::text());
        self.row("metallic", f1(s.metallic), palette::text());
        self.row(
            "specularF0Factor",
            f1(material.specular_f0_factor),
            palette::text(),
        );
        self.row(
            "roughness (authored)",
            f1(content_roughness(state, shading.inspected)),
            palette::text(),
        );
        self.row("", String::new(), palette::text());
        self.row("albedo", f3(s.albedo), DIFFUSE);
        self.row("specularF0", f3(s.specular_f0), SPECULAR);
        self.row("roughnessLinear", f1(s.roughness_linear), palette::accent());
        self.row("roughnessA", f1(s.roughness_a), palette::accent());
        self.row("roughnessA2", f1(s.roughness_a2), palette::accent());
        self.row("diffuseAmbientOcclusion", f1(s.diffuse_ao), DIFFUSE);
        self.row("specularOcclusion", f1(s.specular_occlusion), SPECULAR);
    }

    /// The rows of Decals's readout.
    fn decals(&mut self) {
        let state = self.state;
        let shading = self.shading;
        let px = self.px;
        let objects = self.objects;
        let s = &px.surface;
        // The surface before its decals, then each decal in SortDecals' order.
        let (p, n) = inspected_point(state);
        let plain = Shading {
            decals: Vec::new(),
            ..shading.clone()
        };
        let before = brdf::pixel(
            &plain,
            objects,
            &brdf::Fragment::at(shading.inspected, p, n),
            p + n,
            1.0,
            BrdfMap::get(),
        )
        .surface;
        self.row(tr("demos-albedo_surface"), f3(before.albedo), DIFFUSE);
        if shading.decals.is_empty() {
            self.row("decals", tr("demos-off").into(), palette::text_dim());
        }
        let sorted = super::super::decals::sorted(&shading.decals);
        let top = sorted
            .iter()
            .rev()
            .copied()
            .find(|&i| super::super::decals::sample(&shading.decals[i], p, n).is_some());
        for i in sorted {
            let d = &shading.decals[i];
            let name = format!("{} (key {})", decal_name(d.texture), d.sort_key);
            let color = [palette::accent(), palette::REFLECTED][d.texture.min(1)];
            match super::super::decals::sample(d, p, n) {
                Some(k) => {
                    self.row(
                        &name,
                        format!("uvw ({:.2}, {:.2}, {:.2})", k.uvw.x, k.uvw.y, k.uvw.z),
                        color,
                    );
                    self.row(
                        tr("demos-opacity_angle"),
                        format!(
                            "{:.2}·{:.2}·{:.2} = {:.2}",
                            k.base[3], d.opacity, k.attenuation, k.albedo_opacity
                        ),
                        color,
                    );
                    // The decal on top: what BlendBaseColor blends (its texture times its colour),
                    // and that colour converted to ACEScg, as a material's would be.
                    if top == Some(i) {
                        self.row("b", f3(k.blended), color);
                        self.row(
                            tr("demos-b_in_acescg"),
                            f3(super::super::decals::linear_srgb_to_acescg(k.blended)),
                            palette::text_dim(),
                        );
                    }
                }
                None => self.row(
                    &name,
                    tr("demos-outside_its_box").into(),
                    palette::text_dim(),
                ),
            }
        }
        self.row(tr("demos-albedo_after"), f3(s.albedo), DIFFUSE);
        self.row("specularF0", f3(s.specular_f0), SPECULAR);
        self.row("normal", f3(s.normal), palette::NORMAL);
    }

    /// The rows of Specular's readout.
    fn specular(&mut self) {
        let px = self.px;
        let s = &px.surface;
        let l = &px.lighting;
        let g = &px.terms;
        self.row("roughnessA2", f1(s.roughness_a2), palette::accent());
        self.row("NdotL", f1(g.n_dot_l), LIGHT);
        self.row("NdotV", f1(l.n_dot_v), palette::VIEW);
        self.row("NdotH", f1(g.n_dot_h), HALF);
        self.row("HdotV", f1(g.h_dot_v), HALF);
        self.row("D  NormalDistributionGGX", f1(g.d), SPECULAR);
        self.row("G  (visibility, ÷ 4 NdotL NdotV)", f1(g.g), TERM_G);
        // GGX's F term, in the text's colour.
        self.row("F  FresnelSchlick", f3(g.f), palette::text());
        self.row(
            tr("demos-compensation"),
            f3(l.multi_scatter_compensation),
            palette::text_dim(),
        );
        self.row(
            "D·G·F·NdotL·comp",
            f3(brdf::specular_ggx(
                l.dir_to_camera,
                px.light.dir_to_light,
                s.normal,
                s.specular_f0,
                l.n_dot_v,
                s.roughness_a2,
                l.multi_scatter_compensation,
            )),
            SPECULAR,
        );
        self.row(tr("demos-light_lux"), f3(px.specular()), SPECULAR);
    }

    /// The rows of ClearCoat's readout.
    fn clear_coat(&mut self) {
        let px = self.px;
        let s = &px.surface;
        let l = &px.lighting;
        let to_light = px.light.dir_to_light;
        let base = brdf::specular_ggx(
            l.dir_to_camera,
            to_light,
            s.normal,
            s.specular_f0,
            l.n_dot_v,
            s.roughness_a2,
            l.multi_scatter_compensation,
        );
        match s.clear_coat {
            Some(c) => {
                let h = (to_light + l.dir_to_camera).normalize();
                let fc = brdf::fresnel_schlick(brdf::saturate(h.dot(to_light)), Vec3::splat(0.04))
                    * c.factor;
                let coated = brdf::clear_coat_specular(to_light, l.dir_to_camera, &c, base);
                self.row("clearCoat.factor", f1(c.factor), palette::accent());
                self.row("clearCoat.roughness", f1(c.roughness), palette::accent());
                self.row(tr("demos-specularf0_base"), f3(s.specular_f0), SPECULAR);
                self.row("diffuseResponse", f3(l.diffuse_response), DIFFUSE);
                self.row("Fc = F(HdotL, 0.04) · factor", f1(fc.x), HALF);
                self.row(tr("demos-base_1_fc"), f3(base * (Vec3::ONE - fc)), SPECULAR);
                self.row(
                    tr("demos-coat_specular"),
                    f3(coated - base * (Vec3::ONE - fc)),
                    HALF,
                );
                self.row(tr("demos-light_lux"), f3(px.specular()), SPECULAR);
            }
            None => self.row(
                "clearCoat.enable",
                tr("demos-off").into(),
                palette::text_dim(),
            ),
        }
    }

    /// The rows of Transmission's readout.
    fn transmission(&mut self) {
        let state = self.state;
        let shading = self.shading;
        let px = self.px;
        let s = &px.surface;
        let l = &px.lighting;
        let (_, n) = inspected_point(state);
        let mode = match s.transmission.map(|t| t.mode) {
            Some(super::super::transmission::TransmissionMode::ThickObject) => "ThickObject",
            Some(super::super::transmission::TransmissionMode::ThinObject) => "ThinObject",
            None => "None",
        };
        self.row("o_transmission_mode", mode.into(), palette::accent());
        match (s.transmission, shading.sphere_lights.first()) {
            (Some(tm), Some(light)) => {
                let to_light = (light.position - s.position).normalize();
                self.row(
                    tr("demos-thickness_no_shadow_data"),
                    f1(tm.thickness),
                    palette::accent(),
                );
                if tm.mode == super::super::transmission::TransmissionMode::ThickObject {
                    let k = super::super::transmission::thick_terms(
                        &tm,
                        n,
                        l.dir_to_camera,
                        to_light,
                        0.0,
                    );
                    self.row("V·−(L + N·distortion)", f1(k.v_dot), palette::VIEW);
                    self.row(
                        tr("demos-power_scale"),
                        f1(k.transmittance),
                        palette::accent(),
                    );
                    self.row(
                        "exp(−t·k)·saturate(1 − t)",
                        f1(k.lamber_attenuation),
                        palette::accent(),
                    );
                } else {
                    let e = s.albedo * brdf::saturate(tm.params.y + (-n).dot(to_light));
                    self.row(tr("demos-e_albedo_sat_bias_n"), f3(e), DIFFUSE);
                    self.row(
                        tr("demos-kernel_at_s_0"),
                        f3(super::super::transmission::transmission_kernel(
                            0.0,
                            Vec3::ONE,
                        )),
                        palette::accent(),
                    );
                }
                self.row(
                    tr("demos-back_lighting_lux"),
                    f3(l.translucent_back_lighting),
                    SPHERE_LIGHT,
                );
                self.row(
                    tr("demos-tint"),
                    f3(l.translucent_back_lighting * tm.tint),
                    SPHERE_LIGHT,
                );
                self.row("diffuseLighting", f3(px.diffuse()), DIFFUSE);
            }
            (Some(_), None) => self.row(
                tr("demos-point_light"),
                tr("demos-off").into(),
                palette::text_dim(),
            ),
            (None, _) => {}
        }
    }

    /// The rows of Diffuse's readout.
    fn diffuse(&mut self) {
        let px = self.px;
        let s = &px.surface;
        let l = &px.lighting;
        let g = &px.terms;
        let lambert = brdf::diffuse_lambertian(
            s.albedo,
            s.normal,
            px.light.dir_to_light,
            l.diffuse_response,
        );
        let disney = brdf::normalized_disney_diffuse(
            s.albedo,
            s.normal,
            l.dir_to_camera,
            px.light.dir_to_light,
            s.roughness_linear,
        );
        self.row("NdotV", f1(l.n_dot_v), palette::VIEW);
        self.row("NdotL", f1(g.n_dot_l), LIGHT);
        self.row(
            tr("demos-f_with_roughness"),
            f3(l.specular_response),
            SPECULAR,
        );
        self.row("1 − F (response)", f3(l.diffuse_response), DIFFUSE);
        self.row("albedo", f3(s.albedo), palette::text());
        self.row("DiffuseLambertian", f3(lambert), DIFFUSE);
        self.row(tr("demos-disney_skin"), f3(disney), palette::text_dim());
        self.row(tr("demos-light_lux"), f3(px.diffuse()), DIFFUSE);
    }

    /// The rows of Multiscatter's readout.
    fn multiscatter(&mut self) {
        let px = self.px;
        let s = &px.surface;
        let l = &px.lighting;
        let e_ss = l.brdf.x + l.brdf.y;
        let single = brdf::specular_ggx(
            l.dir_to_camera,
            px.light.dir_to_light,
            s.normal,
            s.specular_f0,
            l.n_dot_v,
            s.roughness_a2,
            Vec3::ONE,
        );
        self.row("roughnessLinear", f1(s.roughness_linear), palette::accent());
        self.row(
            tr("demos-uv_ndotv_smoothness"),
            format!("({:.3}, {:.3})", l.n_dot_v, 1.0 - s.roughness_linear),
            palette::text_dim(),
        );
        self.row("brdf.x  (A, scale)", f1(l.brdf.x), palette::accent());
        self.row("brdf.y  (B, bias)", f1(l.brdf.y), palette::accent());
        self.row(
            tr("demos-a_b_single_scattering_energy"),
            f1(e_ss),
            palette::accent(),
        );
        self.row("specularF0", f3(s.specular_f0), SPECULAR);
        self.row(
            tr("demos-compensation"),
            f3(l.multi_scatter_compensation),
            SPECULAR,
        );
        self.row(
            tr("demos-single_scattering"),
            f3(single),
            palette::text_dim(),
        );
        self.row(
            tr("demos-compensated"),
            f3(single * l.multi_scatter_compensation),
            SPECULAR,
        );
    }

    /// The rows of Ibl's readout.
    fn ibl(&mut self) {
        let px = self.px;
        let i = &px.ibl;
        self.row(tr("demos-albedo_target"), f3(i.albedo), DIFFUSE);
        self.row(tr("demos-irradiance_at_n"), f3(i.irradiance), DIFFUSE);
        self.row(tr("common-diffuse_ibl"), f3(i.diffuse), DIFFUSE);
        self.row("", String::new(), palette::text());
        self.row(
            tr("demos-roughness_8_bit"),
            f1(i.roughness),
            palette::accent(),
        );
        self.row(tr("demos-mip_roughness_5"), f1(i.mip), palette::accent());
        self.row(
            tr("demos-prefiltered_at_r"),
            f3(i.prefiltered),
            palette::REFLECTED,
        );
        self.row("NdotV (≥ 0.01)", f1(i.n_dot_v), palette::VIEW);
        self.row(
            "F0 · A + B",
            f3(i.specular_f0 * i.brdf.x + Vec3::splat(i.brdf.y)),
            TERM_G,
        );
        self.row(
            tr("demos-compensation"),
            f3(i.compensation),
            palette::text_dim(),
        );
        self.row(tr("common-specular_ibl"), f3(i.specular), SPECULAR);
    }

    /// The rows of Reflections's readout.
    fn reflections(&mut self) {
        let shading = self.shading;
        let px = self.px;
        let s = &px.surface;
        let r = &px.ibl.reflection;
        let masked = r.stencil & super::super::reflections::STENCIL_REFLECTION_MASK;
        let region = match masked {
            0..=2 => tr("demos-no_passes"),
            3 => tr("demos-outside_the_inners"),
            _ => tr("demos-inside_an_inner"),
        };
        self.row("stencil", format!("{masked} · {region}"), palette::accent());
        for probe in &shading.probes {
            let name = format!("{} {}", tr("demos-probe"), probe.cubemap + 1);
            let color = [palette::accent(), palette::REFLECTED][probe.cubemap.min(1)];
            let value = if probe.contains(probe.inner_half, s.position) {
                tr("demos-inner_weight_1").to_string()
            } else if probe.contains(probe.outer_half, s.position) {
                format!(
                    "{} {:.3}",
                    tr("demos-outer_weight"),
                    probe.blend_weight(s.position)
                )
            } else {
                tr("demos-outside").to_string()
            };
            self.row(&name, value, color);
        }
        self.row(
            tr("demos-blend_weight_sum"),
            f1(r.blend_weight),
            palette::accent(),
        );
        self.row(
            tr("demos-global_1_min_sum_1"),
            f1(r.global_weight),
            palette::text_dim(),
        );
        self.row(tr("demos-reflection_target"), f3(r.reflection), SPECULAR);
        self.row(tr("demos-composited"), f3(r.composited), SPECULAR);
    }

    /// The rows of Ssao's readout.
    fn ssao(&mut self) {
        let state = self.state;
        let shading = self.shading;
        let px = self.px;
        let objects = self.objects;
        let camera = self.camera;
        let (p, _) = inspected_point(state);
        let projector = lab::camera::Projector::new(camera);
        let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
        let size = [
            camera.viewport_size[0] as u32,
            camera.viewport_size[1] as u32,
        ];
        // DepthToLinearDepth at a pixel: the view distance of the surface its centre sees, or of the far plane.
        let linear_depth = |x: i32, y: i32| -> f32 {
            let (o, d) = projector.ray(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            let hit = lab::scene::trace_scene(objects, o, d, 100.0, ids::PANEL as u32);
            if hit.is_hit() {
                -view.transform_point3(o + d * hit.t).z
            } else {
                camera.far_plane
            }
        };
        let projection = glam::Mat4::from_cols_array_2d(&camera.view_to_clip);
        let tan = Vec2::new(1.0 / projection.x_axis.x, 1.0 / projection.y_axis.y);
        let settings = super::super::ssao::Settings::from_records(&ssao_records(state));
        match projector.project(p) {
            Some((pixel, _)) => {
                let (x, y) = (pixel.x.floor() as i32, pixel.y.floor() as i32);
                let k = super::super::ssao::at_pixel(&linear_depth, size, &settings, tan, x, y);
                self.row(
                    tr("demos-pixel_half_res"),
                    format!("({x}, {y}) · ({}, {})", k.half_pixel[0], k.half_pixel[1]),
                    palette::text(),
                );
                self.row(
                    tr("demos-linear_depth"),
                    format!("{:.3} m", k.linear_depth),
                    palette::accent(),
                );
                self.row(
                    tr("demos-strength_radius"),
                    format!("{:.2} · {:.3}", settings.strength, settings.sampling_radius),
                    palette::accent(),
                );
                self.row(
                    tr("demos-ssaocompute_52_samples"),
                    f1(k.raw),
                    palette::text(),
                );
                self.row(
                    tr("demos-blur_vertical"),
                    f1(k.blur_vertical),
                    palette::text(),
                );
                self.row(tr("demos-blur_horizontal"), f1(k.blurred), palette::text());
                self.row(tr("demos-upsampled_ao"), f1(k.upsampled), palette::text());
                let diffuse = px.diffuse()
                    + px.ibl.diffuse
                        * if shading.ibl {
                            1.0
                        } else {
                            0.0
                        };
                self.row(tr("demos-diffuse"), f3(diffuse), DIFFUSE);
                self.row(tr("demos-ao"), f3(diffuse * k.upsampled), DIFFUSE);
            }
            None => self.row(
                tr("demos-point"),
                tr("demos-off_screen").into(),
                palette::text_dim(),
            ),
        }
    }

    /// The rows of Terrain's readout.
    fn terrain(&mut self) {
        let shading = self.shading;
        // TerrainPBR_ForwardPass.azsl's macro surface.
        self.row("roughnessLinear (macro)", f1(1.0), palette::accent());
        self.row(
            "specularF0 factor (macro)",
            format!("0.5 · F0 {:.2}", 0.08 * 0.5),
            SPECULAR,
        );
        self.row("metallic (macro)", f1(0.0), palette::text());
        self.row(
            tr("demos-terrain_direct_multiscatter"),
            tr("demos-off").to_string(),
            palette::text(),
        );
        self.row(
            tr("demos-terrain_ibl_multiscatter"),
            if shading.multiscatter {
                tr("demos-on")
            } else {
                tr("demos-off")
            }
            .to_string(),
            palette::text(),
        );
    }

    /// The rows of Cascades's readout.
    fn cascades(&mut self) {
        let state = self.state;
        let camera = self.camera;
        use super::super::cascades;
        let (p, n) = inspected_point(state);
        // The cascades the frame renders (the overlay draws the observer's, whose frustum can be seen).
        let observer = shadow_camera(state, camera);
        let set = cascades::cascades(observer, sun_direction(state), state.p("cascadeSplitRatio"));
        self.row(
            tr("demos-near_shadow_far"),
            format!("{:.2} … {:.1} m", observer.near, observer.far),
            palette::text(),
        );
        self.row(
            tr("demos-split_ratio"),
            format!("{:.2}", state.p("cascadeSplitRatio")),
            palette::accent(),
        );
        for (i, c) in set.cascades.iter().enumerate() {
            self.row(
                &format!("{} {i}", tr("demos-cascade")),
                format!("{:.2}–{:.2} m · R {:.2}", c.near, c.far, c.radius),
                CASCADE_COLORS[i],
            );
        }
        let texels: Vec<String> = set
            .cascades
            .iter()
            .map(|c| format!("{:.1}", c.texel() * 1000.0))
            .collect();
        self.row(tr("demos-texel_mm"), texels.join(" · "), palette::accent());
        match set.select(p, n) {
            Some(i) => {
                let t = set.texel_of(i, p, n);
                self.row(
                    tr("demos-point_s_cascade"),
                    format!("{i} · texel ({:.0}, {:.0})", t.x, t.y),
                    CASCADE_COLORS[i],
                );
            }
            None => self.row(
                tr("demos-point_s_cascade"),
                tr("demos-outside_every_cascade").into(),
                palette::text_dim(),
            ),
        }
    }
}
