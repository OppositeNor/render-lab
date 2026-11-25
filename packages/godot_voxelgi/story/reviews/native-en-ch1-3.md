# Native-English review, chapters 1 to 3

Most sentences already read naturally. These are the ones that trip.

## Chapter 1

1. c1-room, 9, "Every pixel reads their light" - "their" points back across a sentence to the cells, and "wide cones, shapes that widen" says the same thing twice. Suggestion: Every pixel reads that light back through a few cones, shapes that widen as they go.
2. c1-map-light, 2, "The light passes run again" - "costs them nothing" is stiff, since the cost belongs to the room, not the passes. Suggestion: The light passes run again only when a light changes, or the VoxelGI's data or settings change, so for a still room they cost nothing.
3. c1-map-opaque, 1, "Then the opaque pass, which" - "as their indirect light" has an ambiguous "their" and the verb "mixes" is loose. Suggestion: Then the opaque pass, which shades the scene's solid surfaces, adds the cones' light to the image as indirect light.
4. c1-box, 5, "It runs that code on" - "the graphics card and again on the processor" sounds dated and over-formal to a graphics programmer. Suggestion: It runs that code on the GPU and again on the CPU, and checks that the two agree.
5. c1-gate, 10, "The other settings arrive in" - settings do not "arrive". Suggestion: The other settings come up in the chapters that use them.

## Chapter 2

1. c2-collect, 6, "Of those meshes, only the" - "triangle surfaces" is an odd compound. Suggestion: Of those meshes, only the triangles are baked.
2. c2-cube, 3, "Its count is halved," - "halved, and halved again, while half the longest side still covers the axis" is hard to follow by ear. Suggestion: Its count is halved again and again, for as long as half the longest side still covers the axis.
3. c2-descent, 5, "Only the children the triangle" - "carves a path one level deeper at a time" has a stiff word order. Suggestion: Only the children the triangle overlaps become cells, so the triangle carves a path that goes one level deeper each time.
4. c2-all, 5, "The cells follow only the" - "follow" is close but not right for cells sitting on surfaces. Suggestion: The cells trace only the surfaces.
5. c2-material, 4, "The other holds the emission," - "multiplied by its energy and the bake's exposure" dangles after "the emission" and reads as a list. Suggestion: The other holds the emission, scaled by its energy and by the bake's exposure.

## Chapter 3

1. c3-sort, 3, "Because the cells moved, each" - "their new places" is ambiguous (the parents' or the children's), and "places" is vague. Suggestion: Because the cells moved, each parent's list of child indices is rewritten to point at the children's new positions.
2. c3-bytes, 2, "The first record holds its" - "children's indices, thirty-two bytes" is a bare appositive that trips when spoken. Suggestion: The first record holds its eight children's indices, in thirty-two bytes.
3. c3-parents, 5, "The coarser the level, the" - two causes chained in one long sentence. Suggestion: The coarser the level, the more of a cell is open air, so fewer of its children exist and its coverage drops.
4. c3-recap, 1, "So every leaf now stores" - "what its surface is" is loose. Suggestion: So every leaf now stores a description of its surface, its albedo and normal.
5. c3-upload, 3, "Godot's source also holds" - "a graphics-card version" is stiff, and it clashes with "GPU" used elsewhere. Suggestion: Godot's source also holds a GPU version of the field, but it is compiled out.

## Habits repeated across the chapters

- "graphics card" and "processor" are used where a graphics programmer says "GPU" and "CPU" (c1-box 5, c3-upload 3, c3-parents 6).
- "Their" and "its" after a long noun phrase can point at the wrong noun (c1-room 9, c1-map-opaque 1, c3-sort 3). Repeat the noun or use "that".
- Bare appositives of a measure ("..., thirty-two bytes") and trailing participles ("..., multiplied by ...") are written habits that trip when spoken.
- Slightly off verbs for plain relations ("follow", "arrive", "carves", "mixes"). Use "trace", "come up", "adds".
- Nearly every sentence is clean and short. The few that fail do so by chaining two clauses with a comma and a "while" or "so".
