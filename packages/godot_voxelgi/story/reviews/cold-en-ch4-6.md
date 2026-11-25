# Cold listen, English chapters 4-6

Problem kinds: 1 lost a reference; 2 undefined or forgotten term; 3 re-parse; 4 fact with no reason; 5 several new ideas; 6 lost the thread.

## Chapter 4 flags
- c4-shadow, 4, "The jump is rounded": 3, 4. Why round down, and "along the axis the ray mostly follows" arrives with no sense of what changes.
- c4-shadow, 7, "But close to a": 3, 6. "The field reads less than one cell, and then the cell's light is only dimmed": which cell, dimmed by what; it does not follow from the ray hitting a solid cell.
- c4-area, 4, "It evaluates the integral in": 2. "Linearly transformed cosines" is only a name, with no idea of what it is.
- c4-area, 7, "Godot's direct lighting divides": 4. I cannot tell why a divide by two pi matters until the next sentence.
- c4-write, 5, "At a range of": 1. "The readout's count of clipped leaves": a readout was never introduced.
- c4-write, 8, "Godot's VoxelGI stores no": 4. Why would it store a direction? I cannot tell what this changes.
- c4-write, 9, "The code shows that the": 2, 4. "Anisotropy value" is undefined, and the fact is "changes nothing".

## Chapter 5 flags
- c5-mips, 4, "Then it divides that sum": 3, 4. "Somewhere between eight and the number of children it really has" is vague, and I do not know why a divisor is needed at all.
- c5-cones, 2, "The bounce pass, the second": 4. Why a set of three axes around the normal? Its purpose is not given.
- c5-cones, 4, "Five more cones lean sixty": 1. Sixty degrees from what, the normal?
- c5-march, 6, "The last cone here takes": 1. Which is "the last cone"?
- c5-restore, 7, "They trace them only once,": 6. "No third bounce": I had counted direct light plus one pass, so what the first and second bounces are is unclear.
- c5-recap, 1, "So the texture now holds": 6. "Two bounces of light" conflicts with chapter four's "direct light" and one bounce pass.

## Chapter 6 flags
- c6-mode, 6, "Each is shown in the": 3, 6. "In the light it received, drawn brighter than the light stored" is a muddle of display against stored value.
- c6-mode, 7, "The lab runs the same": 2, 5. "Plot" is used as a known term and was never defined. This sentence also brings in the GPU run and the texture holding the teapot.
- c6-mode, 8, "These points come from the": 1, 6. Which points, as against the ones just shown, and what "the same plot" is.
- c6-views, 2, "That is fine enough to": 4. "Measure, later, how much of each cell": later when, and why I would care now.
- c6-shrink, 7, "Each further halving plots into": 3, 6. I thought halving ended at one texel a cell, so what is halved beyond that?
- c6-cost, 1, "Before the teapot is plotted,": 6. The title says "rewritten every frame", but chapter four said the static light is written only on change, and nothing says this is a different write.
- c6-cost, 2, "Then the static light is": 2. "The float buffer" was mentioned once in chapter three and I had forgotten it.
- c6-cost, 7, "Here the cost is six": 4. Where "seventy-two" comes from is not said, so I cannot weigh it.

## Counts
Kind 1: 4. Kind 2: 4. Kind 3: 5. Kind 4: 8. Kind 5: 1. Kind 6: 7.

## What I understood
- Chapter 4: The cells are lit only when a light, the bake, or a setting changes. Each cell gets a point light's direct light from direction, falloff and normal. Shadows come from marching the distance field. The panel's area light uses a closed-form integral with no shadow and runs about two pi too bright. The write pass divides the light by a dynamic range so it fits eight bits, with alpha as coverage.
  - Still asking: how do the dimmed edges work; what the rounding of jumps does; what anisotropy is.
- Chapter 5: A second pass has each leaf trace six cones (one along the normal, five leaning out) through coarse mips. The mips are built by summing children and dividing by a divisor that propagation sets. The result is multiplied by albedo, scaled back by the range, and added to the direct light, once only.
  - Still asking: what counts as the first and second bounce; why those angles and weights.
- Chapter 6: Dynamic objects are not baked. Each frame they are drawn from six views at sixteen texels a cell and lit from the distance field. They are shrunk by halves and plotted into each mip, with the light split between two cells. They cast no shadow and give no bounce.
  - Still asking: what "plot" exactly does; where seventy-two dispatches comes from; why the whole texture is cleared when the static light is rewritten.
