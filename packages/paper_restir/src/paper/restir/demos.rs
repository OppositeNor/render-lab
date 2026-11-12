//! What each demonstration of the ReSTIR investigation shows: the room lit by
//! its many lights through the paper's passes (gpu.rs, shaders/), the lights
//! placed and packed from the shot (lights.rs).

use super::lights;

pub mod bias;
pub mod reuse;
pub mod ris;
pub mod spatial;
use super::shot::Demo;
use lab::frame::{FrameContent, Statics};
use lab::layout::FrameUniforms;
use lab::scene::ids;
use lab::shot::ShotState;

/// The scene's records with the shot's lights on them.
pub fn objects_for(state: &ShotState, statics: &Statics) -> Vec<lab::layout::ObjectData> {
    let mut objects = statics.scene.records();
    lights::apply(&mut objects, &lights::place(state));
    objects
}

pub fn build_frame(
    state: &ShotState,
    statics: &Statics,
    viewport: [f32; 2],
    pixel_scale: f32,
    time: f32,
) -> FrameContent {
    let objects = objects_for(state, statics);
    let lighting = statics.scene.lighting;
    let uniforms = FrameUniforms {
        camera: state.camera.camera_data(viewport),
        light_center_radiance: lighting.light.center.extend(0.0).to_array(),
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: [0.0; 4],
        ambient_color: [0.0; 4],
        floor_params: [1.0, lighting.f0, 0.0, 1.0],
        overlay_params: [lighting.exposure, pixel_scale, time, 0.0],
        // The ceiling panel is dark here and not drawn: the raster-hidden mask.
        counts: [
            objects.len() as u32,
            ids::FLOOR as u32,
            0,
            1 << lights::PANEL,
        ],
        view_params: [1.0, 0.0, 0.0, 0.0],
    };
    let mut content = FrameContent::new(objects, uniforms);
    let placed = lights::place(state);
    content.shading_frame = lights::pack(&placed);
    match Demo::of(state.demo) {
        Demo::Ris => ris::draw(state, &mut content, &placed),
        Demo::Reuse => reuse::draw(state, &mut content, &placed),
        Demo::Bias => bias::draw(state, &mut content),
        Demo::Spatial => spatial::draw(state, &mut content, &placed),
        Demo::Room | Demo::Reservoirs | Demo::Reference | Demo::Methods => {}
    }
    content
}
