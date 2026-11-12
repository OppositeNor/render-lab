# Worked examples: mapping and fixing a passage

Load this when you write or fix a passage with `references/storytelling.md`. Each example shows a draft, its map, the
faults the map shows, and the fixed passage with its map. Sections 1, 2, and 4 are written for these examples;
section 3 quotes the published stories' storyboards.

## 1. A draft written from the code

Draft, one sentence per line:

1. Cascaded shadow map texel density is optimised via logarithmic split distribution across frustum partitions.
2. Each cascade has its own projection.
3. This avoids perspective aliasing near the camera.

Map:

| # | Known | New | Job |
|---|---|---|---|
| 1 | nothing: every noun is new | cascades, texel density, logarithmic splits, partitions | none clear (a name, an act, and a purpose at once) |
| 2 | "cascade", named but never shown | its own projection | act |
| 3 | "this", which could mean sentence 1 or 2 | perspective aliasing | purpose, but of a problem never stated |

Faults: sentence 1 is a compressed process with no actor, and it brings four new things. The names come before what
they name. The problem (aliasing) arrives last, after its fix. "This" has two possible referents.

Fixed, by contrast, with the problem first:

1. The camera sees near objects large and far objects small.
2. One shadow map covers both with the same texels, so it wastes them far away and runs short up close.
3. Up close, the shadow's edge breaks into blocks.
4. So we split the view into slices, and give each slice a shadow map of its own.
5. Each slice is longer than the one before it, so the near ones get the most texels.
6. These slices are called cascades, and spacing them this way is called a logarithmic split.

| # | Known | New | Job |
|---|---|---|---|
| 1 | the camera, on screen | near is large, far is small | point |
| 2 | near and far, from 1 | one map wastes texels far and runs short near | act, with its consequence ("so") |
| 3 | "up close", from 2 | blocky edges, shown | check |
| 4 | the blocky edges, from 3 ("so") | slices, each with a map | act |
| 5 | the slices, from 4 | growing lengths, more texels near | act, with its purpose |
| 6 | the slices and their spacing, from 4 and 5 | the names | name, which also compresses 4 and 5 |

## 2. A chain that breaks

Draft:

1. The ray steps forward from the pixel, one texel at a time.
2. Thickness matters because thin objects would otherwise block the ray.
3. At each step it compares its depth with the depth buffer.

Map:

| # | Known | New | Job |
|---|---|---|---|
| 1 | the pixel, on screen | the ray steps forward | act |
| 2 | nothing: "thickness" is not in 1 | thickness, thin objects blocking | purpose of a step not yet told |
| 3 | "it", the ray from 1, two sentences back | the depth comparison | act |

Faults: sentence 2 starts from something never said, and gives the purpose of a test the listener has not heard. The
chain jumps from 1 to 3 across it, and "it" in 3 has lost its referent.

Fixed, in the order the process runs:

1. The ray steps forward from the pixel, one texel at a time.
2. At each step, the ray compares its depth with the depth buffer.
3. When the ray falls behind the stored depth, it may have hit a surface.
4. But a thin object would then block every ray that passes behind it.
5. So the ray also checks how far behind it is, and counts a hit only within a set thickness.
6. This test is called the thickness check.

Each sentence now starts from the end of the one before. The purpose (4, "but") comes right before the step it
explains (5). The name comes last (6).

## 3. The faults a revision finds most, and their fixes

A review of the published stories found these patterns again and again. Each pair is a sentence before and after
its fix, from the storyboards (slashes separate sentences, each its own paragraph).

**"X, and Y": a second new idea hung on with a comma.** The commonest fault. Split it, the second sentence starting
from the first's end.

| Before | After |
|---|---|
| Specular anti-aliasing is on by default, and the extra roughness is capped at zero point one eight. | Specular anti-aliasing is on by default. / The extra roughness it adds is capped at zero point one eight. |
| The halves alternate from frame to frame, and they are shown here in two colours. | The halves alternate from frame to frame. / They are shown here in two colours. |

**A topic turn with no bridge.** The second half of a beat starts a new subject. One short sentence that names the
turn changes no claim.

| Before | After |
|---|---|
| Directional lights are not culled at all. | One kind of light skips the lists. / Directional lights are not culled at all. |
| In O3DE twenty-six oh five, the forward pass first sets up its lighting data from the original surface. | But the order of work matters. / In O3DE twenty-six oh five, ... |
| Up close, detail materials replace the macro one, blended by plain weights. | So much for the macro material. / Up close, detail materials replace it, ... |

Other bridges that worked: "One more case is ...", "Back to how the map is made, ...", "The copy has one more
effect.", "The resolve has one more job."

**A name before what it names.** Tell the act, then the name, at the end of the sentence or in the next one.

| Before | After |
|---|---|
| SDFGI finds each cell's nearest voxel with a jump flood. / In a jump flood, each cell holds a seed. / A seed is the nearest solid voxel the cell has found so far. | To find each cell's nearest voxel, every cell keeps a seed. / A seed is the nearest solid voxel the cell has found so far. / Cells then pass their seeds to each other, in a method called a jump flood. |
| The normal target stores the normal in signed octahedral form. | The normal target packs each normal into two numbers and a sign. / That packing is called signed octahedral form. |
| Then an exact distance transform runs along every column of cells in the z direction. | Then a pass runs along every column of cells in the z direction, and finds each place's distance within its column. / ... / Together, these three passes are called an exact distance transform, ... |

**A term the listener cannot unpack.** Gloss a general term where it is first said, in an appositive.

| Before | After |
|---|---|
| ... one thread group of two hundred and fifty-six threads for each tile. | ... one thread group for each tile, a team of two hundred and fifty-six threads that the GPU runs together. |
| ... by a smoothstep of the angle. | ... by a smoothstep of the angle, a curve that eases the fade in and out. |

**The abstract rule before the example.** Put the anchor first, and the rule after it, as the landing.

| Before | After |
|---|---|
| The CPU stores these light lists in slots ... / But the code that reads them counts the slots differently ... / The two counts agree only when ... / Suppose only the second cascade moves. / ... | The CPU stores these light lists in slots ... / Suppose only the second cascade moves. / The code that reads the lists counts the slots differently ... / ... / The two counts agree only when the cascades that moved are the first ones, in order. |

**A rule the steps obey, never said.** A run of steps ("the ray climbs ... then hits bring it down ...") needs its
rule first: "So a hit lowers the level by one, and a miss raises it by one."

**A "so" with the wrong cause.** "Advances by half its width, so it widens as it goes" says the step makes the cone
wider. The cone widens because of its shape: "The cone widens as it goes, and on each step it advances by half its
current width." Check every "so" against what really causes what.

**A gap in a chain of numbers or steps.** "From sixteen texels a cell to eight, then four, then two" followed by "At
one texel a cell" skips a link: "... then two, then one."

**A purpose several sentences late.** Put it next to its step: "A stencil pass counts how many inner boxes hold each
pixel's surface, so later passes know how deeply each pixel's probes are nested."

**A participle or a long phrase before the subject.** "Switched off, the far highlight thins" becomes "If it is
switched off, the far highlight thins". "Divided by two hundred fifty-five, that is ..." becomes "Dividing it by
two hundred fifty-five gives ...".

**One word for two things, or two words for one.** Give each thing one noun, and two things two nouns: the dirty
"slab" was also "new part" and "region" (pick one per thing); "fade" named four different fades; "cube" named both the sixteen-cell cube
around a probe and the cube with a probe at each corner (now "the probe cube"). When a word names two quantities,
name each and keep them apart. When the scene already holds an object of the same kind, qualify the new one every
time.

**A formula read in one nested breath.** Say it in its true grouping, and as a division where it is one: "it
replaces each colour with the colour over one plus its luminance" becomes "it divides each colour by one plus its
luminance". A sum of nested terms becomes two sentences: one term's value, then the aggregate of those values.

**A contradiction between two sentences.** "Without SDFGI, the opaque pass fills the shadows with the sky's
ambient light" and, three sentences later, "Without SDFGI, ... the shadows are black." Read the code before
rewording: the lab's port adds no sky ambient, so the second sentence becomes a stated lab difference ("The lab's
port leaves the sky's ambient light out, so without SDFGI ...").

## 4. Compressing, then using the noun

After a process has been told in verbs, the noun can carry it. A later chapter says "the thickness check skips this
wall" and the listener unpacks it, because they heard sentences 3 to 5 above. The same sentence in the first chapter,
before the process is told, would be a label with nothing inside. When a chapter uses a noun from an earlier one,
recall it in a few words the first time ("the thickness check, the test that ignores surfaces far in front of the
ray").
