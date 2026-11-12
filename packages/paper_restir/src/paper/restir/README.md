# ReSTIR (Bitterli et al. 2020)

The method of "Spatiotemporal reservoir resampling for real-time ray tracing with dynamic direct lighting"
(Bitterli, Wyman, Pharr, Shirley, Lefohn, and Jarosz, ACM TOG 39(4), Article 148, 2020), implemented from the
paper's equations and algorithms as WGSL program passes (`shaders/restir_passes.wgsl`, `shaders/restir_shading.wgsl`)
and mirrored on the CPU (`cpu.rs`). No authors' code was read. The research and the story's plan are in
`../../../story/`; every claim cites the paper by section, page, and equation or algorithm line through
`source-index.json`, written from the PDF by `source_index.py` (`RESTIR_PAPER_PDF`, or asked).

## The passes

In the order of Alg. 5 (gpu.rs): the pre-pass writes each pixel's normal, object, albedo, and roughness; streaming
RIS draws M candidates per pixel (a light by power, a uniform point on its sphere) through one reservoir (Alg. 3);
visibility reuse traces a shadow ray to each pixel's sample and zeroes W where it is blocked; temporal reuse combines
each reservoir with last frame's at the reprojected pixel, M clamped; n rounds of spatial reuse combine k
neighbours' reservoirs from a disc (Alg. 4, or Alg. 6 unbiased); the frame's reservoirs are kept; and each pixel is
shaded with f(y) · W (Alg. 3), averaged over still frames when `restirAccumulate` is on.

## Demonstrations

`room` (the method as the paper runs it), `ris` (RIS and the reservoir at a picked pixel P, its candidates stepped
by `risStep`), `reservoirs` (each stage's reservoirs, view modes: light, M, W, visibility), `reference` (accumulated
streaming RIS), `spatial` (P's neighbours and the geometry test), `bias` (the 1D example of Fig. 7), `reuse` (P's
reservoir across frames, its shadow ray), `methods` (the paper's comparisons, with the Experiments tab and the
performance panel). A click on the scene picks P. `experiments/methods.toml` measures each method against the
reference on the machine that runs it.

## Simplified

Where the lab differs from the paper:

- The paper's scenes (thousands to millions of emissive triangles) are replaced by the lab's room with 64 small spherical lights that move along loops.
- Lights are picked by power with a CDF and a binary search, where the paper uses an alias table: the same distribution, a different lookup.
- A candidate is a uniform point on a light's sphere; points on the far side have a target of zero, as a triangle facing away does.
- The lights do not block one another's shadow rays; the room's objects do.
- There is no environment map, so the paper's 25% of environment candidates does not apply.
- Both variants keep one reservoir per pixel (N = 1); the paper's biased results keep four.
- Spatial neighbours come from the R2 sequence turned per pixel; the paper says only "a low-discrepancy sequence".
- The geometry test (10% of the depth, 25 degrees) is applied to the temporal neighbour too.
- The temporal neighbour is found by reprojecting the pixel's point with last frame's camera; the room's objects do not move.
- The material is the lab's albedo and roughness through a GGX dielectric over Lambert, as the paper's unified material.
- The lab traces shadow rays against its analytic shapes and the teapot's BVH, without hardware ray tracing.
- A frame drawn alone first draws the same shot eight times, so its temporal reuse has frames before it.

## Verify

```sh
cargo run --release -- --investigation paper/restir selftest --report /tmp/restir.json
RESTIR_PAPER_PDF=/path/to/paper.pdf python3 packages/paper_restir/src/paper/restir/source_index.py
python3 tools/video/build.py --investigation paper/restir --snapshots-only
```
