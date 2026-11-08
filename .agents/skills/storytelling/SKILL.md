---
name: storytelling
description: How Render Lab's explanations are written, sentence by sentence, and how to find and fix a passage that is hard to follow. Each sentence adds one new thing at its end, starts from what the sentence before ended on, tells a process in verbs before naming it as a noun, and does one job (point, act, branch, name, give a purpose, reframe, check, compress, bridge); a passage builds an idea from an anchor, by contrast, or in stated stages. Covers the sentence map (each sentence's known and new part and its job), the faults it shows, and their fixes, in English and in Chinese. Use when writing, revising, or reviewing a story's narration, a chapter's sentences, a bridge, or on-screen explanatory text (descriptions, inspect text, quiz explanations), or when a reviewer or the user says a passage is unclear. What a story says, its facts and visuals, is `new-story`'s; its syntax is `render-lab-lamlet`'s.
---

# Storytelling: how an explanation is written

A listener follows an explanation when they never have to hold an unexplained word, an unnamed thing, or an unanswered
"why" for long. Every sentence either attaches to something already said or shown, or says where it is going. This
skill is how to write that way, and how to find and fix the sentences that break it.

It is loaded by `new-story` whenever narration is written or revised, and its rules sit on top of that skill's
"Written for the ear" (`new-story`, `references/storyboard.md`): those rules are required and are the floor (subject and
verb first, one appended phrase, no colons or semicolons, one referent per pronoun); this skill is how sentences and
passages are built above it. Where the two seem to disagree in narration, the ear rules win. On-screen text read at the
reader's own pace (descriptions, inspect text, quiz explanations) follows this skill, and may use a colon.

## Choose the depth

| Task | Do | Load |
|---|---|---|
| **A sentence** reworded or added | Its map row (below): its known part comes from the sentence before or the screen, its new part is last and single, its job is clear. Map the sentences on each side of it too: a new sentence moves the chain. | this file |
| **A passage or chapter** written or rewritten | Write it with "Building a passage"; then map every sentence, fix every fault, and re-map what you edited | this file, `references/examples.md` |
| **A review** flagged a passage, or the user says it is unclear | "Finding and fixing": map it, name each fault, fix by the table, re-check against the ear rules | this file, `references/examples.md` |
| **A whole story** reviewed or revised, or many sentences that carry cues edited | The narration as heard, reviewers per story with every flag verified, cue-safe edits, the translation in step, only facts a rewrite may add, then the checks | this file, `references/examples.md`, `references/revision.md` |
| **The translation** | "In Chinese" below, with `new-story`'s rules for the translation | this file |

## The sentence

**One new thing per sentence, and it comes last.** A sentence starts with what the listener already has (the subject:
something just said, or on screen) and ends on what it adds. The end of a spoken sentence is where the stress falls,
so the new thing gets the stress. "The probe stores its light in a small octahedral texture" puts the new idea, the
texture, last. "An octahedral texture is where the probe stores its light" puts it first, before the listener knows why
they are hearing it.

**Explain a new word in the same sentence that brings it**, right after it: "the hit distance, how far the ray went
before it struck a surface". A symbol or number shown on screen is a second channel. The sentence still makes sense to
someone who does not look at it.

**Show it first, then name it.** Describe what a thing does, and give its name at the end of that sentence or in the
next one: "Each pass halves the step, and checks the neighbours at that distance. This search is called jump
flooding." The name arrives when the listener already knows what it names. A term heard before its meaning is noise
that the listener must hold until it is explained.

**Tell a process in verbs, then compress it into a noun.** First say who does what: "The ray steps forward, compares
its depth with the depth buffer, and stops when it falls behind the surface." Once that is heard, compress it: "This
march". From then on the noun carries the whole process. Experts write in nouns ("depth-buffer marching with
early termination") because the nouns already hold the processes for them. A newcomer cannot unpack a noun whose
process they have not seen. A sentence made of three nominalisations and no one acting ("Texel density optimisation
via logarithmic split distribution") is the most common fault in a draft written from source code.

**Keep the same noun for the same thing.** Repeat "the probe" rather than switching to "the sample point" or "it" when
anything else could be meant. A new word suggests a new thing. The repeated nouns form a chain the listener can follow
across a chapter.

**Give each sentence one job**, and vary the jobs:

| Job | Form | Example |
|---|---|---|
| **Point** | what to look at, or a scenario | "Look at the teapot's spout." / "Say a ray leaves this pixel." |
| **Act** | who does what, when | "When the ray reaches the wall, it reads the wall's colour." |
| **Branch** | if this, then that; otherwise the other | "If the depth is closer, the step was a hit. Otherwise the ray steps again." |
| **Name** | what it did, then its name | "This test is called the thickness check." |
| **Purpose** | why the step is there | "That way, one bright texel cannot dominate the blur." |
| **Reframe** | the same thing read another way | "So the dot product is also a measure of how alike two directions are." |
| **Check** | the screen confirms what was said | "The readout shows 0.95, just under one, as predicted." |
| **Compress** | a told process becomes named steps | "So each frame does three things. It traces, it filters, and it blends." |
| **Bridge** | where we are, and where we go next | "That fixes the near cascades. The far ones need something else." |

A passage that stacks one job (five actions in a row, or a definition after a definition) loses the listener. A run of
actions needs a purpose or a check every few sentences. A run of names needs an action that uses them.

## The chain between sentences

**Start each sentence where the last one ended.** The new part at the end of one sentence becomes the known part at the
start of the next: "…so the ray stops at the first texel behind the surface. That texel is the hit." The listener meets
each new idea while still holding the one it attaches to. When a sentence starts from something else, say why it moves
on ("Now the other half of the frame…", "Meanwhile…", "But…").

**Say how each sentence relates to the one before.** "So" marks a consequence, "because" a cause, "but" a contrast,
"instead" a replacement, "then" or "next" the following step, "otherwise" the other branch, "that way" a purpose. A
relation the listener has to infer is one they may infer wrongly.

**Something on screen can be the known part.** The screen counts as already said only once the narration has pointed
at it or the cue has just shown it. A sentence that leans on a picture shown three beats ago has broken the chain.

## Building a passage

A passage, from a beat to a chapter, builds one idea. Pick its shape before writing its sentences:

- **From an anchor.** Pick one concrete thing (one pixel on the teapot, one ray, one probe) and follow it through the
  whole mechanism. Every new step is told as what it does to that one thing. Generalise only after it ("Every pixel
  does the same").
- **By contrast.** Introduce each technique as the fix for something that fails: the simple way, why it breaks (shown
  failing), then the engine's answer. The failure is the reason the next sentence exists.
- **In stated stages.** The simplest version first, then one addition at a time, each named against the last
  ("without the history", "now with the history"), and the stage list said at the start.

Inside any shape, give each object **three readings, in order**: what is done to it (the operation), what that means
(why it matters), and only then what the field calls it. Say the **purpose** next to each step, before or right after
it, never several steps later. Raise the listener's own question where the passage turns ("So how does the trace know
it went behind a surface?") and answer it at once. Say what can be skipped when it is true ("You don't need the exact
formula to follow this"). End a passage on a compress or a bridge, so the next one has a known part to start from.

## Finding and fixing

Map the passage. Write each sentence on its own line, with three columns: its **known part** (where it comes from:
the sentence before, an earlier chapter, the screen), its **new part**, and its **job**. Then read the columns, not the
prose.

| What the map shows | Fault | Fix |
|---|---|---|
| A known part that comes from nowhere | the chain broke | Start from the last sentence's end, or add a bridge that says why the topic moves |
| Two new parts | the sentence is overloaded | Split it, one new part each, the second starting from the first's end |
| The new part at the start | the stress is in the wrong place | Turn the sentence round so the known part is the subject |
| A name with no act before it | a label before its meaning | Move the act first, the name after it |
| Nouns and no actor ("…optimisation via…") | a compressed process | Unpack it into who does what, then compress it again if the noun is needed later |
| A word changes for the same thing | a synonym drifts | Use one noun throughout; "it" only when it can mean one thing |
| Acts with no purpose or check for several sentences | the listener does not know why | Add the purpose, or a check against the screen |
| A relation left implicit | a cause or contrast the listener must guess | Add "so", "because", "but", "instead" |
| The passage ends mid-act | no landing | End on a compress or a bridge |

After fixing, map the edited sentences again and check them against the ear rules. A split often breaks a pronoun, and
a reordering can change a claim: re-check its fact (`new-story`, "What is required at every depth").

**A fix changes how a fact is said, never which facts are said.** It may add a bridge, a landing, the standard gloss
of a general term, a consequence that follows by arithmetic from what is narrated, or a lab difference read from the
lab's code. A reason or purpose for the engine's choice comes only from the engine's source (or the package's
research that cites it); without one, restructure the sentence, or state the value as the engine's choice. When two
sentences contradict each other, or a title disagrees with its beat, settle the fact before rewording.

`references/examples.md`, section 3, has the faults a revision finds most, with their fixes;
`references/revision.md` is the whole-story procedure: reading the narration as heard, reviewing at scale, editing
around cues and ranges, and keeping the translation in step.

## In Chinese

The same structure holds, written as Chinese, not mirrored from the English (`new-story`, `references/storyboard.md`,
"Written for the ear"). Chinese puts the topic first even more strongly than English: open each sentence with what is
already known (一句话先说已知，再说新的), and let the new part land at the end of the sentence, before 。. Name a
technique after describing it (……这就是所谓的……, or ……，叫作……). Keep one word for one thing across the edition (its
`glossary`). Mark the relation with 所以, 因为, 但是, 于是, 否则 rather than leaving it to the reader. The translation
keeps one paragraph per English sentence, so a known-new order that does not work in Chinese is restructured inside
that paragraph, never across two.

## Checks

| Change | Check |
|---|---|
| A sentence | Its map row and its neighbours' rows; the ear rules; `--prepare-only` and the beat's stills (`new-story`) |
| A passage or chapter | The map of every sentence, no fault left; the cold-listener review (`new-story`, `references/review.md`) with no flag left of the kinds it requires fixed |
| A whole story | `references/revision.md`, "Check": layout, `tools/video/check.py` in both languages (paragraph counts), `--prepare-only` for both (a chapter over its budget gets a larger budget, never less explanation), the changed beats' stills |
| Explanatory on-screen text | Its map; both languages' keys (AGENTS.md); `render-lab i18n --check` |
| This skill | `python3 tools/check_skills.py` |
