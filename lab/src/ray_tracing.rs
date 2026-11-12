//! Hardware ray tracing of the scene, where the device has it (src/gpu.rs
//! `ray_tracing`): one bottom-level acceleration structure (BLAS) per object,
//! built once per scene from the object's raster mesh (its local space, the
//! scene's mesh buffers, src/scene.rs `Meshes`), and a top-level one (TLAS) per
//! program, rebuilt every frame from the frame's object records, so what moves
//! is traced where it is. A program pass binds it as the resource `tlas` with
//! `access = "acceleration"` (src/shading_file.rs) and traces it with ray
//! queries (`enable wgpu_ray_query;`; packages/render_lab/shaders/rays.wgsl).
//!
//! Each instance's custom index is its object's index, and its mask one of the
//! `MASK_*` kinds, so a ray can pass through what it should not stop at.

use crate::layout::ObjectData;
use crate::scene::Meshes;

/// An object that blocks light: a box, a sphere, a mesh, or the terrain (common.wgsl `occludesLight`), not emitting.
pub const MASK_OCCLUDER: u8 = 1;
/// An object that emits light (`emissive.w > 0`).
pub const MASK_EMITTER: u8 = 2;
/// Anything else: the planes and panels, which the lab's shadows never block on.
pub const MASK_OTHER: u8 = 4;

/// The most instances a TLAS holds: the scene's objects.
pub const MAX_INSTANCES: u32 = 4096;

/// An object's kind of instance (`MASK_*`).
pub fn mask(o: &ObjectData) -> u8 {
    if o.emissive[3] > 0.0 {
        MASK_EMITTER
    } else if crate::scene::occludes_light(o) {
        MASK_OCCLUDER
    } else {
        MASK_OTHER
    }
}

/// A scene's BLASes: one per object whose mesh has triangles, built on the first frame that traces it.
pub struct SceneRays {
    blases: Vec<Option<(wgpu::Blas, wgpu::BlasTriangleGeometrySizeDescriptor, u32)>>,
    built: bool,
}

impl SceneRays {
    pub fn new(device: &wgpu::Device, meshes: &Meshes) -> SceneRays {
        let vertex_count = meshes.vertices.len() as u32;
        let blases = meshes
            .ranges
            .iter()
            .enumerate()
            .map(|(i, range)| {
                // An object's triangles are its index range of the scene's shared buffers.
                (range.end > range.start).then(|| {
                    let size = wgpu::BlasTriangleGeometrySizeDescriptor {
                        vertex_format: wgpu::VertexFormat::Float32x3,
                        vertex_count,
                        index_format: Some(wgpu::IndexFormat::Uint32),
                        index_count: Some(range.end - range.start),
                        // Not opaque: a ray query sees each triangle as a candidate, so it can pass an object by.
                        flags: wgpu::AccelerationStructureGeometryFlags::empty(),
                    };
                    let blas = device.create_blas(
                        &wgpu::CreateBlasDescriptor {
                            label: Some(&format!("object {i} BLAS")),
                            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                        },
                        wgpu::BlasGeometrySizeDescriptors::Triangles {
                            descriptors: vec![size.clone()],
                        },
                    );
                    (blas, size, range.start)
                })
            })
            .collect();
        SceneRays {
            blases,
            built: false,
        }
    }

    /// Builds the BLASes, once: each from its object's index range of the scene's buffers.
    fn build(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        vertices: &wgpu::Buffer,
        indices: &wgpu::Buffer,
    ) {
        if self.built {
            return;
        }
        let entries: Vec<wgpu::BlasBuildEntry> = self
            .blases
            .iter()
            .flatten()
            .map(|(blas, size, first)| wgpu::BlasBuildEntry {
                blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![
                    wgpu::BlasTriangleGeometry {
                        size,
                        vertex_buffer: vertices,
                        first_vertex: 0,
                        vertex_stride: std::mem::size_of::<crate::scene::Vertex>() as u64,
                        index_buffer: Some(indices),
                        first_index: Some(*first),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    },
                ]),
            })
            .collect();
        encoder.build_acceleration_structures(entries.iter(), std::iter::empty());
        self.built = true;
    }

    /// Rebuilds `tlas` from the frame's records (building the BLASes first, the first time): one instance per
    /// object with triangles, at its local-to-world transform, its index as custom data, its kind as mask.
    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        (vertices, indices): (&wgpu::Buffer, &wgpu::Buffer),
        objects: &[ObjectData],
        tlas: &mut wgpu::Tlas,
    ) {
        self.build(encoder, vertices, indices);
        let count = (tlas.get().len()).min(self.blases.len());
        for i in 0..tlas.get().len() {
            // Each object with a BLAS and a record this frame is an instance; the rest of the slots are empty.
            let instance = match (i < count, self.blases.get(i), objects.get(i)) {
                (true, Some(Some((blas, _, _))), Some(o)) => {
                    let m = glam::Mat4::from_cols_array_2d(&o.local_to_world).transpose();
                    let mut rows = [0.0f32; 12];
                    rows.copy_from_slice(&m.to_cols_array()[..12]);
                    Some(wgpu::TlasInstance::new(blas, rows, i as u32, mask(o)))
                }
                _ => None,
            };
            *tlas.get_mut_single(i).expect("a TLAS slot") = instance;
        }
        encoder.build_acceleration_structures(std::iter::empty(), std::iter::once(&*tlas));
    }
}

/// A TLAS for a program that traces the scene.
pub fn tlas(device: &wgpu::Device) -> wgpu::Tlas {
    device.create_tlas(&wgpu::CreateTlasDescriptor {
        label: Some("scene TLAS"),
        max_instances: MAX_INSTANCES,
        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
    })
}
