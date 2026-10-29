title: How O3DE Lights a Frame
subtitle: O3DE's opaque frame, pass by pass: ported to WGSL and read from O3DE's source
engineVersion: 2.8.0
commit: d363107efc92f79c75bb7eb88b78b8fc07ae229c
date: 2026-10-26
fps: 30
width: 1920
height: 1080

# 1 · O3DE's opaque frame · 165 s
summary: The forward pixel shader, its targets, and the passes that follow it.
sources: forward-pixel, evaluate-lighting, direct-lighting, diffuse-specular-merge, opaque-pass-order, ssao-default, msaa-default, light-culling-define, forward-opaque

## c1-room · scene · One light, one material
{@ demo=room callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[23, 22, 5.8], [-6, 12, 6.8], [20, 19, 6.1]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightZ"], "center": [0.0, -0.3], "degrees": 300.0, "seconds": "beat", "ease": "sine"}}]}

This is the Open 3D Engine's Atom renderer, lighting the lab's study room with the shaders of its StandardPBR material, ported to WGSL.

{@ callout=point-light-loop} One light hangs in the room: a simple point light, O3DE's plainest punctual light, with no shape and no shadows.

{@ callout=engine-version} Every formula you will see comes from O3DE version two point eight, read straight from its source.

Each surface here is lit twice.

{@ callout=evaluate-lighting} Once by the point light, in the forward pass, and once by the sky, in passes that run after it.

## c1-frame · mermaid · A pixel's path through the frame
diagram: frame
focus: CULL, FWD, TARGETS, DIFF, REFL, COMP, AO, MERGE

Here is the path one pixel takes through the frame.

{@ callout=light-culling-pass} A depth pre-pass draws the scene's depth first, and light culling sorts the lights into screen tiles.

{@ callout=evaluate-lighting} The forward pass evaluates the material, applies decals, then each light, and writes diffuse and specular light into two separate targets.

{@ callout=forward-targets} It also writes albedo times the diffuse response, the specular colour with the roughness, and the normal, which later passes read back.

{@ callout=opaque-pass-order} Full-screen passes then add the sky's diffuse light and the reflections, from the sky and from reflection probes; SSAO darkens the diffuse, and a merge pass adds the two together.

{@ callout=diffuse-composite} In a project made from O3DE's default template, the DiffuseProbeGrid gem swaps the diffuse pass for a composite of its own, which applies the same sky light, weighted by what any probe grid leaves, and only where the image-based-lighting flag is on.

## c1-code · scene · The forward pixel shader
{@ demo=room callout="" labels=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=forward-pass highlight=surface} In pseudo-code, the forward pixel shader is short.

{@ highlight=surface,init callout=evaluate-lighting} It evaluates the surface from the material's inputs, and initialises the lighting data.

{@ highlight=lights callout=direct-lighting} Then it adds the lights listed for the pixel's screen tile: directional lights, then simple point and spot lights, then area lights. This room has one simple point light.

{@ highlight=ibl callout=ibl-forward-transparent} Here, image-based lighting runs only for transparent materials, or for specular when a mesh or material asks for it; opaque ones get it from full-screen passes.

{@ highlight=targets,merge callout=diffuse-specular-merge} Diffuse and specular end up in separate targets, and the merge pass sums them.

## c1-views · scene · Taking the pixel apart
{@ demo=room callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightZ"], "center": [0.0, -0.3], "degrees": 180.0, "seconds": "beat", "ease": "sine"}}]}

{@ panel=values} Let's take the result apart.

{@ viewMode=1 param.formula=1} This is the point light's diffuse lighting on its own.

{@ viewMode=2 param.formula=2} This is its specular lighting: small, sharp highlights, brightest on the floor below the light.

{@ callout=opaque-pass-order viewMode=0 param.formula=0} And this is everything together, with the sky's light added by the passes after the forward pass.

## c1-sources · sources · Where to read
sources: forward-pixel, evaluate-lighting, direct-lighting, diffuse-specular-merge

These are the functions this chapter followed.

Every claim in this video cites a file, a symbol, and a line in the O3DE checkout, and the build checks each line is still there.

# 2 · Light culling · 205 s
summary: Tiles, depth bins, and the lists the forward pass walks.
sources: light-culling-pass, light-culling-tile, tile-prepare, is-object-inside-tile, cull-point-light, light-culling-main, light-culling-remap, tile-iterator, polygon-not-culled

## c2-why · scene · Which lights a tile needs
{@ demo=culling callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[23, 22, 5.8], [-6, 12, 6.8], [20, 19, 6.1]], "seconds": "beat"}}, {"path": {"params": ["extraLightMotion"], "through": [[30.0]], "seconds": "beat"}}]}

That was the whole frame; the next part looks at its first lighting step, light culling.

{@ callout=light-culling-pass} Before the forward pass, O3DE decides which lights each part of the screen needs.

Here, twenty-four small simple point lights hang over the room; a dot marks each one.

{@ callout=light-culling-tile} The screen is cut into tiles of sixteen by sixteen pixels, and each tile gets its own list of lights.

In false colour, each pixel shows how many lights its tile keeps at its depth: dark blue for none, yellow for eight or more.

{@ callout=light-culling-pass} Three compute passes build these lists, after a depth pre-pass and before any shading.

## c2-prepare · scene · Each tile's depth
{@ demo=culling callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=light-culling highlight=prepare callout=tile-prepare} The first, LightCullingTilePrepare, runs one thread per pixel and one group per tile.

{@ callout=tile-prepare} It finds the tile's nearest depth, and its farthest one short of the sky, opaque and transparent alike, so a wall seen against the sky stays shallow.

{@ callout=tile-prepare} It marks a thirty-two-slice mask between them: one bit for the slice of each opaque sample, and every slice between a transparent surface's nearest and farthest depth.

{@ callout=tile-prepare param.formula=3} And it splits the range into one, two, four, or eight depth bins, from the base-two log of its span in metres.

{@ panel=values} The box drawn here is the inspected point's tile in view space, with its bins as slices.

## c2-cull · scene · Testing lights against tiles
{@ demo=culling callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=light-culling highlight=cull callout=light-culling-main} The second pass, LightCulling, tests each light against each tile.

{@ callout=cull-point-light param.formula=1} A light is kept when its sphere touches the tile's box, and its depth range touches a filled slice of the mask.

{@ callout=light-index-pack param.formula=2} Each kept light is written as its index, shifted sixteen bits, with one bit for each depth bin it reaches.

{@ callout=light-culling-main} Lights are grouped by type, decals first, then simple point lights, spot lights, and the area lights, each group closed by an end marker.

{@ callout=light-culling-main} O3DE's threads append a group's lights in whatever order they finish, decals excepted, since those are sorted; the lab appends lights in index order, so its frames never change.

{@ panel=values param.formula=0} {@ extraLightRadius=0.5 blend=1.2} As the lights' radius {@ extraLightRadius=1.8 blend=1.8} grows, more tiles keep them.

## c2-forward · scene · One list per depth bin
{@ demo=culling callout="" cameraDrift=0}

{@ callout=light-culling-remap} The third pass, LightCullingRemap, rewrites each tile's list as one list per depth bin.

{@ panel=code code=light-culling highlight=forward callout=tile-iterator param.formula=3} The forward pass finds the bin at its own depth, and walks only that list.

{@ panel=values} At the inspected point, the readout counts the lights in the tile's list, those its bin keeps, and those whose spheres reach the point.

The ringed dots are the lights the point's bin keeps, each joined to the point by a dashed line.

{@ extraLightMotion=70 blend=3} Now the lights move: as each one crosses tiles and {@ extraLightMotion=-40 blend=3} depth bins, the colours and the point's list follow it.

The lab compares every tile's list on the GPU with a CPU copy, and checks that no light reaching a surface is missing from its bin.

## c2-finding · scene · The implicit order, read from the code
{@ demo=culling callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ callout=polygon-not-culled} One finding: the forward pass also walks a group for polygon lights.

{@ callout=polygon-not-culled} But the culling pass writes no such group, so with light culling on, which is the default, polygon lights never shade anything.

{@ panel=code code=light-culling highlight=forward callout=punctual-group-skip} A second one, from the same order: the forward pass walks the two simple-light groups only while punctual lights are enabled.

{@ callout=punctual-group-skip} Switch that option off on a material, and the area lights after them read the groups of the lights before them; decals, by contrast, keep walking theirs.

## c2-sources · sources · Where to read
sources: light-culling-pass, light-culling-tile, tile-prepare, nvlc-constants, is-object-inside-tile, cull-point-light, light-culling-main, light-culling-remap, tile-iterator, polygon-not-culled, punctual-group-skip, decals-advance

These entries follow the three culling passes, the tile iterator, and the two findings about the order of its groups.

# 3 · The surface · 204 s
summary: The metallic workflow and the roughness remap.
sources: surface-eval, set-albedo-f0, material-utils, roughness-values, specular-aa-default, specular-aa

## c3-inputs · scene · What the material provides
{@ demo=surface camera.target=-1.05,1.2,-0.55 camera.yaw=40 camera.pitch=20 camera.distance=3.5 callout="" readoutRows=0 cameraDrift=1 blend=3 ease=inOut}
{@ animate=[{"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[-1.8677, 2.0349, 0.4245], [-0.7072, 1.9442, 0.7293], [-1.5321, 2.3891, 0.285], [-2.0467, 2.3007, -0.9128], [-1.8298, 1.8739, -1.6636], [-2.2787, 2.1004, -0.55]], "seconds": "beat"}}]}

With the lights sorted into lists, the next part turns to what they light: the surface.

{@ callout=surface-data readoutRows=4} {@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[15, 24, 3.5], [-12, 20, 3.3], [22, 18, 3.5]], "seconds": "beat"}}]} Every lighting function starts from the same surface record.

{@ callout=surface-eval} The material provides a base colour, a metallic value, a specular F zero factor, and a roughness.

{@ readoutRows=10} From these, O3DE derives what the lighting actually uses. {@ pause=0.6}

## c3-metal · scene · The metallic workflow
{@ callout="" sphereMetallic=0}
{@ animate=[{"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[-1.6245, 2.2042, 0.4451], [-0.597, 1.9442, 0.6945], [-1.7318, 2.3007, -1.3625], [-2.2787, 2.1004, -0.55]], "seconds": "beat"}}, {"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[45, 20, 3.5], [10, 24, 3.5], [32, 18, 3.7], [20, 14, 3.5]], "seconds": "beat"}}]}

{@ panel=code code=surface-setup highlight=albedo callout=material-utils} {@ sphereMetallic=0 blend=0.8} Albedo is the base colour times one minus metallic, {@ sphereMetallic=1 blend=1.5} so a metal has no diffuse colour at all.

{@ highlight=f0 callout=set-albedo-f0} {@ sphereMetallic=0 blend=1.5} Specular F zero blends from a dielectric value {@ sphereMetallic=1 blend=1.5} to the base colour as metallic grows.

{@ panel=values callout=material-utils param.formula=2} The dielectric value is zero point zero eight times the factor, so the default factor of one half gives zero point zero four, the reflectance of most paint and plastic.

{@ sphereMetallic=0.5 blend=1} Metallic values between zero and one blend the two linearly.

## c3-rough · scene · Roughness, squared twice
{@ callout=""}

{@ panel=values sphereMetallic=0 param.formula=3 blend=0.8} {@ camera.target=-1.15,1.6,-0.55 camera.yaw=0 camera.pitch=4 camera.distance=2.6 cameraDrift=0 blend=2.5 ease=inOut} {@ lobe=1 labels=0 blend=1.5} Roughness is authored on a perceptual scale.

{@ panel=code code=surface-setup highlight=roughness callout=roughness-values} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -30.0, "seconds": 2.8, "ease": "sine"}}]} O3DE clamps it to at least zero point zero two {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 40.0, "seconds": 3.0, "ease": "sine"}}]} four, squares it into alpha, and squares it again.

{@ callout=roughness-values sphereRoughness=0.5 blend=1} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -20.0, "seconds": 2.0, "ease": "sine"}}]} So a roughness of one half becomes an alpha squared of one sixteenth, before anti-aliasing widens it.

{@ panel=values} {@ sphereRoughness=0.05 blend=1} Watch the sphere {@ sphereRoughness=0.4 blend=1.2} as roughness sweeps from {@ sphereRoughness=0.85 blend=1.2} polished to matte.

## c3-aa · scene · Specular anti-aliasing
{@ camera.target=-1.05,1.2,-0.55 camera.yaw=-10 camera.pitch=22 camera.distance=3.5 callout="" cameraDrift=1 lobe=0 labels=1 sphereRoughness=0.35 param.formula=0 readoutRows=99 blend=3 ease=inOut}
{@ animate=[{"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[-1.05, 2.1004, 0.6787], [-1.885, 2.3891, -0.0679], [-1.9013, 1.9442, -1.5646], [-2.2787, 2.1004, -0.55]], "seconds": 24.0}}]}

{@ panel=code code=surface-setup highlight=aa callout=specular-aa-default} Then specular anti-aliasing, on by default, widens alpha squared where the normal turns quickly from pixel to pixel.

{@ callout=specular-aa} {@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[25, 20, 3.5], [40, 16, 3.4]], "seconds": 14.0}}]} It adds a kernel of half the squared change in the normal across and down a pixel, never more than zero point one eight.

{@ callout=specular-aa} Then it recomputes alpha and the linear roughness from the result, so every later term sees the wider lobe.

{@ panel=values sphereRoughness=0.1 param.formula=3 blend=1} {@ camera.target=-1.05,0.9,-0.55 camera.yaw=30 camera.pitch=10 camera.distance=2.4 blend=2.5 ease=inOut} {@ specularAA=0} On a smooth sphere the widening shows toward the {@ specularAA=1} rim, where the normal turns fastest across a pixel.

{@ callout=specular-aa} {@ camera.target=-1.05,1.2,-0.55 blend=2.5 ease=inOut} {@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[10, 20, 3.5], [25, 18, 3.6]], "seconds": "beat"}}, {"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[-1.6644, 2.1004, 0.5141]], "seconds": "beat"}}]} The lab takes those derivatives as O3DE does, from the fine screen-space derivatives of the normal.

## c3-occlusion · scene · Occlusion maps
{@ callout="" cameraDrift=1 sphereRoughness=0.35 param.formula=0 blend=1.5}
{@ animate=[{"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[-0.4903, 1.9442, 0.6503], [-1.7318, 2.3007, 0.2625], [-1.9995, 2.3891, -0.7174], [-1.8097, 1.9442, -1.6349], [-2.2787, 2.1004, -0.55]], "seconds": 36.0}}, {"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[35, 24, 3.5], [-10, 20, 3.5], [20, 18, 3.5]], "seconds": "beat"}}]}
{@ animate=[{"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[0.55, 1.85, 0.95]], "seconds": "beat"}}] delay=41.8}

{@ panel=code code=occlusion highlight=input callout=occlusion-eval sphereOcclusion=1 viewMode=0} A material can also carry two occlusion maps: diffuse ambient occlusion, and specular occlusion, sometimes called a cavity map.

{@ callout=occlusion-defaults} Each is the map's red channel, raised to the power of its factor, which defaults to one.

{@ callout=occlusion-input} Without a map, both are one, and nothing changes.

{@ highlight=finalize callout=occlusion-finalize} At the end of the forward pass, diffuse light is multiplied by the ambient occlusion, and specular light by the specular occlusion.

{@ highlight=targets callout=occlusion-finalize} The albedo target carries both on to the skylight's passes, with the specular occlusion in its alpha.

{@ panel=values} {@ diffuseOcclusionFactor=2 blend=1.2} Raising the factor deepens the creases; lowering {@ diffuseOcclusionFactor=0.2 blend=1.5} it toward zero fades the map out.

## c3-examples · scene · Every object, the same rules
{@ demo=room camera.target=0,0.85,-0.3 camera.yaw=18 camera.pitch=16 camera.distance=6.4 callout="" labels=1 cameraDrift=1 blend=3}
{@ cameraDrift=1}

The same rules apply to every object in the room.

{@ teapotMetallic=1 blend=1.5} Make the teapot metal, and its green leaves the diffuse light and moves into the highlight.

{@ boxRoughness=1 blend=1.5} Make the box rough, and its highlight spreads until it is barely there.

{@ callout=set-albedo-f0 teapotMetallic=0 boxRoughness=0.55 blend=1.5} Each of them goes through the same two functions before any light is evaluated.

## c3-sources · sources · Where to read
sources: surface-eval, set-albedo-f0, material-utils, roughness-values

The surface comes from these four places: the material's evaluation, the metallic workflow, its constants, and the roughness remap.

# 4 · Decals · 190 s
summary: Box-projected textures blended into the surface before lighting.
sources: decal-evaluate-order, decal-apply, decal-attenuation, decal-cull, decal-sort, decal-defaults

## c4-what · scene · A texture in a box
{@ demo=decals callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 46, 2.6], [-12, 58, 2.3], [14, 50, 2.5]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightZ"], "center": [-0.7, 0.55], "degrees": 360.0, "seconds": "beat", "ease": "sine"}}]}

Before any light reaches that surface, decals can change it, and they are the next part.

{@ callout=decal-apply} Decals project a texture onto whatever surfaces fall inside a box.

Here, a paint splat and a panel grid lie on the floor; the boxes and arrows show their volumes and projection axes.

{@ callout=decal-options} Decals are on by default, and they change the surface itself, before the lighting data is set up.

{@ callout=decal-evaluate-order} So every light, and the skylight's passes after the forward pass, see the decal's albedo, F zero, and normal.

## c4-apply · scene · ApplyDecal
{@ demo=decals callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 46, 2.6], [-12, 58, 2.3], [14, 50, 2.5]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightZ"], "center": [-0.7, 0.55], "degrees": 360.0, "seconds": "beat", "ease": "sine"}}]}

{@ panel=code code=decals highlight=box callout=decal-apply} ApplyDecal moves the point into the decal's box, through its rotation and its half size, and skips it outside minus one to one.

{@ callout=decal-apply} The box's x and y become the texture coordinates, with v flipped.

{@ highlight=albedo callout=decal-apply param.formula=2} The base map times the decal's colour replaces the albedo, by the map's alpha times the decal's opacity and its angle fade.

{@ callout=decal-apply param.formula=3} F zero moves too, by that amount times metallic, so a decal on metal changes the colour it reflects.

{@ highlight=normal param.formula=4} And its normal map bends the normal, here into the grooves between the panels.

{@ callout=tangent-space-normal} The normal map's two channels are swapped back and its z is rebuilt, as for any O3DE normal map.

## c4-angle · scene · Fading with the angle
{@ demo=decals callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 46, 2.6], [-12, 58, 2.3], [14, 50, 2.5]], "seconds": "beat"}}]}

{@ panel=code code=decals highlight=angle callout=decal-attenuation param.formula=1} Each decal fades with the angle between the surface and its projection axis.

{@ callout=decal-attenuation} The cosine is clamped at zero point five five and smoothstepped to one, so a decal is gone by about fifty-seven degrees.

{@ panel=values} {@ decalTilt=35 blend=1.5} Tilting the splat's axis away from {@ decalTilt=58 blend=1.5} the floor's normal fades it out.

{@ callout=decal-defaults decalAngleAttenuation=0 blend=1.2} The attenuation-angle setting, one by default, blends between none of that fade and all of it.

## c4-order · scene · Which decal is on top
{@ demo=decals callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 46, 2.6], [-12, 58, 2.3], [14, 50, 2.5]], "seconds": "beat"}}]}

Where decals overlap, their order decides which one lands on top.

{@ panel=code code=decal-culling highlight=cull callout=decal-cull} Light culling puts decals in the first group of each tile's list, as spheres around their boxes.

{@ highlight=sort callout=decal-sort} Then it sorts them by sort key, and by index where keys are equal, so a larger key is applied later and wins.

{@ panel=values callout=decal-defaults} The grid keeps the default key of sixteen, and the splat has seventeen, so the splat lies over the grid.

{@ decalSplatSortKey=15} Give the splat fifteen, and the grid covers it.

{@ panel=code code=decal-culling highlight=forward callout=decal-forward} The forward pass walks that sorted group before it sets up any lighting.

## c4-colour · scene · Two colour spaces
{@ demo=decals callout="" cameraDrift=1 decalSplatSortKey=17}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 46, 2.6], [-12, 58, 2.3], [14, 50, 2.5]], "seconds": "beat"}}]}

{@ callout=transform-color} O3DE lights in ACEScg: every material and light colour is converted from linear sRGB on its way in.

{@ panel=values callout=decal-colour-raw} A finding: a decal's colour is stored as it is given, and blended into that ACEScg albedo without the conversion.

{@ callout=base-colour-acescg} The readout shows b, the colour the top decal blends in, its texture times its colour, and what a material's colour of the same value would become.

For colours near grey the two barely differ; saturated decals shift in hue.

## c4-sources · sources · Where to read
sources: decal-evaluate-order, decal-options, decal-forward, decal-apply, decal-attenuation, tangent-space-normal, decal-cull, decal-sort, decal-defaults, decal-half-size, decal-colour-raw, transform-color, base-colour-acescg

These entries show where decals enter the forward pass, how each one is blended, and how they are culled and sorted.

# 5 · The simple point light · 152 s
summary: Inverse square, the radius window, no shadows, the automatic radius.
sources: point-light-struct, point-light-apply, point-light-loop, point-light-auto-radius, point-light-luminance

## c5-record · scene · A position, an intensity, a radius
{@ demo=pointlight callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[15, 24, 6.8], [8, 32, 7.5], [15, 26, 7.0]], "seconds": "beat"}}]}

The surface is ready, decals included; next comes the light that reaches it, starting with the plainest one, the simple point light.

{@ callout=point-light-struct} A simple point light stores a position, an RGB intensity in candelas, one over its attenuation radius squared, and its lighting channels and GI flags.

{@ panel=code code=simple-point-light highlight=falloff,intensity callout=point-light-apply} Its intensity falls off with the inverse square of distance: the dashed curve on the floor.

{@ highlight=window callout=point-light-apply} Then a window pulls it smoothly to zero at the attenuation radius: the solid curve.

{@ callout=point-light-apply} The window is {@ formula="w = \big(1 - (\lit{1}{d^2} / r^2)^2\big)^2" say="one minus the squared ratio of distance squared to radius squared, all squared once more" display=yes colors="#ffd98a"}.

{@ panel=values callout=point-light-apply param.formula=2} At half the radius it still keeps eighty eight percent, so the solid curve follows the dashed one closely and drops only near the edge.

{@ param.formula=0} The readout follows one point on the sphere: its distance, the falloff ratio, the window, and the illuminance that reaches it.

{@ param.formula=1} Six candelas at just under two metres give about one point seven lux, before the window.

## c5-radius · scene · The attenuation radius
{@ demo=pointlight callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[15, 24, 6.8], [8, 32, 7.5], [15, 26, 7.0]], "seconds": "beat"}}]}

{@ highlight=skip callout=point-light-loop param.formula=3} Beyond the radius, the light is skipped entirely, so the radius also limits its cost.

{@ panel=values callout=point-light-radius} {@ attenuationRadius=2.5 blend=1.5} Grow the radius, and {@ attenuationRadius=6 blend=2} the window reaches further.

{@ callout=point-light-auto-radius} In the automatic mode, the default, the radius is the square root of the light's lumens, weighted by its colour's luminance, divided by zero point one.

## c5-channels · scene · Lighting channels
{@ demo=pointlight callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[15, 24, 6.8], [8, 32, 7.5], [15, 26, 7.0]], "seconds": "beat"}}]}

{@ panel=code code=simple-point-light highlight=channel callout=lighting-channels-default falloff=0 radius=0} Lights and meshes also carry lighting channels: five bits each.

{@ callout=lighting-channels-test} A light shades a surface only when their masks share a bit.

{@ callout=lighting-channels-default} Components start with all five channels set, so by default every light reaches every mesh.

{@ panel=values lightChannels=1 sphereLightingChannels=2 viewMode=1} Give the light a mask of one and the sphere a mask of two, and the sphere stops receiving it: here is the point light's diffuse alone.

{@ callout=lighting-channels-test} The skylight, the reflection passes, and decals ignore the channels.

{@ callout=directional-channel-return} A finding: in the directional-light loop, a light of another channel ends the whole loop, so the directional lights after it are skipped too.

## c5-shadows · scene · No shadows
{@ demo=room callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[23, 22, 5.8], [-6, 12, 6.8], [20, 19, 6.1]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightZ"], "center": [0.0, -0.3], "degrees": 180.0, "seconds": "beat", "ease": "sine"}}]}

{@ panel=code code=simple-point-light highlight=shadow callout=point-light-loop viewMode=0} Simple point lights cast no shadows.

Light reaches every surface that faces it, even behind the boxes.

{@ callout=light-culling-define} With no shadow maps, and culled per screen tile, each one costs little more than shading the pixels inside its radius.

## c5-sources · sources · Where to read
sources: point-light-struct, point-light-init, point-light-apply, point-light-loop, point-light-radius, point-light-auto-radius, cutoff-intensity

The light's record, its falloff, the loop that skips it, and the automatic radius are listed here.

# 6 · GGX specular · 155 s
summary: N, L, V, and H; the D, G, and F terms; the specular lobe.
sources: specular-ggx, ggx-d, ggx-g, fresnel-schlick, standard-brdf, clear-coat-default

## c6-vectors · scene · Four directions
{@ demo=specular camera=continuous callout="" vectorsShown=0 lobe=0 readoutRows=0}
{@ camera.target=-1.05,1.3,-0.55 camera.yaw=35 camera.pitch=22 camera.distance=3 cameraDrift=1 blend=3 ease=inOut}
{@ animate=[{"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[-1.6245, 2.2042, 0.4451], [-0.585, 1.8739, 0.7275], [-1.7886, 2.3891, 0.0698], [-2.0451, 1.99, -1.385], [-2.1298, 2.2042, -0.157]], "seconds": 15.0}}]}

That gives the light's intensity at the point; what the surface sends back of it is the BRDF, and its specular half comes first: GGX.

{@ vectorsShown=0} {@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[10, 26, 2.9], [-15, 18, 3.0], [20, 22, 2.8]], "seconds": 8.6}}]} Specular lighting depends on four directions at one point.

{@ vectorsShown=1 blend=0.6} The surface normal, N.

{@ vectorsShown=2 blend=0.6} The direction to the light, L.

{@ vectorsShown=3 blend=0.6} {@ camera.target=-1.05,1.35,-0.55 camera.yaw=0 camera.pitch=6 camera.distance=3.1 cameraDrift=0 blend=5 ease=inOut} {@ animate=[{"path": {"params": ["lightX", "lightY", "lightZ"], "through": [[-2.2787, 2.1004, -0.55]], "seconds": 5.0}}]} The direction to the viewer, V; we look from the blue observer, so that all four lie in a plane facing us.

{@ vectorsShown=4 blend=0.6} And the half vector, H, halfway between L and V.

{@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -25.0, "seconds": 2.2, "ease": "sine"}}]} A microfacet whose normal points along H {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 25.0, "seconds": 2.2, "ease": "sine"}}]} mirrors the light straight into the viewer.

## c6-terms · scene · D, G, and F
{@ demo=specular callout="" cameraDrift=0}

{@ panel=values callout=specular-ggx param.formula=0} SpecularGGX multiplies three terms, D, G, and F, with N dot L and the multiscatter compensation.

{@ panel=values highlight=d callout=ggx-d viewMode=3 param.formula=1} D, the GGX normal distribution, is the density of microfacet normals that point along H; for a smooth surface it rises far above one.

{@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -20.0, "seconds": 2.7, "ease": "sine"}}]} In false colour, D is bright only where {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 20.0, "seconds": 2.7, "ease": "sine"}}]} H meets the normal: the core of the highlight.

{@ highlight=g callout=ggx-g viewMode=4 param.formula=2} G is the height-correlated Smith term: the share of those facets that are neither shadowed nor hidden.

{@ callout=ggx-g} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 20.0, "seconds": 3.5, "ease": "sine"}}]} O3DE returns it already divided by four, N dot L, {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -20.0, "seconds": 3.5, "ease": "sine"}}]} and N dot V, which saves the division at the end.

{@ highlight=f callout=fresnel-schlick viewMode=5 param.formula=3} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 25.0, "seconds": 3.6, "ease": "sine"}}]} F is Schlick's Fresnel, taken on H dot V: reflectance rises {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -25.0, "seconds": 3.6, "ease": "sine"}}]} toward white as the light and the view graze the microfacet.

{@ highlight=product,light callout=standard-brdf viewMode=0 param.formula=4} That product is the specular response; times the light's intensity, it is the specular lighting.

## c6-sweep · scene · Moving the light
{@ demo=specular callout="" cameraDrift=0}

{@ panel=values lobe=1 readoutRows=11 param.formula=0 blend=0.5} Now the numbers at the point, and its lobe.

{@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 12.0, "seconds": 3.5, "ease": "sine"}}]} The pink curve is the specular response for every outgoing direction in {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -12.0, "seconds": 3.5, "ease": "sine"}}]} this plane, and it leans around the mirror direction of L.

{@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -35.0, "seconds": 3.5, "ease": "smooth"}}]} Raise the light toward the normal, and the lobe stands up with it.

{@ param.formula=3 animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 50.0, "seconds": 4.5, "ease": "smooth"}}]} Lower it toward the horizon: N dot L falls, Fresnel grows, and the lobe stretches out.

{@ param.formula=0} {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": -30.0, "seconds": 1.4, "ease": "sine"}}]} The readout follows every {@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 15.0, "seconds": 2.2, "ease": "sine"}}]} term as the light moves.

## c6-rough · scene · Roughness widens D
{@ demo=specular callout="" lobe=1 cameraDrift=0}

{@ callout=ggx-d param.formula=1} Roughness changes D more than anything else.

{@ sphereRoughness=0.15 blend=1.5} A smooth surface concentrates its light into a tall, narrow lobe and a small, bright highlight.

{@ sphereRoughness=0.7 blend=2} A rough one spreads the same light thin.

{@ viewMode=3} {@ sphereRoughness=0.15 blend=1.2} In false colour, D grows from {@ sphereRoughness=0.7 blend=1.5} a pinpoint to a broad glow.

## c6-sources · sources · Where to read
sources: specular-ggx, ggx-d, ggx-g, fresnel-schlick, standard-brdf

SpecularGGX and its three terms live in three short files of O3DE's shader library.

# 7 · Diffuse and its response · 100 s
summary: Lambertian diffuse, scaled by one minus a roughness-aware Fresnel; Disney diffuse compared.
sources: diffuse-lambert, initialize-lighting-data, fresnel-roughness, diffuse-disney, skin-diffuse

## c7-lambert · scene · Lambertian, with a response
{@ demo=diffuse callout="" cameraDrift=0}
{@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 22.0, "seconds": 5.0, "ease": "sine", "repeat": "pingPong"}}]}

From specular, the next part turns to the other half of each light: diffuse, and its response.

{@ panel=code code=diffuse-response highlight=lambert callout=diffuse-lambert} Diffuse light in StandardPBR is Lambertian: albedo, times N dot L, over pi.

{@ highlight=response callout=initialize-lighting-data} But it is scaled by a diffuse response: one minus a Fresnel term at the view angle.

{@ panel=values callout=fresnel-roughness param.formula=3} That Fresnel knows the roughness: its grazing value is one minus roughness, but never less than F zero.

{@ callout=fresnel-roughness param.formula=2} So a smooth surface seen at a grazing angle keeps less of its light for diffuse, because more of it goes to specular.

{@ callout=initialize-lighting-data} Without that response, diffuse and specular together could send back more light than arrives.

## c7-response · scene · Where the response dips
{@ demo=diffuse callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 12, 3.0], [-14, 8, 3.25], [6, 10, 3.05]], "seconds": "beat"}}]}

{@ panel=values viewMode=7 param.formula=2} In false colour, the diffuse response stays near one, and dips only toward the sphere's rim.

{@ viewMode=1 param.formula=0} The green lobe shows the result for every view direction: nearly a half circle, flattened where the response falls.

{@ sphereRoughness=0.1 blend=1.2} Smoother surfaces dip more {@ sphereRoughness=0.9 blend=1.5} at grazing angles.

## c7-disney · scene · A diffuse model StandardPBR does not use
{@ demo=diffuse callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 12, 3.0], [-14, 8, 3.25], [6, 10, 3.05]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 22.0, "seconds": 5.0, "ease": "sine", "repeat": "pingPong"}}]}

{@ callout=diffuse-disney diffuseModel=1 param.formula=0} O3DE's BRDF library also defines a normalized Disney diffuse.

{@ callout=skin-diffuse} StandardPBR never calls it; the Skin material does, and EnhancedPBR does when subsurface scattering is on.

{@ callout=diffuse-disney} It carries its own Fresnel terms for light entering and leaving the surface, so it takes no diffuse response.

{@ callout=diffuse-lambert diffuseModel=0} Back to the Lambertian model that StandardPBR uses.

## c7-sources · sources · Where to read
sources: diffuse-lambert, initialize-lighting-data, fresnel-roughness, diffuse-disney, enhanced-diffuse, skin-diffuse

These entries show the diffuse term, its response, and which materials use the Disney model.

# 8 · Clear coat · 130 s
summary: A dielectric layer over the base: its F zero, diffuse, specular, and IBL.
sources: clear-coat-default, clear-coat-inputs, clear-coat-f0, clear-coat-response, clear-coat-diffuse, clear-coat-specular, clear-coat-ibl, clear-coat-forward-ibl, forward-ibl-stencil

## c8-layer · scene · A layer of varnish
{@ demo=clearcoat callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 12, 3.0], [-14, 8, 3.25], [6, 10, 3.05]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 22.0, "seconds": 5.0, "ease": "sine", "repeat": "pingPong"}}]}

Specular and diffuse together make one layer; clear coat lays a second, clear one over it.

Clear coat adds a thin, clear layer over the base material, like varnish over paint.

{@ callout=clear-coat-default sphereClearCoat=0} It is off by default; {@ sphereClearCoat=1} here the sphere is a rough metal under a smooth coat.

{@ callout=clear-coat-inputs} The coat is a dielectric with an F zero of four percent, and its own factor and roughness.

## c8-base · scene · The base beneath
{@ demo=clearcoat callout="" cameraDrift=0}
{@ animate=[{"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 22.0, "seconds": 5.0, "ease": "sine", "repeat": "pingPong"}}]}

{@ panel=code code=clear-coat highlight=f0 callout=clear-coat-f0 param.formula=1} Under the coat, the base's F zero changes, because light now enters it from the coat rather than from air.

{@ callout=clear-coat-f0} O3DE uses Filament's form: one minus five root F zero, over five minus root F zero, squared, blended in by the factor.

{@ highlight=response callout=clear-coat-response} The diffuse response shrinks by the coat's Fresnel at the view angle.

{@ highlight=diffuse callout=clear-coat-diffuse param.formula=4} And each light's diffuse shrinks again, by the coat's Fresnel at the half vector.

{@ callout=clear-coat-diffuse} A finding: the coat dims diffuse light twice, once through the response and once per light.

## c8-specular · scene · Two specular lobes
{@ demo=clearcoat callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 12, 3.0], [-14, 8, 3.25], [6, 10, 3.05]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 22.0, "seconds": 5.0, "ease": "sine", "repeat": "pingPong"}}]}

{@ panel=code code=clear-coat-specular highlight=specular callout=clear-coat-specular param.formula=3} Each light's specular becomes the base lobe, times one minus the coat's Fresnel, plus the coat's own lobe.

{@ callout=clear-coat-specular param.formula=2} The coat's lobe is GGX with Kelemen's visibility term, at an alpha of its roughness squared, never below zero point zero zero zero five.

{@ panel=values viewMode=2} So the sphere shows a sharp highlight from the coat, on top of the metal's broad one.

{@ viewMode=0 param.formula=0} {@ clearCoatFactor=0 blend=1} As the factor rises, the sharp highlight grows, {@ clearCoatFactor=1 blend=2} and the metal's colour dims beneath it.

## c8-ibl · scene · The sky's reflection, in the forward pass
{@ demo=clearcoat callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[40, 14, 3.0], [-30, 8, 2.92], [10, 10, 3.1]], "seconds": "beat"}}]}

{@ callout=clear-coat-forward-ibl} Clear coat also moves the material's sky reflections into the forward pass.

{@ callout=forward-ibl-stencil} The full-screen reflection pass knows nothing of the coat, so the mesh is drawn without the specular-IBL stencil value, and that pass skips it.

{@ panel=code code=clear-coat-specular highlight=ibl callout=clear-coat-ibl} In the forward pass, the base's reflection is scaled by one minus the coat's Fresnel, squared, and the coat's own reflection is added.

{@ callout=forward-probe-pick} So the full-screen probe passes never touch a clear-coated mesh; instead the forward pass blends in the smallest probe that overlaps it, between its inner and outer boxes.

## c8-sources · sources · Where to read
sources: clear-coat-default, clear-coat-inputs, clear-coat-f0, clear-coat-response, clear-coat-diffuse, clear-coat-specular, clear-coat-ibl, clear-coat-forward-ibl, forward-ibl-stencil

These entries follow the coat from its inputs to its specular, its diffuse, and its forward-pass IBL.

# 9 · The BRDF texture and multiscatter · 152 s
summary: BRDFTextureCS, the energy single scattering loses, and the compensation.
sources: brdf-texture, lighting-init, multiscatter, ms-option, ggx-sample, brdf-texture-size, rhi-format, brdf-execute-once, ms-material-property, bool-property-default, property-init, property-apply, ms-preset

## c9-loss · scene · Energy that single scattering loses
{@ demo=multiscatter callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 12, 3.0], [-14, 8, 3.25], [6, 10, 3.05]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightY"], "center": [-1.05, 1.24], "degrees": 22.0, "seconds": 5.0, "ease": "sine", "repeat": "pingPong"}}]}

With the coat done, the base's own specular still loses light at high roughness, and a texture O3DE fills at startup puts it back.

A single GGX bounce loses energy on rough surfaces.

Light that strikes one microfacet can bounce to another before it escapes, and the single-scattering formula simply drops it.

{@ multiscatter=0} Here is a rough metal sphere with that loss left in.

{@ multiscatter=1} And here it is with O3DE's compensation: brighter, as a real rough metal would be. {@ pause=0.8}

## c9-texture · scene · The BRDF texture
{@ demo=multiscatter callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 12, 3.0], [-14, 8, 3.25], [6, 10, 3.05]], "seconds": "beat"}}]}

{@ panel=code code=brdf-texture highlight=inputs callout=brdf-texture-size} To know how much is lost, O3DE fills a two hundred fifty six by two hundred fifty six texture, two sixteen-bit floats per texel, once at startup; the lab keeps it in thirty-two-bit floats.

{@ callout=brdf-texture} Each texel stands for one view angle and one roughness.

{@ highlight=sample,weight callout=brdf-texture} {@ ggxSamples=16} It draws one thousand and twenty four GGX samples, reflects the view {@ ggxSamples=64} about each, and weights those above the horizon by the visibility times {@ ggxSamples=256} four, V dot H, and N dot L, over N dot H.

{@ highlight=accumulate callout=brdf-texture} It splits the sum into a scale A and a bias B on F zero.

{@ highlight=store callout=lighting-init} The rows are stored upside down, so lighting looks the texture up by smoothness rather than roughness.

## c9-compensation · scene · Putting the energy back
{@ demo=multiscatter callout="" cameraDrift=0}

{@ panel=code code=multiscatter highlight=lookup,energy callout=multiscatter ggxSamples=0} A plus B is the energy that single scattering keeps.

{@ callout=lighting-init} At roughness zero point eight and this view angle, that is only about fifty seven percent.

{@ highlight=compensate callout=multiscatter} The compensation adds the missing part back, in proportion to F zero, following Turquin's method.

{@ highlight=apply callout=ms-material-property} Its shader option defaults to on, but StandardPBR's material property defaults to off, so a material must turn it on, as O3DE's metal presets do; the lab turns it on.

{@ panel=values viewMode=6 param.formula=1} {@ sphereRoughness=0.2 blend=1.2} In false colour, the {@ sphereRoughness=0.95 blend=1.5} compensation grows with roughness.

## c9-finding · scene · A detail of the texture shader
{@ demo=multiscatter callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 12, 3.0], [-14, 8, 3.25], [6, 10, 3.05]], "seconds": "beat"}}]}

{@ callout=ggx-sample ggxSamples=256 sphereRoughness=0.1 param.formula=0 blend=1} One detail of the texture shader stood out when it was ported.

{@ callout=ggx-sample} Its sampler takes the square root of one minus cos theta squared, without a guard.

On the GPU used for this lab, cos theta can round above one for an almost mirror-like lobe; the square root then gives not a number, and the sample is lost.

So the WGSL port loses energy below a roughness of about zero point zero three.

The lab fills its texture on the CPU instead; whether O3DE's own texture shows the loss depends on the GPU.

## c9-sources · sources · Where to read
sources: brdf-texture, brdf-texture-size, lighting-init, multiscatter, ms-option, ggx-sample

The texture's shader, its size, how lighting reads it, and the compensation are in these files.

# 10 · Transmission (EnhancedPBR) · 152 s
summary: Light through thick and thin objects, and the point light it needs.
sources: transmission-defaults, enhanced-brdf, simple-point-no-backlight, sphere-light-apply, back-lighting, transmission-finalize, sphere-light-shadow-branch, shadow-thickness-none

## c10-why · scene · Light from behind
{@ demo=transmission callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[52, 10, 1.95], [20, 4, 1.7], [38, 8, 1.85]], "seconds": "beat"}}, {"wave": {"param": "sphereLightX", "amplitude": 0.4, "period": 11.0, "seconds": "beat", "fade": 1.0}}]}

So far light has only bounced off surfaces; the next part lets it pass through them: transmission.

Thin or translucent things, like leaves, wax, or skin, let light through from behind.

{@ callout=transmission-defaults} StandardPBR has no transmission; EnhancedPBR adds it, off by default, with a thick-object and a thin-object mode.

{@ callout=enhanced-brdf} Its other lighting is StandardPBR's, as long as subsurface scattering and anisotropy stay off.

{@ teapotTransmission=0} Here the teapot is EnhancedPBR, lit from behind {@ teapotTransmission=1} by a point light; the simple point light is off.

## c10-light · scene · A different kind of point light
{@ demo=transmission callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[52, 10, 1.95], [20, 4, 1.7], [38, 8, 1.85]], "seconds": "beat"}}, {"wave": {"param": "sphereLightX", "amplitude": 0.4, "period": 11.0, "seconds": "beat", "fade": 1.0}}]}

{@ callout=simple-point-no-backlight} Simple point lights add no back lighting at all.

{@ callout=sphere-light-bulb} So the light behind the teapot is O3DE's point light, a sphere area light, with the editor's default radius of five centimetres.

{@ panel=code code=transmission highlight=light callout=sphere-light-apply} It lights like a simple point light, but moves its specular direction to the point of its bulb nearest the reflection ray, and adds back lighting.

Here it casts no shadows.

## c10-thick · scene · Thick objects
{@ demo=transmission callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[52, 10, 1.95], [20, 4, 1.7], [38, 8, 1.85]], "seconds": "beat"}}]}

{@ panel=code code=transmission highlight=thick callout=back-lighting param.formula=1} Thick mode follows Barré-Brisebois and Bouchard: how directly the light shines through toward the viewer.

{@ callout=transmission-defaults} The view direction is compared with the light's direction bent by the normal, raised to a power of six, and scaled.

{@ callout=back-lighting param.formula=2} Then it is attenuated through the thickness: e to the minus four times the thickness, times one minus the thickness.

{@ callout=back-lighting} The thickness is the larger of the shadow map's and the material's; this teapot uses zero point one five, where the default is one half.

{@ highlight=tint callout=transmission-finalize param.formula=3} The sum over lights, times the tint, joins the diffuse light after the occlusion multiply, so occlusion never dims it.

{@ panel=values param.formula=0} {@ teapotTransmission=0} Without transmission the teapot's back is {@ teapotTransmission=1} dark; with it, the light glows through.

## c10-thin · scene · Thin objects, and a finding
{@ demo=transmission callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[52, 10, 1.95], [20, 4, 1.7], [38, 8, 1.85]], "seconds": "beat"}}, {"wave": {"param": "sphereLightX", "amplitude": 0.4, "period": 11.0, "seconds": "beat", "fade": 1.0}}]}

{@ panel=code code=transmission highlight=thin callout=back-lighting teapotTransmission=2} Thin mode follows Jimenez and others: a diffusion profile over the distance the shadow map measures through the object.

{@ callout=back-lighting} Its comment says that without shadow data it gives no back lighting.

{@ callout=sphere-light-shadow-branch} A finding: for point lights, the shadow branch runs even for a light without shadows, because its options default to true.

{@ callout=shadow-thickness-none} There the shadow lookup returns a thickness of zero, not the negative value that means no data.

{@ panel=values callout=sphere-light-shadow-branch} So a thin object behind a shadowless point light gets the kernel's full value, the brightest back lighting it can have.

## c10-sources · sources · Where to read
sources: transmission-defaults, transmission-option, transmission-params, enhanced-brdf, simple-point-no-backlight, sphere-light-apply, sphere-light-bulb, back-lighting, transmission-kernel, transmission-finalize, sphere-light-shadow-branch, sphere-light-shadow-option, per-mesh-flags, shadow-thickness-none

These entries cover transmission's inputs, the sphere light, both modes, and the shadow-branch finding.

# 11 · The sun's cascades · 220 s
summary: The directional light, its four cascades, and the one shadow value an opaque pixel reads.
sources: directional-lux, directional-angular-unused, one-shadow-light, cascade-defaults, cascade-splits, cascade-sphere-fit, cascade-snap, cascade-select, fullscreen-shadow, fullscreen-shadow-format, area-lights

## c11-light · scene · Lux, and no size
{@ demo=cascades callout="" cameraDrift=1 cascadesShown=0 cascadeSpheres=0 cascadeBoxes=0 readoutRows=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 36, 14.5], [48, 30, 13.5], [30, 38, 14.0]], "seconds": "beat"}}]}

Point lights reach a few metres; the light that reaches everything is the directional one, the sun, and its shadows are the next part.

{@ callout=directional-lux} Its intensity is an illuminance, in lux: the component's default of four EV one hundred is forty lux.

{@ callout=directional-angular-unused} It also has an angular diameter, half a degree by default; a finding: the forward pass adjusts a direction for it, then passes the plain direction to the specular, so there the size changes nothing.

{@ callout=one-shadow-light} Only one directional light casts shadows; any others are lit without them.

{@ callout=cascade-defaults} The lab has no directional light, so this chapter draws O3DE's arithmetic on the CPU, for an observer camera standing in the room.

## c11-splits · scene · Four slices of the view
{@ callout="" cascadesShown=0 cascadeSpheres=0 cascadeBoxes=0 shadowFar=12 cascadeSplitRatio=0.9}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 36, 15.5], [48, 30, 14.5], [30, 38, 15.0]], "seconds": "beat"}}]}

{@ panel=code code=cascades highlight=far callout=cascade-defaults} The shadow covers the camera's view out to the light's shadow distance, a hundred metres by default, or the camera's far plane if that is nearer; here it is twelve, so the cascades fit around the room.

{@ highlight=split callout=cascade-splits} {@ cascadesShown=1 blend=1.2} The view is cut into {@ cascadesShown=4 blend=4} four cascades, at depths that blend a logarithmic split with a uniform one.

{@ panel=values readoutRows=8 param.formula=2} One map for the whole view would spend as many texels far away as close by; the default ratio, zero point nine, leans toward the logarithmic split, so the first cascade is short and sharp, and the ones after it longer and longer.

{@ cascadeSplitRatio=0 blend=2} At a ratio of zero the slices are equal; {@ cascadeSplitRatio=1 blend=2} at one, each is a constant factor longer than the last.

{@ cascadeSplitRatio=0.9 blend=1.5} {@ shadowFar=40 blend=3} Raise the shadow distance, and every slice grows with it, so each texel covers more of the room.

## c11-fit · scene · A sphere for each slice, snapped to texels
{@ callout="" cascadesShown=4 shadowFar=12 cascadeSplitRatio=0.9 readoutRows=99 param.formula=0 blend=1.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 36, 15.5], [48, 30, 14.5], [30, 38, 15.0]], "seconds": "beat"}}]}

{@ panel=code code=cascades highlight=sphere callout=cascade-sphere-fit} {@ cascadeSpheres=1 blend=1.5} Each slice is fitted with a bounding sphere, centred where its near and far corners are equally far away, or on its far face when that point lies beyond it, as it does for the room's first three.

{@ highlight=box callout=cascade-sphere-fit} {@ cascadeBoxes=1 blend=1.5} In the light's frame, the cascade's box is that sphere's cube, stretched toward the light to catch the casters between it and the view, and behind the camera.

{@ highlight=snap callout=cascade-snap} Its corners are then floored to whole texels of the thousand-and-twenty-four-texel map.

{@ observerYaw=30 blend=5} A sphere's size does not change as the camera turns, so each box keeps its size, and snapping moves it only in whole texels, which keeps the shadow's edges from shimmering.

{@ panel=values param.formula=4} {@ observerYaw=0 blend=3} The readout gives each cascade's radius and its texel: millimetres close by, centimetres far away.

## c11-select · scene · A pixel picks its cascade
{@ callout="" cascadesShown=4 cascadeSpheres=0.4 cascadeBoxes=0.4 observerYaw=0 param.formula=0 blend=1.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 36, 15.5], [48, 30, 14.5], [30, 38, 15.0]], "seconds": "beat"}}]}

{@ panel=code code=cascade-select highlight=offset callout=cascade-select} To find its shadow, a pixel first moves along its normal by two and a half times two over the map's size, about five millimetres.

{@ highlight=select callout=cascade-select} Then it takes the first cascade whose map holds it at least one and a half texels inside the edge: the choice is by the map's bounds, not by the pixel's depth.

{@ panel=values} The readout shows which cascade the inspected point on the sphere lands in, and its texel there.

{@ panel=code code=cascade-select highlight=fullscreen callout=fullscreen-shadow} Opaque pixels never sample the maps in the forward pass: a full-screen pass writes each pixel's visibility into its own target, blurs it, and the forward pass reads that one value.

{@ callout=fullscreen-shadow-format} That target is eight-bit, at the output's size: one value per pixel, not one per sample.

## c11-others · scene · The forward pass's other lights
{@ demo=room callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[23, 22, 5.8], [-6, 12, 6.8], [20, 19, 6.1]], "seconds": "beat"}}]}

{@ callout=direct-lighting} After the directional lights, the forward pass runs the simple lights, then five kinds of area light.

{@ callout=area-lights} Quad lights are integrated with linearly transformed cosines, from two thirty-two by thirty-two tables, or with a faster approximation a light can ask for.

Disk, capsule, and polygon lights have methods of their own; the lab ports none of them, and polygon lights, as chapter two showed, never shade while culling is on.

## c11-sources · sources · Where to read
sources: directional-lux, directional-angular-unused, one-shadow-light, cascade-defaults, cascade-splits, cascade-sphere-fit, cascade-snap, cascade-select, fullscreen-shadow, fullscreen-shadow-format, area-lights

Read the feature processor for the split, the fit, and the snap, then the shadow calculator for a pixel's choice.

# 12 · The global skylight · 210 s
summary: Offline GGX and cosine prefiltering; the two full-screen passes; no occlusion.
sources: filter-ggx, filter-cosine, forward-targets, diffuse-global, reflection-global, reflection-target, reflection-composite, ibl-srg, ssao-default, ao-defaults

## c12-sky · scene · Light from the sky
{@ demo=ibl callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[44, 14, 3.4], [-4, 8, 3.9], [26, 12, 3.5]], "seconds": "beat"}}]}

That covers the lights the forward pass evaluates; what the opaque frame adds after it comes from the sky.

The second source of light is the sky.

{@ callout=ibl-srg} O3DE's global skylight is image-based lighting: an environment, stored as two cubemaps.

{@ vectorsShown=5} This chrome sphere reflects it directly; the lab's sky is a photograph, Poly Haven's Tree Lined Driveway, with its sun clipped.

## c12-bake · mermaid · Baked offline
diagram: bake
focus: GGX, COS

The cubemaps are baked offline, by O3DE's image processor.

{@ callout=specular-preset} A GGX filter turns the environment into a specular cubemap with two hundred fifty six texel faces in R9G9B9E5, and six mips, one for each step of roughness.

{@ callout=diffuse-preset} A cosine filter turns it into a diffuse cubemap of thirty two to one hundred twenty eight texels a face; the lab's is thirty two.

And, separately, the BRDF texture from chapter nine supplies the split-sum terms.

## c12-prefilter · scene · Prefiltering the specular cubemap
{@ demo=ibl callout="" cameraDrift=0}

{@ panel=code code=ibl-prefilter highlight=roughness callout=cubemap-ggx} Mip m of the specular cubemap is filtered for a roughness of m over five.

{@ highlight=frame callout=filter-ggx} For each texel, the filter assumes that the normal and the view both point along the texel's direction.

{@ highlight=sample callout=cubemap-ggx} {@ ggxSamples=32} Then it takes two {@ ggxSamples=128} hundred fifty six {@ ggxSamples=256} GGX samples around it.

{@ highlight=level callout=filter-ggx} Each sample is read one mip blurrier than the mip whose texels match its solid angle, blended between two mips, which hides the noise of so few samples.

{@ highlight=accumulate callout=filter-ggx} {@ sphereRoughness=0.4 blend=2} Rougher mips gather a {@ sphereRoughness=0.8 blend=2} wider cone, so reflections blur.

## c12-cosine · scene · The diffuse cubemap
{@ demo=ibl callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[44, 14, 3.4], [-4, 8, 3.9], [26, 12, 3.5]], "seconds": "beat"}}]}

{@ callout=filter-cosine ggxSamples=0 sphereRoughness=0.2 sphereMetallic=0 blend=1} The diffuse cubemap is a cosine-weighted average over the whole hemisphere, normalised by its weights.

{@ callout=diffuse-global} So it stores irradiance divided by pi, and the diffuse pass multiplies it by albedo with no division.

{@ viewMode=10} Seen alone, the diffuse light from the sky is soft, and coloured by the ground below and the blue above.

## c12-passes · scene · The two full-screen passes
{@ demo=ibl callout="" cameraDrift=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[44, 14, 3.4], [-4, 8, 3.9], [26, 12, 3.5]], "seconds": "beat"}}]}

{@ panel=code code=ibl-passes highlight=targets callout=target-formats viewMode=0} At run time, the forward pass has already stored albedo times the diffuse response and occlusion, F zero, and roughness, each in eight bits.

{@ highlight=diffuse callout=diffuse-global viewMode=10} The diffuse pass multiplies that albedo by the diffuse cubemap at the normal; under the default template, DiffuseComposite does the same.

{@ highlight=reflect callout=reflection-global viewMode=11} The reflection pass reads the specular cubemap along the reflection vector, at mip roughness times five.

{@ highlight=split callout=reflection-global} It applies the split sum, F zero times A plus B, with the compensation and N dot V held at one hundredth or more, then the skylight's exposure and the specular occlusion.

{@ callout=reflection-composite viewMode=0} The diffuse pass adds straight onto the diffuse target; the reflection pass writes its own target, which a composite pass adds onto the specular one.

## c12-readout · scene · The terms at one point
{@ demo=ibl callout="" readoutRows=0 cameraDrift=0}

{@ panel=values callout=roughness-mip readoutRows=11 param.formula=0} At the inspected point, the readout shows each step: the eight-bit roughness, its mip, the prefiltered light, and the split sum.

{@ callout=roughness-mip param.formula=1} {@ sphereRoughness=0.1 blend=1.5} As roughness rises, the mip rises with {@ sphereRoughness=0.6 blend=2} it, and the reflection dims and spreads.

## c12-occlusion · scene · No shadows from the sky
{@ demo=room callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ callout=ssao-default} The global skylight casts no shadows: only occlusion maps and screen-space ambient occlusion, which is on by default, dim it.

{@ callout=ssao-default viewMode=0} The lab's room has no occlusion maps, so inside it only SSAO darkens the sky's light, in the corners and under the objects.

{@ callout=reflection-global} For interiors, O3DE replaces the sky's reflections with reflection probes, in the next chapter, and its diffuse light with a diffuse probe grid, which the lab leaves out.

## c12-sources · sources · Where to read
sources: diffuse-composite, ibl-forward-transparent, forward-targets, target-formats, diffuse-global, reflection-global, filter-ggx, filter-cosine, vect-to-texel

The bake, the targets, and the two passes are in these files; CubeMapGen, the filter library, is AMD's, under a BSD license.

# 13 · Reflection probes · 270 s
summary: Probe cubemaps, their stencil and blend weights, parallax, and the composite.
sources: reflections-order, probe-stencil, probe-blend-weight, reflection-global-weight, probe-render-outer, probe-render-inner, probe-specular, parallax-obb, reflection-clear, probe-bake-capture, probe-bake-preset, probe-visualization-surface

## c13-why · scene · Reflections of the room
{@ demo=reflections callout="" cameraDrift=1 probes=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[23, 22, 5.8], [-6, 12, 6.8], [20, 19, 6.1]], "seconds": "beat"}}]}

The sky's cubemaps light the whole scene alike; the next part, reflection probes, gives places reflections of their own.

Inside a room, reflections of an open sky look wrong: the walls should be in them.

{@ callout=probe-defaults probes=1} Reflection probes fix that: each is a cubemap baked from a point in the scene, with an inner and an outer box.

The lab places two, and bakes them itself on the CPU, shading the room from each probe's centre.

{@ callout=cubemap-quality} Their cubemaps have two hundred fifty-six texels a face, as the default Medium bake quality makes them, and are prefiltered like the sky's.

{@ probes=0} On the polished metal floor, the probes put {@ probes=1} the room's own walls into the reflections.

## c13-bake · scene · What a probe bakes
{@ demo=reflections callout="" cameraDrift=1 probeBakeFaces=0 probeBoxes=0.6}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 18, 5.8], [12, 14, 6.2]], "seconds": "beat"}}]}

{@ callout=probe-bake-capture} Baking a probe renders the scene from the probe's position six times, once for each face of a cube, each a square view ninety degrees wide.

{@ panel=hidden callout=probe-bake-faces probeCubemap=1 blend=1} {@ probeBakeFaces=1 blend=0.4} O3DE draws one face {@ probeBakeFaces=2 blend=0.4} at a time, into a {@ probeBakeFaces=3 blend=0.4} sixteen-bit float target of {@ probeBakeFaces=4 blend=0.4} one thousand and twenty-four {@ probeBakeFaces=5 blend=0.4} texels, waits five frames, reads it {@ probeBakeFaces=6 blend=0.4} back, and turns to the next.

Unfolded, probe one's faces show the back wall ahead, the red wall to its left, the teal wall to its right, the sky above, and the floor below.

{@ callout=probe-bake-write} The editor writes the six faces to a DDS file, and O3DE's image builder turns it into the probe's cubemap with the skylight's own specular preset.

{@ callout=probe-bake-preset} {@ probeCubemapMip=1 blend=0.8} At the default Medium quality, {@ probeCubemapMip=2 blend=0.8} that is two hundred fifty-six {@ probeCubemapMip=3 blend=0.8} texels a face in R9G9B9E5, and {@ probeCubemapMip=4 blend=0.8} six mips, each filtered with {@ probeCubemapMip=5 blend=0.8} GGX for a rougher surface.

{@ probeCubemapMip=0 blend=1.2} The lab skips the large capture: it shades the two hundred fifty-six texel faces directly, on the CPU.

{@ probeShown=2} Probe two, a smaller one in the back-right corner, has the back wall and the teal wall close by. {@ pause=0.6}

## c13-ball · scene · The visualization sphere
{@ demo=reflections panel=hidden callout="" cameraDrift=1 probeCubemap=1 probeBoxes=0 probeBallRadius=0.3}

{@ callout=probe-visualization probeCubemap=0 probeBall=1 blend=1.2} {@ camera.target=0,1.2,-0.1 camera.yaw=-20 camera.pitch=12 camera.distance=2 blend=3 ease=inOut} In the editor, every probe also shows a sphere at its position, on by default.

{@ callout=probe-visualization-surface animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[-38, 8, 1.9], [2, 16, 2.2], [-18, 10, 2.0]], "seconds": "beat"}}]} Its material is a white metal with a roughness of zero, so it mirrors the sharpest mip of the probe's cubemap.

{@ callout=probe-visualization-hidden} The capture leaves the sphere out, so a probe never bakes its own sphere.

{@ probeBallMip=2 blend=1} The lab draws its own mirror ball, {@ probeBallMip=4 blend=1} which reads the cubemap straight along the reflection {@ probeBallMip=0 blend=1} ray; at higher mips, its reflection blurs. {@ pause=0.6}

## c13-stencil · scene · Stencil and blend weight
{@ demo=reflections callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=reflection-probes highlight=stencil callout=reflections-order} Five passes run after the forward pass: a stencil pass, a blend-weight pass, the global pass, and an outer and an inner probe pass.

{@ callout=probe-stencil viewMode=13} The stencil pass draws each probe's inner box, slightly reduced, and adds one to the stencil of every surface inside it that uses the reflection passes.

{@ callout=probe-stencil-reduced} Surfaces outside every inner box keep the value three.

{@ highlight=weight callout=probe-blend-weight viewMode=14 param.formula=1 probeInnerScale=0.7 blend=1} There, the blend-weight pass adds each probe's weight: one at its inner box, falling to zero at its outer one.

{@ callout=probe-blend-weight} Where probes overlap, the weights add up.

## c13-blend · scene · Blending the probes
{@ demo=reflections callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=reflection-probes highlight=global callout=reflection-global-weight viewMode=0 param.formula=2} The global pass then runs only outside the inner boxes, and keeps what the probes leave: one minus their summed weight, with the sum capped at one.

{@ highlight=outer callout=probe-render-outer param.formula=3} The outer pass adds each probe's reflection times its weight, divided by the total wherever that exceeds one.

{@ highlight=inner callout=probe-render-inner} Inside an inner box, the inner pass simply replaces the reflection with the probe's.

{@ callout=probe-sort} Probes are drawn largest first, so a smaller probe inside a larger one has the last word.

{@ callout=probe-specular} Unlike the global pass, the probes apply no specular occlusion.

{@ panel=values viewMode=14 param.formula=0} {@ probeInnerScale=0.4 blend=1.5} Shrinking the inner boxes widens {@ probeInnerScale=1 blend=1.5} the bands where probes blend.

## c13-parallax · scene · Parallax correction
{@ demo=reflections callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[24, 22, 5.6], [8, 14, 6.4], [21, 18, 6.0]], "seconds": "beat"}}]}

{@ panel=code code=probe-specular highlight=parallax callout=probe-specular viewMode=0} A cubemap is captured from one point, but surfaces are everywhere in the box.

{@ callout=parallax-obb} So by default the reflection ray is traced to the outer box, and the cubemap is read toward that hit from the probe's centre.

{@ callout=parallax-obb} A probe's outer box should therefore fit its room: whatever the probe saw beyond the box is drawn as if it lay on the box, and walls appear in the wrong place.

{@ panel=values} Without that correction, {@ probeParallax=0} the reflected walls slide out of place; {@ probeParallax=1} with it, they line up again.

## c13-composite · scene · The composite, and a finding
{@ demo=reflections callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=reflection-probes highlight=composite callout=composite-stencil} The composite pass then adds the reflection target onto the specular light, wherever the stencil marks a surface.

{@ callout=reflection-clear} A finding: that target is cleared to a grey of zero point four, and the global pass never runs inside the inner boxes.

{@ callout=reflection-clear ibl=0} So with image-based lighting switched off, the inner pass discards, and surfaces inside an inner box get that grey added to their specular.

{@ panel=values param.formula=-1} The lab reproduces it, as its readout shows.

## c13-sources · sources · Where to read
sources: reflections-order, probe-stencil, probe-stencil-reduced, probe-blend-weight, reflection-global-stencil, reflection-global-weight, probe-render-outer, probe-render-inner, probe-specular, parallax-obb, obb-lerp, probe-sort, probe-defaults, reflection-clear, composite-stencil

These entries follow the five reflection passes in order, the probe's specular, and the composite.

# 14 · Resolve, ambient occlusion, and the merge · 205 s
summary: The custom specular resolve, SSAO at half resolution, and the merge.
sources: msaa-resolve-custom, resolve-contrast, ao-placement, ssao-default, depth-linear, depth-downsample, ssao-compute, ssao-defaults, blur-step, blur-defaults, depth-upsample, ssao-modulate

## c14-resolve · scene · Specular resolves with a clamp
{@ demo=ssao callout="" panel=hidden viewMode=0 ssao=1 specularResolve=0 sphereRoughness=0.06 camera.target=-1.05,1.1,-0.55 camera.yaw=24 camera.pitch=14 camera.distance=2.4 cameraDrift=0 blend=3 ease=inOut}

With the probes composited, every term sits in two targets, at several samples a pixel; the last passes resolve them, darken the diffuse, and add the two.

{@ callout=msaa-resolve-custom} The diffuse target resolves as a plain average, but the specular one goes through a custom resolve.

{@ callout=resolve-contrast} Each sample is first scaled down to at most one and a half times the brightest luminance among the samples of its four diagonal neighbours.

{@ callout=msaa-resolve-custom} Then the samples are averaged in the log domain, a geometric mean, so one bright sample cannot dominate an edge.

{@ specularResolve=1 viewMode=16} In false colour, its difference from a plain average, from nought to ten percent, lights only where samples differ sharply: rims, and the smallest highlights.

{@ viewMode=0} The lab renders four samples where O3DE's pipeline renders two, and resolves them the same way.

## c14-why · scene · Corners and creases
{@ demo=ssao callout="" cameraDrift=1 viewMode=0 ssao=0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[23, 22, 5.8], [-6, 12, 6.8], [20, 19, 6.1]], "seconds": "beat"}}]}

Once the samples are resolved, ambient occlusion darkens the diffuse target.

Light from the sky reaches into corners and creases as brightly as onto open floor.

{@ callout=ssao-default ssao=1 blend=1} Screen-space ambient occlusion darkens those places from the depth buffer alone, and the main pipeline runs it by default.

{@ callout=ao-placement} It runs after the reflections and subsurface scattering, and multiplies only the diffuse target, direct light and sky alike.

{@ callout=ssao-modulate viewMode=15} This is its result, white where nothing occludes.

## c14-compute · scene · Fifty-two samples
{@ demo=ssao callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=ssao highlight=half callout=depth-linear} It starts from linear depth: the view-space depth of each pixel's first MSAA sample.

{@ callout=depth-downsample} Then it works at half resolution, keeping the upper-left pixel of every two by two.

{@ highlight=compute callout=ssao-compute param.formula=1} SsaoCompute rebuilds each pixel's position and normal from depth, and gathers fifty-two samples around it: twelve close by, and ten rotations of four shared with its neighbours.

{@ callout=ssao-compute} Each sample adds the cosine between the normal and the direction to it, weighted by one over its squared distance plus zero point zero seven, and the occlusion is their weighted average.

{@ callout=ssao-format param.formula=2} The result is one minus the occlusion times the strength, raised to the power two point two, and stored in eight bits.

{@ callout=ssao-defaults} The strength defaults to one, and the sampling radius to five percent of the half-resolution height.

{@ panel=values param.formula=0} {@ ssaoRadius=0.12 blend=1.2} A wider radius reaches farther and darkens more broadly; {@ ssaoRadius=0.03 blend=1.5} a narrow one keeps to the tightest creases.

## c14-blur · scene · A depth-aware blur
{@ demo=ssao callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=ssao highlight=blur callout=blur-order} The raw result is noisy, so a depth-aware blur smooths it, down and then across.

{@ callout=blur-step} It runs from pixel to pixel, keeping two thirds of the value before, and less where the depth's slope changes, so edges stay sharp.

{@ callout=blur-step} Each output is half the chain from eight pixels before it and half the chain from eight after.

{@ callout=blur-defaults} Those values are the blur pass's own defaults, used when the scene has no ambient-occlusion component; the component's are zero point eight five and two hundred.

{@ panel=values} {@ ssaoBlur=0} Without the blur, the samples' pattern shows through; {@ ssaoBlur=1} with it, the pattern is gone.

## c14-upsample · scene · Back to full resolution
{@ demo=ssao callout="" cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 21, 5.9], [4, 13, 6.7], [20, 18, 6.2]], "seconds": "beat"}}]}

{@ panel=code code=ssao highlight=upsample callout=depth-upsample} DepthUpsample brings it back to full resolution from the four nearest half-resolution pixels, weighted by distance and by how close their depths are.

{@ callout=ssao-modulate param.formula=3 viewMode=0} Then the diffuse target is multiplied by it, and the merge pass adds the specular light, untouched.

{@ panel=values} The lab checks each of these stages on the GPU against a CPU copy, to within one eight-bit step.

Because it only sees depth, SSAO knows nothing of what lies off screen or behind the nearest surface.

## c14-sources · sources · Where to read
sources: msaa-resolve-custom, resolve-contrast, ao-placement, ao-parent, ssao-default, ao-defaults, ssao-order, depth-resolve, depth-linear, depth-downsample, ssao-compute, ssao-format, ssao-defaults, blur-order, blur-step, blur-defaults, blur-component-defaults, depth-upsample, ssao-modulate

These entries follow the specular resolve, then SSAO from the linear depth through the samples, the blur, and the upsample to the diffuse multiply.

# 15 · Recap · 75 s
summary: What StandardPBR does, and what the lab leaves out.
sources: light-culling-pass, evaluate-lighting, cascade-splits, opaque-pass-order, msaa-resolve-custom, ssao-default, msaa-default

## c15-recap · scene · The opaque frame, in summary
{@ demo=room callout="" labels=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[23, 22, 5.8], [-6, 12, 6.8], [20, 19, 6.1]], "seconds": "beat"}}, {"orbit": {"params": ["lightX", "lightZ"], "center": [0.0, -0.3], "degrees": 300.0, "seconds": "beat", "ease": "sine"}}]}

That is every part of the opaque frame; here it is once more, in order.

{@ callout=light-culling-pass} A depth pre-pass, then light culling sorts the lights and decals into tiles and depth bins.

{@ callout=cascade-splits} The directional light's cascades are fitted to the view, and its shadow is resolved into one value a pixel.

{@ callout=evaluate-lighting} The forward pass applies the decals to the surface, then each light: GGX specular, a Lambertian diffuse scaled by its response, clear coat where enabled, and transmission for EnhancedPBR.

{@ callout=opaque-pass-order} Full-screen passes add the skylight and the reflection probes, read back from the forward pass's targets.

{@ callout=msaa-resolve-custom} The samples are resolved, specular with its neighbour clamp, SSAO darkens the diffuse, and the merge adds the two.

The lab ports these formulas to WGSL, checks each one against a CPU copy, and cites every line it follows.

{@ callout=msaa-default} It leaves out rendered shadows, the area lights, screen-space reflections, which are off by default, and the diffuse probe grid.

Every demonstration in this video can be explored in the viewer, with its sliders, its pseudo-code, and its sources side by side.

Thank you for watching.

