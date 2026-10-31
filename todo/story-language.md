# General functional language and Render Lab integration (plan, not started)

Build a general-purpose typed functional language with an independent standard library, compiler, and lightweight
runtime. It must run without Render Lab, a renderer, or story files. The STL draft specifies ordinary computation,
types, and lambda semantics; Render Lab's existing `rlstd` package supplies its document reader, presentation,
narration, timelines, and viewer bindings. The compiler, viewer, recorder, and text checks share one implementation
of the language while selecting the appropriate host libraries.

See the [general language STL](language-stl.md), [package/import specification](language-packages.md), and
[Render Lab library design](rlstd-language.md).

This document plans the work; it does not introduce executable syntax yet. The forms below follow the discussion;
constructor names, annotations, module syntax, and host functions are provisional until Phase 0 specifies them.

## Direction from the discussion

- In the optional `rlstd` prose reader, `$name` interpolates a lexical binding into prose. A `$` followed by a braced code form, such as
  `${function args...}`, evaluates and presents that form. Ordinary prose, including unmarked braces, remains text.
- `{function arg1 arg2 ...}` applies a function with whitespace-separated arguments. Nested calls need no `@`
  marker or additional `$`. Bare identifiers inside code look up lexical bindings without consuming them.
- Under the proposed Lisp-style rule, `{function}` is a zero-argument call. `$name` is lookup; `${name}` is a call
  and does not abbreviate lookup. Confirm this one-element-form rule in Phase 0; it replaces the earlier equivalence.
- `[# name expression]` is a special definition form introducing an immutable lexical binding in the enclosing
  document or function body. It contributes no displayed content and is not an ordinary value-returning expression.
- `[Import package-name]` imports a package's public namespace, accessed as `package-name.content`.
  `[Import* package-name]` also introduces public exports without the prefix. `[ImportAs package-name pn]`
  introduces the alias `pn`, accessed as `pn.content`. These are lexical compiler declarations; conflicting
  same-scope names are errors and imports do not make runtime exports static.
- `{^ {arg1 arg2 ...} implementation}` constructs a lambda.
- `{: expression TypeExpression}` is a proposed static type ascription: check the expression against the given
  statically known type and preserve its value. It replaces the unadopted `[: ...]` / `[:: ...]` annotation idea.
- `[# | FuncType]` may bind `|` as an ordinary alias for a function-type constructor. `{| Return Arg1 Arg2 ...}`
  produces a type describing a function with those argument types and result type; `|` is not a new special form.
- `{Struct ...fields}` constructs a type through an ordinary function call; there is no separate type-construction
  punctuation. `Field`/`Struct` library names and their exact API are provisional.
- `[% expression]` explicitly evaluates its enclosed expression during static preparation. Square brackets mark
  compiler/meta forms; this replaces the earlier curly-brace staging syntax.
- `[Assert first second]` is a compile-time meta form: compare statically evaluated values, return `first` when
  equal, and report a diagnostic otherwise. This supersedes both the Boolean and ordinary runtime Assert proposals.
- `{% a b}` is proposed as numeric modulo/remainder, interpreting the user's "modulating numbers" that way:
  `{% 8 3}` yields `2`. Confirm the intended operation and its negative/division-by-zero rules in Phase 0.
- Functions return typed values without implicit display. `rlstd` presents `String` as text and defines
  `ColoredString`, with `string: String` and `color: Color`, plus `Formula`, `AnimationTrigger`, and other lab types.
- Functional composition, closures, branching, and recursion should permit general-purpose computation. Actual
  compiler and runtime executions have resource limits, as any practical implementation must.
- Some evaluation happens before TTS; other evaluation happens during interaction. Make that distinction explicit
  and enforce it rather than leaving authors to discover it through incorrect recordings.

## Language and library boundary

The independent language and `std` own syntax, lexical bindings, lambdas, types, arithmetic, collections, strings,
modules, static evaluation, bytecode, generic host interfaces, diagnostics, and resource budgets. General source
files are code and return values; they do not implicitly render, speak, or collect document content.

`rlstd` owns all Render Lab-specific script features: the prose/`.story` reader, `$` interpolation, rich text and
formula values, presentation adapters, narration projections and anchors, timelines, scene bindings, and viewer
interaction. Its native adapters live with the existing package code; the lab exposes only generic shared
facilities. Investigation-specific algorithms remain in their own packages. A standalone semantic-test runner
must work without importing any of these libraries or loading package manifests.

Module resolution is a language interface. Render Lab supplies an adapter that maps declared package resources
and enforces package imports/exports; the language itself does not know about `packages/` or `lab::resources`.
The language package specification defines namespace, wildcard, and aliased imports, public interfaces, stable
binding identities, dependency resolution, and initialization. A language package's public symbols and a lab
manifest's exported resource keys are distinct visibility layers.

## Current Render Lab implementation and integration points

| Area | Current behavior | Work required |
|---|---|---|
| `lab/src/rich_text.rs` | Parses `{@ name=value ...}` into fixed text, formula, and code runs; measurement and drawing parse text separately. | Consume prepared typed presentation values; evaluation must not happen during measurement or painting. |
| `tools/video/story.py` | Splits chapters, beats, and paragraph sentences; reads tag-only beat keys and named cues. | Preserve the story structure while delegating embedded language parsing and evaluation to Rust. Multiline forms must survive paragraph splitting. |
| `tools/video/markup.py` | Separates caption text, spoken text, cues, and range markers; special-cases formulas and code spans. | Replace syntax-dependent scanning with compiler output containing typed runs and stable narration anchors. |
| `tools/video/narration.py`, `timeline.py`, `outputs.py` | Synthesize English sentences, obtain word timings, schedule cues, and export captions and documents. | Use the same prepared spoken/presentation results; resolve scheduled effects after speech timing exists. |
| `lab/src/i18n.rs`, `lab/check/src/lib.rs` | `{name}` substitutes localized arguments; brace escapes, formula checks, and text-key checks have separate scanners. | Establish one brace grammar and typed message arguments without losing missing/unknown/unused-key checks. |
| `lab/src/shot.rs`, `animation.rs`, `actions.rs` | A timeline frame is evaluated independently; package actions set state using elapsed time. | Integrate script bindings and typed effects while preserving arbitrary seeking and standalone frame rendering. |
| `lab/src/viewer.rs`, `record.rs` | Live controls/clicks and recorded frames share scene construction. | Run the same VM with explicit input contexts and deterministic recording inputs. |
| `lab/src/values.rs`, `shader_data.rs`, `investigation.rs` | Declare values, choices, actions, and package-specific capabilities. WGSL metadata also uses the attribute-tag parser. | Expose typed host schemas; separate shader metadata parsing from executable document syntax. |

The text checker is a build dependency of the lab. It cannot depend on the lab's runtime library without a dependency
cycle. A shared language core must therefore live in a renderer-independent crate usable by both.

## Proposed language contract

### Expressions, application, and scope

Use one grammar inside expression forms: bare words are identifiers, quoted strings are values, and nested calls
are explicitly delimited. Whitespace, including newlines, separates arguments. In document text, `$identifier`
performs name interpolation and `$` followed by a braced form enters code: `${add AGE 1}` embeds `{add AGE 1}`,
without another pair of braces. Inside code, use `AGE` and `{add AGE 1}` directly. Ordinary braced forms apply their
first expression to their remaining expressions; reserved braced forms such as `{^ ...}` and `{: ...}`, and meta
forms such as `[% ...]`, have their specified special rules. Brace forms are not general grouping parentheses. Strings and evaluated text are
data and are never scanned again as document syntax. Document text and interpolation are an `rlstd` reader profile;
general language source starts in code mode without `$` markers or implicit presentation.

For example, `I am $AGE years old. Next year: ${add AGE 1}.` performs two lookups of the same binding. `$AGEyears`
would read one longer identifier; `${format "{}years" AGE}` explicitly formats the value next to letters. Specify
identifier boundaries, literal-dollar escaping, literal `${`/`[#`, and whitespace/punctuation rules in Phase 0.
Under strict application syntax, `${AGE}` attempts a zero-argument call and fails if `AGE` is a number; `$AGE`
reads the number. This supersedes the former braced-lookup shorthand. Plain braces in prose do not enter code.
`$` selects interpolation; `[% ...]` still selects the evaluation stage.

Application evaluates its target and arguments and applies the function, including a zero-argument function.
Applying a non-function is a type error. Passing a function as an argument preserves the function value; it does
not implicitly invoke it. `$name` in prose and bare identifiers inside expressions refer to the same lexical
bindings. `[% ...]` performs static evaluation and is not a lookup or type-construction form. `%` is an ordinary
numeric operator in application position; it does not affect evaluation stages. For example, `{% 8 3}` computes
remainder at runtime, and `[% {% 8 3}]` prepares the same numeric result statically.

Square brackets identify compiler/meta forms: `[# ...]` introduces a definition, `[% ...]` computes an expression
statically, and `[Assert ... ...]` checks static equality. `[Import ...]`, `[Import* ...]`, and `[ImportAs ... ...]`
introduce package namespaces/bindings. A meta form can elaborate to a value even though it is not an ordinary runtime call.
`{...}` forms are code expressions and `${...}` embeds them into prose. The `rlstd` reader recognizes `[#`, `[%`,
`[Assert`, `[Import`, `[Import*`, and `[ImportAs` followed by their specified token boundaries as meta-form openers;
other square brackets remain prose. Specify their literal escapes in Phase 0.
Every expression has a type and evaluates to a value.
A definition is special syntax recognized in a definition context: it establishes a named binding for the enclosing body and has
no expression-result obligation. Its initializer is an ordinary typed expression. Definitions contribute no text
or speech; they are not values inserted into the content list. Reject a definition in an ordinary argument position.
This replaces the earlier `{# ...}` notation and its expression-returning-`Unit` rule. `Unit` remains an ordinary
type with one value; `rlstd` gives it empty presentation. It is not needed to explain definitions.

A document body is a definition context containing prose, definitions, and inline expressions. A definition is
available to subsequent uses throughout its enclosing document scope, including later paragraphs and expressions;
it does not apply only to the immediately following expression. Content-producing forms collect presentation
values in source order. Keep this content collection distinct from the environment used to resolve names.

A lambda has lexical scope and captures outer bindings. Its body may begin with local definitions and import
declarations followed by one result expression. Imports resolve during compilation even inside a lambda; they
introduce lexical names rather than runtime operations. The compiler elaborates definitions into core scoped bindings around the result.
This is a defined body grammar, not implicit sequencing of arbitrary expression results. Simple non-recursive
bindings can elaborate to nested `let` expressions or lambda applications; recursion and forward references need
their own specified rules. Local names do not become visible in callers, and a caller's same-named parameter
does not change a captured binding. Global means the enclosing story/module scope, not process-wide mutable state.

Illustrative examples:

```text
[# greeting "Hello"]
$greeting

[# twice
    {^ {f x}
        {f {f x}}
    }
]
[# increment {^ {x} {add x 1}}]
${twice increment 5}

[# square
    {^ {x}
        [# result {multiply x x}]
        result
    }
]
```

The `greeting` definition contributes no content; the following lookup yields `"Hello"`. In `square`, `result` is
a local binding and the final lookup supplies the function's numeric return value. Constructing a lambda yields
a function value and constructing a structure type yields `Type`; definitions may name either value.

For example, `[# AGE 21]` before a paragraph makes `AGE` available to both `$AGE` and a later `${>= AGE 21}`
in that paragraph. A document can elaborate schematically to bindings surrounding a collected content expression:

```text
let AGE = 21 in
    content ["I am ", AGE, if AGE >= 21 then "!" else "."]
```

This follows the separation of definitions and content in [Scribble's document language](https://docs.racket-lang.org/scribble/doclang.html).
The distinction between declarations and core terms also appears in [Lean's elaboration model](https://lean-lang.org/doc/reference/latest/Elaboration-and-Compilation/).
Use these as design references; copying either language's complete scope or staging rules is not required.

Specify literal brace escaping, quotes/backslashes, raw strings for LaTeX, comments, field access, collection
literals, type annotations, and conditional evaluation before writing a parser. A conditional must evaluate only its
selected branch so recursion can terminate. Interpolation and nested code use the same expression evaluator.
Keep existing story headings and sentence boundaries recognizable.

### Local scopes from existing forms

Invoke a lambda to introduce a local scope. Its body can contain local definitions and one result expression:

```text
${ {^ {}
    [# local 3]
    {multiply local local}
}}
```

This returns `9` without exposing `local` outside the invocation. For a simple binding, elaboration can use the
equivalent parameter-based expression:

```text
${ {^ {local}
    {multiply local local}
} 3}
```

The lambda is the call's target expression. Its argument `3` binds the parameter `local` in a fresh lexical scope;
the body expression returns `9`. Inner bindings may shadow outer names without changing those outer bindings.
Function lookup follows lexical nesting, not the caller's local names. Several parameters can bind independent
local values; nested applications express bindings that depend on earlier ones.

Returning a value does not export the local binding that named it. A returned lambda can retain captured local
values after the enclosing call returns, but those local names do not become visible in the surrounding document.
Preserve captures for as long as the returned closure needs them.

Combine scope with staging by wrapping the invocation:

```text
[% { {^ {local}
    {format "{}" {multiply local local}}
} 3}]
```

This computes the string `"9"` before TTS and keeps `local` private. The lambda invocation introduces the scope;
`[% ...]` chooses the evaluation stage and remains scope-transparent. A body with local definitions elaborates to core
bindings before evaluation; it can be used within the lambda in this example as well.

If ordered effect sequencing is needed, specify it explicitly and check its effects; it is separate from lexical
definitions. The suggested suffix-content `{* ...}` syntax is not adopted: `*` is ordinary multiplication.
Any such document transformation belongs in a named `rlstd` function, separately from lexical name binding.

### Types as static values (proposal from the side discussion)

The user raised making types values. Construct types using ordinary static functions, and give the result a name
with a definition. Use illustrative library functions `Field` and `Struct` with the current application syntax:

```text
[# CounterRecord
    [% {Struct
        {Field "count" Int}
        {Field "label" String}
    }]
]
[# CounterAlias [% CounterRecord]]
```

`Field` constructs a field descriptor from a name string and a type value. It does not introduce a lexical binding.
`Struct` constructs a type from field descriptors; `[% ...]` evaluates that construction statically. These function names
and their API are provisional library operations, not implemented built-ins. `[% ...]` does not determine the result's
type: `[% 3]` has type `Int`, while `[% {Struct ...fields}]` has type `Type`.

The older `{@ Struct {$ count Int} ...}` example is superseded: calls no longer use `@`, and `$` now enters code
from prose. The earlier dedicated type/field syntax is also superseded by ordinary type-producing functions. A field
descriptor and a complete structure type remain different values; static evaluation may compute either.

An author-defined lambda may accept type parameters and return a type, using these same functions. That lambda
is a type constructor; the resulting `Type` value and the function that constructs instances remain distinct.

Type values have a compiler-recognized type, `Type`. Static functions may accept and return them, so aliases and
type constructors such as `{List String}` can use ordinary functional composition. The STL draft chooses structural
identity for `Struct`: identical field maps yield identical types, independent of construction site or field order.
`Nominal` adds distinct identity through an explicit qualified tag and representation type. This supersedes the
earlier tentative fresh identity per structure declaration. Ratify the choice and stable normalization in Phase 0.

Every type used in an annotation or checked signature must resolve during static checking. Live scene state cannot
select a different field layout or change a function's checked argument/return types. Keep initial genericity to
static type parameters; full dependent typing is outside the initial implementation. Runtime reflection, if useful,
is a separate later capability and does not allow runtime changes to checked signatures.

Creating a type and constructing an instance are separate operations. The STL draft supplies `Record`, `Member`,
`wrap`, and other explicit constructors; applying a Type value does not implicitly construct an instance. A type
value has no implicit text presentation; displaying a type name requires explicit conversion. Having first-class
types does not bypass `rlstd`'s presentation contract below.

This is a recommendation to evaluate in Phase 0, not an approved final specification.

### Applying types to existing expressions

Keep a typing environment mapping lexical names to their types, separate from the evaluation environment mapping
names to values. Infer/check expressions before VM execution, including expressions that will run dynamically.
Static evaluation computes type values when needed; type checking then uses those values to check ordinary code.
Returning `Type` from a computation does not by itself implement the checking rules.

For illustration, assume `add` is integer addition, with mathematical signature `Int -> Int -> Int`. These
signatures explain the checker; their notation is not a decision about source annotation syntax.

| Existing form | Typing rule |
|---|---|
| `21`, `"hello"` | Literal types are `Int`, `String`, etc., according to the numeric-literal policy. |
| `AGE` | Look up the type assigned to its lexical binding. Unknown names are errors. |
| `{add AGE 1}` | Check both arguments against `add`'s parameter types; the call has its return type, here `Int`. |
| `{^ {n} {add n 1}}` | Infer `n : Int` from the known operation; the lambda has type `Int -> Int`. Parameter types may also come from an expected full function signature. |
| `[# AGE 21]` | Check the initializer, then introduce `AGE : Int` in the enclosing definition context. The definition has no expression result. |
| `[% expression]` | Preserve the expression's type while requiring its evaluation inputs to be static. |
| `{% a b}` | Check the numeric arguments and produce the numeric result under the chosen remainder signature. |
| `{Struct ...fields}` | Check field descriptors and produce a value whose type is `Type`. Resolve that value statically when used as a checked program type. |
| `{: expression TypeExpression}` | Resolve the type statically and check the expression against it. The result has that type and the expression's unchanged value; this is not a conversion or a forced static evaluation of the value. |
| `{FuncType Return Arg1 Arg2 ...}` | Construct a function-type value with explicit result/argument types and argument count. The constructor's inputs must be static when used for checking. |
| `$name`, `${...}` | Check the expression normally, then require a supported presentation for its result. Narration separately requires static evaluation. |

Reject `{add "hello" 1}` by type checking, not by running it to see whether it fails. Calls to functions with
inferred/declared signatures use the same rule, whether their bodies are native or script lambdas. An unconstrained
parameter cannot always have its intended type inferred; use expected signatures and settle static generic
parameter declarations in Phase 0 rather than claiming every lambda can be inferred from its body alone.

Use one ascription form for both values and full function signatures:

```text
{: {add 2 3} Int}

[# | FuncType]
[# addChecked
    {: {^ {a b} {+ a b}}
       [% {| Int Int Int}]}
]
```

`FuncType` is an illustrative ordinary type-producing function, with return type first and argument types after it.
The full expected signature checks the lambda's parameters and body without separate parameter/return annotation
forms. In this example `addChecked` has type `Int -> Int -> Int`, and the body must produce `Int`. `+` is assumed to
be the integer operation here. The lambda is not invoked by the annotation. `|` simply names the constructor; type
construction and the check remain different operations. Type expressions used by ascription must be statically
known; `[% ...]` explicitly stages computed signatures under the current staging policy.

`[Assert a b]` compares two static values and returns the first, rather than consuming a Boolean condition. The
earlier `{Assert {IsType val T}}` expansion is superseded. A compiler may compare inferred and expected type values
using its type-equality relation; that comparison concerns types, not the value of `val`. The `:` form preserves
the original expression after checking it. Static checking works over expression syntax and type information,
without evaluating a live expression to inspect it.

The computational behavior of successful ascription is identity. After checking, it can elaborate to
`{{^ {x} x} val}` or erase to the checked expression. The static checker supplies the guarantee; the identity
body alone does not implement type checking. Define evaluation order so the value is evaluated once at its chosen
stage. Never infer its type by running a function on sample values. Whether to expose a library-level identity
alongside the `:` syntax is optional.

Square brackets identify compiler/meta forms in the chosen notation, including definitions and static evaluation.
A delimiter choice does not determine whether a construct can elaborate to lambda-based core terms; the semantic
rules do. No `[: ...]` or `[:: ...]` forms were adopted. Direct typed-parameter syntax, if later useful beyond full
expected signatures, remains open.

### Compile-time equality assertion and value semantics

The current spelling is `[Assert a b]`, a compiler/meta form with no ordinary runtime `Assert` binding:

```text
[Assert {+ 1 2} 3]
```

First check both operand expressions for the same equality-supporting type and require both to be statically
available. Then evaluate each once, left to right, and compare the resulting values:

```text
compileAssert(aSyntax,bSyntax) = constant(a)    if Equal(a,b)
                              = diagnostic     otherwise
```

Successful assertion preserves the first argument's value and type; it is not a Boolean test or a conversion.
`[Assert false false]` returns `false`. Unequal operands, unsupported equality, illegal stage dependencies, and
evaluation failures stop compilation with source diagnostics. The STL draft defines the compiler algorithm and
its lambda equality helper; that helper is not exported as a runtime operation. No implicit numeric coercions occur.

The language has immutable value semantics without user-visible borrowing, mutable references, pointer identity,
or ownership operations. Return the first equal operand as requested. Internal sharing, reference counting, and
closure capture may implement values efficiently but must not expose memory identity. These are choices for this
language; functional programming in general does not prohibit every form of reference or effect.

Define `Equal` for each supported type before relying on "same": integers/strings, lists and records, and static
type values. Type equality must respect aliases, normalization, and the chosen identity of generated types. Do
not use object addresses or printed representations as language equality. Initially reject assertions on types
without a defined equality operation, including arbitrary functions. Decide floating-point equality, signed zero,
and NaN behavior explicitly: the equality relation must match the language's permitted observations for the
claim that returning either equal operand is indistinguishable to hold.

`[Assert liveValue expected]` is rejected when an operand depends on runtime input. The meta form itself requests
static evaluation, so an extra `[% ...]` is unnecessary. A comparison of compiler-inferred types can happen without
running the corresponding value expression. Assertions return constants without introducing bindings. As with
other expressions, `rlstd` decides whether a document-position result contributes presentation.

### Types and rlstd presentation

The general language starts with `Unit`, `Bool`, `Int`, `Float`, `String`, lists, structures, unions, and function
types. `Color`, rich text, formulas, vectors used by the lab, and animation descriptions belong to `rlstd` or
other libraries. The STL draft specifies numeric widths, overflow, conversions, non-finite values, and equality.
Provide type inference for simple expressions and checked annotations for function arguments and returns. Reject
unknown names, wrong argument counts, invalid field access, and incompatible return types before execution.

Use an explicit presentation contract for values inserted into a document:

| Type | Presentation |
|---|---|
| `String` | Text in the inherited style; spoken and exported as text. |
| `ColoredString` | Its `string` in its `color`; spoken/exported as the string without color metadata. |
| `Formula` | LaTeX, display style, term colors, and an explicit spoken alternative; retain existing formula limits. |
| `CodeSpan` | Code styling and a defined spoken alternative. |
| `Content` | An ordered composition of presentation values; evaluated strings cannot inject new executable forms. |
| `AnimationTrigger` | An effect description consumed by the scheduler; no visible text or speech by default. |
| `Unit` | No output. |

An arbitrary structure is not displayable merely because it happens to have fields called `string` and `color`.
Specify how an author-defined type implements presentation by returning the standard content types. Keep custom
presentation pure and subject to the same stage checks. Renderers need no investigation-specific type cases.

`rlstd` supplies the canonical `ColoredString` type using the user's two fields; authors can construct
other types using `Struct` or their own type-producing functions. Freeze the type/field/instance constructor APIs
in Phase 0. Decide whether raw
numbers require formatting before document insertion and how multiple returned content/effect values are composed.

### Execution stages and effects

A return type says what a value is. A stage/effect requirement says which inputs evaluation needs and what it may
do. Neither replaces the other. Track requirements through ordinary and higher-order calls, including closure
bodies. Static evaluation is a general compiler service; the following preparation contexts belong to `rlstd`.

| Context | Inputs | Outputs |
|---|---|---|
| Static preparation, before TTS | Source, declared constants, locale, explicit package data | Spoken sentences, static content, declarations, unresolved cue descriptions, compiled runtime functions |
| Timeline construction, after TTS | Prepared output, stable speech anchors, measured word timestamps | Timed cues, animation intervals, runtime-binding lifetimes |
| Runtime | Compiled program, explicit frame time and shot inputs, viewer events | Prepared display values and validated scene-update/effect commands |

Use `[% expression]` as the explicit static-evaluation form, replacing the earlier curly-brace proposals. It
contains one expression, evaluates it during compilation, and yields that expression's value and type. In the
`rlstd` profile this precedes TTS. Use it around a
definition's initializer when that value must be prepared statically; a definition itself is not an expression to
wrap. A lambda expression inside `[% ...]` may still contain local definitions in its body. Nested static wrappers do
not create additional execution stages or duplicate work. In expression position, the result is embedded as a
typed constant in the residual runtime program. In `rlstd` document position, the static result contributes content
under that profile's presentation rules, so `The sum is [% {+ 2 3}].` needs no additional `$` before the meta form. Definitions
still contribute no content. Document that placement rule alongside interpolation rather than leaving it implicit.

```text
[# add [% {^ {x1 x2} {+ x1 x2}}]]

The sum is [% {format "{}" {add 2 3}}].
```

The first form creates and binds `add` during preparation; it does not execute the lambda body. The second invokes
it with static arguments, then formats the result into speech/display text. Unmarked calls within that static
evaluation inherit its context, so each nested call does not need its own `[% ...]`. Keep parameter lists whitespace
separated in these examples; whether to accept commas remains a grammar decision.

Require authors to explicitly stage computed narration inputs. Plain prose and literals need no wrapper. Outside
a static context, executable forms use the runtime context when invoked; eligibility inference or an optimizer's
constant folding must not silently turn a runtime expression into narrated static content. The earlier unwrapped
examples describe definition/expression semantics; static initializers and computed narration need this wrapper.

The wrapper selects an evaluation stage, not a new lexical scope. A definition with a static initializer introduces
a binding in its enclosing definition context, available to subsequent expressions and runtime code. A `[% ...]` inside
a runtime function may refer only to inputs available during preparation, never to that invocation's runtime
arguments. Reject illegal captures with a diagnostic showing the dependency. Freeze these scope rules in Phase 0.

Static construction of a lambda does not make it a static-only function. Its unmarked body executes in the context
of a later call. A pure function with suitable captures may therefore be reused by an explicit static invocation
and by runtime invocations with live arguments. Infer the body's capabilities and check them at each call; creating
a closure is different from executing its body. An explicit `[% ...]` within the body remains a static expression, not
an instruction to evaluate runtime arguments early.

Static code cannot read scene state, pointer input, runtime time, or measured speech timestamps. Runtime code can use
static values. Constructing a closure whose body reads scene state is allowed during static preparation; invoking
that closure requires runtime inputs. Infer the body's requirements separately from constructing its value.
An explicit static wrapper checks a requirement; it cannot make a runtime dependency static. It may construct an
animation description for later scheduling, but it cannot execute a scene update during preparation.

For example, a constant greeting can feed TTS. A function formatting the current scene radius can feed a live label,
but invoking it in narration must fail with the source location and dependency chain. Merely returning `String`
does not make a function eligible for TTS. Check both branches of runtime-dependent control flow for staging errors.

Pure functions compute values. Constructors such as an animation constructor produce descriptions without executing
them. Only the scheduler or an explicit event handler may apply effects. Reading or laying out a display value must
never mutate the scene, restart an animation, or register another handler.

### rlstd narration, localization, and timing

Prepare the exact spoken text before synthesis, including static expression results and formula alternatives.
Assign stable sentence and cue-anchor identities before TTS. Resolve cue positions from the words actually spoken,
not character offsets into source or rendered captions. Specify sentence-start/end anchors, including an effect
after the last word, and errors for anchors that disappear after an expansion.

Preserve the current edition policy: the zh-CN edition uses English narration/timing and translated on-screen text.
Translated content retains one sentence per English sentence. Translation must not duplicate executable control
logic. Allow checked pure presentation expressions with the same declared arguments, and preserve shared cue IDs
independently of translated word order. Runtime text is allowed only in explicitly live presentation contexts; it
must not silently become recorded subtitles or synthesized speech. Define its recorded display from explicit inputs.

Align with `todo/globalization.md`: message lookup, number formatting, and locale selection are host/library
operations; the scripting language does not replace the planned localization backend. Share parsing of placeholders
and expressions, and define which operations a localized message may use. Loading a translation never executes
arbitrary scene updates.

### General runtime and rlstd deterministic seeking

Use a small Rust bytecode VM. Parse, resolve names, type-check, and compile at load/preparation time. Resolve host
names to typed handles before frame execution. Reuse compiled code and immutable constants across frames. Run only
active bindings and handlers; begin with per-frame evaluation of active bindings and add dependency-based caching
only if measurements justify it.

Give every evaluation explicit inputs through a generic host/capability interface. `rlstd` supplies frame time,
elapsed cue time, scene values, locale, and event payloads. Other hosts may supply IO, clocks, or randomness through
explicit registered capabilities; the language has no implicit ambient access. Specific general IO libraries and
effect sequencing need their own design. Investigation logic stays in its package and reaches the lab through
generic extension points.

Preserve `Timeline::state_at(frame)` behavior. Time-based animation state is computed directly from the requested
frame and interval; crossing a cue during playback must not be the only way to reconstruct its result. Pure
bindings can evaluate in any frame order. Interactive handlers operate on explicit viewer state; specify reset,
pause, seek, demo-switch, and conflicting-write precedence. Begin with rejecting ambiguous multiple writers, and
define how script output composes with existing dock, timeline, and package-action updates.

Persistent simulations are a later extension requiring replay/checkpoints and an explicit event log for recording.
Do not implement them as hidden mutable state in a binding. Live interaction and recording match when given the
same explicit inputs; an unrecorded click cannot be reconstructed from a frame number alone.

Bound instructions, recursion/call depth, collection/string allocations, and total evaluation memory. Use managed
closure lifetimes and define ownership of captured values; recursive closures must not leak across reloads. Static
evaluation needs budgets too: a type-correct static program can still fail to terminate. Report a budget error with
source spans and call stack. Specify a consistent failed-frame/binding policy for both viewer and recorder.

## Implementation architecture

1. **Independent language core and std.** A standalone Rust crate with no GPU, window, package-loader, `rlstd`,
   or lab dependency: lexer, source spans, AST, name/type/stage checking, evaluator, bytecode, VM, portable values,
   and the pure general STL. Host operations and module resolution use interfaces supplied by callers. Prefer
   existing dependencies and keep the locked offline build; this plan does not require changing `vendor/`.
2. **One compiler interface.** Supply a standalone compiler/evaluator and package/module resolver interface;
   implement the specified `Import`, `Import*`, and `ImportAs` forms. Lab CLI commands
   configure it with `rlstd`. Python uses structured request/response files or JSON streams for preparation and
   timing resolution; no per-frame subprocesses or second implementation of language semantics. Build-time
   checking imports the independent core directly and never creates a GPU/window or loads a scene.
3. **Program and rlstd document artifacts.** Core artifacts store code, constants, source maps, normalized types,
   and dependencies. `rlstd` artifacts add presentation runs, spoken text, anchors, and timing. Bind runtime host
   handles on load; never serialize pointers. Cache by source/imports, language version, host schema, and relevant
   static inputs; locale is an `rlstd` input rather than a universal language property.
4. **rlstd host adapters.** Package-owned bindings provide lab values, time, actions, scheduling, and interaction
   through the generic interfaces. Rendering consumes prepared runs; Python consumes spoken text, timing anchors,
   and export projections. Native helpers use rlstd's existing `lab::utility` arrangement; no new cdylib is assumed.
5. **Render Lab packages and loading.** The lab's resolver adapter enforces package imports/exports. Render Lab
   source and generated artifacts belong in their owning package, with generated output
   under its ignored `build/`. Compiled programs/content load through declared package resources when needed, not
   ad hoc source paths at viewer startup. Extend the resource pipeline only where a generic processor is useful.
   Versioned bytecode and host-schema checks are separate from the existing dynamic-library negotiation.
6. **Build identity.** Include the independent language implementation and imported modules in the lab's build
   fingerprint and compiled-artifact invalidation; hashing only `lab/src` would miss language changes. Standalone
   language cache keys must not require a renderer fingerprint.

## Implementation phases

### 0. Specify the language and record baselines

- Write the normative grammar and examples for the proposed forms, escaping, scopes, constructors, annotations,
  conditional evaluation, recursion, the `[% ...]` wrapper and its inherited evaluation context, stage requirements,
  presentation, and effects. Specify definition contexts separately from expressions and content collection.
- Choose the remaining policies listed below and an explicit language-version marker. Do not guess a grammar from
  whether old or new parsing happens to succeed.
- Inventory current tags in stories, text tables, and WGSL metadata, including cue ranges and named cues. Record
  prepared narration/caption/timing baselines and representative offscreen stills in both editions.
- Exit: examples have one unambiguous parse and evaluation; every old construct has a documented replacement or
  an explicitly separate metadata grammar.

### 1. Implement the standalone language and std

- Implement source-preserving parsing, lexical scope, immutable definitions, structures, lambdas, closures,
  application, conditionals, lists, and general recursion. Elaborate body definitions to scoped core terms and
  resolve identifiers lexically. Check types and static/runtime requirements. If Phase 0
  adopts first-class types, implement `Type`, static type-returning functions, aliases, and stable type identities.
- Compile to a small instruction set and run with deterministic inputs and resource budgets. Support a minimal
  host registry so test hosts can validate dynamic dependencies without linking the renderer.
- Implement the STL catalog, explicit primitive substrate, variadic argument packing, module environments, and
  a standalone code runner. Use the catalog's actual `[# name {^ {...} ...}]` definitions as implementation fixtures;
  load dependencies before users rather than treating topic order as initialization order. Test independently
  without importing `rlstd` or the lab.
- Implement the package specification: public export tables, qualified lookup, wildcard and alias imports,
  source-order lexical visibility, collision diagnostics, canonical dependency identities, and initialization once
  per execution context. Preserve imported type/function identity and capabilities. Reject cycles and incompatible
  versions/artifacts. Import resolution uses supplied dependencies without automatic installation or downloads.
- Exit: constants, higher-order functions, recursion, structure fields, static rejection, `[Assert ... ...]`, and
  test-host runtime calls work through the same core. Invalid syntax/types/stages produce precise diagnostics;
  infinite recursion stops within its budget. No dependency cycle with `lab/check`.

### 2. Add rlstd modules, typed rich text, and build checks

- Register the package's prose reader, types, and native adapters as specified in the rlstd design. Supply
  presentation conversions to the existing formula/code/text layout. Add colored text runs;
  evaluate once into a reusable result shared by measurement, wrapping, painting, and export.
- Route text-table validation through the core with declared message arguments. Preserve text-key and language
  parity checks; check static formula content and defer runtime-only checks to the appropriate load context.
- Decouple WGSL annotation parsing from rich text. Metadata remains data: sharing a lexer does not make shader
  comments executable scripts.
- Exit: static string/color/formula examples render correctly offscreen, repeated layout causes no effects, and
  existing translated text and shader declarations still validate.

### 3. Integrate rlstd static stories and TTS

- Have Python request the prepared document from Rust. Protect multiline forms and quoted content when forming
  narration sentences. Feed resolved English text to TTS and resolve stable anchors afterward.
- Use prepared projections for caption wrapping, subtitles, documents, and translated editions. Keep formula
  speech alternatives and existing pronunciation rules. Include resolved speech inputs in narration cache keys.
- Exit: one converted public beat uses a constant, lambda, `ColoredString`, and `Formula`; speech, captions, and
  word-timed effects agree in both editions without Python re-evaluating script source.

### 4. Integrate rlstd live bindings, events, and scheduling

- Embed the VM in viewer and recorder. Bind the existing values/choices/actions schemas to host functions.
- Implement explicit binding/handler registration, animation descriptions, lifetimes, update ordering, seeking,
  reset behavior, and failure handling. Validate scene writes against existing types and ranges.
- Exit: a live scene-value label updates while interacting; a word-timed animation has the same state on direct
  seek, forward/backward scrub, playback, and standalone recorded frames. No script execution during drawing.

### 5. Migrate existing syntax and remove duplicate paths

- Convert maintained public stories and text tables, their documentation, and authoring skills to the versioned
  language. Migrate named cues, ranges, formulas, symbols, images, labels, and animation/action calls deliberately.
- Inventory external/older input behavior and either migrate through an explicit versioned adapter or reject it
  with actionable diagnostics. Any converter is temporary tooling with a removal criterion, not a permanent second
  evaluator. Do not revive the previously removed storyboard writers or old scene-type aliases.
- Remove obsolete expression scanners and semantic paths in Rust/Python once consumers have moved. Keep only
  separate metadata grammars that still serve actual data formats.
- Exit: the shared implementation owns embedded language semantics; maintained content and exports no longer
  rely on the old attribute-tag syntax, and documentation describes the actual supported language.

### 6. Measure, validate, and finish the specification

- Benchmark static compilation, VM allocations, and per-frame time separately from scene rendering. Use small
  bindings and a representative composition of text, formulas, animations, and interaction. Set an agreed runtime
  budget from those measurements; do not promise a frame-rate target before a baseline exists.
- Cache immutable content/layout where useful. Measure repeated formula changes separately so typesetting does
  not hide VM costs. Keep dependency tracking, bytecode optimizations, and persistent simulation out of the first
  implementation unless evidence requires them.
- Exit: the validation below passes, performance is recorded, the language reference is complete, and the example
  story demonstrates author-defined logic in both static and interactive contexts.

## Validation and completion criteria

- Run standalone language/STL tests without `rlstd`, package loading, GPU, or TTS. Unit tests cover
  nesting/escaping/raw strings, `[# ...]` / `[% ...]` / `[Assert ... ...]` meta delimiters versus `{...}` expressions,
  declarations/scopes, closure capture, higher-order stage checking, recursive termination/budget failures,
  type errors, and source diagnostics. Cover the STL's arithmetic, collection, control, and constructor cases.
  Check numeric `%` independently of staging, including the chosen sign and division-by-zero behavior.
  Check that definitions bind their initializer, have no ordinary expression result, are rejected as ordinary
  arguments, and are visible across subsequent uses in the enclosing body. Function bodies support local definitions plus
  a result expression and elaborate to scoped bindings; arbitrary expression sequencing needs an explicit rule.
  Check lookup versus application, local definitions, parameter-based bindings, lexical shadowing without outer
  mutation, and escaped closures retaining captured values. A caller's shadowing must not change a captured name.
  Verify the same scope behavior inside `[% ...]`.
  Check wrapper result types, static-binding visibility, nested static contexts, deferred lambda bodies, reuse of
  pure functions across stages, and rejection of runtime captures during static evaluation.
  Check inferred literal/name/call/lambda types, `:` ascription, unknown names, wrong argument/return types, and
  static type parameters. Ascription must preserve the value and its evaluation stage without evaluating it twice.
  Check full function signatures, contextual parameter checking, return-first `FuncType`, and ordinary `|` aliasing.
  A static wrapper preserves type rather than converting it.
  Check binary `[Assert a b]`, first-argument preservation, equal false values, unequal operands, unsupported
  equality, type equality, chosen floating-point rules, and exactly-once static evaluation. Runtime-dependent
  operands fail stage checking; there is no ordinary runtime Assert binding. First-class type tests cover static
  constructors, structural identity, explicit nominal tags, alias identity, stable specialization identity, and
  rejection of runtime-dependent annotation types.
- Package tests use in-memory resolvers and cover `Import`, `Import*`, `ImportAs`, namespace/operator lookup,
  submodule exports, private/unknown exports, alias identity, wildcard conflicts, repeated imports, lexical scopes,
  closure capture, imported stages/types, diamond dependencies, initialization once, cycles, and version/artifact
  errors. They run without Render Lab or network access. An import inside a lambda is still resolved at compile time.
- `rlstd` reader tests cover `$name` and `${...}` interpolation, identifier boundaries, punctuation/literal escapes,
  nested forms, document-position meta forms, and presentation/effect composition. Unmarked prose braces and
  evaluated strings never execute code. Definitions contribute no text/speech and remain visible across following
  paragraphs; collected content stays independent of name resolution. Repeated layout never schedules an effect.
- `rlstd` integration tests prove runtime data cannot enter TTS, a runtime closure can be defined statically, and a
  synthesized sentence's actual words determine its cue positions. Captions/exports use the same evaluated values.
- Repeated compilation with the same inputs produces the same prepared artifacts. Changing a source import,
  locale, host schema, or compiler version invalidates the appropriate cache. Runtime artifacts reject mismatches.
- Direct seek, out-of-order frame evaluation, pause/resume, repeated measurement/drawing, reset, and scripted
  clicks verify deterministic state and effect lifetimes. Viewer and recorder agree for identical explicit inputs.
- English and zh-CN preserve sentence correspondence, narration reuse, localized captions, formula limits, and
  text-key checks. Dynamic text does not silently change the recorded speech track.
- Run `cargo build --release --locked --offline` and `cargo test --release --locked --offline --workspace`, with
  `CARGO_HOME=$PWD/.cache/cargo`, plus the affected Python video tests. Build each local workspace after the lab,
  and verify the public-only tree builds without warnings. Use existing offscreen homepage/viewer, formula, and
  story snapshot checks for affected presentation paths; run one GPU job at a time.
- Review relevant offscreen stills and compare narration, captions, cue times, and scene states against the Phase 0
  baselines. Only documented language/presentation changes may differ. No user-screen windows or screenshots.

## Decisions to settle in Phase 0

1. Exact structure/type-annotation syntax, field access, collection literals, comments, and raw-string/delimiter
   escapes. Define literal `$`, `${`, `[#`, and `[%` in prose and the boundary of `$name`. Definitions use `[# ...]`;
   static evaluation uses `[% ...]`; `$name` and `${...}` enter runtime expressions from prose. Specify meta-form
   placement and document-result presentation; nested ordinary code forms use `{...}`.
2. Ratify the STL's proposed code grammar and its use by the `rlstd` reader. Ordinary braces apply the first expression to the remaining
   arguments, bare identifiers perform lookup, and `[% ...]` selects static evaluation. Confirm `{f}`/`${f}` as zero-argument
   application, distinct from `$f` lookup; there is no implicit grouping or `@` call marker. Ratify the specified
   thunk elaboration for `{if condition thenExpr elseExpr}` despite ordinary eager application.
   Ratify `{: expression TypeExpression}` as static ascription and the return-first `FuncType` API, including
   its variadic type-constructor interface and zero-argument function representation. `|` is an ordinary alias.
   Ratify the newly explicit `{K.variadic prefixCount minimumTailCount implementation}` bridge and the
   arity-preserving `K.fix` primitive. Fixed-arity lambdas alone do not derive either capability; homogeneous tails
   use Lists, heterogeneous tails use checked internal argument vectors and `K.mapArgs`.
3. Finalize the proposed `[% expression]` wrapper, inherited evaluation contexts, explicit staging of narration,
   scope/capture rules, higher-order effect inference, and any explicit timing-context operations.
4. Whether custom presentation uses a language protocol, a conversion function, or both; how composition preserves
   order between visible content and scheduled effects.
5. Exact document/chapter/beat definition contexts, self/mutual recursion and forward-reference rules, and module
   versioning. Apply the package specification's source-order import visibility and conflict rules. Definitions are special forms;
   the former `Unit`-result and immediately-next-expression rules are superseded.
6. Syntax and host API for live bindings and click handlers, write-conflict precedence, failure policy, and what
   interactive state a recording must capture. Persistent simulation remains a later extension.
7. The versioning/migration boundary for existing stories, text-table placeholders, and built timeline artifacts.
8. Standalone CLI entry/result conventions, module source extension, package-author export/re-export declarations,
   package descriptors, version constraints/lockfiles, resolver configuration, host registration, and artifact/version
   rules. Import spellings are specified in the package design. Lab commands are adapters around the independent compiler.
9. Ratify the STL draft's first-class static types, `Field`/`Struct`/`Record`/`Member` APIs, structural identity,
   explicit `Nominal` tags, function-type construction, and generic type parameters. Type values are not implicitly
   callable; no separate special field-clause grammar remains.
10. Ratify the STL's Euclidean `%`/`div` definitions, including Int operands, nonnegative remainder, overflow,
    and division-by-zero errors. These use ordinary application and are independent of static evaluation.
11. Equality rules and supported types for compile-time `[Assert a b]`, including type identity/normalization and
    numeric edge cases. Ratify the STL's finite Float/canonical zero policy and numeric limits. Preserve immutable
    value semantics without observable memory identity; no general function-equality behavior is implied.
12. Specify general effect sequencing and IO libraries separately from `rlstd`. Keep domain-specific types,
    reader rules, narration, timeline seeking, scene schemas, and viewer behavior in the package.
