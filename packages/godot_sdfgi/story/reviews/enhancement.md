# The enhancement: review table and reviews (2026-11-05)

Working file (deleted when the story is finished). The plan is todo/godot-sdfgi-enhancement.md.

## Review table

| Beat | Issue | Severity | Why it matters | Correction (done) |
|---|---|---|---|---|
| every chapter's end | No recap: a chapter ends on its sources card | high | A viewer cannot say what the chapter built or where it is used next | A `cN-recap` beat on the frame graph (what it built, then what uses it next), before each sources card |
| every chapter's start (2-10) | The frame graph is shown only in chapters 1 and 11 | medium | The viewer loses where a stage sits in the frame | A `cN-map` beat opening each chapter on the frame graph, its stage lit |
| c11 ending | Ends "That is SDFGI, as Godot's source builds it." with no takeaways | high | No summary of how SDFGI organises data, reuses work, and what it trades | `c11-takeaways`: the room with SDFGI on and off, three takeaways (data around the camera; work spread over frames and reused; the price: coarse cells, Static meshes only, about thirty frames to catch up) |
| c6 recap (draft) | The first draft said the screen pass uses occlusion; chapter 6 says it is read only with use occlusion on, off by default | high (wrong) | A recap would contradict its chapter | Says the setting is off by default (caught by the zh-CN cold listener) |
| c8 recap | "A pixel cannot read coefficients quickly" gave no reason | medium | Unexplained motivation for chapter 9 | Uses chapter 9's reason: coefficients are not a texture a pixel can sample |

Kept as they were: the opening's comparison, the chapters' bodies, their demonstrations and cues.

## Reviews of the new sentences

- English cold listener, two rounds. Round 2's kind 1-3 flags were fixed (the "lit here" clause, the inverted
  "Only with" sentence, "In return", forward references to "maps", recalled terms glossed: cascades, slab, sees,
  the screen pass). Remaining kind 2/5 flags are recaps naming terms their chapter defined; the picture (the frame
  graph with the stage lit) answers them.
- English native review: applied where it kept the claim (trailing "which the next chapter…" tails removed, lists
  split into sentences, "frames of rays", "catches up with a change over").
- Chinese cold listener, two rounds: terms made consistent with the body (脏区, 屏幕渲染阶段, 可见程度); the chapter 6
  contradiction fixed. Chinese native review: applied (量词, 列入清单, pronoun stacks, 以数据的形式).

## The fact check (facts-c1-c5.md, facts-c6-c11.md)

Two fresh agents checked every factual sentence against the checkout: 265 verified, 15 simplifications, 3
uncertain, 2 wrong. Corrected, each verified in the checkout first:
- c1-prepass: the pre-pass also runs when a material uses the stencil (`force_depth_pre_pass =
  scene_state.used_opaque_stencil`, render_forward_clustered.cpp:2112-2113); the callout is now `prepass-gate`, and
  the freeze sentence says "and no stencil material in view".
- c3-lights: positional lights are omni, spot, or area (the static-light cull filters only by bake mode and cascade,
  renderer_scene_cull.cpp:3266-3271; gi.cpp:2024 handles LIGHT_AREA).
- c1-sources: nothing checks the recorded line hashes; the sentence now says each citation records one.
- The index entry `update-light`: a frame processes every Nth cell (gi.cpp:1287-1298).
Answered: the c1-room ambient (the lab's SDFGI shading has no sky ambient, checked in the lab's code). Kept as
simplifications (the narration's level): the fact files' remaining SIMPLIFICATION rows.

## Change log (2026-11-05)

- Every chapter (2-10) opens on the frame graph with its stage lit (`cN-map`), and every chapter (1-10) ends with a
  recap of what it built and what uses it next (`cN-recap`), before its sources card.
- The last chapter ends on the room, SDFGI faded from on to off, with three takeaways (`c11-takeaways`).
- Chapter 2 says what the cell size costs: 128 cells a side, so a smaller cell gives finer detail and a shorter reach.
- Corrected: the pre-pass condition (stencil materials), area lights among positional lights, the line-hash
  sentence, the chapter 6 recap's occlusion (off by default), the `update-light` entry's every-Nth-cell schedule.
- Budgets: chapter 6 85 → 105 s, chapter 11 65 → 100 s. The video is 32.19 minutes (it was about 28).
- Both editions built and verified (`build/video/en/verification-report.md`, `build/video/zh-CN/…`: passed).
