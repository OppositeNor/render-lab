// Hardware ray tracing of the scene (lab/src/ray_tracing.rs): what a program
// pass that binds the scene's TLAS (`resource = "tlas"`, `access =
// "acceleration"`) traces it with. Only packages that require ray tracing
// (their manifest's `requires = ["ray-tracing"]`) compose this part: its ray
// queries need a device that has them.

enable wgpu_ray_query;

// The instances' masks (ray_tracing.rs `MASK_*`): what blocks light, what emits it, and the rest.
const RAY_MASK_OCCLUDER: u32 = 1u;
const RAY_MASK_EMITTER: u32 = 2u;
const RAY_MASK_OTHER: u32 = 4u;

// Whether nothing of `mask` lies on the ray from `origin` along the unit `dir` before `tMax`, object `skip` aside
// (an instance's custom data is its object's index). Any hit ends the search: a shadow ray needs no closest one.
fn sceneRayClear(tlas: acceleration_structure, origin: vec3<f32>, dir: vec3<f32>, tMax: f32, mask: u32, skip: u32) -> bool {
    var query: ray_query;
    rayQueryInitialize(&query, tlas, RayDesc(RAY_FLAG_NONE, mask, 0.0, tMax, origin, dir));
    while (rayQueryProceed(&query)) {
        // Each candidate triangle blocks the ray unless it belongs to the object skipped.
        let candidate = rayQueryGetCandidateIntersection(&query);
        if (candidate.instance_custom_data != skip) {
            return false;
        }
    }
    return true;
}
