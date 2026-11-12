"""Writes the ReSTIR investigation's citation index, packages/paper_restir/source-index.json, from the paper's PDF.

Each entry cites the paper by section, page (the article's numbering, 148:n, page n of the PDF), and the equation,
algorithm line, or figure it rests on, and keeps a short quote that locates the passage; this script finds each
quote on its page in the PDF's text (`pdftotext`, reading order) and records its hash, so a citation never drifts
by hand. The PDF is wherever the user keeps it: `RESTIR_PAPER_PDF`, or asked. Run from anywhere:

    RESTIR_PAPER_PDF=/path/to/paper.pdf python3 packages/paper_restir/src/paper/restir/source_index.py
"""

import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

PACKAGE = Path(__file__).resolve().parents[3]
PDF_SHA256 = 'a1ae1233eecd4f82cc9502900de08012e9a0c233fb8014f53f20c2e9814f243d'

PAPER = {
    'key': 'bitterli2020',
    'short': 'Bitterli et al. 2020',
    'title': 'Spatiotemporal reservoir resampling for real-time ray tracing with dynamic direct lighting',
    'venue': 'ACM TOG 39(4)',
    'doi': '10.1145/3386569.3392481',
    'sha256': PDF_SHA256,
    'note': 'Pages are the article\'s, 148:1 to 148:17 (page n of the PDF is 148:n). quoteHash is the first 16 hex digits of the SHA-256 of the quote with its whitespace collapsed.',
}

# (key, section, page, symbol, quote, inspect): the quote as the PDF's text has it.
ENTRIES: list[tuple[str, str, int, str, str, str]] = [
    ('problem', 'Abstract', 1, 'Abstract', 'Efficiently rendering direct lighting from millions of dynamic light sources',
     'The problem: direct light from millions of dynamic lights by Monte Carlo integration, hard even offline.'),
    ('approach', '1', 2, 'Introduction', 'Our approach builds on resampled importance sampling',
     'The approach: resampled importance sampling, with a small fixed-size reservoir that stores only accepted samples.'),
    ('reservoir-intro', '1', 2, 'Introduction', 'Given the reservoir, our approach does not use any data structures more complicated than fixed-size arrays',
     'No data structure more complicated than fixed-size arrays; each pixel\'s sampling PDF improves by reusing its neighbours\' statistics.'),
    ('pdfs-not-colors', '1', 2, 'Introduction', 'our reuse informs the sampling probabilities used within the renderer',
     'Unlike denoisers, which reuse colours, the reuse informs sampling probabilities, which makes an unbiased algorithm possible.'),
    ('biased-mode', '1', 2, 'Introduction', 'Our unbiased mode can be modified to be biased',
     'A biased mode reduces noise further at the cost of some darkening near geometric discontinuities.'),
    ('first-vertex', '1', 2, 'Introduction', 'direct lighting at the first camera vertex',
     'The paper considers direct lighting at the first camera vertex only.'),
    ('eq1', '2', 2, 'Eq. 1', 'The reflected radiance L of a point y in direction',
     'Direct light at y: the integral over the emitting surfaces of the BSDF, the emitted radiance, the geometry term, and visibility.'),
    ('eq3', '2', 3, 'Eq. 3', 'Standard Monte Carlo importance sampling',
     'Importance sampling: the mean of f/p over N samples from a source PDF p, unbiased where p > 0 wherever f is not zero.'),
    ('eq4', '2', 3, 'Eq. 4', 'MIS remains unbiased',
     'MIS: samples from several strategies, weighted; unbiased while the weights sum to one; the balance heuristic.'),
    ('eq5', '2.1', 3, 'Eq. 5', 'It then randomly chooses an index',
     'RIS draws M candidates from a source p and picks one with probability proportional to its weight w = p̂ / p.'),
    ('eq6', '2.1', 3, 'Eq. 6', 'Intuitively, the estimator uses y as if it were drawn from',
     'The one-sample RIS estimator: f(y) / p̂(y) times the mean of the candidates\' weights, which corrects for y being only approximately distributed as p̂.'),
    ('ris-unbiased', '2.1', 3, 'Eq. 7', 'RIS is unbiased as long as M, N ≥ 1',
     'RIS is unbiased for M, N ≥ 1 when p and p̂ are positive wherever f is not zero; the paper sets N = 1 from here on.'),
    ('ris-converges', '2.1', 3, 'Section 2.1', 'the distribution of y approaches p̂ as M → ∞',
     'As M grows, the distribution of the selected sample approaches the target p̂.'),
    ('alg1', '2.1', 3, 'Alg. 1', 'Algorithm 1: Resampled importance sampling.',
     'Algorithm 1: generate M proposals and their weights, build a normalised CDF, draw an index from it.'),
    ('wrs', '2.2', 4, 'Eq. 8', 'Weighted reservoir sampling (WRS)',
     'Weighted reservoir sampling: one pass over a stream, each element selected with probability its weight over the sum.'),
    ('eq9', '2.2', 4, 'Eq. 9-10', 'The update rule stochastically replaces',
     'The update replaces the kept sample with the new one with probability its weight over the running sum; every earlier one stays in proportion to its weight.'),
    ('alg2', '2.2', 4, 'Alg. 2', 'Algorithm 2: Weighted reservoir sampling.',
     'Algorithm 2: a reservoir keeps the output sample, the sum of weights, and the count M.'),
    ('alg3', '3.1', 5, 'Alg. 3', 'Algorithm 3: Streaming RIS using weighted reservoir sampling.',
     'Algorithm 3: RIS as a stream through one reservoir, W from the weight sum (line 8), shading f(y) · W (line 11).'),
    ('streaming-cost', '3.1', 5, 'Section 3.1', 'computation remains linear in M',
     'Streaming makes storage constant, but computation stays linear in M.'),
    ('combine', '3.2', 5, 'Section 3.2', 'To combine two reservoirs, we treat each reservoir',
     'Combining reservoirs: each reservoir\'s sample is fed to a new reservoir as a fresh sample, equivalent to resampling their combined streams.'),
    ('reweight', '3.2', 5, 'Alg. 4, line 4', 'we reweight the samples with the factor',
     'A neighbour\'s sample is reweighted by p̂_q / p̂_q′ for this pixel\'s target, written p̂_q(r.y) · r.W · r.M.'),
    ('alg4', '3.2', 6, 'Alg. 4', 'Algorithm 4: Combining the streams of multiple reservoirs.',
     'Algorithm 4: update with each reservoir\'s sample weighted p̂_q · W · M, M the sum of theirs, W from the weight sum.'),
    ('spatial', '3.2', 6, 'Section 3.2', 'Crucially, spatial reuse can be repeated',
     'Spatial reuse can repeat, each round reading the last: O(nk + M) work for k^n M effective candidates, assuming distinct neighbours.'),
    ('temporal', '3.2', 6, 'Section 3.2', 'After rendering a frame, we store each pixel',
     'Temporal reuse: each pixel\'s final reservoir is kept and combined in the next frame, so a frame reuses every frame before it.'),
    ('visibility', '3.2', 6, 'Section 3.2', 'we also perform visibility reuse',
     'Visibility reuse: before reuse, each pixel\'s selected sample is tested, and an occluded one discarded.'),
    ('alg5', '3.2', 6, 'Alg. 5', 'Algorithm 5: Our algorithm for RIS with spatiotemporal reuse.',
     'Algorithm 5: candidates, visibility, temporal reuse, n rounds of spatial reuse, shading; the reservoirs returned for the next frame.'),
    ('bias-source', '4', 6, 'Section 4', 'Each pixel uses a different integration domain and target distribution',
     'Reusing candidates from adjacent pixels mixes samples from different PDFs, which standard RIS does not accommodate.'),
    ('eq12', '4.1', 7, 'Eq. 11-12', 'where W is the stochastic weight for the generated sample',
     'W is a stochastic weight that takes the place of 1/p(y); unbiasedness needs its expected value to be 1/p(y).'),
    ('weight-meaning', '4.1', 7, 'Section 4.1', 'simply represents the standard RIS weight',
     'p̂_q(r.y) · r.W is the RIS weight p̂ / p with r.W estimating the inverse PDF, scaled by the r.M candidates behind it.'),
    ('eq19', '4.2', 7, 'Eq. 19', 'the inverse PDF is consistently underestimated',
     'Where some candidate PDFs are zero, the expected W is |Z(y)|/M times 1/p(y): too small, so the image is darker.'),
    ('fig7', '4.2', 8, 'Fig. 7', 'We show results of RIS for sampling a simple linear target PDF',
     'Figure 7: a linear target 2 − 2x sampled from a constant PDF and a step; RIS\'s weight against the true inverse PDF.'),
    ('eq20', '4.3', 8, 'Eq. 20-21', 'Instead of multiplying by the factor 1/M',
     'Unbiased RIS: replace 1/M with a weight m that sums to one over the candidates that could have produced the sample.'),
    ('naive-m', '4.3', 8, 'Section 4.3', 'we divide by the number of candidates with non-zero PDFs',
     'The naive weight 1/|Z|: unbiased, but noisy where a PDF is near zero rather than zero.'),
    ('eq22', '4.3', 8, 'Eq. 22', 'the balance heuristic of the candidate PDFs',
     'm as the balance heuristic of the candidate PDFs solves both bias and noise.'),
    ('talbot-cost', '4.3', 8, 'Section 4.3', 'Talbot et al. evaluate all PDFs for each candidate sample',
     'When each candidate has its own PDF, Talbot et al.\'s alternative weight costs O(M²) PDF evaluations; the paper\'s O(M).'),
    ('alg6', '4.4', 9, 'Alg. 6', 'Algorithm 6: Unbiased combination of multiple reservoirs.',
     'Algorithm 6: Z sums the M of the sources whose p̂ at the sample is not zero; W uses m = 1/Z (lines 6-11).'),
    ('approx-pdf', '4.4', 9, 'Section 4.4', 'as an approximation to the real PDF of samples',
     'Each source pixel\'s p̂ stands in for its samples\' unknown PDF, zero wherever that PDF is; with visibility reuse, visibility is part of it.'),
    ('unbiased-cost', '4.4', 9, 'Section 4.4', 'this means tracing k additional rays',
     'With visibility reuse, the unbiased combination traces one more shadow ray per neighbour.'),
    ('candidates-param', '5', 9, 'Section 5', 'We sample M = 32 initial candidates',
     'M = 32 candidates, an emissive triangle by power, then a uniform point on it (p ∝ Le); 25% from the environment map when there is one.'),
    ('alias', '5', 9, 'Section 5', 'accelerated using an alias table',
     'Lights are picked with an alias table (Walker 1974).'),
    ('target-param', '5', 9, 'Section 5', 'We use the unshadowed path contribution',
     'The target p̂ ∝ ρ · Le · G, unshadowed; the material a GGX dielectric layer over a Lambertian base.'),
    ('neighbors-param', '5', 9, 'Section 5', 'we instead sample k = 5',
     'k = 5 neighbours (3 unbiased), random points in a 30-pixel radius from a low-discrepancy sequence; deterministic ones gave artifacts.'),
    ('temporal-pick', '5', 9, 'Section 5', 'we compute motion vectors to project the current pixel',
     'The temporal neighbour: the current pixel\'s position projected into the previous frame with motion vectors.'),
    ('geometry-test', '5', 9, 'Section 5', 'compare the difference in camera distance',
     'The biased algorithm rejects a neighbour whose camera distance differs by more than 10% or whose normal turns more than 25°.'),
    ('spatial-passes', '5', 10, 'Section 5', 'We use n = 2 (n = 1 for our unbiased algorithm)',
     'n = 2 spatial rounds (1 unbiased).'),
    ('sample-count', '5', 10, 'Section 5', 'our biased algorithm uses N = 4 instead',
     'N = 1 for the unbiased algorithm; the biased one keeps four reservoirs per pixel.'),
    ('storage', '5', 10, 'Section 5', 'we only store the information of the pixel',
     'Each pixel stores only its reservoir: the selected sample y, M, and W.'),
    ('clamp', '5', 10, 'Section 5', 'clamp the previous frame',
     'Last frame\'s M is clamped to at most 20 times the current frame\'s, bounding stale temporal samples.'),
    ('name', '6', 10, 'Section 6', 'or ReSTIR for short',
     'The paper names its algorithm Reservoir-based SpatioTemporal Importance Resampling, ReSTIR.'),
    ('still-images', '6', 10, 'Section 6', 'makes it possible to evaluate our algorithm for still images',
     'Biased spatial reuse without temporal reuse is the variant for still images.'),
    ('results-setup', '6', 10, 'Section 6', 'We report image errors of each method compared',
     'Errors against an unbiased reference at high sample count, as relative mean absolute error.'),
    ('biased-vs-unbiased', '6', 10, 'Section 6', 'our biased method has considerably less variance',
     'The biased method has less variance at the cost of energy loss at shadow boundaries, sharp highlights, and complex geometry.'),
    ('many-lights', '7', 10, 'Section 7', 'pioneered this area',
     'Many-light sampling: Ward and Shirley et al. classified lights as important and unimportant; light hierarchies sample them in sub-linear time.'),
    ('hierarchies', '7', 12, 'Section 7', 'Our approach eliminates the cost of maintaining complex data structures',
     'Against light hierarchies: no data structure to maintain, and samples that account for the BSDF and visibility.'),
    ('tiled', '7', 12, 'Section 7', 'tiled shading',
     'Tiled shading limits the lights per tile, which biases each light\'s reach.'),
    ('path-reuse', '7', 13, 'Section 7', 'a visibility ray must be traced each time a path is reused',
     'Path reuse traces a visibility ray per reuse; ReSTIR traces rays for only a few samples.'),
    ('ratio', '7', 13, 'Section 7', 'Resampling techniques, including our method, are related to ratio estimators',
     'Ratio, weighted uniform, and weighted importance sampling estimators: biased but consistent; RIS can be seen as making them unbiased.'),
    ('reservoir-history', '7', 14, 'Section 7', 'emerged in the early 1980s',
     'Reservoir sampling began in the early 1980s, to select data on tape drives without rewinding.'),
    ('denoisers', '7', 14, 'Section 7', 'These approaches are largely orthogonal to our work',
     'Denoisers are orthogonal: they can improve ReSTIR\'s output when it is not converged.'),
    ('conclusion', '8', 14, 'Section 8', 'filter PDFs rather than colors',
     'The conclusion: denoising moved into the renderer, filtering PDFs rather than colours.'),
    ('limitations', '8.1', 14, 'Section 8.1', 'near disocclusions, lighting discontinuities',
     'Limits: quality degrades where pixels are not correlated: disocclusions, lighting discontinuities, complex geometry, fast lights.'),
    ('screen-space', '8.1', 14, 'Section 8.1', 'it limits the use of our method to operations on the first vertex',
     'Image buffers limit the method to the primary hit; world-space reuse is future work.'),
]

# The works the story cites, in APA style (7th edition), alphabetical; the card draws plain text, so titles are not italic.
REFERENCES: list[tuple[str, str]] = [
    ('bekaert2002', 'Bekaert, P., Sbert, M., & Halton, J. (2002). Accelerating path tracing by re-using paths. In Proceedings of the 13th Eurographics Workshop on Rendering (pp. 125–134). Eurographics Association. https://doi.org/10/ggdwkn'),
    ('bitterli2020', 'Bitterli, B., Wyman, C., Pharr, M., Shirley, P., Lefohn, A., & Jarosz, W. (2020). Spatiotemporal reservoir resampling for real-time ray tracing with dynamic direct lighting. ACM Transactions on Graphics, 39(4), Article 148. https://doi.org/10.1145/3386569.3392481'),
    ('chao1982', 'Chao, M.-T. (1982). A general purpose unequal probability sampling plan. Biometrika, 69(3), 653–656. https://doi.org/10/fd87zs'),
    ('estevez2018', 'Estevez, A. C., & Kulla, C. (2018). Importance sampling of many lights with adaptive tree splitting. Proceedings of the ACM on Computer Graphics and Interactive Techniques, 1(2), Article 25. https://doi.org/10/ggh89v'),
    ('moreau2019', 'Moreau, P., Pharr, M., & Clarberg, P. (2019). Dynamic many-light sampling for real-time ray tracing. In High-Performance Graphics: Short Papers. Eurographics Association. https://doi.org/10/ggh89m'),
    ('olsson2011', 'Olsson, O., & Assarsson, U. (2011). Tiled shading. Journal of Graphics, GPU, and Game Tools, 15(4), 235–251. https://doi.org/10/bbfdms'),
    ('schied2017', 'Schied, C., Kaplanyan, A., Wyman, C., Patney, A., Chaitanya, C. R. A., Burgess, J., Liu, S., Dachsbacher, C., Lefohn, A., & Salvi, M. (2017). Spatiotemporal variance-guided filtering: Real-time reconstruction for path-traced global illumination. In Proceedings of High Performance Graphics (Article 2). ACM. https://doi.org/10/ggd8dg'),
    ('shirley1996', 'Shirley, P., Wang, C., & Zimmerman, K. (1996). Monte Carlo techniques for direct lighting calculations. ACM Transactions on Graphics, 15(1), 1–36. https://doi.org/10/ddgbgg'),
    ('talbot2005', 'Talbot, J. F., Cline, D., & Egbert, P. (2005). Importance resampling for global illumination. In Eurographics Symposium on Rendering (pp. 139–146). Eurographics Association. https://doi.org/10/gfzsm2'),
    ('veach1995', 'Veach, E., & Guibas, L. J. (1995). Optimally combining sampling techniques for Monte Carlo rendering. In Proceedings of SIGGRAPH 95 (pp. 419–428). ACM. https://doi.org/10/d7b6n4'),
    ('ward1994', 'Ward, G. J. (1994). Adaptive shadow testing for ray tracing. In P. Brunet & F. W. Jansen (Eds.), Photorealistic Rendering in Computer Graphics (pp. 11–20). Springer. https://doi.org/10/b7zrhm'),
]

# The chapters' reading tables, in story order.
CHAPTERS: list[tuple[str, list[str]]] = [
    ('The paper and its problem', ['name', 'problem', 'approach', 'reservoir-intro', 'pdfs-not-colors', 'biased-mode', 'first-vertex']),
    ('Related work', ['eq1', 'eq3', 'eq4', 'eq5', 'eq6', 'ris-unbiased', 'ris-converges', 'alg1', 'wrs', 'eq9', 'alg2', 'many-lights', 'hierarchies', 'tiled', 'path-reuse', 'ratio', 'reservoir-history', 'denoisers']),
    ('Streaming RIS', ['alg3', 'streaming-cost', 'candidates-param', 'alias', 'target-param', 'storage']),
    ('Spatial reuse', ['combine', 'reweight', 'alg4', 'spatial', 'neighbors-param', 'geometry-test', 'spatial-passes']),
    ('Temporal and visibility reuse', ['temporal', 'temporal-pick', 'clamp', 'visibility', 'alg5']),
    ('Bias, and removing it', ['bias-source', 'eq12', 'weight-meaning', 'eq19', 'fig7', 'eq20', 'naive-m', 'eq22', 'talbot-cost', 'alg6', 'approx-pdf', 'unbiased-cost']),
    ('Experiments on this machine', ['still-images', 'results-setup', 'biased-vs-unbiased', 'sample-count']),
    ('Conclusion', ['conclusion', 'limitations', 'screen-space']),
]

DEMOS: dict[str, list[str]] = {'room': ['alg5', 'alg3', 'alg4', 'alg6', 'clamp']}

MAPPINGS = [{
    'lab': 'Reservoir',
    'engineShort': 'reservoir (y, M, W)',
    'engineName': 'A pixel\'s reservoir: the selected sample y, the candidates it stands for M, and the probabilistic weight W (§5, "Reservoir storage")',
    'relationship': 'exact',
    'sources': ['storage', 'alg2'],
    'exact': 'y, M, and W per pixel; N reservoirs per pixel for N samples.',
    'labLayout': 'One Rgba32Uint texel per pixel and stage: the light\'s index, the point on it as two 16-bit coordinates, then W and M as float bits.',
}]

PASSES = [
    {'lab': 'restirCandidates', 'engine': 'Alg. 5 lines 3-5: RIS(q), Alg. 3', 'sources': ['alg3', 'alg5']},
    {'lab': 'restirVisibility', 'engine': 'Alg. 5 lines 6-9: visibility', 'sources': ['visibility', 'alg5']},
    {'lab': 'restirTemporal', 'engine': 'Alg. 5 lines 10-14: temporal reuse', 'sources': ['temporal', 'alg5']},
    {'lab': 'restirSpatialTA', 'engine': 'Alg. 5 lines 15-20: spatial reuse, round 1', 'sources': ['spatial', 'alg4', 'alg6']},
    {'lab': 'restirSpatialAB', 'engine': 'Alg. 5 lines 15-20: spatial reuse, rounds 2 and 4', 'sources': ['spatial', 'alg4', 'alg6']},
    {'lab': 'restirSpatialBA', 'engine': 'Alg. 5 lines 15-20: spatial reuse, round 3', 'sources': ['spatial', 'alg4', 'alg6']},
    {'lab': 'restirShade', 'engine': 'Alg. 5 lines 21-23: shadePixel, Alg. 3', 'sources': ['alg3', 'alg5']},
]


def pdf_path() -> Path:
    """The PDF: RESTIR_PAPER_PDF, else asked."""
    given = os.environ.get('RESTIR_PAPER_PDF', '')
    if not given:
        given = input('Where is the PDF of "Spatiotemporal reservoir resampling …" (Bitterli et al. 2020)? ').strip()
    return Path(given).expanduser().resolve()


def collapse(text: str) -> str:
    """Whitespace collapsed, and a word hyphenated across a line joined."""
    return re.sub(r'\s+', ' ', re.sub(r'-\n(?=[a-z])', '', text)).strip()


def page_text(pdf: Path, page: int) -> str:
    out = subprocess.run(['pdftotext', '-f', str(page), '-l', str(page), str(pdf), '-'], check=True, capture_output=True, text=True)
    return collapse(out.stdout)


def entries(pdf: Path) -> dict[str, dict[str, object]]:
    """Each entry with its quote found on its page and hashed; a quote not found stops the script."""
    pages: dict[int, str] = {}
    out: dict[str, dict[str, object]] = {}
    missing: list[str] = []
    for key, section, page, symbol, quote, inspect in ENTRIES:
        # Each quote must be on the page it cites.
        text = pages.setdefault(page, page_text(pdf, page))
        if collapse(quote) not in text:
            missing.append(f'{key} (p. 148:{page}): {quote!r}')
            continue
        digest = hashlib.sha256(collapse(quote).encode()).hexdigest()[:16]
        # The abstract has no section number: its entries cite the page alone.
        entry: dict[str, object] = {'page': f'148:{page}', 'symbol': symbol, 'quote': quote, 'quoteHash': digest, 'inspect': inspect, 'details': []}
        if section != 'Abstract':
            entry['section'] = section
        out[key] = entry
    if missing:
        raise SystemExit('quotes not found on their pages:\n  ' + '\n  '.join(missing))
    return out


def main() -> None:
    pdf = pdf_path()
    digest = hashlib.sha256(pdf.read_bytes()).hexdigest()
    if digest != PDF_SHA256:
        raise SystemExit(f'{pdf} is not the edition the index cites (SHA-256 {digest}, expected {PDF_SHA256})')
    index = {
        'schema': 1,
        'paper': PAPER,
        'references': [{'key': k, 'apa': apa} for k, apa in REFERENCES],
        'entries': entries(pdf),
        'chapters': [{'number': n, 'title': t, 'reading': r} for n, (t, r) in enumerate(CHAPTERS, 1)],
        'demos': DEMOS,
        'mappings': MAPPINGS,
        'passes': PASSES,
    }
    (PACKAGE / 'source-index.json').write_text(json.dumps(index, ensure_ascii=False, indent=2) + '\n')
    print(f'wrote {len(index["entries"])} entries to {PACKAGE / "source-index.json"}', file=sys.stderr)


if __name__ == '__main__':
    main()
