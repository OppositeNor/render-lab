# Native English review, chapters 7 to 10

18 suggestions. Chapter 10 reads naturally and needs none.

## Chapter 7

1. c7-why 9, "The target is darker in the creases, like where the wall meets the floor." "like where" is colloquial for narration.
   Suggestion: The target is darker in the creases, such as where the wall meets the floor.

2. c7-why 10, "There, many of the samples around..." The two nested appositives pile up and the ear loses the subject before "land".
   Suggestion: There, many of the samples around a pixel land on the other surface, above the pixel's own, and each sample is a depth read from the depth buffer.

3. c7-samples 11, "The spiral is shared between..." "between" suits two things, and sixteen pixels share it.
   Suggestion: The spiral is shared by the sixteen pixels of a four by four block.

4. c7-samples 12, "Each sample counts by how..." "counts by" is close but not idiomatic.
   Suggestion: Each sample counts for more the higher it rises above the point's surface.

5. c7-samples 14, "The sample's weight is one..." The nested "one over the sum of ... and" is hard to parse by ear.
   Suggestion: The sample's weight is one divided by its squared distance plus zero point zero seven.

6. c7-output 4, "It reaches eight pixels each way..." "reaches each way" is vague and stiff.
   Suggestion: It spans eight pixels in each direction, and it stops at depth edges.

7. c7-output 6, "It weighs the four nearest..." "weighs" should be "weights", and "their depth" should be plural.
   Suggestion: It weights the four nearest values by how close their depths are, so the dark does not bleed across a silhouette.

8. c7-output 9, "The last view is the..." The dangling "which ... is multiplied by" trails off at the end.
   Suggestion: The last view is the full-resolution upsample, the one that multiplies the diffuse light.

9. c7-recap 1, "So occlusion comes from the..." The long clause with "every frame" in the middle makes "both" hard to tie back.
   Suggestion: So occlusion comes from the screen, every frame, and from the material's maps, and both act as multipliers.

## Chapter 8

10. c8-thick 5, "The glow fades as the..." "the exponential of minus four times" is stiff when spoken.
    Suggestion: The glow fades as the object gets thicker, following e to the power of minus four times the thickness.

11. c8-thin 3, "The thin model then weakens..." "weakens ... by a falloff" is close but not right.
    Suggestion: The thin model then weakens that light with a falloff that depends on the measured thickness and on the scatter distance, how far light travels inside the material.

12. c8-thickness 4, "The depth stored there belongs..." "the point's own depth is the far side" equates a depth with a side.
    Suggestion: The depth stored there belongs to the side facing the sun, and the point's own depth belongs to the far side.

13. c8-thickness 10, "Last comes where the glow..." The inverted "comes where" trips the ear.
    Suggestion: Last is the place where the glow is added.

14. c8-sss 1, "Subsurface scattering spreads light..." "it leaves a little away from where it entered" is ambiguous and reads like a translation.
    Suggestion: Subsurface scattering spreads light under the surface, so it exits a little way from where it entered.

15. c8-sss 5, "The pack is the factor's..." "the pack" is not an English noun for this.
    Suggestion: The packed value is the factor's byte times two hundred and fifty-six, plus the quality's byte.

16. c8-sss 6, "That integer is stored in..." "larger packs" repeats the same oddity.
    Suggestion: That integer is stored in a half float, which holds integers exactly only up to two thousand and forty-eight, so larger packed values come back rounded.

17. c8-recap 3, "That keeps transmission cheap, but..." "makes the thickness scale approximate" is stiff.
    Suggestion: That keeps transmission cheap, but it leaves the thickness scale only approximate.

## Chapter 9

18. c9-hair 1, "One more kind of surface..." "has shading of its own, and that is hair" is a clumsy cleft.
    Suggestion: One more kind of surface has its own shading, and that is hair.

## Habits seen across the chapters

- "pack" and "packs" used as nouns for a packed value (c8-sss 5 and 6). English wants "packed value".
- "by" and "between" chosen by translation logic, such as "counts by", "shared between", and "weakens by a falloff". Native speakers pick "for", "by", and "with" differently.
- Long sentences with a nested appositive or a trailing "which ... is multiplied by", where the subject is lost before the verb (c7-why 10, c7-output 9). Move the apposition last or split it.
- Spoken arithmetic written as nested "the sum of ... and" or "the exponential of" (c7-samples 14, c8-thick 5). Speak it as "divided by ... plus" and "e to the power of".
- Inverted openers such as "Last comes where" that read as translated. Use "Last is" or "Last, where".
- "like" for "such as" in a comparison inside an example (c7-why 9).
- Chapter 10 and most of chapter 9 are natural, with short declarative sentences and good rhythm.
