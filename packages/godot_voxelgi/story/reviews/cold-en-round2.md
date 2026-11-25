# Cold listener, English, round 2

Format: beat id, sentence number, first five words, problem kinds, what I lost.

## Chapter 1 (6 flags)
- c1-map-bake #4 "The bake turns the meshes": 5. Octree, tree of cells and eight-way split arrive in one sentence.
- c1-map-sdf #2 "Later, that field helps": 4. "Which cells a light cannot reach" gives no idea what it changes.
- c1-map-light #1 "At run time, compute": 2. "The smallest cells" has not been named or defined.
- c1-gate #6 "The Mobile renderer never": 1. "Records which VoxelGI covers an object" means nothing to me yet.
- c1-gate #9 "Each VoxelGI's box is": 4. 128 cells along the axis, but I do not know why it matters.
- c1-gate #10 "But one project setting": 6. "But" fights the "no setting" two sentences back, and the cell count in between hides the link.

Understood: VoxelGI bakes the room into cells once, lights them when things change, and each pixel reads them back with a few cones. It works only in Forward+. The lab's box has 5 cm cells.

## Chapter 2 (10 flags)
- c2-collect #11 "In the lab, the": 4. The panel being an area light: no reason given yet.
- c2-collect #12 "The lab leaves the panel's": 4. Why leave the panel out, and what changes?
- c2-cube #5 "Half of six point": 3. "Still covers that height, so the count is halved" took a re-parse.
- c2-cube #6 "Half again, one point": 3. Same halving logic, still unresolved.
- c2-cube #7 "So the voxelizer halves": 3. The rule is stated after two examples and I had to reassemble it.
- c2-sat #5 "First come nine, each": 3. "At right angles to one of the edges and one of the axes" was hard to hold.
- c2-leaf #5 "Each sample adds the": 5. Three quantities, the sums and the weights, all together.
- c2-leaf #6 "Each sample is weighted": 3. "One share for each of sixteen columns" needed a re-parse.
- c2-material #2 "The voxelizer reads each": 4. Two tables of 128 texels: why?
- c2-material #6 "The emission is also": 4. Exposure scaling: why does it matter?

Understood: the bake collects static meshes in the box, makes the box a cube of 128 cells per side (the top half empty), sends each triangle down an octree with a separating-axis test, and fills leaves with summed albedo, emission, normal and coverage. Only about 4% of a full grid is used.

## Chapter 3 (12 flags)
- c3-sort #1 "In fact, the": 6. "In fact" contradicts the previous beat's "cells finished, sorted, packed" order, and I lost how it follows.
- c3-sort #6 "The bake also hands": 4. Why does the renderer need per-level counts?
- c3-bytes #6 "The three coordinates are": 4. Why 11, 10, 11 bits?
- c3-bytes #7 "Then come three bytes": 3. A list of field sizes with no verbs.
- c3-bytes #8 "Its emission is three": 5. Nine-bit numbers and a shared exponent in one sentence.
- c3-field #2 "It measures how far": 1. "Leaf-sized place" is a new unit I could not picture.
- c3-field #6 "Then a pass runs": 3. Long, with "column of places in z" resolved at the end.
- c3-field #8 "Each later pass adds": 3. Adds squared distance to what the pass before found and keeps the smallest: I could not follow it by ear.
- c3-field #13 "Any other place stores": 3. "In whole cells, plus one" is cryptic until the next sentence.
- c3-upload #4 "Godot's source also holds": 4. A compiled-out version: no reason to care.
- c3-upload #9 "A buffer of four": 1. "It" is unclear (the texture?), and the buffer's purpose comes much later.
- c3-upload #10 "In the lab, the": 4. Megabyte figures with no point.

Understood: finishing divides sums by alpha, parents keep only coverage, cells are sorted by level and packed into 48 bytes (two records), a distance field is built in three passes, and the GPU gets buffers plus a mipped light texture.

## Chapter 4 (9 flags)
- c4-when #2 "They run only when": 2. "One of three VoxelGI settings": which three? Not said.
- c4-when #7 "Each light's position and": 4. Converting to cell units: why?
- c4-direct #3 "The first of the": 1. "The light pass" against "the light passes" is easy to confuse.
- c4-shadow #5 "A cell here is": 1. What "a cell measured along the axis" refers to is unclear.
- c4-shadow #8 "But close to a": 6. "But" after "the field is zero" at a leaf. I lost how a reading below one cell fits.
- c4-area #5 "This method is called": 5. Method name, integral and diffuse cosine at once.
- c4-area #8 "Godot's own direct lighting": 4. The divide-by-two-pi comparison: why does it matter?
- c4-area #9 "The code shows that": 4. Same: the consequence of the missing division is unstated.
- c4-write #9 "The code shows that": 4. An anisotropy value nobody reads: why mention it?

Understood: light passes relight leaves only on change. Each leaf gets a direct light with a cosine, a distance-field shadow march, and the area light with no shadow. The write pass stores light divided by a dynamic range of 2 in an 8-bit texture.

## Chapter 5 (5 flags)
- c5-map #1 "So far, only the": 1. "That light is the first bounce": I had heard it called direct light.
- c5-mips #4 "Then it divides that": 3. "A number between eight and the count of children" resolves late.
- c5-cones #2 "The bounce pass builds": 5. Three axes, a frame and aiming cones together.
- c5-march #4 "Each sample is blended": 3. "Behind what the cone has already gathered" needed a re-parse.
- c5-march #7 "The last of the": 4. Eleven steps and two thirds opaque: why that one cone?

Understood: the mip pass makes parents from children with a propagation-controlled divisor. Each leaf traces six cones that widen and read coarser mips. The bounce is multiplied by albedo and rescaled, then the mips are rebuilt, for two bounces total.

## Chapter 6 (9 flags)
- c6-mode #7 "The lab draws each": 4. Four times brighter: no reason.
- c6-mode #8 "These points come from": 6. A processor plot, then a GPU plot: I lost which one fills the texture.
- c6-views #1 "The object is drawn": 3. "Images of its bounding box with sixteen texels across each cell" is dense.
- c6-views #2 "Later these texels are": 5. Averaging down and the meaning of alpha together.
- c6-shrink #4 "A texel's depth usually": 3. "Between two cells along the direction the view looks" needed a re-parse.
- c6-shrink #7 "The halving goes on": 3. Plotting "into the next, coarser mip" while halving: tangled.
- c6-shrink #8 "Unlike the first four,": 6. I lost which halvings are "the first four".
- c6-cost #2 "Then only the write": 2. "Chapter three's float buffer" was too long ago.
- c6-cost #8 "Each view takes one": 3. Eleven more to halve through eight mips: arithmetic I could not follow.

Understood: Dynamic objects are drawn six times, lit, shrunk 2x2 and plotted into every mip each frame. This happens even when the object is still. The texture is cleared and rewritten each frame, so there is no bounce and no shadow from the object.

## Chapter 7 (9 flags)
- c7-tag #3 "Those two reach the": 1. "Those two" and "reach the pixels" are vague.
- c7-tag #8 "The screen pass also": 6. A sudden turn to a settings buffer.
- c7-space #6 "Afterwards, the pass divides": 3. Dividing by the fades "added up" took a re-parse.
- c7-space #8 "But with one box": 3. The cancellation is spoken too fast.
- c7-low #4 "At that angle, the": 3. Radius equals distance travelled.
- c7-low #5 "Each step advances by": 5. Step by radius, doubling and mips together.
- c7-spec #3 "The screen pass squares": 4. Why square the roughness?
- c7-scale #3 "The cones' alpha, their": 4. What changes depending on the coverage?
- c7-scale #4 "The baked data also": 6. The interior setting comes out of nowhere before its effect.

Understood: depth pre-pass tags pixels with up to two VoxelGIs. The screen pass converts to box coordinates and applies the edge fade. Four wide cones step by their radius and read mips. Then there is a reflection cone by roughness, multiplied by range and energy, with alpha kept.

## Chapter 8 (6 flags)
- c8-panel #5 "The row called scene": 1. "Scene pass" is not a pass I know.
- c8-levels #10 "With the second bounce": 3. "A second round repeats them" gives 32 only if I redo the arithmetic.
- c8-cones #11 "The documentation says High": 6. Documentation claim, then "but" a different point.
- c8-spec #2 "In this box, a": 4. The step counts have no stated point yet.
- c8-spec #4 "Steps that short never": 4. Why that matters is not said.
- c8-half #10 "Multisampling, an anti-aliasing mode,": 6. Multisampling comes out of nowhere in the half-resolution story.

Understood: the bake runs once, light passes only on change over the occupied cells (one dispatch per level, with barriers), shadow steps are long in open air, and the screen pass costs a few samples per pixel. Mirrors are the costliest, and half resolution traces a quarter of the pixels at the price of ragged edges.

## Chapter 9 (5 flags)
- c9-mix #8 "After that, the mixed": 1. "Any other ambient light" is vague.
- c9-others #6 "At each point of": 4. Fog reading every mip: unexplained.
- c9-others #7 "Godot's lighting debug view": 4. Why mention the debug view?
- c9-follow #6 "And each leaf's albedo,": 3. A long clause, the subject buried.
- c9-recap #9 "In this room, the": 6. The trade list jumps to cell size without a link.

Understood: the opaque pass mixes the sky in where alpha is low, then applies occlusion and albedo. Transparents and fog read the texture too. One pixel traces back through cones, mips and leaves to the baked triangles. The takeaways are baked octree into a texture, light on change, and a trade of detail for speed.

## Totals
Kinds: 1=9, 2=3, 3=20, 4=22, 5=7, 6=10 (71 flags).
By chapter: ch1 6, ch2 10, ch3 12, ch4 9, ch5 5, ch6 9, ch7 9, ch8 6, ch9 5.
