# Phase 1: Research

Load this for a new story's research, or when a revision needs claims researched again. The precision rules every
brief carries are `new-investigation`'s (its SKILL.md, "Precision"); this file is how the research is organised.

## Who does it

With subagents: send the briefs below, the subtopic agents and the two cross-cutting ones in parallel when your
environment runs agents in parallel, one after another otherwise. A finished agent's context is the cheapest place to
ask a follow-up: send it there when your environment can message a finished agent; otherwise start a new agent with the
original brief, the agent's report, and the questions.

Without subagents: do each brief yourself, in the same order, one scope at a time. Write each scope's findings into
its own section of `story/research.md` before starting the next, so later scopes, the questions round, and the merge
read the notes rather than relying on memory.

Either way, a finding is a claim with its file, symbol, and line; verify a sample of each report against the checkout
yourself before merging it.

## Steps

1. **Overview.** Survey the feature in the checkout. Report the subtopics (four to eight), in the order the frame runs
   them, and for each the entry points, files, and symbols to start from.
2. **One brief per subtopic.** Each gives:
   - the checkout's path, and the engine's version and commit;
   - the subtopic's scope, its neighbours (so two scopes don't cover the same ground), and what is already known or
     ruled out;
   - the rule to follow what runs, not what exists: from the entry point the pipeline executes, through its passes
     and shaders, with every compile-time switch, shader option, console variable, and default on the way
     (`new-investigation`, "Precision");
   - what to return: every claim with its file, symbol, and line; numbers as the code has them (formats, sizes,
     counts, limits, clamps); what depends on configuration, and its default; and what is still unclear.
3. **Two cross-cutting briefs**, with the same checkout brief:
   - **Where it is used.** What consumes the feature's output and what the feature reads (the passes, shaders, and
     systems on either side of it), where it is switched on (project settings, quality levels, platforms, editor and
     runtime paths, their defaults), what other engine features depend on it or replace it, and the engine's own
     samples, templates, and documentation that use it. Every claim with its file, symbol, and line.
   - **How it is optimised.** Everything the engine does to make it affordable: what it amortises over frames, caches,
     or reuses (history, temporal accumulation, incremental updates); what it skips or culls; where it lowers
     resolution, precision, or sample counts; its budgets and their console variables and defaults; its scheduling
     (async compute, pass merging, early-outs); its data layouts chosen for bandwidth; and any timings or profiling
     markers in the code or the engine's documentation. What each optimisation costs in quality, with file, symbol,
     and line.

   Their reports become sections of their own in `story/research.md`, and usually chapters: where it runs and what
   uses it (the first chapter), and how it is kept fast (near the end).
4. **Questions.** Read every report. Write down what is unclear, contradicts another report, rests on a function's
   name rather than its body, or depends on a switch nobody checked. Put each question to the scope that owns it, for
   one or two rounds at most.
5. **Merge** the reports into `story/research.md`: one section per subtopic, each claim with its file:line, and a
   column (or note) for **where the lab differs**: what the lab already computes another way (a fit where the engine
   samples a table, a CPU stand-in for a GPU pass, a smaller resolution). Catching a difference here is cheap; finding
   it in a finished storyboard means rewriting sentences.
6. **Gate**: the subtopics, where it is used, how it is optimised, the scope, and what is left out (SKILL.md,
   "Gates").

Done when every subtopic in `story/research.md` has its claims cited, its open questions answered or listed as open,
and its lab differences noted.
