# Worked examples: mapping and fixing a passage

Load this when you write or fix a passage with the `storytelling` skill. Each example shows a draft, its map, the
faults the map shows, and the fixed passage with its map. The passages are written for these examples; they are not
taken from any story or engine.

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

## 3. Compressing, then using the noun

After a process has been told in verbs, the noun can carry it. A later chapter says "the thickness check skips this
wall" and the listener unpacks it, because they heard sentences 3 to 5 above. The same sentence in the first chapter,
before the process is told, would be a label with nothing inside. When a chapter uses a noun from an earlier one,
recall it in a few words the first time ("the thickness check, the test that ignores surfaces far in front of the
ray").
