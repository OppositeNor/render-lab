# Fact check (2026-11-12, a fresh agent against the PDF)

About 135 of 160 sentences verified or correct-but-simplified; every index entry's page, section, and `inspect` right.
Fig. 7's "too small by half" confirmed (|Z|/M = 1/2 past x = 1/2, Eq. 19). Rows acted on:

| Where | Finding | Fixed as |
|---|---|---|
| c1 contributions | the paper has no numbered list | "This video groups the paper's ideas into four contributions." |
| c1 title card | "SIGGRAPH" not in the PDF | "ACM Transactions on Graphics 39(4), July 2020" |
| c1 problem | "about one shadow ray" stronger than "only a few rays" (§1) | "only a few" |
| c1 answer | "every pixel still traces one" ignores the unbiased form's rays | "the rays per pixel stay few" |
| c1 answer | "can keep it free of bias" | "makes an unbiased version possible" (§1) |
| c1 answer | "faster" is at equal error | "reaches the same quality faster" |
| c2 order | related work is §7, after the method | said so |
| c2 trees, tiles | "rebuilt"; "cut off" | "maintained"; "typically limited to an area" (§7) |
| c2 IS | formula sentence garbled | split: estimate is f divided by p (Eq. 3) |
| c2 power | "can only draw by power" is the lab's baseline, not a law | "The simplest practical choice, which this lab starts from" |
| c4 neighbours | "visible patterns"; R2 unnamed | "distracting artifacts" (§5); "The lab uses the R2 sequence" |
| c4 test | stated for the biased algorithm only | "In its biased algorithm"; the lab's temporal use said as the lab's |
| c4 rounds | k^n M assumes distinct neighbours | added |
| c5 visibility | "shadows fill with noise" is the lab's observation | "as the lab shows here" |
| c6 balance | Alg. 6 uses 1/Z; MIS version "analogous" | said so; the lab implements 1/Z |
| c6 Talbot | O(M²) when each candidate has its own PDF | added (narration and index) |
| c7 biased | paper's biased form keeps N = 4 | "The lab keeps one reservoir per pixel, where the paper's biased results keep four." |
| c7 energy | "keeps all the energy" not stated | "loses no energy to bias" |
| c8 limits | "struggles" stronger than "degrades … modest" | "Its gain shrinks" |
| c8 takeaway | "without adding shadow rays" false for unbiased | "for the cost of only a few shadow rays" |
| index | `spatial`, `talbot-cost` conditions | added (en, zh-CN) |
