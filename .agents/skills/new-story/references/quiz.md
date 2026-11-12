# The quiz: questions on how the algorithm works

Load this to write or revise a story's quiz. A quiz is multiple-choice questions on what the story taught, each asked
over a live view of a demonstration: the homepage's Quiz button opens it, the question at the right and the view at
the left, which the viewer can turn, click, and change to find the answer. It is a package's `quiz/<name>` resource;
its format is in `lab/src/quiz.rs`'s module comment.

## What a question asks (required)

- **The algorithm, not the configuration.** Every question asks how the algorithm works: what a pass computes and
  from what, why a step is there, what a choice changes downstream, what would go wrong without it, how two stages
  hand data to each other. Never the configuration (an engine's, or a paper's reported settings): no setting's value or default, no console variable, no
  scalability level, no project setting, and nothing about the lab's own scene (where this room leaks, how many lights
  it has). A number belongs in a question only when it is part of the method (a tile of 8 × 8 pixels, one over the
  probability), not when it is a tunable's default (twelve frames, a tenth).
- **Narrated and verified.** Every right answer is a fact the story narrates, verified against its primary source as the
  story's are (the variant's precision rules).
- **Understanding over recall.** Prefer a question a viewer answers by reasoning from the mechanism (what happens to
  the history when the light changes?) over one answered by remembering a word from the narration.

## The quiz agent (required)

The questions are proposed by a dedicated agent, so the quiz is not the narration's sentences turned around. With
subagents, a fresh agent on a cheaper model (AGENTS.md) that did not write the story; without them, a separate pass of
your own that reads only the materials below, and writes its table before you choose.

**Brief it with:**

- the storyboard (`video/storyboard.lam`): the sentences, by chapter, the only facts a question may rest on;
- the outline's chapters and the topics the story teaches (`render-lab --investigation <id> knowledge`): what the
  viewer should now understand;
- the demonstrations and their interactions (`story/demonstrations.md`), the values and click handlers
  (`render-lab --investigation <id> values`, `actions`): the views a question can be asked over;
- the rules of "What a question asks" above, the algorithm-not-configuration rule first, with the rejected example:
  "At Epic settings, how many shadow rays does it trace per pixel?" asks a setting's value; "Why does the frame's cost
  hardly grow as lights are added?" asks the mechanism.

**It returns** a table of candidate questions, more than the quiz needs (about twice), across the chapters:
`| Chapter | Question | Right answer | Wrong choices, each the misconception it tests | The view and values that show
the answer | The narrated sentence it rests on | What understanding it checks |`. Each wrong choice is a plausible
misreading of the mechanism (a step done elsewhere, a cause mistaken for an effect, a neighbouring technique's way), not
a joke and not a configuration value. It marks any candidate that touches configuration so it can be dropped.

**Choosing.** Take about one question per chapter that teaches a mechanism (six to ten in all), the ones that best
check understanding and that a view can answer; drop every candidate that asks about configuration or rests on no
narrated sentence; verify each right answer's sentence against its primary source. Save the agent's table in
`story/reviews/quiz.md` with what was chosen and why.

## Writing it

- Each question names the demonstration (and the values, the camera) whose view shows the answer, so the viewer can
  find it there, and its explanation says where to look (an inset, a readout, a value to change in the dock) and
  why the answer follows from the mechanism.
- Two to six choices, exactly one right; vary where the right answer sits.
- The text is the package's keys in both languages (fields ending in `-text`, which the package's build checks); no
  formulas or code names yet (the panel does not draw rich text; `quiz::load` refuses them). The Chinese is written as
  Chinese, as the storyboard's translation is.

## Checking it

- The package builds (it checks the keys); `home --snapshot … --page viewer-quiz:<n>@<letter>` for each question
  answered right and wrong, and `viewer-quiz:<count + 1>` for the score; look at them in both languages.
- Read the questions once more against "What a question asks": any about configuration is rewritten or dropped.
