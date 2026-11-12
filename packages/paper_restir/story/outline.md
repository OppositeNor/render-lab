# ReSTIR: outline

Phase 2 (`new-story`, `references/outline.md`; `new-story-paper`, "The chapters of a paper's story"). Citations are
index keys (source-index.json). Budgets are `target=` seconds, about 1.2 × the narration.

## Listener card

A graphics programmer who has written a physically based renderer: knows the GPU pipeline, G-buffers, microfacet
BRDFs, area lights, and Monte Carlo integration with importance sampling. Hears each sentence once. Terms defined where
first said: candidate, source PDF, target PDF (p̂), RIS weight, W, reservoir, M, combining reservoirs, temporal and
spatial reuse, visibility reuse, bias, unbiased weight (m, Z). Per chapter, the one question it answers is below.

## Knowledge level

Known: compute, passes, spaces, depth, G-buffer, GGX, area lights, importance sampling (`[audience]`). Taught here:
RIS and WRS (chapter 2, each with a demonstration, ~150 s together); reprojection is recalled (chapter 5). MIS is
recalled in a sentence (chapter 2).

## Architecture map

`diagrams/alg5.toml`: candidates (RIS) → visibility → temporal (← last frame's reservoirs) → spatial × n → shade;
the frame's reservoirs fed back to the next frame. Same names and colours in every chapter; each chapter lights its
part.

## Chapters

### 1. Introduction (target 100) — what problem does this paper solve, and how?

- Title card: title, authors, venue, year [`problem`]. Visual: the paper's card (2D) then the room.
- The problem: direct light from many dynamic lights, one shadow ray per pixel [`problem`, `first-vertex`]. Visual: the
  room, 64 lights moving (`lightPhase` sweep), M = 1 without reuse: very noisy.
- How it solves it: resample candidates, reuse neighbours' and last frame's [`approach`, `reservoir-intro`]. Visual:
  the same camera with ReSTIR: clean; a wipe between the two.
- Its reuse informs probabilities, not colours, so it can stay unbiased [`pdfs-not-colors`, `biased-mode`].
- Contributions list (4) on screen [`approach`, `alg3`, `alg5`, `alg6`].
- What the story teaches, what to know; the architecture map.

### 2. Related work (target 230) — what does ReSTIR build on, and what does it replace?

- Brief: many-light sampling and light hierarchies need maintained structures [`many-lights`, `hierarchies`]; tiled
  shading is biased [`tiled`]; path reuse traces a ray per reuse [`path-reuse`]; denoisers filter colours and are
  orthogonal [`denoisers`]. Visual: the room, one group of lights lit per sentence; a list card.
- Recall importance sampling (Eq. 3) and MIS (Eq. 4) in a sentence each [`eq3`, `eq4`].
- **RIS, taught** [`eq5`, `eq6`, `ris-unbiased`, `ris-converges`, `alg1`]: the question (sample in proportion to
  ρ·Le·G when we can only sample lights by power); intuition (draw many cheap guesses, keep one in proportion to how
  good it looks); worked example at P: M candidates as rays to the lights, each one's p and p̂, the weight bar
  w = p̂/p, the pick; W; exploration: M 1 → 32 at P and in the image (`ris` demo); the rule (Eq. 6 lit term by term);
  limits: cost linear in M, visibility not in p̂; link back: ReSTIR's first pass is this.
- **WRS, taught** [`wrs`, `eq9`, `alg2`]: the question (keep one of a stream without storing it); intuition (a running
  prize, each newcomer takes it with probability its share); worked example: the same candidates streamed one at a
  time (`risStep`), wsum growing, the kept one replaced; the rule (Eq. 9); the property that makes reuse possible:
  a reservoir can be fed to another; link back.

### 3. Streaming RIS (target 130) — what does each pixel keep after its candidates?

- Alg. 3: RIS through one reservoir per pixel; W at line 8, shading at line 11 [`alg3`]. Visual: listing `ris`, lit
  lines with the `ris` demo stepping.
- What a reservoir stores: y, M, W [`storage`]; the reservoir images, view modes: light id, W [`storage`].
- The paper's choices: M = 32, lights by power (alias table; the lab a CDF), p̂ unshadowed ρ·Le·G
  [`candidates-param`, `alias`, `target-param`].
- Cost is linear in M [`streaming-cost`]; the noise left (M sweep in the image).

### 4. Spatial reuse (target 170) — how can a pixel use its neighbours' candidates without redoing them?

- Combining reservoirs: each reservoir's y as a fresh sample of weight p̂_q·W·M [`combine`, `reweight`, `alg4`].
  Visual: P and one neighbour, the neighbour's sample reweighted for P.
- k neighbours in a 30-pixel disc, random from a low-discrepancy sequence [`neighbors-param`]. Visual: P's neighbours
  as points on screen and in the room, lines to their samples.
- M grows: view M; rounds n 0 → 4, k^n·M [`spatial`, `spatial-passes`]. Visual: the image cleaning with each round.
- The geometry test (10%, 25°) [`geometry-test`]: neighbours across the box's edge rejected, drawn red.

### 5. Temporal and visibility reuse (target 150) — how do frames and shadow rays help?

- Last frame's reservoir at the point's place in last frame's view [`temporal`, `temporal-pick`]. Visual: camera
  moving, P and its previous pixel.
- M grows frame by frame; clamp 20× [`clamp`]. Visual: P's M over frames (plot).
- Dynamic lights: the lights move (`lightPhase`) and samples follow them (the sample is a light and a point on it).
- Visibility reuse: one shadow ray per pixel before reuse; occluded samples' W = 0 [`visibility`]. Visual: view 4,
  P's shadow ray green/red; toggle off: noise in the box's shadow.
- Alg. 5 complete, the map lit end to end [`alg5`].

### 6. Bias, and removing it (target 200) — why does reuse darken edges, and how does the paper fix it?

- Each pixel has its own target and domain [`bias-source`]; W stands for 1/p(y) [`eq12`, `weight-meaning`].
- Eq. 19: when some sources cannot produce y, the expected W is |Z|/M of 1/p: darker [`eq19`].
- The 1D example (Fig. 7), interactive: p̂ = 2 − 2x, p₁ constant, p₂ a step; biased, naive 1/|Z|, balance heuristic
  [`fig7`, `eq20`, `naive-m`, `eq22`, `talbot-cost`].
- Alg. 6 in the room: Z counts only the sources whose p̂ at y is not zero, with visibility [`alg6`, `approx-pdf`];
  extra shadow ray per neighbour [`unbiased-cost`]. Visual: biased vs unbiased beside the reference at the box's edge.

### 7. Experimentation (target 110) — how does it compare on this machine?

- "These are the experimentation data run on this current machine." No numbers spoken.
- Comparisons the paper makes [`results-setup`, `biased-vs-unbiased`]: streaming RIS vs ReSTIR biased vs unbiased at
  the same frame; each against the accumulated reference; the performance panel's GPU time per pass, live.
- How to read each: noise, darkening at edges, cost per pass.

### 8. Conclusion (target 110) — what should you remember?

- Recap: the map walked again, each mechanism switched on in order.
- The paper's conclusion: filtering PDFs rather than colours [`conclusion`]; limitations: disocclusions, fast lights,
  primary hit only [`limitations`, `screen-space`].
- Three takeaways: a reservoir is a whole stream in four numbers; reuse multiplies candidates, not rays; reuse is
  biased unless each source's ability to produce the sample is counted.
- References card (APA).

## Demonstrations needed (Phase 3)

`room` (final), `ris` (P, candidates, `risStep`), `reservoirs` (view modes), `spatial` (P's neighbours, rounds),
`temporal` (reprojection, M plot, visibility), `bias` (Fig. 7 inset, biased/unbiased panes), `compare` (method panes,
performance panel). Pick P by click; its surface read back from `surfP/surfM/surfA` (`lab::fetch`), its candidates
recomputed by cpu.rs (identical to the GPU's, selftest).
