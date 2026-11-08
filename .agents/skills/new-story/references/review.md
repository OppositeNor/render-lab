# Phase 7: Review, per chapter; and revising a finished story

Load this to review a drafted chapter or story, or to act on a viewer's feedback about a built one.

## Stills for the reviewers

Render a contact sheet of stills for each chapter (several per animated beat). Renders are GPU jobs, so the main agent
makes them one at a time and gives the reviewers the image paths.

## Reviewers

One review per chapter. With subagents, send one reviewer per chapter (in parallel when your environment allows);
without them, review each chapter yourself in a separate pass, reading only that chapter's materials, and answer every
question below before moving on. Each review gets the chapter's storyboard text, its outline section, its stills, and
these questions:

- **Depth.** Is each step explained, not just named? What would a viewer who knows graphics but not this engine still
  not understand?
- **Intuition.** Does each animation make its point clearer than a still would? Which sentence points at nothing on
  screen?
- **Highlighting.** Is each key concept lit (a highlight, a callout, a lit formula term, a label) while it is named?
- **Code and animation.** At each listing highlight, does the scene change to show that line's work? Which highlight
  leaves the scene as it was?
- **Change.** Where the narration says a value depends on another, does a sweep show it changing?
- **Audience.** Against the chapter's line of `render-lab --investigation <id> knowledge` (the `knowledge-graph`
  skill): is every topic it teaches explained, every level topic recalled, nothing the audience knows explained at
  length?
- **Teaching.** For each topic it teaches: are the steps of "Teaching a topic" (`references/outline.md`) there (the
  question, an intuition, a worked example animated, exploration the viewer can try, the rule, its limits, the link
  back)? Could a viewer who did not know it explain it now? Which step is missing or rushed?
- **Visibility.** In every still, is what the sentence names in view, or does a wall, an object, an inset, or the
  readout block it, or does it fall outside the frame or past the far plane?
- **Reading the views.** For every texture or buffer view, does the narration say, when it appears, what it is, its
  layout, what its colours mean, and where the element in question sits in it? Which view is shown and passed over?
- **Flow.** Does the chapter's first sentence bridge from the one before, and does each beat follow from the last?
  Where does the viewer lose the thread?
- **Sentences.** Does any sentence carry more than one primary idea, or a second change on screen
  (`references/storyboard.md`)? Which sentence was split so far that a comparison or a cause lost its link?
- **Fit.** Is the chapter near its budget?
- **Purpose.** Could a viewer who has just watched it explain not only what happens, but why, and where the result is
  used next? For each mechanism, which of the questions in "The shape of a story" (problem, data, algorithm, result,
  trade-off, next stage) does it leave unanswered?
- **Mental model.** Act as a senior rendering engineer and a graphics professor, and do not simply agree with the
  draft: find missing reasoning steps, wrong causal links, unexplained terms, terms used inconsistently, data shown as
  if it were computation (or one kind of data drawn as another), oversimplified mechanisms, and visuals that look
  convincing but teach the wrong model. Don't ask for complexity for its own sake, nor for less depth to make it
  easier.
- **Load.** Does any moment show more than one idea, or a panel denser than the sentence needs? Is the code only the
  lines that matter, and readable?

Each review returns a table, `| Beat (or timestamp) | Issue | Severity | Why it matters | Specific correction |`,
specific enough to act on ("add a sweep of `rayCount` 1 → 16 while the second sentence is said", never "improve
clarity").

## The interaction review (required)

Per chapter, the dedicated interaction review of `references/demonstrations.md` ("The interaction review"), on the
chapter as built: its views, their stills, and its values and handlers. A fresh agent, not one of the reviewers above,
so its eye is on what the viewer can try, not on what the narration says.

## The cold-listener review (required)

The reviewers above see the source, the outline, and the stills, which fill in what the words leave out. This one
does not. It checks that the narration is clear on one hearing.

It is a fresh agent (a cheaper model will do) given:
- the chapter as it is heard: its sentences, one per line, with `[brackets]` for what is on screen while each is said
  (headings, a formula as shown, what an inset or the scene shows);
- the narration of the chapters before it, as "already heard";
- the listener card.

It gets no source, no outline, and no storyboard markup. Its prompt:

> Play this listener, and stay in that role: you know only what is below. For each sentence, in order, say honestly
> what you get on one hearing. Flag every sentence where you: (1) lost what a word or phrase refers to; (2) met a term
> not defined, or defined too long ago to remember; (3) had to re-parse: the structure resolved only at the end, a
> clause attached to the wrong thing, facts listed with no verbs, a colon or semicolon joining two ideas; (4) heard a
> fact with no reason: you could not tell why it matters or what it changes; (5) heard two or more new ideas at once;
> (6) lost the thread: you could not tell how this sentence follows from the one before, or heard a name before what
> it names.
> Do not flag an engine constant the narration states as the engine's choice. Do not judge correctness, and do not
> suggest rewrites. For each flagged sentence: its first five words, the problem numbers, and one line on what you
> lost. Then what you understood the chapter to say, and what you would still ask.

Fix the flagged sentences under "Written for the ear" (`references/storyboard.md`) and the `storytelling` skill's
"Finding and fixing" (map the flagged sentence with its neighbours: a flag is often caused by the sentence before),
re-check every edited sentence against those rules, and run it again. Judge by the flags, not by a score: the chapter
passes when no flag of kinds 1, 2, 3, or 6 is left, and each flag of kinds 4 and 5 is either fixed or answered by the
picture. Its summary of the chapter
should match the outline's claims. The same review runs for the translation, with a listener who hears Chinese.

## The native-language review (required)

One reviewer per language checks that the edition reads the way a native speaker writes and speaks it: idiom, word
choice, word order, rhythm, register, and collocations, not grammar alone. It proposes polishing suggestions; it does
not rewrite the chapter. **Each reviewer is briefed, and talked to, in its own language**: the English reviewer in
English, the Chinese reviewer in Chinese, so that the brief itself sets the language it judges.

It is a fresh agent given only its edition's chapter as heard (sentences one per line, `[brackets]` for what is on
screen) and the listener card, translated into its language; no source, no other edition. For the translation, it does
not see the English, so it judges the Chinese on its own rather than against the original.

The English brief:

> You are reviewing the narration of a technical explainer video, read aloud to a graphics programmer. Judge it as a
> native English speaker who writes technical narration well. For each sentence that a native speaker would not say
> this way (an unidiomatic phrase, an odd word order, a word that is close but not right, a stiff or translated-
> sounding construction, a rhythm that trips when spoken), give: its first five words, what sounds wrong, and one
> polished version that keeps its exact meaning and its technical terms. Keep each suggestion one sentence, with no
> colon or semicolon. Do not judge the facts, do not add content, and leave sentences that already sound natural
> alone. End with the habits you saw repeated across the chapter.

The Chinese brief:

> 你在审阅一段技术讲解视频的中文字幕旁白，读者是图形程序员。请以一位擅长写技术文章的中文母语者的标准来判断。
> 对每一句母语者不会这样说的句子（不地道的搭配、别扭的语序、意思接近但用词不准、生硬或带翻译腔的结构、“的”字连用过多、
> 读起来拗口的节奏），给出：该句的前几个字、哪里不自然、以及一个润色后的版本，要求意思完全不变、技术术语保持原样。
> 每条建议仍是一句话，不要用冒号或分号连接两个意思。不要评判事实，不要增加内容，已经自然的句子不要改动。
> 最后总结你在本章中反复看到的习惯问题。

Apply a suggestion only when it keeps the sentence's claim, its cue words (the words a cue is placed on), and the rules
of "Written for the ear"; a polished English sentence changes its zh-CN paragraph only if the meaning moved. Re-check
every applied sentence against those rules.

## The fact check (required)

A check not anchored on the drafts verifies the facts (`new-investigation`, procedure step 8, "Audit"): every factual
sentence and every `inspect`, against the checkout, in that step's classes, with file:line evidence. With subagents, a
fresh agent that has not seen the drafting does it. Without them, do it yourself from the checkout and the source index
only, claim by claim, writing each verdict and its evidence into `story/reviews/facts.md` before reading the next
claim's narration context. Uncertain claims are never rewritten as facts: they are verified, narrated as uncertain, or
cut.

## Acting on the reviews

Save the reviews in `story/reviews/`. Then revise: more animation where the reviewers found a still idea, more
explanation where they found a gap, and every fact that is not verified (or a stated simplification) fixed in the
narration, the index, the README, and the translation. Report the corrections to the user with their evidence.

Done when every row of every review (the interaction review's included) is fixed or answered with a reason, the fact check has no claim left that is
neither verified nor a stated simplification, and the revised chapters pass Phase 5's checks again.

## Revising a finished story

When a story is built and a viewer (the user, or a review they pass on) asks for it to be better, improve it; don't
remake it. Review feedback is input to check, not a script: verify each point against the video and the checkout
before acting on it, say where it is wrong, and look for what it missed.

1. **Audit.** Watch the built edition through its stills (the beats the feedback names, and the rest) and map each
   point to beats (feedback timestamps are approximate: use the edition's `chapters.txt` and the timeline's captions).
   Run the reviews and the fact check above over the chapters the feedback reaches (over the whole story when the
   feedback is about the whole, or the change reaches several chapters).
2. **Plan.** Rank the corrections by what they do for understanding and correctness, in a table like the reviewers'.
   Keep what works (named, so it is not lost in the rewrite); change only what is needed.
3. **Revise** the storyboard, its translation, the demonstrations, and the diagrams, by `references/storyboard.md`. A
   chapter may grow longer when that buys understanding.
4. **Validate and deliver** as in Phase 8 (SKILL.md), with a short change log of what changed and why, and the
   corrections with their evidence.
