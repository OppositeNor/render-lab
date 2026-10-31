title: How O3DE Lights a Frame
subtitle: The lighting model of O3DE's Atom renderer, pass by pass: ported to WGSL and read from O3DE 26.05's source
engineVersion: 26.05 (2.6.0)
commit: 3db6943249d8bd7960b9ed7e9aee310b7668586e
date: 2026-10-30
fps: 30
width: 1920
height: 1080

# 1 | Where the lighting runs | 675 s
summary: The opaque frame of MainRenderPipeline: a depth pre-pass, tiled light culling, shadows, a forward pass into five targets, and the full-screen passes that finish its light.
sources: engine-version, msaa-default, opaque-pass-order, forward-pixel, evaluate-lighting, forward-targets, target-formats, normal-encoding, forward-ibl-stencil, lighting-model-select, lowend-forward, deferred-inert

## c1-open | 3d | Light, assembled pass by pass
{@ demo=room callout="" image="" panel=hidden cameraDrift=1 labels=0 forwardStage=0 sunLux=20 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 lightOn=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[24, 20, 6.2], [4, 14, 6.6], [-14, 18, 6.0]], "seconds": "beat"}}]}

This is a frame of the Open 3D Engine's Atom renderer, lighting the lab's study room with its shaders ported to WGSL.

{@grow1> sunLux=20..40} A sun shines in through the open ceiling, and a small point light hangs over the floor.{<grow1}

{@stage1> forwardStage=0..1} Right now we only see the light the forward pass computed for itself; now the sky's diffuse light is added, and the shadowed sides fill in.{<stage1}

{@stage2> forwardStage=1..2} Next come the reflections of the sky, on the sphere and along the floor.{<stage2}

{@stage3> forwardStage=2..3} Last, ambient occlusion darkens the creases, here where the teapot meets the floor.{<stage3}

{@ delay=0.4} Each of these parts came from a different pass, at a different time in the frame.

That raises the question this video answers: how does O3DE light a room from many lights and the sky without shading every pixel against every light?

## c1-pipeline | 3d | The default pipeline
{@ demo=room callout="" image="" panel=hidden cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[-10, 16, 6.2], [12, 22, 5.6], [26, 18, 6.4]], "seconds": "beat"}}]}

{@ callout=engine-version} This is O3DE twenty-six oh five, version two point six, read from its source checkout.

{@ callout=render-pipeline-path} On Windows, Linux, and macOS, the engine loads one render pipeline by default, {@ codespan="MainRenderPipeline"}; Android and iOS load a mobile one.

{@ callout=msaa-default} It renders with two samples of multisampling per pixel; the lab renders four, so its edges are a little smoother.

## c1-map | 2d | First, depth
diagram: frame
focus: DEPTH

{@ callout=depth-prepass} First a depth pre-pass draws only the depth of every opaque mesh.

## c1-map-cull | 2d | Then the lights are sorted into tiles
diagram: frame
focus: DEPTH, CULL

{@ callout=light-culling-pass} Light culling reads that depth and sorts the lights into lists, one per sixteen by sixteen pixel tile, so a pixel later reads only the lights near it.

## c1-map-shadow | 2d | The sun's shadow, per pixel
diagram: frame
focus: DEPTH, SHADOW

{@ callout=fullscreen-shadow} The shadow passes draw the sun's shadow maps and turn them into a shadow value per pixel.

## c1-map-forward | 2d | The forward pass writes five targets
diagram: frame
focus: CULL, SHADOW, FWD, TARGETS, LUT

{@ callout=forward-pixel} Then the forward pass shades each pixel's surface with the lights of its tile, and writes five targets.

{@ callout=brdf-execute-once} It reads a BRDF look-up texture, built once at start-up.

## c1-map-after | 2d | Full-screen passes finish the light
diagram: frame
focus: TARGETS, DIFF, REFL, SSS, AO, MERGE, CUBES

{@ callout=opaque-pass-order} Full-screen passes read those targets: diffuse image-based light, reflections, subsurface scattering, and ambient occlusion; a merge adds diffuse and specular, and transparent meshes are drawn on top.

{@ callout=specular-preset} The sky's two cubemaps they read were built with the assets, long before the frame.

## c1-prepass | 3d | Depth first, so lighting knows what is visible
{@ image="" demo=room callout="" panel=hidden cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 20, 6.0], [2, 15, 6.6], [16, 21, 5.9]], "seconds": "beat"}}]}

{@ image=depth imageFrom=chainTarget image.target=5 image.far=4 imageCorner=top-left imageTitle=views.depth_prepass} {@ callout=depth-prepass} The pre-pass stores only depth, in a thirty-two bit float with an eight bit stencil, with near surfaces stored as larger values.

{@ image=depth image.far=12 blend=2} To read it, the view turns each stored value back into a distance from the camera.

{@ delay=0.4} It draws that distance from bright, at the camera, to black, at twelve metres.

{@ delay=0.4} {@ image=depth imageMark=0.05,0.66,0.95,1 imageMarkText=views.mark_near blend=0.6} So the floor in front of us, a metre or two away, is bright.

{@ delay=0.4} {@ image=depth imageMark=0.22,0.1,0.78,0.46 imageMarkText=views.mark_far blend=0.6} The back wall, several metres farther, is a darker blue.

{@ delay=0.4} Where two surfaces at different distances meet, the brightness jumps: that is how the sphere's and the boxes' outlines stand out against the wall behind them.

{@ delay=0.4} {@ image=depth imageMark=0,0,1,0.1 imageMarkText=views.mark_nothing_drawn blend=0.6} The sky, where no mesh was drawn, keeps the clear value, zero, which the view shows as black.

{@ delay=0.4} Every later pass that needs a position, the light culling and the shadows among them, starts from this one number per pixel.

{@ image=depth imageMark=""} {@ image=depth image.far=12 blend=3} {@ callout=forward-depth-state} The forward pass then tests each fragment against that depth, so it shades a pixel only for the surface the camera really sees.

{@ callout=depth-linear} The same depth is also turned into linear distance, which the shadow pass and ambient occlusion read later.

## c1-targets | 3d | Five targets, not one colour
{@ image="" demo=room callout="" cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 panel=values readoutMode=1 param.formula=-1}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[10, 14, 5.4], [-6, 10, 5.8], [8, 16, 5.2]], "seconds": "beat"}}]}

{@ image=t0 imageFrom=chainTarget image.target=0 imageCorner=top-left imageTitle=views.target_diffuse} {@ callout=forward-targets} The forward pass writes five targets at once, and we follow one point, here, low on the teapot.

Each view shows one target as the pass leaves it.

{@ delay=0.4} {@ image=t0 imageMark=0.41,0.37,0.57,0.57 imageMarkText=views.mark_p blend=0.6} In every one, this ring is P's pixel.

{@ delay=0.4} Reading the same pixel across the five views is how we follow what the forward pass stored for P.

{@ image=t0 imageMark=""} {@ image=t1 imageFrom=chainTarget image.target=1 image.stops=2 imageCorner=top-left imageTitle=views.target_specular} {@ callout=target-formats} Diffuse light and specular light go to two separate half-float targets.

The diffuse target holds, for each pixel, the diffuse light the forward pass computed, in half floats that can go above one.

{@ delay=0.4} The view tone maps it, as the final frame would.

{@ delay=0.4} {@ image=t0 imageMark=0.5,0.62,0.98,0.98 imageMarkText=views.mark_lit blend=0.6} So the floor is bright where the sun and the point light reach it.

{@ delay=0.4} {@ image=t0 imageMark=0.3,0.5,0.62,0.72 imageMarkText=views.mark_shadow blend=0.6} In the sun's shadow it is black, not dark grey, because the sky's light has not been added yet.

{@ delay=0.4} {@ image=t1 imageMark=0.68,0.05,0.97,0.62 imageMarkText=views.mark_sheen blend=0.6} The specular target is shown two stops brighter, because the light reflected toward us is much weaker.

{@ delay=0.4} It holds only that reflected light: the highlights, and the sheen of the walls facing the sun, which grows where we look along a wall at a grazing angle.

{@ delay=0.4} Adding the two, then the sky's light and the occlusion, gives the final pixel.

{@ image=t0 imageMark=""} {@ image=t1 imageMark=""} {@ image=t2 imageFrom=chainTarget image.target=2 imageCorner=bottom-left imageTitle=views.target_albedo} {@ callout=forward-albedo-target} Then the albedo, already multiplied by the diffuse response and the occlusion, with the specular occlusion in its fourth channel.

{@ delay=0.4} {@ image=t2 imageMark=0,0.05,0.24,0.8 imageMarkText=views.mark_base_colour blend=0.6} Read it as the surfaces' own colours, eight bits a channel, from zero to one.

{@ delay=0.4} They are a little darker than the base colours, because the diffuse response, the share of light not reflected at the surface, is already multiplied in, with the occlusion.

{@ delay=0.4} So the sky's light pass can multiply this colour by the sky and add it, with no material data of its own.

{@ image=t2 imageMark=""} {@ image=t3 imageFrom=chainTarget image.target=3 imageCorner=bottom-left imageTitle=views.target_specular_f0} {@ callout=forward-f0-target} Then the specular colour at normal incidence, F zero, with the roughness in its fourth channel.

{@ delay=0.4} {@ image=t3 imageMark=0.03,0.05,0.97,0.97 imageMarkText=views.mark_f0_grey blend=0.6} Here every surface is nearly black: a grey of four percent, how much a dielectric reflects when seen head on.

{@ delay=0.4} That is the default specular factor, one half, times the maximum of eight percent.

{@ delay=0.4} A metal would show its own colour here, because a metal's reflection is tinted by its base colour.

{@ delay=0.4} The fourth channel, the roughness, does not show in this view; the panel lists it for P.

{@ image=t3 imageMark=""} {@ image=t4 imageFrom=chainTarget image.target=4 image.channel=5 imageCorner=bottom-left imageTitle=views.target_normal} {@ callout=normal-encoding} And the normal, packed into ten bits per channel.

Its view decodes the stored normal back to a direction, and draws x as red, y as green, and z as blue.

{@ delay=0.4} Each runs from minus one, dark, through zero, half, to one, full.

{@ delay=0.4} {@ image=t4 imageMark=0.05,0.62,0.95,1 imageMarkText=views.mark_up_green blend=0.6} The floor faces straight up, along y, so its green is full and its red and blue sit at half: a light green.

{@ delay=0.4} {@ image=t4 imageMark=0.2,0.05,0.8,0.45 imageMarkText=views.mark_toward_blue blend=0.6} The back wall faces us, along z, so it is a lavender blue.

{@ delay=0.4} {@ image=t4 imageMark=0,0.05,0.2,0.8 imageMarkText=views.mark_x_red blend=0.6} And the left wall faces along x, so its red is full: pink.

{@ delay=0.4} Curved surfaces, like the sphere, run through every colour, because their normal turns from pixel to pixel.

{@ image=t4 imageMark=""} {@ image=t3 image.channel=4 blend=1.5} {@ callout=forward-f0-target} In that fourth channel, roughness is stored with eight bits, so the later passes see it to one part in two hundred and fifty-five.

{@ delay=0.4 imageHide=t0,t1} Together these targets are a thin G-buffer: what the passes after the forward pass need to finish the light.

## c1-normal | 3d | A normal in two numbers and a sign
{@ demo=room callout="" cameraDrift=0.5 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 panel=values readoutMode=2 image="" param.formula=-1}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[8, 12, 4.6], [-4, 10, 4.9], [6, 14, 4.5]], "seconds": "beat"}}]}

{@ image=n imageFrom=chainTarget image.target=4 image.channel=0 imageCorner=top-left imageTitle=views.target_normal} {@ callout=normal-encoding} The normal target stores the normal in signed octahedral form.

This view shows the target's channels as stored.

{@ delay=0.4} Red and green are the two octahedral numbers, each from zero to one, and blue is the sign of z.

{@ delay=0.4} So its colours are not a direction: a flat surface shows one flat colour, and the panel decodes P's.

The normal is divided by the sum of its three absolute components, which puts it on an octahedron.

{@ image=n image.channel=1 blend=1.2} Its x and y are turned by forty-five degrees and moved into zero to one: the first two channels.

{@ image=n image.channel=3 blend=1.2} The third channel keeps only the sign of z, so a whole ten bit channel holds a single bit.

{@ image=n image.channel=4 blend=1.2} {@ callout=normal-flags} The two bit alpha channel holds two flags, which the reflection passes read: whether they apply image-based light here, and whether they compensate multiple scattering.

## c1-split | 3d | The forward pass leaves the sky for later
{@ demo=room callout="" cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 panel=hidden image="" forwardStage=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 18, 6.0], [-8, 14, 6.4], [14, 20, 5.8]], "seconds": "beat"}}]}

{@ image=t0 imageFrom=chainTarget image.target=0 imageCorner=top-left imageTitle=views.target_diffuse} {@ callout=forward-opaque} For an opaque surface, the forward pass adds only direct light: the sun and the point light, and the material's emission.

The diffuse target is back: its shadowed sides are still black, because the sky's light is not in it yet.

{@d1> forwardStage=0..1} {@ callout=diffuse-global} The diffuse light of the sky is added afterwards, by a full-screen pass that reads the albedo and normal targets.{<d1}

{@d2> forwardStage=1..2} {@ callout=reflection-global} Its reflections come from another chain of full-screen passes, which reads F zero, roughness, and the normal.{<d2}

{@ callout=ibl-forward-transparent} Only transparent materials and Skin take image-based light inside the forward pass itself; a clear coat brings in just its specular part.

## c1-stencil | 3d | Stencil bits route each pixel
{@ demo=room callout="" cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 panel=hidden forwardStage=3 imageHide=t0 sphereClearCoat=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 18, 6.0], [0, 12, 6.4], [18, 20, 5.8]], "seconds": "beat"}}]}

{@ image=st imageFrom=chainTarget image.target=6 imageCorner=top-left imageTitle=views.stencil} {@ callout=stencil-refs} Each mesh also writes a stencil value, and the full-screen passes test it to choose their pixels.

{@ callout=forward-ibl-stencil} The top bit, one hundred and twenty-eight, sends a pixel to the diffuse image-based-light pass; the low bits, three, send it to the reflections.

{@ delay=0.4} {@ image=st imageMark=0,0,1,0.1 imageMarkText=views.mark_nothing_drawn blend=0.6} In the stencil view, black is a pixel nothing drew: its stencil stays zero.

{@ delay=0.4} Every mesh writes its own reference value, and the view gives each value a colour.

{@ delay=0.4} Violet would be the bit one hundred and twenty-eight alone: the sky's diffuse light, but no reflections.

{@ delay=0.4} {@ image=st imageMark=0.04,0.1,0.96,1 imageMarkText=views.mark_meshes blend=0.6} Teal is that bit with the low bits, three: both passes. Every mesh here is teal.

{@ delay=0.4} Later, the reflection probes add bits of their own, and the view draws those pixels brighter.

{@ image=st imageMark=""} {@coat> sphereClearCoat=0..1} {@ callout=clear-coat-forward-ibl} Give the sphere a clear coat, and its low bits drop to zero, because it now takes its reflections inside the forward pass.{<coat}

{@ sphereClearCoat=0 blend=1.5} The stencil is how the frame keeps a pixel's reflections from being added twice, once in the forward pass and once after it.

## c1-models | 3d | A material's lighting model picks its shaders
{@ demo=room callout="" cameraDrift=1 labels=1 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 panel=hidden image="" sphereClearCoat=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 20, 6.2], [-6, 14, 6.6], [12, 18, 6.0]], "seconds": "beat"}}]}

{@ callout=lighting-model-select} Every material type names a lighting model, and that name decides which shaders the material pipeline builds for it.

{@ callout=lighting-models} There are four: Base, Standard, Enhanced, and Skin; every surface in this room is Standard, the model of {@ codespan="StandardPBR"}, the material type of the engine's default materials.

{@ callout=enhanced-draw-list} Enhanced and Skin add subsurface scattering and transmission, so they draw in a second forward pass that writes one more target.

## c1-alternatives | 2d | One target on mobile; deferred, but off
diagram: forward-one-target
focus: FWD1

{@ callout=lowend-forward} The low-end and mobile pipelines use a classic forward pass: one colour target, with the sky's light added inside it.

{@ callout=mobile-pipeline} The mobile one has no depth pre-pass and no light culling at all; it caps each kind of simple light at five.

{@ callout=deferred-inert} O3DE also holds code for deferred shading through a visibility buffer, but no pass draws it and its console variable is off, so nothing in this video uses it.

## c1-recap | 3d | What the forward pass leaves behind
{@ demo=room callout="" cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 panel=hidden image="" forwardStage=3}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 20, 6.2], [2, 14, 6.6], [-12, 18, 6.0]], "seconds": "beat"}}]}

{@ image=t0 imageFrom=chainTarget image.target=0 imageCorner=top-left imageTitle=views.target_diffuse} So the forward pass does not finish a pixel: it leaves direct light in two targets and the surface in three more.

{@ image=t4 imageFrom=chainTarget image.target=4 image.channel=5 imageCorner=top-left imageTitle=views.target_normal} The passes after it read those targets to add the sky, the reflections, and the occlusion.

{@ delay=0.4} Before any of that, the engine has to decide which lights reach each tile, and that is the next part.

## c1-sources | 2d | Where to read
sources: render-pipeline-path, msaa-default, opaque-pass-order, depth-prepass, forward-pixel, evaluate-lighting, forward-targets, normal-encoding, stencil-refs, lighting-model-select, deferred-inert

These are the files and lines this chapter followed, in reading order.

Every claim in this video cites a file, a symbol, and a line in the O3DE checkout, and the build checks that each line is still there.

# 2 | Direct light: which lights reach a tile | 475 s
summary: Tiled light culling: each 16 x 16 tile's depth range and mask, its depth bins, the lists the forward pass walks, and how each light type is evaluated.
sources: light-culling-pass, light-culling-tile, tile-prepare, nvlc-constants, is-object-inside-tile, cull-point-light, light-culling-main, light-culling-remap, tile-iterator, point-light-radius, sphere-light-apply, polygon-not-culled

## c2-why | 3d | Every tile gets its own light list
{@ demo=culling callout="" image="" panel=hidden cameraDrift=1 labels=0 extraLights=0 cullTile=0 lightMarkers=1 viewMode=final splitView=0}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 22, 6.4], [2, 16, 6.8], [16, 24, 6.2]], "seconds": "beat"}}]}

With the frame laid out, its first lighting step runs before any surface is shaded: deciding which lights each part of the screen needs.

{@many> extraLights=0..24} Here are twenty-four small point lights over the floor; shading every pixel against all of them would waste most of the work.{<many}

{@ splitView=1} {@ callout=light-culling-tile} So the screen is cut into tiles of sixteen by sixteen pixels, and each tile keeps only the lights that can reach it.

{@ callout=light-culling-tile} At nineteen twenty by ten eighty that is one hundred and twenty by sixty-eight: eight thousand one hundred and sixty tiles, each with its own lists.

The left view colours each pixel by how many lights the culling kept for it.

{@ delay=0.4} That is the lights in its tile's list, in the depth bin its own depth falls in.

{@ delay=0.4} {@ label=few labelAt=1.8,2.4,-2.95 labelText=views.mark_few_lights blend=0.6} Dark blue is none: high on the back wall, every light is out of reach.

{@ delay=0.4} Teal is a few, and the ramp runs up to yellow at eight or more.

{@ delay=0.4} {@ label=many labelAt=-1.05,1.3,-0.55 labelText=views.mark_many_lights blend=0.6} Over the cluster of lights around the sphere, tiles keep eight or more.

{@ delay=0.4} Because the count follows depth bins, two pixels of one tile can show different counts when they lie at different depths.

{@ delay=0.4} The right view is the same camera's lit frame, so each tile sits over the pixels it lights.

{@ labelHide=few,many} {@ animate=[{"wave": {"param": "extraLightMotion", "amplitude": 60, "period": 9, "seconds": "sentence"}}]} As the lights move, each tile's count changes: few where nothing is near, many over the cluster.

## c2-tile | 3d | P's tile: its depth range
{@ image="" demo=culling callout="" panel=values cameraDrift=0.6 labels=0 extraLights=24 cullTile=1 lightMarkers=1}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[12, 18, 5.6], [-4, 14, 6.0], [10, 20, 5.4]], "seconds": "beat"}}]}

{@ callout=tile-prepare} We follow the tile that holds P: first, a compute pass reads the depth of its two hundred and fifty-six pixels.

{@ callout=tile-prepare} Each sample's depth becomes a view distance, and the tile keeps the nearest and the farthest, z near and z far.

{@ image=d imageFrom=chainTarget image.target=5 image.far=8 imageCorner=bottom-left imageTitle=views.depth_prepass} {@ callout=tile-prepare} With two samples per pixel, it reads both, so a thin edge still counts.

The depth view reads as before: bright near, black at eight metres.

## c2-mask | 3d | A 32-bit mask of the depths actually used
{@ demo=culling callout="" panel=values cameraDrift=0.5 labels=0 extraLights=24 cullTile=1 lightMarkers=1 image=""}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 16, 5.0], [44, 12, 5.4], [30, 18, 4.8]], "seconds": "beat"}}]}

{@ callout=nvlc-constants} Between z near and z far, the tile splits its depth into thirty-two equal slices.

{@ callout=tile-prepare} Each pixel sets the bit of the slice its depth falls in, and the bits of the whole tile are combined into one thirty-two bit mask.

{@ callout=is-object-inside-tile} A tile spanning a teapot and the far wall sets bits only at those two depths, leaving the empty space between them clear.

{@ delay=0.4} The mask is what lets the culling reject a light floating in that gap.

## c2-bins | 3d | One to eight bins, from the span in metres
{@ image="" demo=culling callout="" panel=values cameraDrift=0.3 labels=0 extraLights=24 cullTile=1 lightMarkers=1}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}

{@ image=bins imageFrom=tileGrid image.mode=2 imageCorner=top-left imageTitle=views.tile_bins} {@ callout=tile-prepare} The tile also chooses how many depth bins its lists are split into: one, two, four, or eight.

This view shades each tile by the number of depth bins it has: dark for one, brighter for two, four, and eight.

{@ delay=0.4} Most tiles cover one surface and keep a single bin.

{@ delay=0.4} {@ image=bins imageMark=0.25,0.32,0.85,0.62 imageMarkText=views.mark_more_bins blend=0.6} The bright ones run along edges, where one tile spans the sphere and the wall behind it.

{@ delay=0.4} There, more bins mean a pixel at the front walks only the lights near the front.

{@ image=bins imageMark=""} {@ camera.distance=4} {@ animate=[{"path": {"params": ["camera.distance"], "through": [[9]], "seconds": "sentence"}}]} {@ callout=tile-prepare} The count is two to the power of the base two log of the span in metres, rounded and clamped: under one point four one metres one bin, under two point eight three two, under five point six six four, and eight beyond.

{@ camera.distance=6 blend=2} Tiles over a single surface keep one bin; tiles that cross from the teapot to the far wall get more, and each bin gets its own list, so a pixel walks only the lights near its own depth.

## c2-cull | 3d | A light is kept by its sphere, then by the mask
{@ image="" demo=culling callout="" panel=values cameraDrift=0.6 labels=0 extraLights=24 cullTile=1 lightMarkers=1}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 20, 6.0], [0, 14, 6.4], [14, 22, 5.8]], "seconds": "beat"}}]}

{@ callout=light-culling-main} The culling pass runs one thread group per tile, two hundred and fifty-six threads, each taking every two hundred and fifty-sixth light.

{@ callout=cull-point-light} A point light is kept when its sphere of influence touches the box of the tile's depth range.

{@grow> extraLightRadius=0.6..1.8} {@ callout=is-object-inside-tile} Then its depth interval is turned into slice bits and tested against the mask: no shared bit, no light; a larger radius reaches more tiles and more bins.{<grow}

{@ extraLightRadius=1.0 blend=1.5} The test also records which of the tile's bins the light overlaps, as eight bits.

## c2-list | 3d | Seven groups, each closed by a marker
{@ image="" demo=culling callout="" panel=hidden cameraDrift=0.6 labels=0 extraLights=24 cullTile=1 lightMarkers=1}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 20, 6.0], [4, 15, 6.3], [14, 21, 5.9]], "seconds": "beat"}}]}

{@ image=list imageFrom=lightList imageCorner=bottom-left imageTitle=views.light_list} {@ callout=light-index-pack} Each kept light becomes one entry: its index shifted sixteen bits up, with its eight bin bits below.

{@ delay=0.4} {@ image=list imageMark=0,0,1,0.13 imageMarkText=views.mark_tile_list blend=0.6} Read the list view from the top: the first row is the tile's list as the culling wrote it, one cell per light it kept.

{@ delay=0.4} Each cell is coloured by the light's type, and the darker cells between them are the end-of-group markers.

{@ delay=0.4} {@ image=list imageMark=0,0.14,1,1 imageMarkText=views.mark_bin_rows blend=0.6} Below it are the remapped lists, one row per depth bin, each holding only the lights that bin overlaps and ending in a black end of list.

{@ delay=0.4} {@ image=list imageMark=0,0.14,0.1,0.3 imageMarkText=views.mark_p_bin blend=0.6} The row marked on the left is P's bin: the one list the forward pass will read for P.

{@ image=list imageMark=""} {@ callout=light-culling-main} The list is written in seven groups, decals first, then simple point, simple spot, sphere, disk, capsule, and quad lights, each closed by an end-of-group marker.

{@ callout=light-culling-remap} A remap pass then copies each entry into every bin it overlaps: eight lists of up to two hundred and fifty-six entries, each ended by an end-of-list marker.

{@ callout=nvlc-constants} With seven markers, a tile can keep at most two hundred and forty-eight lights and decals; past that, new entries overwrite the last slot, and the code itself expects flickering.

## c2-iterate | 3d | The forward pass walks its bin's list
{@ image="" demo=culling callout="" panel=hidden cameraDrift=0.4 labels=0 extraLights=24 cullTile=1 lightMarkers=1}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[12, 18, 5.6], [2, 15, 5.9], [12, 19, 5.5]], "seconds": "beat"}}]}

{@ image=list imageFrom=lightList image.walked=0 imageCorner=bottom-left imageTitle=views.light_list} {@ callout=tile-iterator} In the forward pass, P finds its tile, then the bin its own depth falls in.

{@ delay=0.4} {@ image=list imageMark=0,0.14,0.45,0.3 imageMarkText=views.mark_read_so_far blend=0.6} In P's row, the lit cells are the entries the loop has read so far.

{@ delay=0.4} The dim ones are still ahead of it; when the loop reaches the black end of list, it stops.

{@ image=list imageMark=""} {@ image=list image.walked=12 blend=6} {@ callout=tile-iterator} It walks that bin's list group by group: each light type's loop reads indices until it meets the group's marker.

{@ image=list image.walked=24 blend=3} {@ callout=point-light-loop} For each index it loads the light and adds its diffuse and specular light at P.

{@ callout=directional-lux} Directional lights are not culled at all: every pixel loops over them first.

## c2-falloff | 3d | Every light fades to zero at its radius
{@ demo=pointlight callout="" panel=values cameraDrift=0.6 labels=0 image=""}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[12, 28, 7.2], [-4, 22, 7.6], [10, 30, 7.0]], "seconds": "beat"}}]}

{@ callout=point-light-radius} A light's intensity falls as one over the distance squared, times a window that reaches zero exactly at its attenuation radius.

{@r> attenuationRadius=2..6} {@ callout=point-light-radius} With f the distance squared over the radius squared, the window is one minus f squared, all squared; the light ends where the sphere the culling tested ends.{<r}

{@ attenuationRadius=3.5 blend=1.5} {@ callout=point-light-auto-radius} The editor sets that radius from the light's brightness: the distance where its intensity falls to a cutoff of zero point one.

## c2-types | 3d | Spheres, disks, capsules, quads: one approximation each
{@ image="" demo=specular callout="" panel=values cameraDrift=0.6 labels=0 sphereLight=1 sphereLightBulb=0.05}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[8, 18, 3.4], [-6, 14, 3.7], [6, 20, 3.3]], "seconds": "beat"}}]}

{@ callout=area-lights} Lights with a size are evaluated in closed form, each with its own approximation.

{@bulb> sphereLightBulb=0.05..0.4} {@ callout=sphere-light-apply} A sphere light's specular uses a representative point: the point of the sphere nearest the reflection ray, so a bigger bulb widens the highlight.{<bulb}

{@rough> sphereRoughness=0.2..0.6} {@ callout=sphere-light-apply} Its intensity is then normalised by the roughness, so the wider highlight does not add energy.{<rough}

{@ callout=area-lights} Disks and capsules work the same way, and quads use linearly transformed cosines, a fit stored in two thirty-two by thirty-two tables.

## c2-findings | 3d | Read from the code: two lights that do not behave
{@ demo=culling callout="" panel=hidden cameraDrift=0.6 labels=0 extraLights=24 cullTile=0 lightMarkers=1 image="" viewMode=final splitView=0}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 22, 6.4], [2, 16, 6.8], [16, 24, 6.2]], "seconds": "beat"}}]}

{@ image=list imageFrom=lightList imageCorner=bottom-left imageTitle=views.light_list} {@ callout=polygon-not-culled} Reading the culling code, polygon lights have no group: their loop starts after the quad group's marker, meets the end of the list, and lights nothing.

{@ callout=directional-channel-return} And in the directional loop, a light on another lighting channel returns from the loop instead of skipping ahead, so the directional lights after it are lost.

{@ callout=directional-angular-unused} The sun's angular size is computed into an adjusted direction that the specular never uses.

These come from reading O3DE's source; the lab does not run the engine itself.

## c2-recap | 3d | The lights each pixel will see
{@ image="" demo=culling callout="" panel=hidden cameraDrift=1 labels=0 extraLights=24 cullTile=0 lightMarkers=1 viewMode=final splitView=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 22, 6.4], [-2, 16, 6.8], [14, 24, 6.2]], "seconds": "beat"}}]}

{@ splitView=1} So culling turns all the scene's lights into one short list per tile and depth bin, before a single pixel is shaded.

On the left, each tile's count again; on the right, the frame those lists light.

That keeps the forward pass's cost close to the lights a pixel can really see.

{@ delay=0.4} With the lights sorted into lists, the next part turns to what they light: the surface.

## c2-sources | 2d | Where to read
sources: light-culling-pass, light-culling-tile, tile-prepare, nvlc-constants, is-object-inside-tile, cull-point-light, light-culling-main, light-index-pack, light-culling-remap, tile-iterator, point-light-radius, sphere-light-apply, polygon-not-culled

These are the files and lines this chapter followed, in reading order.

# 3 | The surface: from material to albedo, F0, roughness | 260 s
summary: How StandardPBR's inputs become a surface: the metallic workflow, roughness and its clamps, specular anti-aliasing, occlusion and emission, and the decals that change the surface after its lighting is set up.
sources: surface-eval, set-albedo-f0, material-utils, roughness-values, specular-aa, occlusion-input, decal-apply

## c3-inputs | 3d | The material's inputs become a surface
{@ demo=surface callout="" image="" panel=values cameraDrift=0.6 labels=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[4, 8, 3.1], [-10, 5, 3.3], [6, 10, 3.0]], "seconds": "beat"}}]}

With the lights sorted into lists, the next part turns to what they light: the surface.

{@ panel=code code=surface-setup highlight=albedo} {@ callout=surface-eval} For each pixel the material first evaluates a surface: its base colour, metallic, specular factor, roughness, normal, and occlusion.

{@ image=a imageFrom=chainTarget image.target=2 imageCorner=top-left imageTitle=views.target_albedo} {@ callout=base-colour-acescg} A colour property is converted from linear sRGB into ACEScg, the space the lighting works in, before the shader sees it.

The albedo view shows the colour after that conversion, as the lighting uses it.

## c3-metal | 3d | Albedo and F0 from one base colour
{@ image="" demo=surface callout="" panel=values cameraDrift=0.5 labels=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[4, 8, 3.1], [-8, 6, 3.3], [6, 10, 3.0]], "seconds": "beat"}}]}

{@ image=a imageFrom=chainTarget image.target=2 imageCorner=top-left imageTitle=views.target_albedo} {@ image=f imageFrom=chainTarget image.target=3 imageCorner=top-left imageTitle=views.target_specular_f0} {@ callout=set-albedo-f0} One base colour feeds two outputs: the albedo, which colours diffuse light, and F zero, the colour of the reflection head on.

{@m> sphereMetallic=0..1} {@ callout=material-utils} As metallic rises, the albedo fades to black and F zero turns from a dielectric's grey to the base colour itself.{<m}

{@ delay=0.4} {@ image=a imageMark=0.28,0.42,0.62,0.98 imageMarkText=views.mark_albedo blend=0.6} Watch the sphere in the albedo target, on the left: as metallic rises, it goes from its base colour to black.

{@ delay=0.4} {@ image=f imageMark=0.28,0.42,0.62,0.98 imageMarkText=views.mark_f0 blend=0.6} In F zero, beside it, the same sphere goes from four percent grey to its base colour.

{@ delay=0.4} The colour has moved from the diffuse light to the reflection: that is what metallic means here.

{@ image=a imageMark=""} {@ image=f imageMark=""} {@ sphereMetallic=0 blend=1} {@fac> specularF0Factor=0..1} {@ callout=material-utils} A dielectric's F zero is zero point zero eight times the specular factor; the default factor of a half gives zero point zero four.{<fac}

{@ specularF0Factor=0.5 blend=1} {@me> sphereMetallic=0..1} A metal therefore has no diffuse light at all; its colour is all in its reflection.{<me}

## c3-rough | 3d | Roughness, clamped and squared twice
{@ demo=surface callout="" panel=values cameraDrift=0.5 labels=1 image=""}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[4, 8, 3.1], [-8, 6, 3.3], [6, 10, 3.0]], "seconds": "beat"}}]}

{@ panel=code code=surface-setup highlight=roughness} {@ callout=roughness-values} Roughness, as authored, is clamped to at least zero point zero two four, so no highlight becomes infinitely sharp.

{@r> sphereRoughness=0.05..0.8} {@ callout=roughness-values} It is squared into roughness A, the width the microfacet formulas use, and squared again into roughness A two.{<r}

{@ sphereRoughness=0.35 blend=1.5} Each later formula picks the form it needs from these three numbers.

## c3-aa | 3d | Specular AA widens roughness where the normal changes fast
{@ image="" demo=surface callout="" panel=values cameraDrift=0.3 labels=0 sphereRoughness=0.1 sphereMetallic=1}
{@ camera.distance=6 camera.yaw=4 camera.pitch=8}

{@ callout=specular-aa} Far away, a smooth sphere's highlight covers less than a pixel, and it flickers as the camera moves.

{@ animate=[{"path": {"params": ["camera.distance"], "through": [[11]], "seconds": "sentence"}}]} {@ callout=specular-aa} Specular anti-aliasing takes a quarter of the normal's squared change between neighbouring pixels as a variance, and adds twice that to roughness A two.

{@aa> specularAA=1..0} {@ callout=specular-aa-default} It is on by default, and the extra roughness is capped at zero point one eight; switched off, the far highlight thins and sparkles.{<aa}

{@ specularAA=1 camera.distance=3.1 blend=2} Close up, the normal changes slowly from pixel to pixel, so much less is added.

## c3-occl | 3d | Occlusion maps, and emission
{@ image="" demo=surface callout="" panel=values cameraDrift=0.5 labels=1 specularAA=1 sphereRoughness=0.35 sphereMetallic=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[4, 8, 3.1], [-8, 6, 3.3], [6, 10, 3.0]], "seconds": "beat"}}]}

{@oc> sphereOcclusion=0..1} {@ callout=occlusion-input} An occlusion map's red channel is raised to the power of its factor: a factor of one keeps the map, zero ignores it.{<oc}

{@o> diffuseOcclusionFactor=0..2} {@ callout=occlusion-input} Without a map the factor does nothing at all; with one, the factor only sharpens or softens it.{<o}

{@ sphereOcclusion=0 blend=1} {@ callout=emissive-input} Emission is authored in EV100, converted to nits, and added after the lighting.

## c3-decal | 3d | A decal changes the surface, after its lighting is set up
{@ image="" demo=decals callout="" panel=values cameraDrift=0.5 labels=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[10, 55, 2.4], [-6, 48, 2.7], [8, 58, 2.3]], "seconds": "beat"}}]}

{@ callout=decal-apply} A decal projects a texture through a box onto whatever surface lies inside it.

{@ decalOpacity=0} {@op> decalOpacity=0..1} {@ callout=decal-apply} Inside the box, the decal's colour replaces the albedo by its opacity, and its normal map bends the normal; roughness and F zero stay as they were.{<op}

{@tilt> decalTilt=0..60} {@ callout=decal-attenuation} A decal also fades as the surface turns away from its projection, by a smoothstep of the angle.{<tilt}

{@ decalTilt=0 blend=1} {@ callout=evaluate-lighting} In O3DE twenty-six oh five, the forward pass first initialises its lighting data from the surface, and only then applies the decals to a copy of it.

{@ image=a imageFrom=chainTarget image.target=2 imageCorner=top-left imageTitle=views.target_albedo} {@ callout=forward-albedo-target} The direct lights see the decal, but the targets are written from the surface before it, so the full-screen sky light, read later from those targets, ignores the decal: a finding read from the code.

{@ delay=0.4} {@ image=a imageMark=0.08,0.6,0.92,1 imageMarkText=views.mark_no_decal blend=0.6} In the albedo view, the floor under the decals is plain: no paint, no grid.

{@ delay=0.4} The targets were written from the surface before the decals were applied, so the passes that read them later never see the decals.

## c3-recap | 3d | A surface, ready to be lit
{@ demo=surface callout="" panel=values cameraDrift=1 labels=1 image=""}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[4, 8, 3.1], [-10, 6, 3.3], [8, 10, 3.0]], "seconds": "beat"}}]}

{@rr> sphereRoughness=0.1..0.7} So a pixel's surface comes down to an albedo, an F zero, three forms of one roughness, a normal, and two occlusion factors.{<rr}

{@mm> sphereMetallic=0..1} These are the numbers every light will be weighed against, and the targets keep them for the passes after the forward pass.{<mm}

{@ delay=0.4} The next part follows what each light does with them: the BRDF.

## c3-sources | 2d | Where to read
sources: surface-eval, base-colour-acescg, set-albedo-f0, material-utils, roughness-values, specular-aa, specular-aa-default, occlusion-input, emissive-input, decal-apply, evaluate-lighting

These are the files and lines this chapter followed, in reading order.

# 4 | The BRDF: how the surface answers each light | 380 s
summary: GGX specular and its D, G, and F, the Lambertian diffuse and its response, the per-light formula, the four lighting models, the BRDF look-up texture, and multiscatter compensation, which is off unless a material turns it on.
sources: specular-ggx, ggx-d, ggx-g, fresnel-schlick, diffuse-lambert, fresnel-roughness, point-light-apply, standard-brdf, brdf-texture, multiscatter, ms-material-property

## c4-vectors | 3d | Four directions at a point
{@ demo=specular callout="" image="" panel=values cameraDrift=0.5 vectors=1 lobe=0 param.formula=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-12, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 70, "seconds": "beat", "ease": "sine", "repeat": "pingPong"}}]}

A surface is a set of numbers; the BRDF turns them, a light's direction, and the eye's into reflected light.

{@ vectors=0} {@vv> vectors=0..1} {@ callout=specular-ggx} At the top of this sphere: the normal N, the direction to the light L, to the eye V, and halfway between those two, H.{<vv}

{@ callout=specular-ggx} As the light swings, H swings with it, and every term of the specular reflection is measured against these four.

## c4-d | 3d | D: how many microfacets face H
{@ image="" demo=specular callout="" panel=values cameraDrift=0.4 vectors=1 lobe=1 param.formula=1 term=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-10, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@ panel=code code=specular-ggx highlight=d} {@ callout=ggx-d} The surface is modelled as tiny mirrors, and D, the GGX distribution, says how many of them face exactly along H.

{@rough> sphereRoughness=0.1..0.7} {@ callout=ggx-d} Smooth, almost all face the normal and the lobe is tall and narrow; rough, they spread and the lobe widens and drops.{<rough}

{@ viewMode=d} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 60, "seconds": "sentence", "ease": "sine"}}]} {@ callout=ggx-d} Over the sphere, D is brightest where H meets the normal, and that spot follows the light.

## c4-g | 3d | G: the facets that see both light and eye
{@ image="" demo=specular callout="" panel=values cameraDrift=0.4 vectors=1 lobe=1 param.formula=1 term=2 viewMode=g sphereRoughness=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-10, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@ panel=code code=specular-ggx highlight=g} {@ callout=ggx-g} G removes the facets hidden from the light or from the eye by their neighbours: the height-correlated Smith term.

{@swing> lightX=-1.6..-2.5} {@ callout=ggx-g} G itself falls at grazing angles, here as the light sinks toward the sphere's horizon.{<swing}

{@ lightX=-1.6 blend=1.5} {@ callout=ggx-g} O3DE folds the one over four N dot L N dot V of the microfacet formula into this term, so the code multiplies D by G and N dot L directly.

## c4-f | 3d | F: Fresnel, from F zero toward white
{@ image="" demo=specular callout="" panel=values cameraDrift=0.3 vectors=1 lobe=1 param.formula=1 term=3 viewMode=final sphereRoughness=0.35 sphereMetallic=0}

{@ panel=code code=specular-ggx highlight=f} {@ callout=fresnel-schlick} F is Schlick's Fresnel, measured between H and the eye: F zero when they agree, rising toward white as they part.

{@graze> lightX=-1.6..-2.6} {@ callout=fresnel-schlick} As the light sinks toward the side, light and eye spread apart, H turns away from V, and F climbs.{<graze}

{@ lightX=-2.28 blend=2} {@ panel=code code=specular-ggx highlight=product} The specular light is D times G times N dot L times F, times the light's intensity.

## c4-diffuse | 3d | Diffuse: Lambert, scaled by what reflection leaves
{@ image="" demo=diffuse callout="" panel=values cameraDrift=0.4 vectors=1 lobe=0 param.formula=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-10, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@ panel=code code=diffuse-response highlight=lambert} {@ viewMode=ndotl} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 70, "seconds": "sentence", "ease": "sine"}}]} {@ callout=diffuse-lambert} The diffuse light is Lambert's: the albedo over pi times N dot L, brightest where the sphere faces the light.

{@ viewMode=diffuseResponse} {@ panel=code code=diffuse-response highlight=response} {@ callout=fresnel-roughness} It is scaled by the diffuse response, one minus a Fresnel term at the view angle, so light that reflects off the surface does not also diffuse.

{@r2> sphereRoughness=0.05..0.9} {@ callout=fresnel-roughness} That Fresnel term is made gentler on rough surfaces, so the response stays near one minus F zero across the sphere.{<r2}

## c4-perlight | 3d | Per light: (diffuse + specular) × intensity × shadow
{@ image="" demo=pointlight callout="" panel=values cameraDrift=0.5 viewMode=final}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[12, 28, 7.2], [-4, 22, 7.6], [10, 30, 7.0]], "seconds": "beat"}}]}

{@ panel=code code=simple-point-light highlight=falloff} {@ viewMode=attenuation} {@rad> attenuationRadius=2..6} {@ callout=point-light-apply} For one simple point light, the shader first finds the falloff from the distance and the radius.{<rad}

{@ panel=code code=simple-point-light highlight=intensity} {@ viewMode=final attenuationRadius=3.5 blend=1} {@cd> lightIntensity=2..12} {@ callout=point-light-apply} The intensity is the light's candelas over the distance squared, times the window.{<cd}

{@ lightIntensity=6 blend=1} {@ animate=[{"path": {"params": ["lightX"], "through": [[1.4], [-0.4]], "seconds": "sentence"}}]} {@ panel=code code=simple-point-light highlight=shade} {@ callout=point-light-apply} Diffuse and specular are each multiplied by that intensity and by the shadow's lit ratio, always one for a simple point light, which casts no shadow, and added to the pixel's running sums.

## c4-models | 3d | Four lighting models, four BRDFs
{@ image="" demo=specular callout="" panel=values cameraDrift=0.5 vectors=0 lobe=1 viewMode=final}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-10, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@ callout=standard-brdf} Standard, the default, is everything so far: Lambert's diffuse and GGX specular, with an optional clear coat.

{@ diffuseModel=normalizedDisney} {@ callout=skin-diffuse} Skin, and Enhanced with subsurface scattering on, use Disney's normalised diffuse instead, which brightens toward grazing on rough surfaces.

{@ diffuseModel=lambertian} {@coat> sphereClearCoat=0..1} {@ callout=clear-coat-specular} A clear coat adds a second, smooth GGX lobe with F zero fixed at zero point zero four, and dims what lies beneath by its Fresnel.{<coat}

{@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 60, "seconds": "sentence", "ease": "sine"}}]} {@ callout=enhanced-brdf} Enhanced also offers anisotropic GGX, and Skin a second specular lobe blended with the first; Base is Standard without a coat.

{@ sphereClearCoat=0 blend=1} {@ callout=clear-coat-diffuse} Read from the code, a coat dims direct diffuse light twice: once in the diffuse response and again per light.

## c4-lut | 3d | The BRDF look-up texture, built once
{@ image="" demo=multiscatter callout="" panel=values cameraDrift=0.4 sphereRoughness=0.5 viewMode=final}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-10, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@ image=lut imageFrom=brdfLut imageCorner=top-left imageTitle=views.brdf_lut} {@ callout=brdf-execute-once} At start-up, before the first frame, a compute pass fills a two hundred and fifty-six by two hundred and fifty-six texture.

{@ panel=code code=brdf-texture highlight=inputs} {@ callout=brdf-texture} Across it runs N dot V, down it one minus roughness, and each texel averages the GGX reflection over the hemisphere for F zero zero and one.

{@ delay=0.4} {@ image=lut imageMark=0,0,0.5,1 imageMarkText=views.mark_scale_a blend=0.6} The view shows its two channels side by side: on the left the scale A, in red, the part of the reflection that grows with F zero.

{@ delay=0.4} {@ image=lut imageMark=0.5,0,1,1 imageMarkText=views.mark_bias_b blend=0.6} On the right the bias B, in green, the part added whatever F zero is, which matters most at grazing angles.

{@ delay=0.4} Across each, N dot V runs from zero, grazing, at the left, to one, head on, at the right.

{@ delay=0.4} {@ image=lut imageMark=0,0,1,0.16 imageMarkText=views.mark_rough blend=0.6} And down each, roughness runs from one, at the top, to zero, at the bottom.

{@ delay=0.4} The ring is P's texel: it gives the two numbers P's reflection of the sky is scaled by.

{@ image=lut imageMark=""} {@ panel=code code=brdf-texture highlight=sample} {@ callout=brdf-texture} It takes one thousand and twenty-four directions per texel, chosen by importance sampling GGX.

{@ panel=code code=brdf-texture highlight=store} {@ callout=brdf-texture-size} Two numbers per texel come out, a scale A and a bias B, so the hemisphere's reflection is F zero times A plus B.

{@lr> sphereRoughness=0.2..0.8} {@ callout=brdf-texture} The ringed texel is the one this point reads: it moves up the texture as the sphere gets rougher.{<lr}

## c4-multi | 3d | Multiscatter: the energy single scattering loses
{@ image="" demo=multiscatter callout="" panel=values cameraDrift=0.3 sphereRoughness=0.2 viewMode=final multiscatter=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-8, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@ panel=code code=multiscatter highlight=energy} {@ animate=[{"path": {"params": ["sphereRoughness"], "through": [[0.9]], "seconds": "sentence"}}]} {@ callout=multiscatter} A rough metal looks darker than it should: GGX counts light that bounces once, and loses light that bounces between facets.

{@rr> sphereRoughness=0.2..0.95} {@ callout=multiscatter} A plus B is the share that one bounce keeps, and it shrinks as roughness grows.{<rr}

{@ panel=code code=multiscatter highlight=compensate} {@ multiscatter=1 blend=1.5} {@ callout=multiscatter} The compensation is one plus F zero times one over A plus B, minus one, multiplied into the specular light.

{@ viewMode=multiscatter} {@g> sphereRoughness=0.2..0.95} It grows with roughness and with F zero, so rough metals gain the most.{<g}

## c4-default | 3d | Finding: compensation is off by default
{@ image="" demo=multiscatter callout="" panel=values cameraDrift=0.3 sphereRoughness=0.8 viewMode=final multiscatter=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-8, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@on> multiscatter=0..1} {@ callout=ms-option} The shader option for compensation defaults to on.{<on}

{@ viewMode=multiscatter} {@off> multiscatter=1..0} {@ callout=ms-material-property} But the material property that drives it has no default value, so it loads as false and switches the option off for every material that does not set it.{<off}

{@ viewMode=final} {@ callout=ms-preset} Only the metal presets and a few sample materials set it; read from the code, a default StandardPBR material is rendered without compensation.

## c4-recap | 3d | One light, one formula
{@ image="" demo=specular callout="" panel=values cameraDrift=1 vectors=1 lobe=1 viewMode=final multiscatter=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-12, 4, 3.3], [8, 8, 3.0]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 60, "seconds": "beat", "ease": "sine", "repeat": "pingPong"}}]}

So each light adds diffuse and specular light, weighed by D, G, F, and the diffuse response, all from the surface's numbers.

The look-up texture made once at start-up lets the later passes reuse that reflection for the whole sky.

{@ delay=0.4} One factor in the formula is still open: the shadow's lit ratio, and the sun's is computed before the forward pass even starts.

## c4-sources | 2d | Where to read
sources: specular-ggx, ggx-d, ggx-g, fresnel-schlick, diffuse-lambert, fresnel-roughness, point-light-apply, standard-brdf, skin-diffuse, clear-coat-specular, brdf-execute-once, brdf-texture, multiscatter, ms-material-property

These are the files and lines this chapter followed, in reading order.

# 5 | Shadows: a map from the sun, read through the screen | 415 s
summary: The sun's cascaded shadow maps: four 1024-texel cascades split near-dense, each fitted with a sphere and snapped to texels, a point's lookup with its biases, the screen-space shadow buffer the forward pass reads, its blur, and the shadows of local lights.
sources: cascade-image, cascade-splits, cascade-sphere-fit, cascade-snap, shadow-coords, shadow-normal-offset, cascade-select, shadow-is-shadowed, shadow-filter-default, fullscreen-shadow, fullscreen-shadow-normal, shadow-blur-settings, shadow-use-fullscreen, shadow-atlas, point-shadow-faces

## c5-why | 3d | The sun's shadow is a depth map seen from the sun
{@ image="" demo=cascades callout="" panel=hidden cameraDrift=1 labels=0 cascadeCamera=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[28, 26, 7.5], [8, 20, 8.2], [24, 30, 7.2]], "seconds": "beat"}}]}

The per-light formula ended in a lit ratio, and for the sun that ratio was computed before the forward pass ran.

{@ callout=one-shadow-light} O3DE shadows one directional light, drawing the scene's depth as seen from that light into a shadow map.

{@ image=cm imageFrom=cascadeMap image.cascade=4 image.from=0.9 imageCorner=bottom-left imageTitle=views.cascade_map} {@ callout=cascade-image} The map is one texture array of thirty-two bit float depth: four slices of one thousand and twenty-four by one thousand and twenty-four texels, one per cascade.

{@ delay=0.4} {@ image=cm imageMark=0,0,0.25,1 imageMarkText=views.mark_cascade_0 blend=0.6} The view lays the four slices side by side, the nearest cascade first, each tinted with its debug colour.

{@ delay=0.4} Each texel holds the depth, from the sun, of the first surface the sun meets there.

{@ delay=0.4} Bright is near the sun and dark is far, over the depth range set for this view.

{@ delay=0.4} {@ image=cm imageMark=0.5,0,1,1 imageMarkText=views.mark_whole_room blend=0.6} The wide cascades cover the whole room in a sliver of their depth, so it all reads as one flat bright patch.

{@ delay=0.4} P is ringed in the slices that hold it; the shadow test compares P's own depth with the depth stored under that ring.

{@ image=cm imageMark=""} {@ animate=[{"path": {"params": ["sunYaw"], "through": [[75]], "seconds": "sentence"}}]} {@ callout=shadow-caster-state} Every shadow-casting mesh is drawn into it with only a depth test and a small depth bias, every frame, so the cascades follow the camera and the sun.

## c5-cascades | 3d | Four cascades, split near-dense
{@ image="" demo=cascades callout="" panel=values cameraDrift=0.5 labels=0 cascadeCamera=1 cascades=1 cascadesShown=4 shadowFar=30 sunYaw=35}
{@ camera.target=0,0,0 camera.distance=26 camera.pitch=48 camera.yaw=36}

{@ callout=cascade-splits} The cascades cut the camera's view into four slices of depth, near ones small and sharp, far ones wide and coarse.

{@ callout=cascade-defaults} The defaults are four cascades and a shadow far distance of one hundred metres; here it is shortened to thirty, so all four fit in view.

{@split> cascadeSplitRatio=0..1} {@ callout=cascade-splits} Each split blends a uniform split with a logarithmic one by a ratio: zero is uniform, one logarithmic, and the default is zero point nine.{<split}

{@ cascadeSplitRatio=0.9 blend=1.5} {@ callout=cascade-splits} With the default ratio and this camera's near plane of zero point one metres, these thirty metres split at about one point one, three point one, and eight point seven metres; the full hundred would split at three, seven point nine, and twenty-three point five.

{@ image=cm imageFrom=cascadeMap image.cascade=4 image.from=0.9 imageCorner=bottom-left imageTitle=views.cascade_map} {@ cascadeColors=1} {@ callout=cascade-select} O3DE's debug colouring tints each pixel by its cascade: red, green, blue, then yellow.

## c5-fit | 3d | A sphere around each slice, snapped to texels
{@ image="" demo=cascades callout="" panel=values cameraDrift=0.4 labels=0 cascadeCamera=1 cascades=1 cascadesShown=4 shadowFar=30 cascadeSpheres=0 cascadeBoxes=0 cascadeColors=0}
{@ camera.target=0,0,0 camera.distance=26 camera.pitch=48 camera.yaw=36}

{@sph> cascadeSpheres=0..1} {@ callout=cascade-sphere-fit} Each slice is wrapped in a bounding sphere, so its size does not change as the camera turns.{<sph}

{@box> cascadeBoxes=0..1} {@ callout=cascade-sphere-fit} The cascade's box in the light's view is that sphere's cube, stretched back toward the sun, so that casters between the sun and the view, even behind the camera, still land in the map.{<box}

{@ animate=[{"path": {"params": ["observerYaw"], "through": [[25], [-25], [0]], "seconds": "sentence"}}]} {@ callout=cascade-snap} The box is snapped to whole shadow-map texels, so as the camera moves, the map's texels stay fixed in the world and shadow edges do not shimmer.

{@ callout=cascade-sphere-fit} A cascade's texel is its sphere's diameter over one thousand and twenty-four, so the far cascades are much coarser.

## c5-lookup | 3d | P in its cascade: offset, biased, compared
{@ image="" demo=cascades callout="" panel=hidden cameraDrift=0.4 labels=0 cascadeCamera=0 shadowFar=100}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 22, 5.0], [6, 18, 5.4], [18, 24, 4.8]], "seconds": "beat"}}]}

{@ panel=code code=fullscreen-shadow highlight=position} {@ callout=fullscreen-shadow} To shade P, the shadow pass first rebuilds P's position from the depth.

{@ panel=code code=fullscreen-shadow highlight=offset} {@ callout=shadow-normal-offset} It moves P along its normal by the normal bias times two over the map size: two point five times two over one thousand and twenty-four, about five millimetres.

{@ panel=code code=fullscreen-shadow highlight=range} {@ image=cm imageFrom=cascadeMap image.cascade=4 image.from=0.9 imageCorner=bottom-left imageTitle=views.cascade_map} {@ callout=cascade-select} It takes the first cascade whose map holds P at least one and a half texels in from the edge.

{@ panel=code code=fullscreen-shadow highlight=compare} {@ callout=shadow-coords} P's depth from the sun is moved a millimetre and a half toward it, then compared with the depth stored at P's texel.

{@ callout=shadow-is-shadowed} If something nearer the sun was stored there, P is in shadow; with the default filter, None, the answer is exactly zero or one.

## c5-bias | 3d | Biases against acne; the slope bias is off
{@ image="" demo=cascades callout="" panel=hidden cameraDrift=0.3 labels=0 cascadeCamera=0 shadowFar=100 normalShadowBias=2.5}
{@ camera.target=0.8,0,0.8 camera.distance=3.2 camera.pitch=40 camera.yaw=30}

{@ image=sb imageFrom=shadowBuffer image.blurred=0 imageCorner=top-left imageTitle=views.shadow_buffer} {@ callout=shadow-caster-state} Without a bias, a surface can shadow itself in a pattern of stripes, called acne.

{@ delay=0.4} {@ image=sb imageMark=0.62,0.08,1,0.62 imageMarkText=views.mark_lit blend=0.6} The shadow buffer view shows each pixel's result, one byte per pixel: white where the sun is seen.

{@ delay=0.4} {@ image=sb imageMark=0,0,0.5,0.42 imageMarkText=views.mark_shadow blend=0.6} Black where it is not.

{@ delay=0.4} Each pixel is one test against the cascade, before any blur, so its edges follow the shadow map's texels.

{@ delay=0.4} Acne would show here as black stripes across faces the sun lights, where a surface fails the test against itself.

{@ image=sb imageMark=""} {@nb> normalShadowBias=0..6} {@ callout=shadow-normal-offset} The normal offset lifts the lookup off the surface, and a larger offset lets shadows detach from their casters' feet.{<nb}

{@ normalShadowBias=2.5 blend=1.5} {@ callout=shadow-slope-bias-off} A slope bias is computed for each cascade and uploaded, but the shader's switch for it is off, so it never acts.

{@ callout=shadow-normal-offset} The normal offset depends only on the map's size, not on the cascade, so a far cascade, many times coarser, gets the same five millimetres: read from the code.

## c5-fullscreen | 3d | Opaque pixels read a shadow buffer, not the cascades
{@ image="" demo=cascades callout="" panel=hidden cameraDrift=0.6 labels=0 cascadeCamera=0 shadowFar=100}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[28, 26, 7.5], [10, 20, 8.0], [26, 28, 7.2]], "seconds": "beat"}}]}

{@ image=sb imageFrom=shadowBuffer image.blurred=0 imageCorner=top-left imageTitle=views.shadow_buffer} {@ callout=fullscreen-shadow} All of this runs in a full-screen pass, once per pixel, before the forward pass: the result is one eight bit shadow value per pixel.

{@ callout=fullscreen-shadow-normal} The normal it offsets along is not the mesh's: it is rebuilt from the depths of the pixel's neighbours, so it is the flat face's normal.

{@ callout=shadow-use-fullscreen} Every opaque and cut-out material reads this buffer in the forward pass; transparent ones look up the cascades themselves.

## c5-blur | 3d | A binary shadow, softened in screen space
{@ image="" demo=cascades callout="" panel=hidden cameraDrift=0.3 labels=0 cascadeCamera=0 shadowFar=100 shadowBlur=0}
{@ camera.target=-0.6,0.2,-0.4 camera.distance=3.4 camera.pitch=34 camera.yaw=24}

{@ image=raw imageFrom=shadowBuffer image.blurred=0 imageCorner=top-left imageTitle=views.shadow_buffer} {@ callout=shadow-filter-default} The default filter is None, so the raw buffer holds hard zeros and ones, stepped at the cascade's texels.

{@ image=blur imageFrom=shadowBuffer image.blurred=1 imageCorner=top-left imageTitle=views.shadow_buffer} {@bl> shadowBlur=0..1} {@ callout=shadow-blur-settings} A depth-aware blur then runs eight pixels to each side, vertically and then horizontally, mixing each pixel with its neighbour by two thirds.{<bl}

{@ delay=0.4} {@ image=raw imageMark=0.3,0.2,0.72,0.62 imageMarkText=views.mark_stepped blend=0.6} The left view is the raw buffer: zeros and ones, its edges stepped at the cascade's texels.

{@ delay=0.4} {@ image=blur imageMark=0.3,0.2,0.72,0.62 imageMarkText=views.mark_softened blend=0.6} The right one is after the blur: the same edges, softened into greys.

{@ delay=0.4} A grey pixel is now partly in shadow, and the forward pass multiplies the sun's light by that fraction.

{@ image=raw imageMark=""} {@ image=blur imageMark=""} {@ callout=shadow-blur-settings} Where the depth's slope changes, at an object's edge, the mixing stops, so a shadow does not bleed onto a surface behind it.

{@ callout=pcf-squared} Percentage-closer filtering and exponential shadow maps exist as options; the lab does not port them, and read from the code, the PCF kernels square their result.

## c5-local | 3d | Spot and point shadows live in an atlas
{@ image="" demo=cascades callout="" panel=hidden cameraDrift=0.8 labels=0 cascadeCamera=0 shadowFar=100}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[28, 26, 7.5], [12, 22, 7.8], [24, 28, 7.2]], "seconds": "beat"}}]}

{@ callout=shadow-atlas} Spot and disk lights, and sphere lights, can cast shadows too, but they are off by default; their maps share one atlas of depth slices.

{@ callout=point-shadow-faces} A sphere light needs six maps, one per face of a cube around it, each with a ninety-one degree field of view so the faces overlap at their seams.

{@ callout=shadow-atlas} Maps of different sizes are packed into the atlas like a quadtree, at least two hundred and fifty-six texels a side.

## c5-recap | 3d | The shadow, ready before the light
{@ image="" demo=cascades callout="" panel=hidden cameraDrift=1 labels=0 cascadeCamera=0 shadowFar=100}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[28, 26, 7.5], [8, 20, 8.2], [24, 30, 7.2]], "seconds": "beat"}}, {"path": {"params": ["sunYaw"], "through": [[55], [30]], "seconds": "beat"}}]}

{@ image=blur imageFrom=shadowBuffer image.blurred=1 imageCorner=top-left imageTitle=views.shadow_buffer} So the sun's shadow is computed once per pixel, from four cascades, before any light is shaded.

The forward pass only reads one number per pixel, and the blur makes a hard compare look soft.

{@ delay=0.4} The direct light is now complete; the rest of a pixel's light comes from everything around it, and the forward pass left it out.

## c5-sources | 2d | Where to read
sources: cascade-image, shadow-caster-state, cascade-splits, cascade-defaults, cascade-sphere-fit, cascade-snap, fullscreen-shadow, shadow-normal-offset, cascade-select, shadow-coords, shadow-is-shadowed, shadow-slope-bias-off, fullscreen-shadow-normal, shadow-blur-settings, shadow-use-fullscreen, shadow-atlas, point-shadow-faces

These are the files and lines this chapter followed, in reading order.

# 6 | Indirect light: the sky and the probes | 360 s
summary: Image-based lighting from the global skylight's two cubemaps (cosine-convolved diffuse, GGX-prefiltered specular with mip = 5 x roughness), the split sum, reflection probes with their stencil, weights, and parallax correction, and, in passing, SSR and the diffuse probe grid.
sources: diffuse-global-stencil, probe-bake-capture, probe-defaults, reflection-clear, ibl-srg, ibl-diffuse-lookup, diffuse-global, diffuse-composite, specular-preset, cubemap-ggx, roughness-mip, reflection-global, probe-stencil, probe-blend-weight, parallax-obb, probe-render-inner, ssr-default-off

## c6-two | 3d | Two cubemaps: one for diffuse, one for specular
{@ image="" demo=ibl callout="" panel=hidden cameraDrift=0.6 labels=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 10, 3.6], [4, 6, 3.9], [18, 14, 3.4]], "seconds": "beat"}}]}

The sky's light reaches the frame through two cubemaps, made from the sky image when the assets are built.

{@ image=spec imageFrom=cubemap image.mip=0 imageCorner=top-left imageTitle=views.cubemap_specular} {@ image=spec image.mip=5 blend=6} {@ callout=specular-preset} The specular cubemap has six mips, by default two hundred and fifty-six texels a side at the top, each mip prefiltered for a rougher surface.

{@ image=diff imageFrom=cubemap image.diffuse=1 imageCorner=top-right imageTitle=views.cubemap_diffuse} {@ callout=diffuse-preset} The diffuse cubemap is the sky convolved with a cosine over the whole hemisphere: one small mip, the average light arriving at each normal.

{@ delay=0.4} {@ image=spec imageMark=0.25,0,0.5,0.34 imageMarkText=views.mark_up_face blend=0.6} Each cube is unfolded into a cross: the face looking up is on top.

{@ delay=0.4} {@ image=spec imageMark=0,0.33,1,0.67 imageMarkText=views.mark_sides blend=0.6} The four faces looking around the horizon are in a row: there are the trees and the field.

{@ delay=0.4} {@ image=spec imageMark=0.25,0.66,0.5,1 imageMarkText=views.mark_down_face blend=0.6} The face looking down is below it.

{@ delay=0.4} A direction from the cube's centre picks a point on one face, and every lookup reads the cube that way.

{@ delay=0.4} The specular cube on the left shows its sharpest mip; the diffuse one on the right is the same sky averaged over half a sphere, so it is almost flat.

{@ image=spec imageMark=""} {@ex> iblExposure=-2..1} {@ callout=ibl-srg} Both are bound for the whole scene, with one exposure and one orientation; the lab builds them from its own sky at those sizes.{<ex}

## c6-diffuse | 3d | Diffuse IBL: one lookup by the normal
{@ image="" demo=room callout="" panel=hidden cameraDrift=0.6 labels=0 forwardStage=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 18, 5.8], [2, 14, 6.2], [16, 20, 5.6]], "seconds": "beat"}}]}

{@ image=diff imageFrom=cubemap image.diffuse=1 imageCorner=top-left imageTitle=views.cubemap_diffuse} {@ callout=ibl-diffuse-lookup} For P, diffuse image-based light is a single lookup of that cubemap along P's normal.

The ring in the diffuse cross marks that direction.

{@dd> forwardStage=0..1} {@ callout=diffuse-global} A full-screen pass multiplies it by the albedo target, which already holds the diffuse response and occlusion, and adds it to the diffuse target.{<dd}

{@ image=st imageFrom=chainTarget image.target=6 imageCorner=top-right imageTitle=views.stencil} {@ callout=diffuse-global-stencil} It runs only on pixels whose stencil has the bit one hundred and twenty-eight.

{@ callout=diffuse-composite} In a project made from O3DE's default template, the diffuse probe grid gem replaces that pass with its own composite, which applies the same sky light wherever no probe grid covers the pixel.

## c6-specular | 3d | Specular IBL: the reflection, a mip chosen by roughness
{@ image="" demo=ibl callout="" panel=values cameraDrift=0.4 labels=0 sphereRoughness=0.05}
{@ inspectX=-1.05 inspectY=1.24 inspectZ=-0.55 inspectNX=0 inspectNY=1 inspectNZ=0 inspectObject=5 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 10, 3.6], [6, 8, 3.8], [18, 12, 3.5]], "seconds": "beat"}}]}

{@ image=spec imageFrom=cubemap image.mip=-1 imageCorner=top-left imageTitle=views.cubemap_specular} {@ callout=reflection-global} For specular, the pass reflects the view about the normal, and looks up the specular cubemap along that direction.

The specular cross now shows the mip P reads, ringed where its reflected ray points, so it blurs as P's roughness rises.

{@m> sphereRoughness=0.05..0.95} {@ callout=roughness-mip} It reads the mip roughness times five, between two mips at once, so a rougher surface reads a blurrier sky.{<m}

{@ callout=cubemap-ggx} Mip m was prefiltered with the GGX lobe of roughness m over five, with two hundred and fifty-six samples a texel, so the mip a surface reads matches its own lobe.

{@ sphereRoughness=0.3 blend=1.5} The ringed area is the sky that this point of the sphere reflects.

## c6-splitsum | 3d | The split sum: the cube times F zero A plus B
{@ image="" demo=ibl callout="" panel=values cameraDrift=0.4 labels=0 sphereRoughness=0.3}
{@ inspectX=-1.05 inspectY=1.24 inspectZ=-0.55 inspectNX=0 inspectNY=1 inspectNZ=0 inspectObject=5 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 10, 3.6], [6, 8, 3.8], [18, 12, 3.5]], "seconds": "beat"}}]}

{@ image=lut imageFrom=brdfLut imageCorner=top-left imageTitle=views.brdf_lut} {@sr> sphereRoughness=0.1..0.8} {@ callout=reflection-global} The prefiltered colour is multiplied by F zero times A plus B, from the look-up texture made at start-up.{<sr}

In the look-up view, the ring is P's texel: its red is A, its green is B.

{@ callout=reflection-global} That splits an integral over the hemisphere into two pieces computed ahead: the light, in the cube, and the reflection's response, in the texture.

{@ex2> iblExposure=0..1} {@ callout=reflection-global-weight} The result is scaled by two to the exposure and by the specular occlusion from the albedo target's fourth channel.{<ex2}

## c6-probe | 3d | A reflection probe: a local cube for a box of space
{@ image="" demo=reflections callout="" panel=hidden cameraDrift=0.6 labels=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 20, 6.4], [4, 14, 6.8], [16, 22, 6.2]], "seconds": "beat"}}]}

The sky's cubemap was made outside the room, so the floor reflects a sky it should not see; a reflection probe fixes that.

{@ probeCubemap=0} {@cb> probeCubemap=0..1} {@ callout=probe-bake-capture} A probe renders the room around it into its own cubemap, then prefilters it like the sky's.{<cb}

{@ probeCubemap=0 blend=1} {@bx> probeBoxes=0..1} {@ callout=probe-defaults} It has an outer box, where its reflection fades in, and an inner box, where it replaces everything else.{<bx}

{@ viewMode=probeStencil} {@ callout=probe-stencil} A stencil pass counts, per pixel, how many inner boxes hold its surface: three, plus one for each.

{@in> probeInnerScale=0.2..0.9} {@ viewMode=probeWeight} {@ callout=probe-blend-weight} Between the boxes, a weight ramps from zero at the outer box to one at the inner, and the weights of all probes add up in their own target.{<in}

## c6-parallax | 3d | Parallax correction: aim where the ray meets the box
{@ image="" demo=reflections callout="" panel=hidden cameraDrift=0.4 labels=0 viewMode=final probeParallax=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 20, 6.4], [6, 16, 6.7], [16, 22, 6.2]], "seconds": "beat"}}]}

{@ callout=parallax-obb} A cubemap holds the room as seen from one point, so a reflection read straight from it lands in the wrong place on a floor far from that point.

{@px> probeParallax=0..1} {@ callout=parallax-obb} Parallax correction traces the reflection to the probe's outer box, and looks up the cube toward that hit instead: the reflections slide into line with the room.{<px}

{@fr> floorRoughness=0.05..0.4} {@ callout=parallax-obb} It assumes the room is the box and the probe is at its centre, which is exact only for walls on the box.{<fr}

## c6-blend | 3d | Probes and the sky blend by weight
{@ image="" demo=reflections callout="" panel=values cameraDrift=0.4 labels=0 viewMode=final probeParallax=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 20, 6.4], [6, 16, 6.7], [16, 22, 6.2]], "seconds": "beat"}}]}

{@in2> probeInnerScale=0.9..0.2} {@ callout=reflection-global-weight} The sky's reflection is drawn first, weighted by one minus the probes' total weight.{<in2}

{@ callout=probe-render-outer} Each probe then adds its own reflection, its weight divided by the total when probes overlap.

{@ probeInnerScale=0.55 blend=1.5} {@ callout=probe-render-inner} Inside an inner box, the probe overwrites the result at full weight; with nested probes, the smallest is drawn last and wins.

## c6-findings | 3d | Read from the code: what the probes leave out
{@ image="" demo=reflections callout="" panel=hidden cameraDrift=0.6 labels=0 viewMode=final}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 20, 6.4], [2, 14, 6.8], [16, 22, 6.2]], "seconds": "beat"}}]}

{@ callout=probe-render-outer} The probe passes are not given the albedo target, so the specular occlusion in its fourth channel never reaches a probe's reflection; only the sky's is occluded.

{@ callout=reflection-clear} The reflection target is cleared to a grey of zero point four, and a pixel with image-based light switched off inside an inner box keeps that grey and has it added to its specular: both read from the code, not run in the engine.

## c6-ssr-dpg | 3d | Two more sources, off or out of reach
{@ image="" demo=room callout="" panel=hidden cameraDrift=1 labels=0 forwardStage=3 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 20, 6.2], [0, 14, 6.6], [16, 18, 6.0]], "seconds": "beat"}}]}

{@ callout=ssr-default-off} Screen-space reflections exist, at half resolution with ninety-six steps per ray, but they are off unless a level adds their component.

{@ callout=diffuse-composite} The diffuse probe grid traces rays from probes to give rooms bounced diffuse light; its update shaders ship only as compiled binaries, so only its composite can be read.

## c6-recap | 3d | Light from everything around
{@ image="" demo=ibl callout="" panel=hidden cameraDrift=1 labels=0 sphereRoughness=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 10, 3.6], [0, 6, 3.9], [16, 14, 3.4]], "seconds": "beat"}}, {"path": {"params": ["sphereRoughness"], "through": [[0.7], [0.15]], "seconds": "beat"}}]}

{@ image=spec imageFrom=cubemap image.mip=-1 imageCorner=top-left imageTitle=views.cubemap_specular} So the sky's light is prefiltered once, into cubes and a texture, and each pixel then pays for only two lookups and a multiply.

Probes replace the sky where a room needs its own reflection, blended by weights the stencil and their boxes decide.

{@ delay=0.4} Those lookups assume nothing blocks the sky; near the floor, something does, and that is the next part.

## c6-sources | 2d | Where to read
sources: ibl-srg, specular-preset, diffuse-preset, ibl-diffuse-lookup, diffuse-global, diffuse-composite, reflection-global, roughness-mip, cubemap-ggx, reflection-global-weight, probe-stencil, probe-blend-weight, parallax-obb, probe-render-outer, probe-render-inner, ssr-default-off

These are the files and lines this chapter followed, in reading order.

# 7 | Occlusion: what the sky cannot reach | 270 s
summary: Screen-space ambient occlusion from the depth buffer alone: 52 samples per half-resolution pixel, a radius set as a fraction of the screen, a gamma of 2.2, a depth-aware blur and upsample, and a multiply into all diffuse light; and the material's own occlusion maps.
sources: ssao-defaults, ssao-default, ssao-order, ssao-compute, ssao-normal, ssao-view-position, ssao-local-samples, ssao-accumulate, ssao-radius, ssao-format, depth-upsample, ssao-modulate, occlusion-input, occlusion-finalize

## c7-why | 3d | SSAO darkens creases from depth alone
{@ image="" demo=ssao callout="" panel=hidden cameraDrift=0.6 labels=0 viewMode=final ssao=0}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ camera.target=-0.25,0.15,-1.55 camera.distance=1.5 camera.pitch=24 camera.yaw=-28}

{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 18, 2.4], [8, 26, 2.8]], "seconds": "sentence"}}]} Image-based light assumes the whole sky is visible; near the teapot's foot, the floor and the pot hide much of it.

{@on> ssao=0..1} {@ callout=ssao-default} Screen-space ambient occlusion is on by default: it darkens where nearby geometry hides the sky, using nothing but the depth buffer.{<on}

{@ image=raw imageFrom=ssaoStage image.stage=1 imageCorner=top-left imageTitle=views.ssao_stage} {@ callout=ssao-order} It runs after the forward pass and the sky's light, on a half-resolution copy of the linear depth, into an eight bit target.

{@ delay=0.4} {@ image=raw imageMark=0,0.5,0.42,1 imageMarkText=views.mark_unoccluded blend=0.6} The view shows that target, one byte per pixel at half resolution: white where nothing occludes.

{@ delay=0.4} {@ image=raw imageMark=0.3,0.22,1,0.48 imageMarkText=views.mark_crease blend=0.6} It is darker in the creases, like where the wall meets the floor, because the depth samples around a pixel there lie in front of its surface.

{@ delay=0.4} The darker a pixel, the less of the sky's diffuse light it keeps.

## c7-samples | 3d | Fifty-two samples around P, weighted by distance
{@ image="" demo=ssao callout="" panel=values cameraDrift=0.3 labels=0 viewMode=final ssao=1 ssaoSamples=0}
{@ inspectX=-0.25 inspectY=0.0 inspectZ=-1.40 inspectNX=0 inspectNY=1 inspectNZ=0 inspectObject=0 pickShown=1}
{@ camera.target=-0.3,0.2,-1.55 camera.distance=1.6 camera.pitch=26 camera.yaw=22}

{@ animate=[{"path": {"params": ["camera.yaw"], "through": [[34]], "seconds": "sentence"}}]} {@ callout=ssao-view-position} We follow a floor point at the teapot's foot: for its half-resolution pixel, the shader rebuilds its position in view space from its depth.

{@ image=n imageFrom=chainTarget image.target=4 image.channel=5 imageCorner=top-left imageTitle=views.target_normal} {@ callout=ssao-normal} Its normal is rebuilt from the neighbours' depths too, taking on each axis the side whose depth differs less, so normal maps never reach SSAO.

The normal view reads as before: colour is direction, green up, blue toward the camera.

{@loc> ssaoSamples=0..12} {@ imageHide=n} {@ callout=ssao-local-samples} First come twelve fixed taps, four and eight pixels away, in a cross and its diagonals.{<loc}

{@out> ssaoSamples=12..52} {@ callout=ssao-compute} Then forty more along a spiral, ten rounds of four, shared between the sixteen pixels of a four by four block.{<out}

{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[40, 14], [10, 30]], "seconds": "sentence"}}]} {@ callout=ssao-accumulate} Each sample counts by how far it rises above the point's surface, the cosine against the normal, and weighs one over its distance squared plus zero point zero seven: the samples on the teapot occlude, those on the floor do not.

{@ callout=ssao-accumulate} The occlusion is the weighted average of those cosines; there is no range check, so a far sample only fades through its weight.

## c7-radius | 3d | The radius is a share of the screen, not metres
{@ image="" demo=ssao callout="" panel=values cameraDrift=0.3 labels=0 viewMode=final ssao=1 ssaoSamples=52}
{@ inspectX=-0.25 inspectY=0.0 inspectZ=-1.40 inspectNX=0 inspectNY=1 inspectNZ=0 inspectObject=0 pickShown=1}
{@ camera.target=-0.3,0.2,-1.55 camera.distance=1.6 camera.pitch=26 camera.yaw=22}

{@ callout=ssao-radius} The spiral's radius is zero point zero five of the half-resolution image's height: at ten eighty, twenty-seven half-resolution pixels.

{@ animate=[{"path": {"params": ["camera.distance"], "through": [[4.5]], "seconds": "sentence"}}]} {@ callout=ssao-radius} So it does not scale with distance: as the camera pulls back, the same disc on screen reaches much farther across the room.

{@rad> ssaoRadius=0.02..0.1} {@ callout=ssao-defaults} A larger radius spreads the darkening wider and softer.{<rad}

## c7-output | 3d | One minus AO, to the power 2.2, blurred and upsampled
{@ image="" demo=ssao callout="" panel=values cameraDrift=0.4 labels=0 viewMode=final ssao=1 ssaoSamples=0 ssaoRadius=0.05}
{@ camera.target=-0.3,0.3,-1.6 camera.distance=2.6 camera.pitch=22 camera.yaw=18}

{@ image=raw imageFrom=ssaoStage image.stage=1 imageCorner=top-left imageTitle=views.ssao_stage} {@st> ssaoStrength=0.5..1.5} {@ callout=ssao-compute} The pass stores one minus the occlusion times the strength, raised to the power two point two, in eight bits.{<st}

{@ ssaoStrength=1 blend=1} {@ image=blur imageFrom=ssaoStage image.stage=2 imageCorner=top-left imageTitle=views.ssao_stage} {@bo> ssaoBlur=0..1} {@ callout=blur-step} A depth-aware blur smooths the structured noise of the fixed pattern, eight pixels each way, stopping at depth edges.{<bo}

{@ image=up imageFrom=ssaoStage image.stage=3 imageCorner=top-left imageTitle=views.ssao_stage} {@ callout=depth-upsample} An upsample back to full resolution weighs the four nearest values by how close their depth is, so the dark does not bleed across a silhouette.

{@ delay=0.4} {@ image=raw imageMark=0,0,1,1 imageMarkText=views.mark_raw blend=0.6} The first view is the raw half-resolution result, with the fixed pattern's noise.

{@ delay=0.4} {@ image=blur imageMark=0,0,1,1 imageMarkText=views.mark_blurred blend=0.6} The next is after the depth-aware blur: smoother, with its edges still sharp.

{@ delay=0.4} {@ image=up imageMark=0,0,1,1 imageMarkText=views.mark_upsampled blend=0.6} And the last is the full-resolution upsample, the one the diffuse light is multiplied by.

## c7-where | 3d | Finding: SSAO multiplies all diffuse light, direct too
{@ image="" demo=room callout="" panel=hidden cameraDrift=0.4 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 forwardStage=2}
{@ camera.target=-0.3,0.3,-1.6 camera.distance=2.6 camera.pitch=22 camera.yaw=18}

{@ao> forwardStage=2..3} {@ callout=ssao-modulate} The AO multiplies the whole diffuse target, which holds the sun's direct light as well as the sky's.{<ao}

{@ animate=[{"path": {"params": ["sunYaw"], "through": [[80]], "seconds": "sentence"}}]} {@ callout=ssao-modulate} So even the sunlit side of a crease darkens; the specular target, and the emission in it, are left alone.

{@ callout=ssao-modulate} Read from the pass connections, this is not ambient-only occlusion: O3DE applies it to everything diffuse.

## c7-material | 3d | A material's occlusion, an exponent on a map
{@ image="" demo=surface callout="" panel=values cameraDrift=0.5 labels=1 sphereOcclusion=1 diffuseOcclusionFactor=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[4, 8, 3.1], [-8, 6, 3.3], [6, 10, 3.0]], "seconds": "beat"}}]}

{@sp> specularOcclusionFactor=0..2} {@ callout=occlusion-input} A material can also carry occlusion maps, one for diffuse light and one for specular, each raised to the power of its factor.{<sp}

{@f> diffuseOcclusionFactor=0..2} {@ callout=occlusion-finalize} They multiply the diffuse and specular light at the end of the forward pass, direct light included, and the albedo target carries them to the sky's passes.{<f}

{@ callout=forward-ibl-ao-twice} Where the sky's light is added inside the forward pass, for transparent materials and Skin, the diffuse occlusion is applied to it twice: once on its way in, and once at the end.

## c7-recap | 3d | Occlusion, from two sources
{@ image="" demo=ssao callout="" panel=hidden cameraDrift=1 labels=0 viewMode=final ssao=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 22, 3.0], [4, 18, 3.4], [16, 24, 2.8]], "seconds": "beat"}}]}

{@ image=up imageFrom=ssaoStage image.stage=3 imageCorner=top-left imageTitle=views.ssao_stage} So occlusion comes from the screen, every frame, and from the material's maps, both as multipliers.

The screen's is cheap and needs no data, at the cost of darkening direct light and ignoring what lies off screen.

{@ delay=0.4} So far light has only bounced off surfaces; the next part lets it pass through them.

## c7-sources | 2d | Where to read
sources: ssao-default, ssao-order, ssao-view-position, ssao-normal, ssao-local-samples, ssao-compute, ssao-accumulate, ssao-radius, ssao-format, blur-step, depth-upsample, ssao-modulate, occlusion-input, occlusion-finalize, forward-ibl-ao-twice

These are the files and lines this chapter followed, in reading order.

# 8 | Light through surfaces | 280 s
summary: Transmission in the Enhanced and Skin lighting models (thick objects after Barré-Brisebois and Bouchard, thin ones after Jimenez), the shadow-map thickness it reads and its units, what a leaf is in O3DE, screen-space subsurface scattering, and the Skin and Eye models.
sources: skin-lighting-defines, sss-sample-count, transmission-option, back-lighting, transmission-params, transmission-kernel, shadow-thickness, shadow-thickness-camera-span, transmission-finalize, foliage-sample-material, double-sided-cull, back-face-normal, vertex-normal-unflipped, sss-pack, sss-kernel, skin-diffuse, eye-direct-lighting

## c8-which | 3d | Only Enhanced and Skin let light through
{@ image="" demo=transmission callout="" panel=values cameraDrift=0.5 labels=0 sunLux=40 iblExposure=1 skyboxExposure=2 exposure=0.35 sunYaw=10 sunPitch=22 sphereLight=0 walls=0 teapotTransmission=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 6, 1.8], [48, 4, 2.0], [40, 8, 1.7]], "seconds": "beat"}}]}

So far light has only bounced off surfaces; the next part lets it pass through them.

{@ animate=[{"path": {"params": ["sunYaw"], "through": [[30], [10]], "seconds": "sentence"}}]} {@ callout=transmission-option} Transmission is compiled only into the Enhanced and Skin lighting models; StandardPBR, the default, has none.

{@md> teapotTransmission=0..1} {@ callout=transmission-params} In an EnhancedPBR material it is off by default, mode None; the two modes are thick object and thin object.{<md}

## c8-thick | 3d | Thick objects: light from behind, scattered toward the eye
{@ image="" demo=transmission callout="" panel=values cameraDrift=0.4 labels=0 sunLux=40 iblExposure=1 skyboxExposure=2 exposure=0.35 sunYaw=10 sunPitch=22 sphereLight=0 walls=0 teapotTransmission=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 6, 1.8], [48, 4, 2.0], [38, 8, 1.7]], "seconds": "beat"}}]}

{@ animate=[{"path": {"params": ["sunYaw"], "through": [[-20], [30], [10]], "seconds": "sentence"}}]} {@ panel=code code=transmission highlight=thick} {@ callout=back-lighting} The thick model, after Barré-Brisebois and Bouchard, brightens where the eye looks toward the light through the object.

{@pw> transmissionPower=2..16} {@ callout=back-lighting} The view's alignment with the light, bent a little by the normal, is raised to a power: a higher power gathers the glow toward the light's direction.{<pw}

{@ transmissionPower=6 blend=1} {@th> transmissionThickness=0.05..0.9} {@ callout=back-lighting} It fades with thickness as e to the minus thickness times four, and reaches zero at a thickness of one.{<th}

## c8-thin | 3d | Thin objects: the back face's light, through a kernel
{@ image="" demo=transmission callout="" panel=values cameraDrift=0.4 labels=0 sunLux=40 iblExposure=1 skyboxExposure=2 exposure=0.35 sunYaw=10 sunPitch=22 sphereLight=0 walls=0 teapotTransmission=2 transmissionThickness=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 6, 1.8], [48, 4, 2.0], [38, 8, 1.7]], "seconds": "beat"}}]}

{@ animate=[{"path": {"params": ["sunPitch"], "through": [[10], [22]], "seconds": "sentence"}}]} {@ panel=code code=transmission highlight=thin} {@ callout=back-lighting} The thin model, after Jimenez, takes the light falling on the far side, the albedo times the back face's N dot L.

{@tk> transmissionThickness=0.1..1} {@ callout=transmission-kernel} It attenuates that by a kernel of the measured thickness and the scatter distance, per colour channel.{<tk}

{@ transmissionThickness=0.5 blend=1} {@ callout=transmission-params} The default scatter distance is eight, two point one six, and one point zero four, so red passes much farther than blue: the glow turns orange.

{@sw> sunYaw=10..60} {@ callout=back-lighting} As the sun moves from behind the teapot to its side, the back face turns away from it and the glow fades.{<sw}

## c8-thickness | 3d | Thickness from the shadow map, and its units
{@ image="" demo=transmission callout="" panel=values cameraDrift=0.3 labels=0 sunLux=40 iblExposure=1 skyboxExposure=2 exposure=0.35 sunYaw=10 sunPitch=22 sphereLight=0 walls=0 teapotTransmission=2}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 6, 1.8], [48, 4, 2.0], [38, 8, 1.7]], "seconds": "beat"}}]}

{@ image=cm imageFrom=cascadeMap image.cascade=4 image.from=0.9 imageCorner=bottom-left imageTitle=views.cascade_map} {@ callout=shadow-thickness} The thickness comes from the sun's cascade: the depth between the shaded point and the first surface the sun meets.

In the cascade view, the ring is the teapot's point: the depth stored there is the side facing the sun, and the point's own depth is the far side.

{@ callout=shadow-thickness-camera-span} That difference is in the cascade's own depth range, but the code multiplies it by the camera's depth span instead, so the result is not in metres: a finding read from the code.

{@ callout=shadow-thickness} For a flat leaf with the default biases, the normal offset and the thin mode's shrink nearly cancel, so the measured thickness is about zero and the glow runs at full strength.

{@ts> transmissionScale=0..1} {@ callout=transmission-finalize} The transmitted light is added to the diffuse light after the material's occlusion, tinted, so that occlusion never darkens it.{<ts}

## c8-foliage | 3d | A leaf in O3DE is a material, not a lighting model
{@ image="" demo=transmission callout="" panel=values cameraDrift=0.5 labels=0 sunLux=40 iblExposure=1 skyboxExposure=2 exposure=0.35 sunYaw=10 sunPitch=22 sphereLight=0 walls=0 teapotTransmission=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 6, 1.8], [48, 4, 2.0], [40, 8, 1.7]], "seconds": "beat"}}]}

{@ callout=foliage-sample-material} O3DE has no foliage shading of its own; its vegetation system places ordinary meshes with whatever material they carry.

{@ animate=[{"path": {"params": ["sunYaw"], "through": [[40], [10]], "seconds": "sentence"}}]} {@ callout=cutout-clip} A plain leaf is StandardPBR, cut out by its alpha and drawn double-sided: lit from behind, it stays dark, because Standard has no transmission.

{@lf> teapotTransmission=0..2} {@ callout=foliage-sample-material} O3DE's own leaf-like test material is EnhancedPBR, double-sided, in thin mode with a green tint; the lab shows the model on its teapot.{<lf}

{@ callout=vertex-normal-unflipped} On a back face, the shading normal is flipped, but the vertex normal that transmission and the shadow offset use is not.

## c8-sss | 3d | Subsurface scattering: a blur of the diffuse light
{@ image="" demo=transmission callout="" panel=hidden cameraDrift=0.5 labels=0 sunLux=40 iblExposure=1 skyboxExposure=2 exposure=0.35 sunYaw=60 sunPitch=40 sphereLight=0 walls=1 teapotTransmission=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 6, 1.8], [32, 5, 1.8], [38, 8, 1.7]], "seconds": "beat"}}]}

{@ callout=enhanced-draw-list} Enhanced and Skin materials with scattering on write one more target, the scatter distance, and pack a factor and a quality into the diffuse target's alpha.

{@ callout=sss-pack} The pack is the factor's byte times two hundred and fifty-six plus the quality's byte, an integer stored in a half float, which holds integers exactly only up to two thousand and forty-eight.

{@ callout=sss-kernel} A compute pass then blurs the diffuse light with Burley's normalised diffusion profile, two hundred samples a pixel at full quality, eighty at the default quality of zero point four, its footprint set by the scatter distance in millimetres.

{@ callout=sss-sample-count} The lab does not run this pass: its targets carry the packed alpha of minus one, no scattering, for every surface.

## c8-skin | 3d | Skin and Eye
{@ image="" demo=diffuse callout="" panel=values cameraDrift=0.4 vectors=0 lobe=0 viewMode=final}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 6, 3.1], [-10, 4, 3.3], [6, 8, 3.0]], "seconds": "beat"}}]}

{@dm> sphereRoughness=0.3..0.9} {@ diffuseModel=normalizedDisney} {@ callout=skin-diffuse} Skin always uses Disney's normalised diffuse, rough or not, and offers a second specular lobe blended with the first.{<dm}

{@ animate=[{"path": {"params": ["camera.pitch"], "through": [[40], [6]], "seconds": "sentence"}}]} {@ callout=skin-lighting-defines} It takes the sky's light inside the forward pass, and uses a human skin profile for thin transmission.

{@ diffuseModel=lambertian} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 60, "seconds": "sentence", "ease": "sine"}}]} {@ callout=eye-direct-lighting} Eye uses the Skin model's setup but StandardPBR's direct lighting, with its iris refracted by shifting the texture inside a sphere.

## c8-recap | 3d | Light that passes through
{@ image="" demo=transmission callout="" panel=hidden cameraDrift=1 labels=0 sunLux=40 iblExposure=1 skyboxExposure=2 exposure=0.35 sunYaw=10 sunPitch=22 sphereLight=0 walls=0 teapotTransmission=2}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 6, 1.8], [48, 4, 2.0], [40, 8, 1.7]], "seconds": "beat"}}, {"path": {"params": ["sunYaw"], "through": [[40], [10]], "seconds": "beat"}}]}

So transmission is extra light from behind, added per light inside the forward pass, and only for Enhanced and Skin materials.

It reuses the sun's shadow map for thickness, which keeps it cheap and makes its scale approximate.

{@ delay=0.4} Every surface so far went through a material type; terrain brings its own forward shader.

## c8-sources | 2d | Where to read
sources: transmission-option, transmission-params, back-lighting, transmission-kernel, shadow-thickness, shadow-thickness-camera-span, transmission-finalize, foliage-sample-material, cutout-clip, vertex-normal-unflipped, enhanced-draw-list, sss-pack, sss-kernel, skin-diffuse, eye-direct-lighting

These are the files and lines this chapter followed, in reading order.

# 9 | Terrain, and a word on hair | 215 s
summary: The Terrain gem's own forward shader: fixed macro values, normals made per vertex on the CPU, detail materials blended by weight, multiscatter in the sky's light but not in direct light; and the Marschner model of the AtomTressFX hair gem.
sources: opaque-pass-order, terrain-pass-request, terrain-forward-draw-list, terrain-macro-constants, terrain-normal-cpu, terrain-normal-ps, terrain-height-blend, terrain-height-blend-option, terrain-no-direct-multiscatter, terrain-multiscatter-flags, hair-default-model, hair-marschner

## c9-own | 3d | Terrain draws through the forward pass with its own shader
{@ image="" demo=terrain callout="" panel=values cameraDrift=0.8 labels=0 viewMode=final terrainO3deNormal=0 forwardStage=3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 28, 22], [10, 22, 24], [36, 32, 20]], "seconds": "beat"}}]}

Every surface so far went through a material type; terrain brings its own forward shader.

{@ image=ta imageFrom=chainTarget image.target=2 imageCorner=top-left imageTitle=views.target_albedo} {@ callout=terrain-forward-draw-list} The Terrain gem's material type lists its own forward, depth, and shadow shaders, drawn into the same forward pass and the same five targets.

{@ delay=0.4} {@ image=ta imageMark=0,0.18,1,0.92 imageMarkText=views.mark_hills blend=0.6} In the albedo view, the hills sit in the same target as everything else, in their macro colour.

{@ delay=0.4} So the later passes read a terrain pixel exactly as they read the room's.

{@ image=ta imageMark=""} {@st> forwardStage=0..3} {@ callout=opaque-pass-order} So the sky's light, the reflections, and the occlusion reach the hills through the same full-screen passes as everything else.{<st}

{@ imageHide=ta} {@ callout=terrain-pass-request} Its feature processor adds only compute passes, for the clipmaps that cache its textures, and those are off by default.

{@ animate=[{"path": {"params": ["sunYaw"], "through": [[170]], "seconds": "sentence"}}]} {@ callout=terrain-forward-draw-list} It casts and receives the sun's shadows like any opaque surface, and reads the same shadow buffer.

## c9-fixed | 3d | Fixed macro values: roughness one, F zero four percent
{@ image="" demo=terrain callout="" panel=values cameraDrift=0.5 labels=0 viewMode=final param.formula=0 sunYaw=120 sunPitch=35}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 18, 16], [14, 14, 18], [36, 20, 15]], "seconds": "beat"}}]}

{@ term=1} {@ callout=terrain-macro-constants} Where only the macro material covers the ground, the shader fixes roughness at one, metalness at zero.

{@ term=2} {@ viewMode=specular} {@lo> sunPitch=35..8} {@ callout=terrain-macro-constants} Its specular factor of a half gives F zero of zero point zero four, so even with the sun low, the hills keep only a broad, faint sheen.{<lo}

{@ term=3} {@ viewMode=final sunPitch=35 blend=1.5} {@ callout=terrain-no-direct-multiscatter} Its lighting data never computes the multiscatter compensation, so direct light uses one, while its normal target's flag still asks the sky's reflection pass for it.

{@ms> multiscatter=1..0} {@ callout=terrain-multiscatter-flags} Turning the flag off changes only the sky's reflection on the hills; their direct light was never compensated.{<ms}

{@ multiscatter=1 blend=1} {@ callout=terrain-height-blend-option} Up close, detail materials replace the macro one, blended by plain weights: their height-based blend is off by default; the lab's hills have no detail materials.

## c9-normal | 3d | Normals made on the CPU, per vertex, in eight bits
{@ image="" demo=terrain callout="" panel=values cameraDrift=0.4 labels=0 viewMode=final terrainO3deNormal=0 sunYaw=120 sunPitch=20}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 34, 18], [14, 30, 20], [36, 36, 17]], "seconds": "beat"}}]}

{@ callout=terrain-normal-cpu} The terrain's normals are computed on the CPU, at each vertex, from central differences of the heights.

{@ animate=[{"path": {"params": ["sunYaw"], "through": [[60]], "seconds": "sentence"}}]} {@ callout=terrain-normal-cpu} Each axis's slope is normalised on its own, slope over the square root of slope squared plus one, and stored as an eight bit signed number.

{@on> terrainO3deNormal=0..1} {@ callout=terrain-normal-ps} The vertex shader rebuilds the up component as the square root of one minus the other two, exact on single-axis slopes and wrong where the ground slopes both ways.{<on}

{@ viewMode=terrainNormalError} {@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[60, 40], [10, 30]], "seconds": "sentence"}}]} {@ callout=terrain-normal-ps} Here is the angle between O3DE's normal and the exact one on the lab's hills: near zero on gentle ground, many degrees on the steepest two-way slopes.

{@ viewMode=final} {@ animate=[{"path": {"params": ["sunYaw"], "through": [[150]], "seconds": "sentence"}}]} Without a macro normal map, this interpolated vertex normal is the only normal the terrain has, and the sun's shading follows it.

## c9-hair | 3d | Hair: three Marschner lobes
{@ image="" demo=room callout="" panel=hidden cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 forwardStage=3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 20, 6.2], [0, 14, 6.6], [16, 18, 6.0]], "seconds": "beat"}}]}

{@ callout=hair-pass-after-opaque} Hair, from the AtomTressFX gem, is drawn by passes of its own after the opaque frame, and composited over it.

{@ callout=hair-default-model} {@ formula="R + TT + TRT" say="R plus T T plus T R T"} Its default lighting is Marschner's model, three lobes: R reflects off the strand's surface, TT passes through it, and TRT reflects off its inside.

{@ callout=hair-marschner} Each lobe is a Gaussian along the strand times an azimuthal term, with a refractive index of one point five five and the cuticle tilted by zero point zero eight radians; the lab has no strands to show it on.

## c9-recap | 3d | Special geometry, the same frame
{@ image="" demo=terrain callout="" panel=hidden cameraDrift=1 labels=0 viewMode=final terrainO3deNormal=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 28, 22], [6, 22, 24], [36, 32, 20]], "seconds": "beat"}}, {"path": {"params": ["sunYaw"], "through": [[160], [100]], "seconds": "beat"}}]}

So terrain brings its own shader but writes the same targets and reads the same lights and shadows, while hair lights itself in passes of its own after the opaque frame.

That leaves one thing to do: follow one pixel back through everything it took.

## c9-sources | 2d | Where to read
sources: terrain-forward-draw-list, terrain-pass-request, terrain-stencil, terrain-macro-constants, terrain-no-direct-multiscatter, terrain-multiscatter-flags, terrain-normal-cpu, terrain-normal-ps, terrain-height-blend-option, hair-pass-after-opaque, hair-default-model, hair-marschner

These are the files and lines this chapter followed, in reading order.

# 10 | One pixel, traced back | 110 s
summary: The worked point P followed back through the frame in the engine's order, and three takeaways.
sources: forward-targets, shadow-filter-default, forward-pixel, light-culling-pass, fullscreen-shadow, evaluate-lighting, diffuse-global, reflection-global, ssao-modulate, diffuse-specular-merge

## c10-map | 2d | The frame, once more
diagram: frame
focus: DEPTH, CULL, SHADOW, FWD, TARGETS, DIFF, REFL, SSS, AO, MERGE, LUT, CUBES

Here is the frame again, and one pixel to follow through it: P, low on the teapot.

## c10-trace | 3d | P's light, in the order the engine made it
{@ image="" demo=room callout="" panel=hidden cameraDrift=0.6 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 forwardStage=0 extraLights=0 camera.target=-0.3,0.25,-1.6}
{@ inspectX=-0.315 inspectY=0.124 inspectZ=-1.478 inspectNX=-0.23 inspectNY=-0.02 inspectNZ=0.97 inspectObject=8 pickShown=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[-22, 18, 2.8], [-34, 14, 3.0], [-18, 20, 2.6]], "seconds": "beat"}}]}

{@ image=d imageFrom=chainTarget image.target=5 image.far=6 imageCorner=top-left imageTitle=views.depth_prepass} {@ callout=depth-prepass} First the depth pre-pass wrote P's depth, and that depth fixed P's tile and its depth bin.

{@ image=sb imageFrom=shadowBuffer image.blurred=1 imageCorner=top-left imageTitle=views.shadow_buffer} {@ callout=fullscreen-shadow} The shadow passes compared P against the sun's cascades and left one blurred number in the shadow buffer.

{@ image=t0 imageFrom=chainTarget image.target=0 imageCorner=bottom-left imageTitle=views.target_diffuse} {@ callout=evaluate-lighting} The forward pass built P's surface, walked its bin's lights and the sun, and wrote their light and the surface into five targets.

{@a> forwardStage=0..1} {@ callout=diffuse-global} The sky's diffuse light was added by the normal's lookup of the diffuse cubemap.{<a}

{@b> forwardStage=1..2} {@ callout=reflection-global} Its reflection came from the specular cubemap at the mip of P's roughness, times F zero A plus B.{<b}

{@c> forwardStage=2..3} {@ callout=ssao-modulate} SSAO darkened P's diffuse light from fifty-two samples around it, and the merge added diffuse and specular together.{<c}

## c10-takeaways | 3d | Three takeaways
{@ image="" demo=room callout="" panel=hidden cameraDrift=1 labels=0 sunLux=40 iblExposure=2 skyboxExposure=2 exposure=0.18 sunYaw=40 sunPitch=52 forwardStage=3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[24, 20, 6.2], [4, 14, 6.6], [-14, 18, 6.0]], "seconds": "beat"}}, {"path": {"params": ["sunYaw"], "through": [[70], [30]], "seconds": "beat"}}]}

{@ callout=forward-targets} First, the data: the forward pass leaves a thin G-buffer of five targets and a stencil, so full-screen passes can finish the light without drawing the scene again.

{@ callout=light-culling-pass} Second, reuse: lights are sorted once per tile, the sky and the BRDF are prefiltered once, and the sun's shadow is resolved once per pixel, so each pixel pays only for what it reads.

{@ callout=shadow-filter-default} Third, the trade: a binary shadow blurred in screen space, an occlusion sized by the screen that darkens direct light too, and a compensation left off by default, each trading accuracy for speed.

{@ delay=0.5} That is how O3DE lights a frame.

## c10-sources | 2d | Where to read
sources: forward-pixel, depth-prepass, light-culling-pass, fullscreen-shadow, evaluate-lighting, diffuse-global, reflection-global, ssao-modulate, diffuse-specular-merge

These are the functions the whole frame passes through, in the order a pixel meets them.
