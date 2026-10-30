title: How Godot's SDFGI Lights a Room
subtitle: Signed distance field global illumination in Godot 4.7, followed through the source at the engine's sizes
engineVersion: 4.7.2-stable
commit: ed1daf0bf001b61586d9930840f2f1394092c079
date: 2026-10-29
fps: 30
width: 1920
height: 1080

# 1 | Where SDFGI runs | 150 s
summary: The frame's SDFGI work, what turns it on, and two settings that are not what they say.
sources: frame-order, sdfgi-update, settings, mobile-stub, prepass-gate, ray-count-table

## c1-room | 3d | SDFGI lights the room with light that bounced
{@ demo=room sdfgiMix=0 cameraDrift=1 camera.yaw=24 camera.pitch=20 camera.distance=6.2}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[10, 16, 6.6], [18, 18, 6.3]], "seconds": "beat"}}]}

This is the lab's study room, lit by the ceiling panel alone: only direct light, so every surface the panel cannot see is black.

{@sdfgi> sdfgiMix=0..1}Now Godot's signed distance field global illumination adds the light that bounced{<sdfgi}: the red wall tints the sphere, and the sky through the open top tints the floor.

{@ callout=frame-order} This is SDFGI, in Godot four point seven, followed through its source on the lab's CPU at the engine's own sizes.

{@ callout=settings} It is a setting of the scene's environment, and it is off by default.

## c1-frame | 2d | Every frame runs it in stages
diagram: frame
focus: UPD, VOX, LIGHT, PRE, STORE, GI

{@ callout=sdfgi-update} Each frame, SDFGI first moves its grids, called cascades, to follow the camera, and works out which parts of them are new.

{@ callout=render-region} The new parts are cut into voxels, small boxes marked solid or empty, and from them a distance field is built.

{@ callout=update-light} Light is then stored in the solid voxels, and probes, points that each record the light arriving from every direction, trace rays through the field to gather it.

{@ callout=process-gi} Last, a screen pass reads the probes around every pixel, and the opaque pass shades with the result.

## c1-switches | 3d | Forward+ only, and the defaults that shape it
{@ demo=room focusRow=1 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[18, 16], [8, 22]], "seconds": "beat"}}]}

{@ callout=mobile-stub} Only the Forward+ renderer has it: in the Mobile and Compatibility renderers, the update, the region queries, and the voxelizer are empty functions.

The defaults are four cascades, each a grid of one hundred twenty-eight cells a side, {@ focusRow=2}the finest with cells of twenty centimetres.

{@ callout=project-settings focusRow=7} Three project settings decide how many rays the probes trace, how many frames they average, and how often lights are updated.

## c1-prepass | 3d | The probes update only during the depth pre-pass
{@ demo=room focusRow=9 cameraDrift=0.5}

{@ callout=prepass-gate} The depth pre-pass draws the scene's depth before shading; the probes are traced in one place only, inside that pass's branch.

{@ callout=prepass-call} SDFGI never asks for the pass, so with the depth pre-pass setting turned off, the probes stop updating and the light freezes, read from the code.

## c1-rays | 3d | The default is eight rays, not sixteen
{@ demo=room focusRow=5 cameraDrift=0.5}

{@ callout=ray-count-hint} The ray count's setting lists eight, sixteen, thirty-two, and on, so its default, the second entry, reads sixteen.

{@ callout=ray-count-table focusRow=6} The code's table starts one entry earlier, at four, so the second entry gives eight rays per probe each frame, half of what the label promises.

## c1-sources | 2d | Where to read
sources: frame-order, sdfgi-update, prepass-gate, ray-count-table

These are the functions this chapter followed: the frame's order, the update, and the two lines behind the pre-pass and the ray count.

Every claim in this video cites a file, a symbol, and a line in the Godot checkout, and the build checks each line is still there.

# 2 | Cascades that follow the camera | 115 s
summary: Four nested grids, their squashed height, their probes, and how they jump after the camera.
sources: cascade-create, cell-size-double, y-mult-table, probe-axis-count, scroll-update, dirty-all-rule

## c2-nested | 3d | Four nested grids of 128 cells, each twice as coarse
{@ demo=cascades cascadesShown=0 probes=0 cameraDrift=1 camera.yaw=30 camera.pitch=38 camera.distance=60}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 40, 140], [36, 42, 230]], "seconds": "beat"}}]}

Before any voxel is drawn, SDFGI decides where its grids sit, so that is where the frame's work begins.

{@ callout=cascade-create} Around the camera sit {@grow> cascadesShown=0..4}four cascades, all one hundred twenty-eight cells wide{<grow}.

{@ callout=cell-size-double} The first has cells of twenty centimetres and spans twenty-five point six metres; each next one doubles its cells and its span, out to two hundred and five metres: fine detail near the camera, long reach far away.

The whole study room, six metres across, sits at the centre of the first cascade.

## c2-yscale | 3d | "Seventy-five percent" squashes the grid by one and a half
{@ demo=cascades cascadesShown=1 probes=0 yScale=2 cameraDrift=0.6 camera.distance=40}

{@ callout=y-mult-table} The y scale setting has three choices, and its default reads seventy-five percent; the documentation says it trades vertical reach for vertical detail.

{@squash> yScale=2..1}In the code it is a factor of one and a half, and height is multiplied by it before it is cut into cells{<squash}.

So a cell is only thirteen centimetres tall, and the cascade is seventeen point oh seven metres high: two thirds of its width, not three quarters, read from the code.

## c2-probes | 3d | Seventeen probes per axis, one every eight cells
{@ demo=cascades cascadesShown=1 probes=1 cameraDrift=1 camera.distance=18}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[30, 28], [-10, 24], [20, 30]], "seconds": "beat"}}]}

{@ callout=probe-axis-count} Inside each cascade sits a lattice of probes: one hundred twenty-eight cells make sixteen intervals of eight, so seventeen probes along each axis, both ends included.

{@ callout=probe-position} In the first cascade they are one point six metres apart, so only a handful of them, drawn brighter, fall inside the room.

## c2-scroll | 3d | The cascade jumps eight cells when the camera strays four
{@ demo=cascades cascadesShown=1 probes=0 scrollView=1 cameraDrift=0 camera.yaw=0 camera.pitch=89 camera.distance=14 camera.target=1,0,0}

{@ callout=scroll-update panel=code code=cascade-update highlight=cell} The camera's position is cut into cells, and each cascade compares that cell with its own centre.

{@ highlight=jump}{@walk> cameraX=0..1.0}When the camera strays more than four cells away, the cascade jumps eight cells, a whole probe, after it{<walk}.

{@ regions=1 highlight=dirty camera.distance=40 camera.target=10,0,0} The eight-cell slab the jump uncovered, at the far edge, is now dirty: only it will be voxelized again.

{@ callout=dirty-all-rule highlight=all} When more than half of the kept volume would be new, the whole cascade is rebuilt instead.

## c2-sources | 2d | Where to read
sources: cascade-create, y-mult-table, probe-axis-count, scroll-update

These are the lines that place and move the cascades.

# 3 | What is voxelized, and which lights | 90 s
summary: The dirty slab becomes a box; Static meshes and static lights are listed for it.
sources: pending-regions, region-bounds, sdfgi-cull, gi-mode, static-light-cull, static-lights-index

## c3-regions | 3d | Each dirty axis becomes one box in the world
{@ demo=regions cameraDrift=1 camera.yaw=30 camera.pitch=40 camera.distance=46 camera.target=6,0,0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[44, 34], [52, 30]], "seconds": "beat"}}]}

A scroll leaves a dirty slab, and the CPU now decides what goes into it.

{@ callout=pending-regions} Each axis that moved gives one region, and a later axis leaves out the corner an earlier one already took.

{@ callout=region-bounds} Each region becomes a box in real metres, with its height divided back by the y scale: here one point six by seventeen by twenty-five point six metres.

## c3-geometry | 3d | Only Static meshes are voxelized, seen or not
{@ demo=regions cameraX=10 cameraJump=1 cameraDrift=1 camera.distance=50}

{@ callout=sdfgi-cull} A long jump makes the whole first cascade one region, and every visible instance is tested against it, with no test for the camera's view.

{@ callout=gi-mode} Only meshes whose GI mode is Static are voxelized; a Dynamic mesh still receives SDFGI's light, but is not part of its field.

## c3-lights | 3d | Static lights are baked once; dynamic ones every update
{@ demo=regions extraLight=1 lightBake=0 cameraDrift=1}

{@ callout=light-bake-route} Positional lights split by their bake mode: a dynamic light is evaluated every update, {@ lightBake=1}a static one only when a region is built; directional lights are always dynamic.

{@ callout=static-light-cull} A static light is listed for a region it overlaps, but only up to its maximum cascade, which defaults to two, so the coarsest cascade never sees it.

## c3-finding | 3d | A list read by the wrong index
{@ demo=regions extraLight=1 lightBake=1 cameraX=2.22 cameraJump=1 cameraDrift=0.5}

{@ callout=static-lights-index panel=code code=regions highlight=stored} The lists are stored under each cascade's number, {@ highlight=read}but read back by position in the list of dirty cascades.

{@ panel=values} When only the second cascade moves, its light is stored under slot one and read from slot zero, so it gets none, read from the code and measured on the lab's model.

## c3-sources | 2d | Where to read
sources: pending-regions, sdfgi-cull, gi-mode, static-lights-index

These are the culling lines.

# 4 | Voxelization | 105 s
summary: Three orthographic views write albedo, facing, and emission into the grid.
sources: render-sdfgi, sdf-fragment, albedo-pack, facing-mask, emission-pack

## c4-passes | 3d | Three orthographic views, one pixel per voxel
{@ demo=voxels voxels=1 voxelPasses=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 16, 6.4], [-8, 20, 6.0], [24, 18, 6.6]], "seconds": "beat"}}]}

With the geometry chosen, the GPU draws it into the grid of voxels.

{@ callout=render-sdfgi} The region is drawn three times, by a camera looking along each axis, with one pixel per cell of the face it looks at.

{@three> voxelPasses=0..3}A surface seen edge-on covers no pixels in one view, so each view adds whatever is not edge-on to it, and together they cover the room{<three}.

{@ callout=sdf-double-sided} Culling is turned off for every material drawn here, so a surface is drawn from both sides, while alpha-blended materials are not drawn at all; the lab finds the same fragments by tracing each pixel's ray through its shapes.

## c4-albedo | 3d | A solid bit and five bits per colour
{@ demo=voxels voxels=1 cameraDrift=0.4 camera.distance=4.0 camera.target=-1.05,0.9,-0.55}

{@ callout=sdf-fragment panel=code code=voxelize highlight=position} Each fragment writes the voxel its own position falls in.

{@ callout=albedo-pack highlight=albedo} The albedo is sixteen bits: the lowest marks the voxel solid, and five bits each hold red, green, and blue.

{@ panel=values} The readout decodes the voxel at the top of the sphere, bit by bit.

## c4-facing | 3d | Six bits say which way the surface faced
{@ demo=voxels voxels=1 voxelFacing=0 cameraDrift=0.4 camera.distance=3.4 camera.target=-1.05,0.9,-0.55}

{@ callout=facing-mask panel=code code=voxelize highlight=facing} A second image keeps six bits, one per axis direction, {@ voxelFacing=1}and each fragment sets the one nearest its normal.

{@ panel=values} Several fragments in one voxel combine their bits, so a voxel on the sphere's curve can face two or three ways at once.

## c4-emission | 3d | Emission in a shared exponent, split over six faces
{@ demo=voxels voxels=1 voxelFacing=1 cameraDrift=0.6}

{@ callout=emission-pack panel=code code=voxelize highlight=emission} An emissive surface also writes its light: red, green, and blue sharing one exponent in thirty bits, with a five-bit share for each of the six faces.

In this room the panel's light arrives as an area light, so no voxel is emissive.

## c4-shell | 3d | Only surfaces: a thick object is hollow
{@ demo=voxels voxels=1 cutHeight=4 cameraDrift=0.4 camera.pitch=40 camera.distance=3.6 camera.target=-1.05,0.6,-0.55}

{@cut> cutHeight=4..0.62}Cut the room at the sphere's middle{<cut}, and the voxels form only its shell: nothing fills the inside, read from the code.

The distance field does not need the inside, because a ray stops at the shell.

## c4-sources | 2d | Where to read
sources: render-sdfgi, sdf-fragment, albedo-pack, facing-mask

These are the voxelizer's lines.

# 5 | The distance field | 130 s
summary: A jump flood finds each cell's nearest solid voxel; STORE keeps one byte per cell and a list of the solid cells.
sources: jump-flood-init, jump-flood, jump-flood-schedule, sdf-store, solid-cell-store

## c5-why | 3d | One byte per cell: the distance to the nearest surface
{@ demo=field slice=1 fieldMarch=0 cameraDrift=0.3}

The voxels say where surfaces are; rays need to know how far away they are.

{@ callout=sdf-store} So every cell gets the distance to its nearest solid voxel, shown here as a slice through the room, darker near a surface.

{@ fieldMarch=1} A ray can then step that far without passing through anything, and each step lands closer, until it reaches a surface.

## c5-seeds | 3d | Each half-size cell starts with a solid voxel of its block
{@ demo=field slice=2 floodStep=0 cameraDrift=0}

{@ callout=jump-flood-init panel=code code=jump-flood highlight=init} The nearest voxel is found by a jump flood, run at half size to do an eighth of the work: sixty-four cells a side.

Each half cell starts with one solid voxel of its two by two by two block, if it has one; the rest start empty.

## c5-flood | 3d | Neighbours at thirty-two, sixteen, eight, four, two, and one pass their seeds on
{@ demo=field slice=2 floodStep=0 cameraDrift=0}

{@ callout=jump-flood highlight=neighbours} Each pass, a cell looks at its twenty-six neighbours a step away, {@ highlight=nearest}and keeps whichever seed is nearest to the cell itself.

{@flood> floodStep=0..6}The step starts large and halves, thirty-two, sixteen, down to one, so a seed first jumps across the room, then small steps correct the cells it misled{<flood}.

{@ callout=jump-flood-schedule highlight=regular} The first two passes are the regular kind; {@ highlight=optimized}the last four keep their neighbourhood in shared memory, which is faster for small steps.

## c5-upscale | 3d | Back to full size, then one more pass
{@ demo=field slice=2 floodStep=6 cameraDrift=0}

{@ callout=jump-flood-upscale highlight=upscale floodStep=7} The result is scaled up to the full one hundred twenty-eight, and each cell checks the block around its seed for a nearer voxel.

{@ highlight=final floodStep=8} One last pass with a step of one fixes the cells whose seed was a cell off, the error searching at half size leaves, shown here in orange.

## c5-store | 3d | A byte of one plus the distance; zero inside
{@ demo=field slice=1 cameraDrift=0}

{@ callout=sdf-store panel=code code=store highlight=distance} STORE writes one byte per cell: zero at a solid voxel, otherwise one plus the distance, in cells.

{@ callout=sdf-decode panel=values} The field is not signed: the probe and shadow marches subtract the one again, so a solid voxel reads minus one, and the readout shows a cell two cells away storing three.

## c5-cells | 3d | Sixteen bytes for every solid cell
{@ demo=field slice=1 cameraDrift=0}

{@ callout=solid-cell-store panel=code code=store highlight=cell} Every solid voxel also goes into a list of sixteen-byte records: its position, albedo, facing, and stored light, so later passes can visit only the solid cells.

{@ callout=process-voxel highlight=neighbours} Twenty-six spare bits mark the empty neighbours whose nearest surface is this voxel, so light can be copied out to them later.

## c5-sources | 2d | Where to read
sources: jump-flood, jump-flood-schedule, sdf-store, solid-cell-store

These are the distance field's lines.

# 6 | Occlusion | 45 s
summary: For each probe, how visible every voxel around it is.
sources: occlusion-dispatch, occlusion-shells, occlusion-store, occlusion-read

## c6-shells | 3d | Visibility spreads from a probe, shell by shell
{@ demo=occlusion occlusionShell=0 cameraDrift=0}

The same pass also asks, for each probe, how much of the room around it the probe can see, since a probe behind a wall should not lend its light to the other side.

{@ callout=occlusion-dispatch panel=code code=occlusion highlight=shells} A window of sixteen cells is opened around each probe.

{@ callout=occlusion-shells highlight=average} {@shells> occlusionShell=0..22}Starting next to the probe, each shell of cells takes the average of its neighbours one step closer, and a solid cell counts as zero, darkening the cells behind it{<shells}.

## c6-store | 3d | Four bits for each of eight probes
{@ demo=occlusion cameraDrift=0}

{@ callout=occlusion-store panel=code code=occlusion highlight=store} Each cell keeps four bits for each of the eight probes around it.

{@ callout=occlusion-read panel=values} By default nothing reads them: use occlusion is off, yet the pass runs every time a cascade is built, read from the code.

## c6-sources | 2d | Where to read
sources: occlusion-dispatch, occlusion-shells, occlusion-store, occlusion-read

These are the occlusion lines.

# 7 | Light in the voxels | 140 s
summary: Every fourth solid cell per frame is lit, shadowed through the field, and packed per face; static light, and the bounce.
sources: update-light, light-schedule, omni-attenuation, shadow-march, aniso-pack, static-pass, scroll-reinject, bounce-feedback

## c7-cells | 3d | A thread per solid cell, a quarter of them each frame
{@ demo=light extraLight=0 shadowRay=0 lightPhase=4 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 16, 6.4], [12, 18, 6.5], [20, 18, 6.6]], "seconds": "beat"}}]}

With the field and the occlusion built, the solid cells can be lit, so the rays will have something to find.

{@ callout=update-light} One thread runs for each solid cell in the list, coloured here by the light it stores.

{@ callout=light-schedule lightPhase=0} By default a frame updates only every fourth cell, {@ lightPhase=1}then the next {@ lightPhase=2}quarter, {@ lightPhase=3}and so on, so a moving light takes up to four frames to reach a cell.

## c7-omni | 3d | Each light falls off with distance, in the squashed space
{@ demo=light extraLight=1 shadowRay=0 lightHeight=2 cameraDrift=0.5}

{@ callout=dynamic-lights} The extra omni light shows one floor cell's sum.

{@ callout=omni-attenuation panel=code code=direct-light highlight=attenuation} {@up> lightHeight=0.5..2.8}Its falloff is one minus the fourth power of distance over range, squared, over the distance, at the default attenuation of one{<up}.

{@ panel=values highlight=distance} The distance is measured after the height is squashed, so this light seems one and a half times higher than it is, read from the code and measured on the lab's model.

## c7-shadow | 3d | A shadow ray steps through the field
{@ demo=light extraLight=1 shadowRay=1 cellLight=0 lightX=2.4 lightZ=-0.6 lightHeight=0.8 cellX=1.0 cellZ=0.9 cameraDrift=0.2 camera.target=1.1,0.5,0.2 camera.distance=3.4 camera.yaw=10 camera.pitch=24}

{@ callout=shadow-march panel=code code=shadow-march highlight=march} A ray starts at the cell and steps toward the light, each step as long as the distance the field stores, the longest step that cannot jump over a surface.

{@ highlight=hit}{@behind> lightX=2.4..0.9}When it reaches a solid cell, the light is blocked{<behind}; {@ highlight=lit}past the last cascade, it is lit.

{@ callout=light-thread} The shadow is all or nothing, and each light is traced whatever its own shadow setting says.

## c7-faces | 3d | Light is kept per face, then packed
{@ demo=light extraLight=1 shadowRay=0 cameraDrift=0.5}

{@ callout=aniso-pack panel=code code=direct-light highlight=faces} A voxel has no single normal, so the light is summed for each of the six faces it shows, {@ highlight=store}then stored as one colour with six shares.

{@ highlight=neighbours} The same values are copied into the empty neighbours that point back at the cell, so blending next to a wall does not fade to black.

## c7-static | 3d | Static light is written into the cell once
{@ demo=light extraLight=1 lightBake=1 scrolls=0 cellLight=0 cameraDrift=0.5}

{@ callout=static-pass} A static light is evaluated when its region is built, and added to the light the cell stores.

## c7-double | 3d | And added again after every scroll
{@ demo=light extraLight=1 lightBake=1 scrolls=0 cellLight=0 cameraDrift=0.5}

{@ callout=scroll-reinject} After a scroll, the kept cells carry their stored light, static part included, into the new list.

{@more> scrolls=0..3}Whenever the light overlaps the new slab, the static pass adds it again, so a kept cell holds it twice, then three times, then four times{<more}, read from the code and measured on the lab's model.

## c7-bounce | 3d | Last frame's probes light the cells again
{@ demo=light extraLight=0 bounceFeedback=0 cameraDrift=1}

{@ callout=bounce-feedback} Before the lights, each cell reads the probes around it, and adds their light times its own colour times the bounce feedback, a half by default.

{@feed> bounceFeedback=0..1}The readout's red-wall cell brightens as the feedback grows{<feed}.

The probes were filled from these same cells a frame earlier, so each frame the light travels one bounce further.

## c7-sources | 2d | Where to read
sources: update-light, shadow-march, static-pass, bounce-feedback

These are the light pass's lines.

# 8 | Probes trace the field | 110 s
summary: Eight rays a frame per probe, marched through the field; sixteen coefficients kept in a ring of thirty frames.
sources: update-probes, vogel-rays, probe-march, probe-hit, probe-sky, sh-accum, history-ring

## c8-rays | 3d | Eight rays a frame, two hundred forty over thirty frames
{@ demo=probes probeRays=1 rayFrames=1 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[25, 18, 3.6], [0, 24, 3.2], [30, 16, 3.8]], "seconds": "beat"}}]}

The cells now hold light, but only where surfaces are; the probes gather it for every point between them.

{@ callout=update-probes} Every frame, each probe traces eight rays.

{@ callout=vogel-rays} {@fill> rayFrames=1..30}The rays follow a spiral, and over thirty frames they fill in two hundred forty directions{<fill}.

{@ rayFrames=2} Because thirty is even, each frame's eight rays point into one half of the sphere, and the halves alternate, shown in two colours, read from the code.

## c8-march | 3d | Each ray steps by the stored distance
{@ demo=probes probeRays=0 raySteps=0 cameraDrift=0.3 camera.distance=2.6}

{@ callout=probe-march panel=code code=probe-rays highlight=march} {@march> raySteps=0..30}A probe's ray marches the same way, a circle per step, until it is within a twentieth of a cell of a surface{<march}.

When it leaves the first cascade, it carries on through the next one, coarser and larger.

## c8-hit | 3d | A hit reads the voxel light, weighted by its faces
{@ demo=probes probeRays=1 rayFrames=1 cameraDrift=0.5}

{@ callout=probe-hit highlight=hit} At a hit, the field's slope gives the surface's direction; it picks how much of each face's light to take, and the ray brings that light back, last frame's bounce included.

## c8-miss | 3d | A miss reads the sky
{@ demo=probes probeRays=1 rayFrames=4 cameraDrift=0.5}

{@ callout=probe-sky highlight=sky} A ray that escapes reads the sky at mip level two, blurred so that a few rays still give a smooth colour.

{@ callout=sky-define} In four point seven stable, the probe shader is built for a single sky texture while the desktop default makes it an array, a mismatch a later commit fixes, read from the code.

## c8-sh | 3d | Sixteen coefficients, in fixed point
{@ demo=probes probeRays=1 rayFrames=1 cameraDrift=0.5}

{@ callout=sh-accum panel=code code=probe-rays highlight=sh} Each ray's light is added into sixteen spherical harmonic coefficients, a compact way to store light from every direction, {@ highlight=fixed}then scaled by one thousand twenty-four and kept as integers, so the history can be added and subtracted exactly.

## c8-history | 3d | A ring of thirty frames, summed exactly
{@ demo=probes probeRays=0 frames=0 historyChart=1 cameraDrift=0.5}

{@ callout=history-ring panel=code code=probe-rays highlight=ring} The probe keeps its last thirty frames; each frame it subtracts the oldest and adds the newest.

{@ramp> frames=0..40}So the probe brightens by a thirtieth each frame, a straight line on the chart, then holds{<ramp}: a light that moves takes thirty frames to show in full.

## c8-sources | 2d | Where to read
sources: vogel-rays, probe-march, sh-accum, history-ring

These are the probe tracing lines.

# 9 | From coefficients to maps | 85 s
summary: Two six-by-six octahedral maps per probe, a brightness gain, a half-texel shift, and scrolled probes.
sources: store-probes, oct-store, oct-border, oct-read, probe-scroll, parent-seed

## c9-oct | 3d | Two six-by-six octahedral maps per probe
{@ demo=maps probeMaps=1 cameraDrift=0.5}

A pixel cannot sample sixteen numbers directly, so this step turns each probe's coefficients into maps it can.

{@ callout=oct-store panel=code code=probe-store highlight=direction} Each probe gets two maps of six by six texels, the sphere of directions folded flat into a square: {@ highlight=irradiance}one holds irradiance, the light a surface facing that way receives, {@ highlight=radiance}the other radiance, the light arriving from that way.

{@ callout=oct-border highlight=border} A border of one texel, darker here, holds mirrored copies of the map's own edge texels, where the fold continues them, so filtering never reads past the map.

## c9-gain | 3d | A uniform sky comes out four over pi times too bright
{@ demo=maps probeMaps=1 showGain=1 cameraDrift=0.5}

{@ callout=sh-gain panel=code code=probe-store highlight=gain} The rays' sum should be scaled by four pi over the ray count, four pi being the sphere's solid angle, but with a four where it is gathered and a four where it is stored, {@ panel=values}a uniform sky comes out four over pi, about twenty-seven percent, too bright, read from the code and measured on the lab's model.

## c9-texel | 3d | Directions of texel corners, values at texel centres
{@ demo=maps probeMaps=1 cameraDrift=0.5}

{@ callout=oct-read panel=code code=probe-store highlight=direction} Each texel is given the direction of its corner, but its value sits at its centre, so a lookup for that direction lands between two texels and returns their blend, a half-texel shift, read from the code.

## c9-scroll | 3d | Scrolled probes keep their history; new ones borrow the parent's
{@ demo=cascades cascadesShown=2 probes=1 regions=1 cameraX=0 cameraDrift=0.6 camera.distance=34}

{@ callout=probe-scroll} {@go> cameraX=0..1.0}When a cascade scrolls, each probe's history moves with it{<go}.

{@ callout=parent-seed} A probe the scroll uncovers starts from the coarser cascade's light around it, rather than from black.

## c9-sources | 2d | Where to read
sources: oct-store, sh-gain, oct-read, parent-seed

These are the probe map lines.

# 10 | Shading a pixel | 110 s
summary: The screen pass: the finest cascade, eight probes, diffuse and specular, and how the frame uses them.
sources: process-gi, cascade-select, cascade-blend, probe-weights, specular-trace, gi-compose, forward-gi, fog-sdfgi

## c10-pass | 3d | A compute pass reads each pixel's position and normal
{@ demo=pixel pixelProbes=0 cameraDrift=1}

Every probe now has its maps; the last pass turns them into each pixel's light.

{@ callout=process-gi} After the depth pre-pass, a compute pass rebuilds each pixel's position from depth and reads its normal, like this point on the red wall.

## c10-cascade | 3d | The finest cascade that holds it
{@ demo=pixel pixelProbes=0 cascadesShown=2 blendPoint=1 cameraDrift=0.4 camera.distance=40 camera.target=8,0,0}

{@ callout=cascade-select} A pixel reads the finest cascade whose box holds it.

{@ callout=cascade-blend panel=code code=screen-pass highlight=blend} {@out> blendPoint=1..14}Near the box's edge, between five and a half and seven and a half probe spacings from the camera, it blends toward the next cascade{<out}, {@ panel=values}as the readout's test point shows.

## c10-probes | 3d | Eight probes, weighted by distance and by facing
{@ demo=pixel pixelProbes=1 cameraDrift=0.3 camera.distance=4.0 camera.yaw=70}

{@ callout=probe-weights panel=code code=screen-pass highlight=bias} The point is first nudged off the wall along its normal, by one point one cells.

{@ highlight=weight} The eight probes at the corners of its cell are weighted by closeness and by facing, so a probe behind the wall counts almost nothing.

{@move> pixelZ=-0.4..1.4}As the point slides along the wall, the weights move between the probes{<move}.

## c10-specular | 3d | Rough surfaces read the maps
{@ demo=pixel spherePixel=1 sphereRoughness=1 pixelProbes=0 cameraDrift=0.5 camera.target=-1.05,0.8,-0.55 camera.distance=3.2}

{@ callout=specular-probes} A reflection looks up the radiance map along the mirror direction, {@rough> sphereRoughness=1..0.25}blending toward the irradiance map as the surface gets rougher{<rough}.

{@ callout=specular-trace} Below a roughness of a fifth, Godot also marches a reflection ray through the field; the lab's room is rougher than that.

## c10-compose | 3d | The result replaces the sky's ambient
{@ demo=room sdfgiMix=1 cameraDrift=1}

{@ callout=gi-compose} The opaque pass takes SDFGI's ambient in place of the sky's, multiplies it by the albedo, and adds the reflection.

{@off> sdfgiMix=1..0}Without it, only the panel's direct light reaches the room, and the shadows are black{<off}.

{@back> sdfgiMix=0..1}With it, the bounced light comes back{<back}.

## c10-others | 3d | Transparent surfaces and fog read the probes themselves
{@ demo=room cameraDrift=1}

{@ callout=forward-gi} Transparent surfaces, not shown here, repeat the lookup in their own shader, without the reflection march and without the energy setting.

{@ callout=fog-sdfgi} Volumetric fog reads the probes' average light, but never applies the y scale, so it samples them at the wrong height, read from the code.

## c10-sources | 2d | Where to read
sources: process-gi, cascade-blend, probe-weights, gi-compose

These are the screen pass's lines.

# 11 | The frame again | 40 s
summary: The frame, in order.
sources: frame-order

## c11-cascades | 2d | The cascades follow the camera
diagram: frame
focus: UPD

{@ callout=sdfgi-update} The cascades follow the camera, and each jump leaves a slab to rebuild.

## c11-voxels | 2d | The slab becomes a distance field
diagram: frame
focus: VOX

{@ callout=render-region} The slab is voxelized, flooded into a distance field, and its occlusion is found.

## c11-light | 2d | The cells are lit
diagram: frame
focus: LIGHT

{@ callout=update-light} The solid cells are lit, a quarter each frame, with last frame's probes as the bounce.

## c11-probes | 2d | The probes trace and average
diagram: frame
focus: PRE, STORE

{@ callout=update-probes} The probes trace eight rays a frame, average thirty frames, and store two small maps each.

## c11-screen | 2d | Every pixel reads eight probes
diagram: frame
focus: GI

{@ callout=process-gi} Every pixel finds its cascade, weights the eight probes around it, and takes their light in place of the sky's.

That is SDFGI, read from Godot's source.
