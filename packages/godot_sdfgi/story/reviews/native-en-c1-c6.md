# Native-English review, chapters 1 to 6

## Chapter 1 (8)
1. "This is SDFGI the" - the bare "the way" reads as spoken-translated. Suggest: This is SDFGI as Godot four point seven builds it.
2. "Every claim in this" - "cites a file" makes the video the subject of the citing. Suggest: Every claim in this video points to a file, a symbol, and a line in Godot's source.
3. "Each citation also records" - "cites" repeats "citation" in one sentence. Suggest: Each citation also records a hash of the line it points to, so the lab can tell when that line changes.
4. "The code starts the" - "in one place only, inside the code that draws" is heavy and stacks "code". Suggest: The code starts the probe update in just one place, inside the depth pre-pass's draw function.
5. "A comment there says" - "then run" is vague and the clause is long. Suggest: A comment there says the probe update runs at the same time as the pre-pass.
6. "The code shows two ways" - the code does not "show"; it does. Suggest: The code turns the pre-pass on in two ways.
7. "The setting for the probes'" - possessive plus "setting for" is clumsy. Suggest: The probe ray count setting offers eight, sixteen, thirty-two, and more.
8. "So each frame, SDFGI" - fronted "each frame" trips. Suggest: So SDFGI runs in stages every frame, and only in the Forward+ renderer.

## Chapter 2 (3)
1. "The first cascade has" - "cells of twenty centimetres" is not idiomatic. Suggest: The first cascade has twenty-centimetre cells, so it spans twenty-five point six metres.
2. "So a cell covers" - the appended "twenty divided by" hangs. Suggest: So a cell covers only thirteen centimetres of height in the world, which is twenty divided by one and a half.
3. "The code shows that" - "shows that a cascade is" is stiff. Suggest: In the code, a cascade ends up two thirds as tall as it is wide, not the three quarters that seventy-five percent suggests.

## Chapter 3 (5)
1. "Each axis that moved" - "gives" is slightly off. Suggest: Each axis that moved leaves one dirty slab, called a region.
2. "There is no test" - "no test against" is awkward. Suggest: Nothing tests against the camera's view, so a mesh behind the camera is voxelized too.
3. "Something goes wrong with" - vague and abrupt. Suggest: Something goes wrong in how these lists are read.
4. "The code shows that" - same stiff "shows that". Suggest: In the code, the second cascade's light list is stored in the second slot but read from the first.
5. "The slot a list" - long, with a trailing "in order". Suggest: A list's stored slot and read slot agree only when the cascades that moved are the first ones, in order.

## Chapter 4 (5)
1. "Each camera's image has" - "of the region, across the two axes that camera sees" is tangled. Suggest: Each camera's image has one pixel per cell across the two axes it sees.
2. "Each fragment uses its" - "the voxel it falls in" dangles the preposition. Suggest: Each fragment uses its own 3D position to find the voxel it falls into, and writes its colour there.
3. "That light's colour is" - possessive on "light" is awkward. Suggest: The light's colour is packed into thirty bits.
4. "Here the view slices" - "slices the room" is odd. Suggest: Here the view cuts through the middle of the sphere.
5. "A separate texture splits" - "over" is not the right preposition. Suggest: A separate texture splits that light across the six axis directions.

## Chapter 5 (5)
1. "Those voxels then become" - "still in the stage" is stiff. Suggest: Those voxels then become a distance field, in the same stage that made them.
2. "Cells then pass their" - "in a method called a jump flood" names the method as a thing. Suggest: Cells then pass their seeds to each other, in a method called jump flooding.
3. "If one of those" - "those" refers back loosely. Suggest: If any of them is solid, the half-size cell takes it as its first seed.
4. "The bits left over" - wordy. Suggest: The leftover bits in the record hold twenty-six flags, one for each neighbour of the voxel.
5. "Then the cell checks" - "voxels of the block that holds its seed" is heavy to hear. Suggest: Then the cell checks the eight voxels in its seed's block.

## Chapter 6 (1)
1. "A solid cell has" - "average in that zero" trips when spoken. Suggest: A solid cell has zero visibility, so the cells behind it average that zero in and fall below one.

## Habits across the chapters
- "The code shows that ..." and "The code shows two ways ..." make the code the agent of a plain fact. Say "In the code, ..." or let the code act.
- Stacked prepositional tails ("of the region, across the two axes that camera sees", "in one place only, inside the code that ...") break the rhythm. Split or trim them.
- Dangling prepositions ("the voxel it falls in") and loose "those"/"one of those" references read as written, not spoken.
- Possessives on non-people ("the probes' ray count", "that light's colour") sound stiff. Use a noun compound.
- "Still", "then", "also" and "so" often pad a sentence that is already linked by position.
