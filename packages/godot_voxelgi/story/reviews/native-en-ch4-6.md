# Native-English review, chapters 4 to 6

Format: chapter, beat, number, first five words, problem, polished version.

## Chapter 4

1. ch4 c4-map 2, "The light passes, the compute", the appositive sits between subject and verb and the listener loses the verb. Polished: The light passes (the compute passes from chapter one) give each cell the light it receives.
2. ch4 c4-when 3, "The culler, the code that", "saved earlier" trails off after a long clause. Polished: The culler, the code that decides what is in view, compares every light each frame with a saved copy of that light's settings.
3. ch4 c4-when 4, "Move a light, and", "again" and the trailing "here the floor" are stiff. Polished: Move a light and the cells are relit, here the floor under its new position.
4. ch4 c4-when 5, "At most thirty-two lights take", the aside is clipped. Polished: At most thirty-two lights take part, which is the engine's limit.
5. ch4 c4-when 6, "Each light's position and range", "first" and "converted into the box's cells" are awkward. Polished: Each light's position and range are first converted to cell units, so the passes can work in cells.
6. ch4 c4-direct 3, "The pass first finds the", "first finds" sits oddly. Polished: The pass starts by finding the direction from the cell to the light.
7. ch4 c4-direct 4, "Then it finds how much", "finds how much the light falls off" is clumsy. Polished: Then it works out how much the light falls off over the distance between them.
8. ch4 c4-direct 6, "Otherwise, the light is weighted", passive and stilted. Polished: Otherwise, it weights the light by the cosine between the cell's normal and the direction to the light.
9. ch4 c4-direct 7, "That light times the cell's", "is the light the cell sends back out" is heavy to speak. Polished: That light times the cell's albedo, plus the cell's emission, gives the light the cell sends back out.
10. ch4 c4-shadow 1, "When the light casts shadows,", "on the light's side" is vague. Polished: When the light casts shadows, a ray starts at the light and marches back toward the cell through the distance field.
11. ch4 c4-shadow 2, "At each step, the ray", "reads from the field" is off. Polished: At each step, the ray reads the distance to the nearest surface from the field, shown as the circle.
12. ch4 c4-shadow 4, "The jump is rounded down", "the axis the ray mostly follows" is awkward. Polished: The jump is rounded down to whole cells, along the ray's dominant axis.
13. ch4 c4-shadow 5, "Here the ray reaches the", "reads a solid cell, where the field is zero" is stiff. Polished: Here the ray reaches the sphere's cells and hits a solid cell, where the field is zero.
14. ch4 c4-shadow 7, "But close to a surface,", the "and then" is an unnatural join. Polished: But close to a surface, the field reads less than one cell, so the cell's light is only dimmed.
15. ch4 c4-area 1, "The ceiling panel is an", "rather than a point" lands late. Polished: The ceiling panel is an area light, a glowing rectangle instead of a point.
16. ch4 c4-area 4, "It evaluates the integral in", "a standard fit called" is odd. Polished: It evaluates the integral in closed form, using a standard fit called linearly transformed cosines.
17. ch4 c4-area 7, "Godot's direct lighting divides this", "divides this integral by two pi" lacks a hook. Polished: Godot's direct lighting divides this integral by pi twice over, that is, by two pi.  (Alternative, simpler:) Godot's direct lighting divides this integral by two pi.
   Note: the original is acceptable, so this one is optional. Leave it as is.
18. ch4 c4-area 8, "The code shows that VoxelGI", "two pi times as brightly" is a heavy tongue-twister. Polished: The code shows that VoxelGI leaves that division out, so the panel comes out two pi times brighter than direct lighting would make it.
19. ch4 c4-all 1, "Every leaf runs the same", "on the graphics card" after "workgroup" trips. Polished: Every leaf runs the same pass, sixty-four leaves to a workgroup on the GPU.
20. ch4 c4-all 2, "The room's cells take on", "take on" is slightly off. Polished: The room's cells pick up the panel's light.
21. ch4 c4-all 4, "Cells it cannot see stay", "waiting for the bounce" is fine, but "in the next chapter" trails. Polished: Cells it cannot see stay dark until the bounce in the next chapter.
22. ch4 c4-write 1, "A last pass, the write", "A last pass" should be "The last pass". Polished: The last pass, the write pass, stores each cell's light in that cell's texel of the three-dimensional texture.
23. ch4 c4-write 5, "At a range of one,", "the readout's count of clipped leaves climbs" is stiff. Polished: At a range of one, the bright cells under the panel clip to white, and the readout's count of clipped leaves goes up.
24. ch4 c4-write 7, "Texels with no cell stay", "stay at zero, fully transparent" is clipped. Polished: Texels with no cell stay at zero, fully transparent, so a cone sees straight through them.  (Reads naturally; leave if preferred.)
25. ch4 c4-write 9, "The code shows that the", "no line reads it" is odd. Polished: The code shows that the pass still receives an anisotropy value, but nothing reads it, so it changes nothing.
26. ch4 c4-recap 2, "Every shadow in that light", "in that light" is ambiguous and stiff. Polished: Every shadow here comes from marching the distance field, and the area light casts none.
27. ch4 c4-recap 3, "So far, each cell", "has only light that came straight from a light" repeats "light". Polished: So far, each cell has only the light that reached it directly.

## Chapter 5

28. ch5 c5-map 1, "One pass of light reaches", "One pass of light" is odd. Polished: A single pass reaches only what the lights can see.
29. ch5 c5-mips 2, "The bake left the coarse", "fills them first" is vague. Polished: The bake left the coarse levels without light, so a pass called the mip pass fills them in first.
30. ch5 c5-mips 4, "Then it divides that sum", "somewhere between eight and the number" is clumsy. Polished: Then it divides that sum by a divisor that lies between eight and the number of children it really has.
31. ch5 c5-mips 5, "A setting called propagation picks", "picks where that divisor falls" is odd. Polished: A setting called propagation decides where in that range the divisor lies.
32. ch5 c5-mips 6, "Level by level, each mip", "holds a coarser copy" is fine, but the rhythm is flat. Polished: Level by level, each mip holds a coarser copy of the light.  (Optional, natural as is.)
33. ch5 c5-prop 2, "So empty children count as", "count as black" is fine, but the sentence ends on a bare result. Polished: So empty children count as black, and the coarse levels darken.  (Natural, leave as is.)
34. ch5 c5-prop 5, "Propagation is a choice between", "a choice between light that fades ... and light that spreads too far" reads as a trade-off, so tighten it. Polished: Propagation trades light that fades with distance against light that spreads too far.
35. ch5 c5-cones 2, "The bounce pass, the second", "builds a set of three axes" is stiff. Polished: The bounce pass, the second light pass, builds three axes around the cell's normal.
36. ch5 c5-cones 4, "Five more cones lean sixty", "weight of point one five each" is awkward spoken. Polished: Five more cones lean out sixty degrees, each with a weight of point one five, so the six weights add up to one.
37. ch5 c5-cones 5, "Each cone opens thirty degrees", "opens thirty degrees from its axis" is ambiguous. Polished: Each cone has a half-angle of thirty degrees around its axis.
38. ch5 c5-march 1, "A cone starts one and", "clear of the cell itself" is fine, but "from the cell's centre" is stiff. Polished: A cone starts one and a half cells out from the cell's centre, clear of the cell itself.
39. ch5 c5-march 2, "The cone widens as it", "advances by half its current width" is OK, but the "on each step" is off. Polished: The cone widens as it goes, and each step advances it by half its current width.
40. ch5 c5-march 4, "Each sample is blended in", "blended in behind what the cone has already seen" is awkward. Polished: Each sample is blended in behind what the cone has already gathered, front to back, like layers of glass.
41. ch5 c5-march 6, "The last cone here takes", "still ends only two thirds opaque" is clunky. Polished: The last cone here takes eleven steps and is still only two thirds opaque at the end.
42. ch5 c5-march 7, "That is because the room's", "thin shells around open air" is fine, but "That is because" is flat. Polished: The room's cells are thin shells around open air, so the cone passes through them.
43. ch5 c5-restore 2, "It is also multiplied by", "which undoes the division the texture applied" is awkward. Polished: It is also multiplied by the dynamic range, which undoes the division applied when the texture was written.
44. ch5 c5-restore 6, "Only the leaves trace bounce", "trace bounce cones" is awkward. Polished: Only the leaves trace bounce cones.  (Fine, leave.)
45. ch5 c5-recap 1, "So the texture now holds", "from five-centimetre cells to the whole box" is slightly stiff. Polished: So the texture now holds two bounces of light, at every level from the five-centimetre cells up to the whole box.
46. ch5 c5-recap 2, "All of it was baked", "from objects that stand still" is odd. Polished: All of it was baked from objects that never move.

## Chapter 6

47. ch6 c6-map 1, "The bake knows only what", "what stood still" is odd tense. Polished: The bake knows only what stays still.
48. ch6 c6-map 3, "So an object marked Dynamic", "is drawn into the texture every frame instead" ends weakly. Polished: So an object marked Dynamic is drawn into the texture every frame instead.  (Fine, leave.)
49. ch6 c6-mode 3, "In every frame where the", "is in view" then "each of its dynamic objects" is ambiguous. Polished: In every frame where the room's VoxelGI is in view, each dynamic object is drawn into the texture.
50. ch6 c6-mode 4, "That happens whether the object", "costs work" is not idiomatic. Polished: That happens whether the object moved or not, so it costs time even when the object stands still.
51. ch6 c6-mode 6, "Each is shown in the", "drawn brighter than the light stored" is clipped. Polished: Each is shown in the light it received, drawn brighter than the light that is stored.
52. ch6 c6-mode 7, "The lab runs the same", "runs the same plot on the graphics card" is odd. Polished: The lab runs the same plot on the GPU, so the texture the room is shaded from holds the teapot too.
53. ch6 c6-mode 8, "These points come from the", "the same plot, run again on the processor" is awkward. Polished: These points come from the same plot, run again on the CPU.
54. ch6 c6-views 1, "The object's bounding box is", "drawn at sixteen texels per cell" is fine. Polished: The object's bounding box is drawn at sixteen texels per cell.  (Leave.)
55. ch6 c6-views 2, "That is fine enough to", "fine enough to measure, later," is stiff. Polished: That is fine enough to measure, later on, how much of each cell the object covers.
56. ch6 c6-views 3, "The object is drawn six", "so no side is missed" is passive. Polished: The object is drawn six times, with orthographic views along each axis in both directions, so no side is missed.  (Fine, leave.)
57. ch6 c6-views 4, "Each view records the material's", "along with its depth" trails. Polished: Each view records the material's albedo, normal, emission, and depth at every texel.
58. ch6 c6-views 5, "A texel's depth gives its", "its place in the grid" is vague. Polished: A texel's depth tells us where it sits in the grid.
59. ch6 c6-views 6, "Each texel is then lit", "from the baked distance field alone" is awkward. Polished: Each texel is then lit like a cell, with shadows taken from the baked distance field alone.
60. ch6 c6-shrink 1, "The lit view is then", "halved again and again" is OK, but "becoming one" is abrupt. Polished: The lit view is then halved again and again, each two-by-two block of texels becoming one texel.
61. ch6 c6-shrink 2, "It goes from sixteen texels", "a cell" after numbers is fine. Polished: It goes from sixteen texels per cell to eight, then four, then two, then one.
62. ch6 c6-shrink 3, "At one texel a cell,", "plots each texel, writing it into" is clunky. Polished: At one texel per cell, a compute pass plots each texel into the texture's finest mip.
63. ch6 c6-shrink 4, "A texel's depth usually falls", "falls between two cells along the view" is fine. Polished: A texel's depth usually falls between two cells along the view.  (Leave.)
64. ch6 c6-shrink 8, "Each halving divides the light", "the way the mip pass does, by the divisor" is clumsy. Polished: Each halving divides the light by the divisor that propagation sets, just as the mip pass does.
65. ch6 c6-mips 3, "Each mip halves the detail", "again" is fine. Polished: Each mip halves the detail again.  (Leave.)
66. ch6 c6-cost 2, "Then the static light is", "which keeps every cell's light at full precision" is heavy. Polished: Then the static light is written back from the float buffer, which holds every cell's light at full precision.
67. ch6 c6-cost 3, "So the dynamic texels exist", "exist only for this frame, at every mip" is stiff. Polished: So the dynamic texels last only for this frame, at every mip.
68. ch6 c6-cost 7, "Here the cost is six", "Here the cost is" is stiff. Polished: Here each object costs six draws and seventy-two compute dispatches, every frame.
69. ch6 c6-cost 9, "The code shows that this", "reads beyond the end of the light buffer" is fine, but "no such limit" is vague. Polished: The code shows that this pass has no such limit, so with more than thirty-two lights it reads past the end of the light buffer.
70. ch6 c6-recap 2, "The object is lit directly,", fine. Polished: The object is lit directly, but no light bounces off it.  (Leave.)
71. ch6 c6-recap 4, "The next chapter reads it", "one pixel at a time" is fine. (Leave.)

## Habits seen across the chapters

- Trailing clauses that hang after a complete thought ("here the floor under its new position", "which undoes the division..."), which lose the listener. Close the thought first.
- Apposition between subject and verb ("The culler, the code that ... , compares"), which delays the verb. Use parentheses-like phrasing or split the sentence.
- Near-miss verbs: "finds" for "works out", "picks" for "decides", "takes on" for "picks up", "costs work" for "costs time".
- Passive where a plain active verb would speak better ("is weighted by", "is rounded down", "is blended in").
- "Graphics card" and "processor" where the audience says GPU and CPU.
- Stiff "a cell" or "a texel" rates ("a cell" for "per cell"), and the "That is because ..." opener.
- "The code shows that ..." repeated as a formula for the findings, which is natural once but heavy when it recurs.
- Numbers in speech ("point one five each", "two pi times as brightly") need a rhythm that can be said in one breath.

Note: a few entries above are marked "(Leave)" or "optional" because on a second reading they already sound natural, so the count of real suggestions is lower than the count of entries.
