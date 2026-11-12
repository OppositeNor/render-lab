# Revising a storyboard's narration

Load this to review or fix the narration of a whole story, or of a chapter, in its storyboard
(`packages/<package>/video/storyboard.lam` and `storyboard.zh-CN.lam`). `references/storytelling.md` says what a good sentence is;
this file is how to find the faults across a story, how to edit sentences that carry cues, how the translation
follows, and what a rewrite may add.

## 1. Read the narration as it is heard

Review the spoken text, not the source: cues, ranges, and formulas hide the sentence. For one beat,
`python3 tools/video/check.py --investigation <id> --explain <beat>` lists its sentences. For a whole story, write
the narration to a file in the scratchpad, one numbered sentence per line under its beat, with its storyboard line:

```python
import sys
from pathlib import Path
sys.path.insert(0, 'tools/video')
from config import Config
from storyboard import load_board
import markup

cfg = Config.load(sys.argv[1], sys.argv[2], None)          # investigation id, language
lines = []
for c in load_board(cfg)['chapters']:
    lines.append(f"\n## {c.get('title', c['id'])}  [{c['id']}]")
    for b in c['beats']:
        lines.append(f"\n### [{b['id']}, {b.get('kind', '')}]")
        for i, s in enumerate(b.get('narration', [])):
            line = (s.get('source') or {}).get('line', '')
            lines.append(f"{i + 1}. (L{line}) {markup.sentence(s).spoken}")
Path(sys.argv[3]).write_text('\n'.join(lines) + '\n')
```

Run it from the repository's root with the lab built. The same file is what a reviewer reads.

## 2. Review a whole story

A story has hundreds of sentences, so map it with reviewers, one per story (or per few chapters), on a cheaper
model (AGENTS.md). Give each the same brief: read `references/storytelling.md`, `references/storytelling-examples.md`, and
"Written for the ear" (`references/storyboard.md`); judge language only; map every sentence; report only faulty sentences, each with its beat,
number, line, map row, fault, and a rewrite that keeps every fact; mark each rewrite that splits a sentence; mark
"needs fact" where a fix needs a reason the narration lacks; end with the story's drifting terms and its ten most
important fixes. The review of a local package stays in that package.

Verify every flag before acting on it. Reviewers overstate about one flag in four. The usual false flags:

- A pronoun flagged as ambiguous when the sentence before has only one candidate.
- A term flagged as never introduced when the screen shows it (a chapter or beat title, an inset's label, a callout)
  as the sentence is heard. It still deserves a narrated introduction, but the fix is smaller.
- A sentence flagged as stating one effect when it states two in one clause.
- A suggested reason that is plausible but not in the source. Never take a reason from a reviewer.

A flag can also hide a factual error, not a wording one (a heading whose number disagrees with its beat's;
two sentences that contradict each other). Settle those from the lab's code, the package's
`story/research.md`, or the engine's source before rewording anything.

## 3. Edit sentences that carry cues

Each paragraph is one sentence, and its cues are timed to its words (`render-lab-lamlet`). An edit that moves words
moves timing, so:

- **Split.** Keep a range (`overText`, `over`, `overCallout`) inside one sentence, on the words it animates; the
  split-off sentence is plain text in its own paragraph. A cue on the first word stays with the first sentence.
- **Add a bridge or a purpose sentence.** Put it in its own paragraph before the sentence it introduces, and before
  that sentence's cues, so the cues still fire on the words they name.
- **Reorder.** When a beat's cues are general motion (a camera sway, a range that only moves a value), swap the
  sentences' texts and leave the cues where they are. When a cue belongs to a sentence's content (a code highlight,
  a callout, an inset, a panel switch), move the sentence with its cues. A `highlightCode` needs the code panel on
  screen, so a reordered `panelValues` cue must stay after it.
- **Do not move what a motion cue depends on.** A sentence carrying a long camera move or a light's sweep can
  change what every later sentence of the beat shows. If moving it would break the beat's picture, keep the order
  and add a bridge ("Back to how the map is made, ...").
- **Remove a sentence carefully.** A chain of ranges on one value (`pickYRange 205.0 200.0`, then `200.0 197.5`)
  must stay continuous: when a sentence and its range go, start the next range where the removed one started.
- **Edit as a batch.** For many edits, write them as exact `(file, old, new)` replacements and apply them with a
  script that refuses any `old` that does not occur exactly once, so a failed match changes nothing. Back up a local
  package's storyboards first; they are not in git.

## 4. Keep the translation in step

`storyboard.zh-CN.lam` has one paragraph per English sentence, matched by beat:

- A split English sentence splits its Chinese paragraph; an added sentence adds a paragraph in the same place; a
  reordered pair reorders the pair. A changed English sentence changes its Chinese paragraph only when its meaning or
  its order of ideas changed.
- Write the new paragraph as Chinese (`references/storytelling.md`, "In Chinese"), with the same term the edition uses elsewhere. Check
  that a new word does not collide with one already taken: if 脏区 is the dirty slab, the new space before it is
  marked needs another word (薄层).
- Change a renamed term everywhere in both editions (once "the probe cube" is the English term, the Chinese edition says
  探针立方体 in every paragraph that means it), and a renamed heading in both.

`python3 tools/video/check.py --investigation <id> --lang zh-CN` fails when a beat's paragraph count differs from the
English one, and names the beat.

## 5. What a rewrite may add

A rewrite changes how a fact is said, not which facts are said. It may add:

- a bridge, a stage statement, or a landing that restates what the narration has said ("So the alpha byte holds the
  roughness, and says whether the object moves");
- a gloss of a general term in its standard meaning ("a thread group, a team of threads the GPU runs together");
- a consequence that follows by arithmetic or logic from facts already narrated ("leaves out the division by two
  pi, so the panel lights the cells two pi times as brightly as direct lighting would");
- a lab difference read from the lab's code (`render-lab` shaders and the package's `src/`), said as the lab's.

It may not add a reason, a purpose, or a consequence that needs the engine's source, unless it is read there (or in
the package's `story/research.md`, which cites it) for this edit. Where a fault needs such a fact and none is at hand,
restructure the sentence without it, or state a constant as the engine's choice. A new or changed fact still needs
its callout or its chapter's `sources` (SKILL.md).

## 6. Check

After the edits, for each changed story:

1. `python3 tools/lamlet_fmt.py` (tracked files), and the local packages' storyboards by path; then `--check`.
   In zsh, pass several paths through an array (`L=(a b); … "${L[@]}"`), since a string is one word.
2. `python3 tools/video/check.py --investigation <id>` for `en` and `zh-CN`.
3. `python3 tools/video/build.py --investigation <id> --prepare-only`, English then `--lang zh-CN`, one at a time.
4. The stills of the changed beats (`references/storyboard.md`), looked at.
5. A new map of every edited sentence and its neighbours: a split often leaves a pronoun pointing at the wrong half.
