# ReSTIR: research

Phase 1 of the story (`new-story-paper`, `references/reading.md`). Citations are the article's pages, 148:1 to 148:17
(page *n* of the PDF is 148:*n*), with section and equation, algorithm line, or figure.

## 1. The edition

- Benedikt Bitterli, Chris Wyman, Matt Pharr, Peter Shirley, Aaron Lefohn, and Wojciech Jarosz. 2020.
  *Spatiotemporal reservoir resampling for real-time ray tracing with dynamic direct lighting.* ACM Transactions on
  Graphics 39, 4, Article 148 (July 2020), 17 pages. DOI 10.1145/3386569.3392481 (148:1).
- The PDF is the ACM version (acmart, 17 pages, created 2020-05-08), SHA-256
  `a1ae1233eecd4f82cc9502900de08012e9a0c233fb8014f53f20c2e9814f243d`. Supplementary material and video are mentioned
  (148:9 §4.3, 148:10 §6) but not at hand; nothing in the story rests on them.
- Authors' code: none provided by the user (2026-11-12). The paper's prototype was built in Falcor (148:10 §6); no
  Divergence chapter.

## 2. Abstract, introduction, conclusion

**The problem** (148:1 abstract; 148:2 §1). Direct lighting from many dynamic lights by Monte Carlo is hard even
offline: shadow rays to every light are not feasible, and which lights matter at a point depends on visibility, the
BSDF, and each light's power. Real time adds dynamic scenes, no knowledge of future frames, few rays per pixel, and
little time to build or update light data structures. The paper restricts itself to direct lighting at the first
camera vertex (148:2 §1).

**How it solves it** (148:1 abstract; 148:2 §1). Repeatedly resample a set of candidate light samples, with spatial
and temporal resampling to reuse information from nearby samples. It builds on RIS, using a fixed-size "reservoir"
that stores only accepted samples and weighted reservoir sampling, so no data structure is more complicated than a
fixed-size array. Its reuse informs sampling probabilities (not pixel colours, as denoisers do), which makes an
unbiased algorithm possible; a biased mode reduces noise further at the cost of darkening near geometric
discontinuities.

**Contributions** (gathered from 148:1–148:2 §1 and 148:13 §7 "Resampling"; the paper has no numbered list):
1. Streaming RIS: RIS turned into a streaming algorithm with weighted reservoir sampling, constant storage per pixel
   (§3.1, 148:4–148:5, Alg. 3).
2. Spatiotemporal reuse: combining reservoirs of neighbouring pixels and of the previous frame in constant time per
   reservoir, so a pixel's effective candidate count grows by orders of magnitude (§3.2, 148:5–148:6, Alg. 4, Alg. 5),
   with visibility reuse (148:6).
3. An analysis of RIS with candidates from different PDFs, the source of bias, and an unbiased combination with
   per-sample weights m that sum to one, uniform (Alg. 6) or the balance heuristic (Eq. 22) (§4, 148:6–148:9).
4. A GPU implementation with biased and unbiased variants (§5, §6).
The abstract's claimed results (equal-error 6×–60× faster than state of the art for unbiased, 35×–65× for biased,
3.4 million emissive triangles under 50 ms with at most 8 rays per pixel) are the paper's claims about its own setup
(an RTX 2080 Ti, a Titan RTX for Amusement Park; 148:10 §6). The story speaks no numbers in its Experimentation
chapter.

**Conclusion and limitations** (148:14 §8, §8.1). Filtering and denoising "need not remain a post-process": the method
filters PDFs rather than colours. Limits: it relies on correlation between pixels, so quality degrades near
disocclusions, lighting discontinuities, high geometric complexity, and fast-moving lights; its image buffers limit it
to the primary hit; future work: world-space reuse, path reuse, spatial-only rounds for stills, reservoir granularity.

## 3. Related work

| Work (APA key) | What it does | Relation | Depth | Where discussed |
|---|---|---|---|---|
| Veach & Guibas 1995b (MIS) | combines several sampling strategies with weights that sum to one; balance heuristic | builds on (Eq. 4; Eq. 22 is the balance heuristic over candidate PDFs) | foundation, recalled with importance sampling | 148:3 §2 |
| Talbot 2005; Talbot, Cline & Egbert 2005 (RIS) | draws M candidates from an easy source PDF, picks one in proportion to p̂/p, corrects with the average weight | builds on (Eqs. 5–7, Alg. 1); compared against (Fig. 8, 10, 11) | **foundation, in depth** | 148:3 §2.1; 148:8 §4.3 "Comparison to Talbot et al." |
| Chao 1982 (WRS); Vitter 1985; Efraimidis & Spirakis 2006; Efraimidis 2015 | one pass over a stream, keeping one sample with probability proportional to its weight | builds on (Eqs. 8–10, Alg. 2) | **foundation, in depth** | 148:4 §2.2; 148:14 §7 |
| Moreau, Pharr & Clarberg 2019 | light BVH for real-time many-light sampling | competes with (Fig. 1, 9, 11) | context | 148:10–148:12 §6, §7 |
| Ward 1994; Shirley, Wang & Zimmerman 1996 | classify lights as important or unimportant | context of many-light sampling | context | 148:10 §7 |
| Estevez & Kulla 2018; Yuksel 2019; Lin & Yuksel 2020 | light hierarchies (lightcuts) sampled in sub-linear time | alternative | context | 148:10–148:12 §7 |
| Donikian et al. 2006; Vévoda et al. 2018; Walter et al. 2005 | adaptive PDFs for direct light (image blocks, Bayesian regression over lightcuts) | alternative, no BSDF in the light sample | context | 148:12 §7 |
| Olsson & Assarsson 2011; Tokuyoshi & Harada 2016 | tiled shading: per-tile lists of important lights | alternative, biased by limited light range | context | 148:12 §7 |
| VPLs: Keller 1997; Dachsbacher et al. 2014 and others | many point emitters approximating the lighting | alternative | context | 148:12 §7 |
| Photon mapping, (ir)radiance caching (Jensen 1996; Ward et al. 1988 and others) | cache and interpolate incident light | alternative; struggles with fast visibility changes | context | 148:13 §7 |
| Bidirectional and path reuse (Bekaert et al. 2002 and others) | reuse paths, tracing visibility per reuse | related; ReSTIR traces rays for few samples only | context | 148:5 §3.2; 148:13 §7 |
| MCMC (Veach & Guibas 1997 and others) | perturb paths toward the radiance distribution | alternative | context | 148:13 §7 |
| Ratio, WUS, WIS estimators (Powell & Swann 1966; Bekaert et al. 2000; Heitz et al. 2018; Hartley & Ross 1954) | weight samples by a correlated distribution; biased but consistent | related; RIS seen as making them unbiased (App. B) | context | 148:13–148:14 §7; 148:17 App. B |
| Denoisers (Schied et al. 2017; Chaitanya et al. 2017; NVIDIA Research 2017) | filter colours after rendering | orthogonal; ReSTIR improves their input | context | 148:2 Fig. 2; 148:14 §7 |

The foundations' own texts are not in the user's library (2026-11-12), so claims about RIS and WRS are cited to this
paper's §2.1 and §2.2 and narrated as the paper presents them.

## 4. The method, step by step

**Preliminaries** (148:2–148:3 §2). Direct light L = ∫_A ρ Le G V dA (Eq. 1–2), with f = ρ Le G V. Importance
sampling: ⟨L⟩ = (1/N) Σ f(xᵢ)/p(xᵢ), unbiased if p > 0 wherever f ≠ 0 (Eq. 3). MIS (Eq. 4), unbiased when the weights
are a partition of unity; balance heuristic.

**RIS** (148:3 §2.1, Eqs. 5–7, Alg. 1). M ≥ 1 candidates from source p; choose index z with probability
w(x_z)/Σ w(xᵢ), w = p̂/p (Eq. 5); 1-sample estimator f(y)/p̂(y) · (1/M) Σ w(xⱼ) (Eq. 6). Unbiased for M, N ≥ 1 when p
and p̂ are positive wherever f ≠ 0. The paper takes N = 1 from there on. As M → ∞, y's distribution approaches p̂.

**WRS** (148:4 §2.2, Eqs. 8–10, Alg. 2). Reservoir {y, wsum, M}; update(xᵢ, wᵢ): wsum += wᵢ; M += 1; replace y with
probability wᵢ/wsum (Eq. 9); every earlier sample stays with probability w(xᵢ)/Σ (Eq. 10). With replacement, one
sample (N = 1).

**Streaming RIS** (148:4–148:5 §3.1, Alg. 3). Per pixel: M candidates xᵢ ~ p, update(xᵢ, p̂_q(xᵢ)/p(xᵢ)); then
W = (1/p̂_q(y)) · (1/M) · wsum (Alg. 3 line 8, Eq. 6); shade f_q(y) · W (line 11). Fig. 4: candidates uniform over the
emitters' area, p̂ = ρ Le G (unshadowed), shadow rays only for the surviving samples. Storage constant, computation
linear in M.

**Combining reservoirs** (148:5–148:6 §3.2, Alg. 4). To combine, treat each reservoir's y as a fresh sample with
weight p̂_q(r.y) · r.W · r.M (the p̂_q/p̂_q′ reweighting for the neighbour's different target, written with r.W);
s.M = Σ rᵢ.M; s.W = (1/p̂_q(s.y)) · (1/s.M) · s.wsum. O(k) for k reservoirs, equivalent to resampling the combined
input streams. 148:7 §4.1 explains the weight: p̂_q(r.y)·r.W is the RIS weight p̂/p with r.W estimating 1/p, scaled by
r.M.

**Spatial reuse** (148:5–148:6 §3.2; parameters 148:9 §5 "Neighbor selection"). Each pixel picks k neighbours and
combines their reservoirs with its own; n iterations, each reading the previous pass's output: O(nk + M) work, k^n·M
effective candidates if neighbours are distinct (148:6). Benefit not indefinite (Fig. 5). Paper's parameters: k = 5
(k = 3 unbiased), random points in a 30-pixel radius from a low-discrepancy sequence (deterministic neighbours gave
artifacts); n = 2 (n = 1 unbiased) (148:9–148:10 §5). Biased mode rejects a neighbour whose camera distance differs by
more than 10% of the current pixel's depth or whose normal is more than 25° away (148:9 §5).

**Temporal reuse** (148:6 §3.2; 148:9 §5). Store each pixel's final reservoir; next frame combine with the
reservoir at the backprojected pixel (motion vectors). Feeds forward all previous frames. The previous frame's M is
clamped to at most 20× the current reservoir's M (148:10 §5, "Reservoir storage and temporal weighting").

**Visibility reuse** (148:6 §3.2). After initial candidates and before reuse, trace a shadow ray for each pixel's y;
if occluded, discard the reservoir (Alg. 5 lines 7–9 set W ← 0).

**The full algorithm** (148:6 Alg. 5). RIS per pixel → visibility (W = 0 if shadowed) → temporal combine with
prev[q′] → n spatial iterations → shade → return reservoirs for the next frame.

**Bias** (148:6–148:9 §4). Each pixel has its own target and domain; mixing candidates from different PDFs biases
standard RIS. W(x, z) = (1/p̂(x_z)) · (1/M) Σ wᵢ(xᵢ) must have E[W | x_z = y] = 1/p(y) (Eqs. 11–12). With
candidate-specific PDFs pᵢ, wᵢ = p̂/pᵢ (Eqs. 13–15); p(y) marginalises over configurations (Eqs. 16–17);
E[W | x_z = y] = (1/p(y)) · |Z(y)|/M (Eq. 19; proof App. A, Eqs. 23–25), so when some pᵢ(y) = 0 the inverse PDF is
underestimated: darker. 1D example (148:8 Fig. 7): p̂(x) = 2 − 2x, p₁ = 1, p₂ = 2H(1/2 − x), half the candidates from
each.

**Unbiased RIS** (148:8 §4.3, Eqs. 20–22). Replace 1/M by m(x_z): E = (1/p(y)) Σ_{i∈Z(y)} m(xᵢ) (Eq. 21), unbiased iff
those m sum to 1. Naive: m = 1/|Z(x_z)| (Fig. 7b), noisy when a PDF is near zero (Fig. 7c, p₂ ∝ max(H(1/2 − x),
10⁻⁴)). Balance heuristic m(x_z) = p_z(x_z)/Σ pᵢ(x_z) (Eq. 22; Fig. 7d). Talbot et al.'s alternative weight
p̂/Σpᵢ costs O(M²) PDF evaluations; the paper's O(M) (148:8–148:9).

**Practical unbiased reuse** (148:9 §4.4, Alg. 6). Use p̂_qᵢ (with visibility at qᵢ when visibility reuse is on) as
the approximate PDF of neighbour qᵢ's samples; Z = Σ rᵢ.M over the qᵢ with p̂_qᵢ(s.y) > 0; m = 1/Z;
s.W = (1/p̂_q(s.y)) · m · s.wsum. Costs a shadow ray per neighbour with visibility reuse. Both biased and unbiased
variants were implemented (148:9).

**Design choices** (148:9–148:10 §5). M = 32 initial candidates: an emissive triangle by power (alias table, Walker
1974), then a uniform point on it, p ∝ Le; 25% from the environment map when there is one. Target p̂ ∝ ρ·Le·G,
unshadowed; GGX dielectric over Lambertian. N = 1 unbiased; N = 4 biased (four reservoirs per pixel). Storage per
reservoir: y, M, W.

## 4a. The equations, classified (`new-story`'s references/storytelling-formula.md)

For this story's audience (Monte Carlo with importance sampling known; RIS, WRS, and reservoir reuse taught):

| Equation | Kind | Beats |
|---|---|---|
| f(x)/p(x), the IS estimate (Eq. 3) | reference: recalled in words | c2-is |
| MIS weights summing to one (Eq. 4) | reference: recalled, used again in c6 | c2-is |
| w = p̂/p (Eq. 5) | **core** | c2-ris, c2-toy |
| W = Σw / (M p̂(y)), shading f(y)·W (Eq. 6, Alg. 3 line 8) | **core** | c2-ris, c2-toy; recalled c3-alg3 |
| WRS replacement chance w/wsum (Eq. 9) | bridge: one rule, shown streaming | c2-wrs |
| combine weight p̂_P(y)·W·M (Alg. 4 line 4, §4.1) | **core** | c4-combine |
| k^n M (148:6) | bridge | c4-rounds |
| E[W given y] = (1/p(y))·abs(Z(y))/M (Eq. 19) | **core** | c6-1d |
| m = 1/abs(Z) and the balance heuristic (Eqs. 20-22, Alg. 6) | bridge on Eq. 19 | c6-fix, c6-alg6 |
| p̂(x) = 2 − 2x (Fig. 7) | reference: the example's definition | c6-1d |

**w = p̂/p.** Need: the cheap choice draws candidates on bright lights often, whatever they bring to P. Anchor: P's
eight candidates (readout: p̂ / p = w per candidate), then the two-sample world (toy.rs). Construct: dividing by p
takes away the head start a likely candidate has (IS's own reasoning, Eq. 3, applied per candidate: derived, not
quoted). Worked: A (p 3/4, p̂ 1) gets 4/3, B (p 1/4, p̂ 3) gets 12. Counterfactual: without the division the
estimates average 1.5, not 4 (lab's illustration). Handoff: the kept candidate is picked by these weights.

**W.** Need: the kept candidate was not drawn by the target, yet it is shaded as if it were (§2.1, Eq. 6). Anchor:
the same two-sample world, M = 2 (toy values, the lab's own; p̂ = f there, so f(y)/p̂(y) = 1). Worked: lists AA 9/16 →
4/3, AB 6/16 → 20/3, BB 1/16 → 12; average 4 = f(A) + f(B). Claim type: unbiasedness is the paper's (§2.1, positive
p and p̂ wherever f ≠ 0); the toy only checks it, and one run is not four. Counterfactual: without the average every
estimate is 1 (lab's illustration); the average weight estimates the target's total (4), the scale
the estimate needs. W's second meaning, used by c4 and c6: on average W is one over the chance RIS keeps y (§4.1,
Eqs. 11-12); in the toy RIS keeps A 6/10 and B 4/10, W then averaging 1/0.6 and 2.5. Handoff: more candidates narrow the spread (c2-ris-more); a reservoir keeps the
sum and the count W needs (c2-wrs). The selftest checks every number narrated (`check_toy`).

**p̂_P(y)·W·M.** Need: a neighbour's sample enters P's reservoir as a candidate, and a candidate needs target over
probability, but no probability was recorded. Construct: r.W estimates 1/p (§4.1, stated), the target is P's because
the weight is P's (§3.2), M lets a reservoir count for its candidates (§4.1). Worked: the spatial readout lists the
three factors and their product for P's own reservoir and each neighbour combined so far (real values, cpu.rs
`combine`). Boundary: in round one every M is 32, so M scales all weights alike and changes nothing (stated as the
lab's observation); it matters for the previous frame's reservoir (c5, the 20× clamp). Handoff: s.W from wsum and the
summed M, as in Eq. 6 (Alg. 4 line 7).

**Eq. 19.** Need: W must average 1/p(y) (Eqs. 11-12). Anchor: Fig. 7's 1D example (bias.rs, the paper's setup).
Construct: on the right half only the flat candidates can produce y, but W divides by all M. Worked: the readout at
the marker (y = 0.6, 20 candidates): 10 of 20 could, true 1/p 2.27, mean W 1.10. Counterfactual and fix: dividing by
abs(Z) instead brings the mean to the truth (c6-fix readout, abs(Z) = 2 of 4); a near-zero step makes it noisy;
the balance heuristic steadies it (Eq. 22). Handoff: Alg. 6 counts the M of the reservoirs whose target at y is
not zero.

## 5. Results (the paper's own setup; no numbers in narration)

Prototype in Falcor; RTX 2080 Ti (Titan RTX for Amusement Park); timings include sample generation, ray tracing, and
shading but not G-buffer rasterisation; error is RMAE against a high-sample reference; temporal methods are shown at
the last frame of a 20-frame animation with fast camera movement (148:10 §6). Comparisons: equal time against Moreau
et al.'s light BVH (Fig. 1, 9); against streaming RIS (Talbot) for biased and unbiased ReSTIR (Fig. 8, 10); error over
render time for six methods (Fig. 11: power sampling with MIS, light BVH, streaming RIS, biased and unbiased
spatiotemporal, biased spatial-only). Biased has less variance with some energy loss at shadow boundaries, sharp
highlights, and complex geometry; with more render time, unbiased eventually has lower error (148:10 §6).

## 6. Where the lab differs or chooses

- **Scene and lights**: the lab's own room with many small emissive lights the lab places (the paper's scenes are not
  available); emitters are point-like or small quads chosen by the lab.
- **Visibility**: no hardware ray tracing in the lab (wgpu without ray queries); shadow rays are traced by the lab's
  own means (decided in Phase 3), stated as the lab's.
- **Candidate generation**: the paper samples a triangle by power with an alias table, then a uniform point; the lab
  follows "by power" with a CDF or an alias table over its own emitters.
- **Neighbour sequence**: "a low-discrepancy sequence" in a 30-pixel disc is all the paper says; the lab's sequence
  is its choice.
- **Material**: the paper's GGX-over-Lambert; the lab's study-room material is the lab's.
- **Environment map**: none in the lab's scene; the 25% split does not apply.
- **N**: the lab runs N = 1 for both variants unless the dock asks for more (the paper's biased N = 4 is noted).
- **Resolution, frame rate**: the lab's viewport, measured on the viewer's machine.

## 7. Open questions

- How the paper picks the temporal neighbour beyond "motion vectors … use the pixel there" (148:9): no rejection test
  for the temporal neighbour is stated; the biased-mode geometry test is stated for "neighboring pixels" (148:9),
  which the lab applies to both spatial and temporal neighbours and narrates as its reading.
- Whether the 20× clamp applies to spatial neighbours: stated only for temporal (148:10); the lab clamps temporal only.
- How visibility reuse interacts with the unbiased weights: stated (148:9 §4.4): p̂_qᵢ includes visibility at qᵢ, one
  extra shadow ray per neighbour.
