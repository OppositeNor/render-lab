# General-purpose functional language STL: lambda semantics (design draft)

This is the proposed general-purpose language standard library, not an implementation or an existing package.
It accompanies the [language and integration plan](story-language.md). It has no Render Lab dependency: no text
renderer, narration, timeline, scene, GPU, or lab package loader is required to check or run it. Those features are
defined in the separate [`rlstd` language library](rlstd-language.md), owned by `packages/rlstd/`.

This draft fixes a coherent first set of operations, mathematical meanings, and errors. Library names are
case-sensitive. Definitions below use our source notation: `[# name {^ {parameters} body}]`, ordinary `{f args}`
calls, and local `[# ...]` bindings. The choices here are proposals to ratify before implementation. General code modules
contain definitions and expressions; they do not implicitly print or collect a document. The optional text reader,
`$` interpolation, presentation, and `.story` profile belong to `rlstd`.

Library definition blocks assume their earlier dependencies are in scope inside `std`. Standalone consumer
examples use `[Import* std]`; `[Import std]` keeps names qualified as `std.map`, `std.Int`, or `std.+`.

`[Assert first second]` is a **compile-time meta form**. The former ordinary `{Assert ...}` operation is removed.
`[% expression]` evaluates statically; `{% a b}` is numeric modulo. They are different forms.

## 1. Foundation and notation

`{^ {x y} body}` constructs a two-argument function. `{^ {} body}` constructs a zero-argument thunk. A returned
function is written explicitly, for example `{^ {f g} {^ {x} {f {g x}}}}`. This is different from a three-argument
lambda; no implicit currying or partial application occurs in source.

The blocks are proposed language definitions, not executable files for today's Render Lab parser. Every `K.*`
name denotes a required core primitive with the contract below. A wrapper around a primitive is constructible
once that primitive exists; it is not a proof that the primitive itself follows from typed lambdas. Compiler
forms are identified separately. No unmarked mathematical application, tuple literals, or `let` notation is needed.

Ordinary source calls evaluate the target and arguments **once, left to right**, then apply the function. An earlier
error stops later evaluation. Source functions have declared arity; no implicit partial application occurs.
`{f}` calls a zero-argument function, while bare `f` in code reads the function value. A zero-argument function has
type `Fn([], R)`, distinct from `R`; it lowers to a hidden Unit argument in the unary lambda model.

Signatures use `Fn([A,B],R)` for a two-argument function returning `R`. `A -> B` is mathematical function notation.
Each source signature fixes where argument application stops and a returned lambda begins. For instance,
`{compose f g}` accepts two functions and returns a one-argument function.

Variadic signatures explicitly pack their arguments into a list or heterogeneous argument vector before entering
the lambda definition. This is a calling convention, not a scope containing all document expressions. Arity/type
checking happens before execution; argument packing preserves source order. A variadic list of values is distinct
from a function that takes one list argument.

For a fixed-arity source function with n parameters, the compiler emits one closure with arity n; its mathematical
model can use n nested unary lambdas internally. An exact n-argument call evaluates all n arguments first, then performs the n
internal applications. Intermediate curried closures are implementation artifacts, not implicit partial source
calls. A returned source lambda has its own arity metadata. For variadics, evaluate all source arguments once,
keep the fixed prefix, pack the remaining tail, then enter the body with prefix plus one packed-tail argument:

| Source operations | Fixed prefix | Packed tail received by the lambda |
|---|---|---|
| `+`, `-`, `*`, `/`, comparisons, `min`, `max`, `list`, `'`, `union`, `Struct`, `Union`, `concat` | None | All arguments, subject to each operation's minimum arity. |
| `FuncType` | Return Type | Remaining argument Types, including an empty tail. |
| `Record` | Record Type | Remaining heterogeneous Member values. |
| `format` | Template String | Remaining heterogeneous formattable scalars. |

This adapter is part of the declared function signature and is preserved by aliases and higher-order use.
`map`, `fold`, `foldr`, `append`, `Field`, and `Member` are fixed arity and do not pack a tail.

The earlier design did not provide a way to construct a variadic function from a fixed-arity lambda. This draft
proposes the primitive `{K.variadic prefixCount minimumTailCount implementation}`. The counts are static nonnegative
Ints; `implementation` has exactly `prefixCount + 1` parameters. The result accepts the fixed prefix and at least
the stated number of trailing arguments, packing the tail into its last parameter. A homogeneous tail becomes a
List; a heterogeneous tail uses the checked internal argument vector. The compiler derives a checked callable
signature from the implementation and per-argument constraints, retaining its stage requirements. It never uses
an implicit `Any` type. This bridge is an explicit addition to ratify, not something fixed-arity lambdas derive.

For example, the source definition of a variadic sum is:

```text
[# + {K.variadic 0 0 {^ {xs} {fold xs K.zero K.add}}}]
```

`K.zero` is the identity in the inferred numeric domain; `K.add` is primitive binary addition. `{+ 1 2 3}` enters
the lambda with the one list `[1,2,3]`. Calling that fixed-arity lambda directly would require one List argument.
The bridge's wrapper and the underlying implementation are different functions.

Names include ordinary word identifiers and standalone symbolic identifiers such as `+`, `-`, `%`, `'`, and `|`.
They are valid binding names in `[# ...]`. Double quotes delimit String literals; apostrophe is not a quote reader
prefix. Signed numeric tokens such as `-8` are literals when the sign directly precedes digits; `-` alone is an
operator name, and `{- 8}` is unary application. Qualified module names can be resolved through the module system;
precise identifier/escape Unicode rules must be frozen with the lexer.

The semantic substrate supplies these primitives; definitions using them do not claim to derive their meaning
from lambdas alone:

| Primitive | Contract |
|---|---|
| Numeric primitives | `K.add`, `K.subtract`, `K.multiply`, `K.negate`, `K.divideFloat`, `K.toFloat`, `K.quotientEuclidean`, `K.remainderEuclidean`; section 3 fixes their domains and errors. `K.zero`/`K.one` are constrained numeric identity constants. |
| `K.nil`, `K.cons`, `K.caseList` | Typed immutable lists. `{K.caseList K.nil empty nonempty}` calls `{empty}`; on a cons cell it calls `{nonempty head tail}`. Both handlers are function values, so the unused branch does not run. |
| `K.none`, `K.some`, `K.caseOption` | Typed immutable Option constructors/elimination, with zero- and one-argument handlers. |
| `K.unit` | The one Unit value. |
| `K.choose` | `{K.choose b x y}` selects `x` if `b:Bool` is true, otherwise `y`; both operands evaluate before selection. Pass thunks for lazy control. |
| `K.fix`, `K.variadic` | Recursive-function construction and the explicit variadic bridge specified here. |
| `K.equal`, `K.less` | Defined equality/ordering on supported types, never object addresses. |
| Typed data/type primitives | The `K.*` operations listed in section 5 provide validated immutable type nodes, records, pairs, nominal wrappers, and union projection. Their checking contracts are part of the core, not inferred from the wrapper lambdas. |
| `K.stringAppend`, `K.scalarString`, `K.stringScalars` | General String operations. `K.stringScalars` returns a List of one-scalar Strings; scalar formatting is specified in section 7. |
| `K.mapArgs` | Internal ordered mapping over a checked heterogeneous argument vector. It specializes the callback at each argument's static type and returns a homogeneous list of results. It does not introduce a heterogeneous source List or implicit `Any`. |
| `K.fail` | Abort the current evaluation with a diagnostic. No language value is returned. |
| Explicit host context | A caller-provided capability interface. No built-in domain-specific keys, clocks, resources, or effects. |

`format` uses `K.mapArgs` with `str`; `rlstd.content` uses it with its registered presentation conversion. Each
application is checked before evaluation, and callbacks run left to right. `Record` instead validates its Member
argument vector against the static record schema. These are compiler-supported adapters, not ordinary homogeneous
`map` calls disguised as functions over mixed types.

Recursion has one explicit bootstrap primitive:

```text
[# fix {^ {build} {K.fix build}}]
```

For any function type F with a fixed argument count, `K.fix` has signature `Fn([Fn([F],F)],F)`. It constructs a
recursive function `self` of type F and passes it to `build`. The builder must produce a function without invoking
`self` during construction; premature use is an initialization error. Later calls to `self` delegate to that
function with the same exact argument count. All recursive definitions below use a builder returning a lambda.
This supports multi-argument source functions directly, unlike assuming partial application of a unary fixpoint.
General recursion is a primitive here: the usual untyped self-application combinator is not typable in this core.
All evaluation, including static recursion, has instruction, depth, and allocation budgets. A budget error is not a
normal result. Finite list recursions below terminate if their callbacks terminate and remain within the budget.

## 2. Control flow and basic functions

`true` and `false` are core Bool values. They are not callable functions in the typed source language. The primitive
`K.choose` selects a value; passing two zero-argument lambdas and invoking only the selected one implements laziness:

```text
[# ifThunk {^ {b thenThunk elseThunk}
    {{K.choose b thenThunk elseThunk}}
}]
[# not {^ {b} {ifThunk b {^ {} false} {^ {} true}}}]
[# id {^ {x} x}]
[# const {^ {x ignored} x}]
[# compose {^ {f g} {^ {x} {f {g x}}}}]
[# flip {^ {f} {^ {x y} {f y x}}}]
[# error {^ {message} {K.fail message}}]
```

`{{K.choose b thenThunk elseThunk}}` first obtains a zero-argument function from the inner call, then invokes it
with the outer braces. `compose` returns a one-argument lambda; `flip` returns a two-argument lambda.

| Source call | Signature and meaning |
|---|---|
| `{id x}` | `Fn([T],T)`; unchanged value. |
| `{const x y}` | `Fn([A,B],A)`; returns `x`; both arguments still evaluate. |
| `{compose f g}` | Given `g:A->B`, `f:B->C`, return `A->C`. |
| `{flip f}` | Given a two-argument `f`, return a two-argument function with argument order reversed. |
| `{fix f}` | The recursive-function construction above; no termination guarantee. |
| `{error message}` | `String -> Never`; can inhabit an expected result position because it never returns. |
| `{if c yes no}` | Syntactic conditional, not eager three-argument application. `c:Bool`; both branches must check against the same result type, except `Never`. |
| `{ifThunk c yesThunk noThunk}` | Ordinary first-class function; explicit delayed branches. |
| `{not b}` | `Bool -> Bool`. |

The compiler elaborates `{if c yes no}` to the following, where `c`, `yes`, and `no` stand for the original
operand expressions inserted without evaluating them first:

```text
{ifThunk c {^ {} yes} {^ {} no}}
```

Only the selected branch runs. Both branches are type-checked. Static-capability checking conservatively includes
both branches, even if a literal condition would avoid a live dependency. `if` is reserved syntax and cannot be
aliased as an ordinary eager function; use `ifThunk` for higher-order use. No implicit truthiness is allowed.

`and` and `or` are variadic short-circuit forms, implemented using lists of thunks:

```text
[# allThunks {fix {^ {self} {^ {thunks}
    {K.caseList thunks
        {^ {} true}
        {^ {first rest}
            {ifThunk {first} {^ {} {self rest}} {^ {} false}}
        }
    }
}}}]
[# anyThunks {fix {^ {self} {^ {thunks}
    {K.caseList thunks
        {^ {} false}
        {^ {first rest}
            {ifThunk {first} {^ {} true} {^ {} {self rest}}}
        }
    }
}}}]
```

For example, `{and e1 e2}` elaborates to `{allThunks {list {^ {} e1} {^ {} e2}}}`; `or` uses `anyThunks`.
Zero operands yield `true` for `and`, `false` for `or`. Only required operands execute, in source order; all must
type-check as Bool. Like `if`, the short-circuit forms require syntax elaboration; the thunk helpers are ordinary
constructible functions.

## 3. Numbers, arithmetic, comparisons, and equality

Proposed v1 numeric domains:

- `Int` is signed 64-bit. Primitive arithmetic computes its mathematical integer result and errors outside
  `[-2^63, 2^63-1]`. Each primitive step is checked, including intermediate fold results.
- `Float` is finite IEEE binary64. Basic operations round to nearest, ties to even; non-finite results error.
  All zero results/inputs normalize to positive zero; NaN and infinity are not language values. This makes numeric
  equality consistent with returning either equal operand. No bit-pattern/pointer observation is exposed.
- An integer token defaults to Int; a decimal/exponent token defaults to Float. No implicit mixed arithmetic.
  `float` converts Int to Float with the stated rounding. No vector arithmetic overload is included in v1.
- `Num N` below means one selected numeric domain for all arguments. Context resolves the domain; a zero-argument
  numeric call without an expected domain defaults to Int. This is compile-time dispatch, not runtime duck typing.

Numeric operator bindings retain a constrained polymorphic scheme over N. An alias such as `[# add +]` preserves
that scheme; each use is instantiated from its arguments/expected callback type, not fixed by its first call.
An inferred lambda with a concrete Int literal/operation can remain Int-specific; v1 need not infer unrestricted
higher-rank polymorphism. Numeric constraints and stage requirements are never dropped by aliasing.

`xs` in the variadic definitions is the packed list of arguments. `fold` is defined in section 4. Definitions
are grouped by topic, not module initialization order: load `fix`, control helpers, and list folds before numeric
wrappers. The list definitions use primitive arithmetic where necessary, avoiding a dependency cycle.

```text
[# + {K.variadic 0 0 {^ {xs} {fold xs K.zero K.add}}}]
[# * {K.variadic 0 0 {^ {xs} {fold xs K.one K.multiply}}}]
[# - {K.variadic 0 1 {^ {xs}
    {K.caseList xs
        {^ {} {error "- needs an argument"}}
        {^ {first rest}
            {K.caseList rest
                {^ {} {K.negate first}}
                {^ {next remaining} {fold rest first K.subtract}}
            }
        }
    }
}}]
[# / {K.variadic 0 1 {^ {xs}
    {K.caseList xs
        {^ {} {error "/ needs an argument"}}
        {^ {first rest}
            {K.caseList rest
                {^ {} {K.divideFloat 1.0 first}}
                {^ {next remaining} {fold rest first K.divideFloat}}
            }
        }
    }
}}]
[# abs {^ {x}
    {ifThunk {K.less x K.zero} {^ {} {K.negate x}} {^ {} x}}
}]
[# float {^ {n} {K.toFloat n}}]
```

| Operation | Arity/domain | Defined behavior |
|---|---|---|
| `+` | Zero or more, uniform `Num N` | Sum left to right; `{+}` is zero. |
| `*` | Zero or more, uniform `Num N` | Product left to right; `{*}` is one. |
| `-` | One or more, uniform `Num N` | Unary negation; otherwise left subtraction. `{- 10 3 2}=5`. |
| `/` | One or more Float | Unary reciprocal; otherwise left division. Zero divisor errors. Use `div` for Int. |
| `abs` | One numeric argument | Nonnegative magnitude; `abs` of the minimum Int overflows. |
| `float` | One Int | Explicit conversion; large integers may round. |

`*` is multiplication. The previously explored `{* ...}` document-suffix transformation is not adopted under this
spelling. Content transformation can use ordinary functions on explicit content values.

Integer `div` and `%` use **Euclidean division**. For `b != 0`, calculate in mathematical integers:

```text
r(a,b) = a - abs(b) * floor(a / abs(b))
q(a,b) = (a - r(a,b)) / b

```

The equations specify the numeric primitives, rather than source expressions using overflowing Int intermediates.
The source wrappers are:

```text
[# div {^ {a b} {K.quotientEuclidean a b}}]
[# % {^ {a b} {K.remainderEuclidean a b}}]
```

Thus `a = b*q + r` and `0 <= r < abs(b)`. Examples: `{% -8 3}=1`, `{% 8 -3}=2`. The mathematical quotient
can exceed Int; `div` then errors. `%` checks only its own result, so modulo by `-1` yields zero even for minimum
Int. `div`/`%` each require exactly two Int arguments. Their mathematical intermediate calculation is not checked
Int arithmetic; otherwise computing `abs(minInt)` would incorrectly break valid modulo inputs.

Equality is same-type structural equality for comparable types: canonical numbers, Bool, String (exact Unicode
scalar sequence), Unit, lists/options/pairs recursively, records by type and field values, nominal values by tag
and representation, and normalized static Type values. No implicit numeric conversion or approximate Float
comparison is performed. Functions, opaque host handles, and aggregates containing non-comparable values have no
generic equality operation. Unsupported comparisons are type errors, not silently false results.

Order is defined for Int, Float, and String; string order is lexicographic by Unicode scalar value, independent of
locale. Comparisons require at least two values of the same supported type. They compare adjacent pairs:

```text
[# chain {fix {^ {self} {^ {compare xs}
    {K.caseList xs
        {^ {} true}
        {^ {a tail}
            {K.caseList tail
                {^ {} true}
                {^ {b rest}
                    {ifThunk {compare a b}
                        {^ {} {self compare tail}}
                        {^ {} false}
                    }
                }
            }
        }
    }
}}}]
[# = {K.variadic 0 2 {^ {xs} {chain K.equal xs}}}]
[# < {K.variadic 0 2 {^ {xs} {chain K.less xs}}}]
[# <= {K.variadic 0 2 {^ {xs}
    {chain {^ {a b} {not {K.less b a}}} xs}
}}]
[# > {K.variadic 0 2 {^ {xs} {chain {^ {a b} {K.less b a}} xs}}}]
[# >= {K.variadic 0 2 {^ {xs}
    {chain {^ {a b} {not {K.less a b}}} xs}
}}]
[# != {^ {a b} {not {K.equal a b}}}]
[# nonemptyFold {^ {xs f}
    {K.caseList xs
        {^ {} {error "empty reduction"}}
        {^ {first rest} {fold rest first f}}
    }
}]
[# min {K.variadic 0 1 {^ {xs}
    {nonemptyFold xs {^ {a b} {ifThunk {K.less b a} {^ {} b} {^ {} a}}}}
}}]
[# max {K.variadic 0 1 {^ {xs}
    {nonemptyFold xs {^ {a b} {ifThunk {K.less a b} {^ {} b} {^ {} a}}}}
}}]
```

`!=` has exactly two operands; it does not mean variadic pairwise distinctness. `min`/`max` require one or more
ordered operands. `nonemptyFold xs f` extracts the first element and left-folds the tail, or errors on empty input.
All ordinary comparison operands evaluate before `chain`; callback short-circuiting does not make source argument
evaluation lazy.

For compatibility with the examples used during design, ordinary aliases preserve arity, dispatch, and errors:

```text
[# add +]
[# subtract -]
[# multiply *]
[# divide /]
```

## 4. Lists, apostrophe, map, folds, and collection union

Lists are immutable and homogeneous. Empty lists get an element-type variable resolved from context; require an
explicit type ascription if the compiler cannot resolve it. There is no implicit `Any` fallback.

`list` is variadic. Bind apostrophe as an ordinary alias, `[# ' list]`, so `{' 1 2 3}` constructs the evaluated list
`[1,2,3]`. `{' {+ 1 2} 4}` contains `3` and `4`. **This is not Lisp quotation**: it does not preserve identifiers or
expressions as unevaluated code. No separate `'expression` reader syntax is introduced.

```text
[# list {K.variadic 0 0 {^ {xs} xs}}]
[# ' list]
[# cons {^ {first rest} {K.cons first rest}}]
[# isEmpty {^ {xs}
    {K.caseList xs {^ {} true} {^ {first rest} false}}
}]
[# head {^ {xs}
    {K.caseList xs {^ {} {error "empty head"}} {^ {first rest} first}}
}]
[# tail {^ {xs}
    {K.caseList xs {^ {} {error "empty tail"}} {^ {first rest} rest}}
}]
[# fold {fix {^ {self} {^ {xs initial f}
    {K.caseList xs
        {^ {} initial}
        {^ {first rest} {self rest {f initial first} f}}
    }
}}}]
[# foldr {fix {^ {self} {^ {xs initial f}
    {K.caseList xs
        {^ {} initial}
        {^ {first rest} {f first {self rest initial f}}}
    }
}}}]
[# map {fix {^ {self} {^ {xs f}
    {K.caseList xs
        {^ {} K.nil}
        {^ {first rest} {K.cons {f first} {self rest f}}}
    }
}}}]
[# filter {fix {^ {self} {^ {xs predicate}
    {K.caseList xs
        {^ {} K.nil}
        {^ {first rest}
            {ifThunk {predicate first}
                {^ {} {K.cons first {self rest predicate}}}
                {^ {} {self rest predicate}}
            }
        }
    }
}}}]
[# length {^ {xs} {fold xs 0 {^ {n item} {K.add n 1}}}}]
[# reverse {^ {xs} {fold xs K.nil {^ {acc item} {K.cons item acc}}}}]
[# append {^ {xs ys} {foldr xs ys {^ {item acc} {K.cons item acc}}}}]
[# flatten {^ {xss} {foldr xss K.nil append}}]
[# contains {fix {^ {self} {^ {xs value}
    {K.caseList xs
        {^ {} false}
        {^ {first rest}
            {ifThunk {K.equal first value} {^ {} true} {^ {} {self rest value}}}
        }
    }
}}}]
[# unique {^ {xs}
    {reverse {fold xs K.nil {^ {seen item}
        {ifThunk {contains seen item} {^ {} seen} {^ {} {K.cons item seen}}}
    }}}
}]
[# union {K.variadic 0 0 {^ {lists} {unique {flatten lists}}}}]
```

| Source call | Signature/behavior |
|---|---|
| `{map xs f}` | `List A`, `A->B` -> `List B`; preserves order; callbacks execute left to right. |
| `{fold xs initial f}` | `List A`, `B`, `Fn([B,A],B)` -> `B`; left fold with accumulator first. |
| `{foldr xs initial f}` | `List A`, `B`, `Fn([A,B],B)` -> `B`; strict right fold; callbacks execute right to left. |
| `{filter xs p}` | `p:A->Bool`; preserve matching elements in source order. |
| `{append xs ys}` | Two `List T` values; concatenate without deduplication. |
| `{flatten xss}` | `List(List T)` -> `List T`; flatten exactly one list level. |
| `{unique xs}` | Comparable `T`; retain first occurrence of each value. |
| `{union xs ys ...}` | Comparable `T`; concatenate then deduplicate, preserving first-occurrence order. Zero lists produce empty `List T`. |
| `{contains xs x}` | Comparable `T`; exact membership. |
| `{length xs}`, `{reverse xs}`, `{head xs}`, `{tail xs}`, `{isEmpty xs}` | Their lambda meanings above; `length` errors if it exceeds Int capacity. |

These argument orders deliberately preserve the discussion's list-first `map` convention. For example:

```text
{map {' 1 2 3} {^ {x} {+ x 1}}}
{fold {' 1 2 3} 0 {^ {sum x} {+ sum x}}}
{union {' 2 1 2} {' 1 3}}
```

The mathematical results are `[2,3,4]`, `6`, and `[2,1,3]`, respectively. Collection functions do not introduce
variable bindings or inspect document source.

## 5. Types, fields, records, and type union

Types are static immutable values of `Type`. They are not instance constructors and have no automatic text
presentation. The library supplies type constants `Int`, `Float`, `Bool`, `String`, `Unit`, `Never`, `Type`, and
type-producing functions. The compiler rejects ill-formed recursive type descriptions and runtime-dependent
signatures. A complete universe hierarchy/dependent proof language is outside v1.

Source definitions use typed primitive constructors. The compiler gives these operations their static-parameter
and result-type schemes; the wrappers do not implement type checking by themselves:

```text
[# List {^ {T} {K.listType T}}]
[# Option {^ {T} {K.optionType T}}]
[# Pair {^ {A B} {K.pairType A B}}]
[# Field {^ {name T} {K.fieldDescriptor name T}}]
[# Struct {K.variadic 0 0 {^ {fields} {K.recordType fields}}}]
[# FuncType {K.variadic 1 0 {^ {result arguments}
    {K.functionType result arguments}
}}]
[# | FuncType]
[# Nominal {^ {tag representation} {K.nominalType tag representation}}]
[# Union {K.variadic 0 0 {^ {types}
    [# alternatives {K.normalizeUnionAlternatives types}]
    {K.caseList alternatives
        {^ {} Never}
        {^ {first rest}
            {K.caseList rest
                {^ {} first}
                {^ {second remaining} {K.unionTypeNode alternatives}}
            }
        }
    }
}}]
```

| Operation | Source API and constraints |
|---|---|
| `Field` | `{Field "count" Int}`; exactly a name String and a Type; produces `FieldDescriptor`, not a lexical binding. |
| `Struct` | Variadic FieldDescriptor arguments -> Type; rejects duplicate names and invalid types. |
| `FuncType` | `{FuncType Return Arg1 Arg2 ...}` -> Type. Zero argument types describe a zero-argument function. `[# | FuncType]` makes `{| Int Int Int}` an ordinary alias call. |
| `List`, `Option` | One Type -> Type. |
| `Pair` | Two Types -> Type. |
| `Union` | Zero or more Types -> Type; tagged alternatives, as specified below. |
| `Nominal` | A static qualified tag String and representation Type -> a distinct wrapper type. |

`K.listType`, `K.optionType`, `K.pairType`, and `K.functionType` build the named normalized Type descriptors from
their Type arguments. `K.fieldDescriptor` creates a field descriptor; `K.recordType` validates and canonicalizes
the list of descriptors. `K.nominalType` validates the qualified tag and representation; `K.unionTypeNode` builds
a tagged union from at least two normalized alternatives. These operations have static requirements.

For a pure deterministic `Struct`, choose **structural type identity**: `K.recordType` validates unique names,
then sorts descriptors by exact name in Unicode scalar order. Two calls with equal field maps yield equal types,
regardless of field order or call site. Names are nonempty strings with exact spelling; no locale-dependent
normalization occurs. Aliases preserve identity. For distinct identity, use `Nominal` with an explicit qualified tag;
the same tag with incompatible representations is a static error. This replaces the earlier tentative proposal
of a fresh identity for every Struct declaration, which would require an implicit generation effect.

`K.normalizeUnionAlternatives` produces a list: recursively flatten type unions, remove `Never` and duplicate normalized
alternatives, and sort by canonical type encoding. `Union` collapses zero alternatives to `Never`, one to that
alternative, and two or more to a tagged union. Normalization includes aliases and structural type construction; it does not try
to prove arbitrary functions equivalent. `Union A B` equals `Union B A`; collection `union` and type `Union` are
separate operations, distinguished by spelling and signatures.

Instance construction and field access use explicit functions:

```text
[# Member {^ {name value} {K.recordMember name value}}]
[# Record {K.variadic 1 0 {^ {T members} {K.record T members}}}]
[# get {^ {record name} {K.getField record name}}]
[# withField {^ {record name value} {K.replaceField record name value}}]
[# pair {^ {a b} {K.makePair a b}}]
[# first {^ {p} {K.first p}}]
[# second {^ {p} {K.second p}}]
```

`K.recordMember` constructs a typed Member; `K.record` validates its member vector against T. `K.getField` selects
the named field; `K.replaceField` returns a new same-type record with that field replaced. `K.makePair`, `K.first`,
and `K.second` construct/project typed pairs. Their metadata is not an exported `typeOf` reflection API. `Member` is
polymorphic; a variadic Record member vector may contain fields of different statically checked types. Record's
type and member names must be static; member values may be live. The checker requires exactly the declared field
names, no duplicates/missing/extra members, and the right value types. `get`/`withField` require a statically known
field name; replacement preserves the record type and returns a new value. `Record` accepts structural record
types; applying a Type as a function is not implicit constructor invocation.

```text
[# Counter [% {Struct {Field "count" Int} {Field "label" String}}]]
[# counter {Record Counter {Member "count" 3} {Member "label" "items"}}]
{get counter "count"}
```

For nominal and union data, provide:

```text
[# wrap {^ {N value} {K.wrap N value}}]
[# unwrap {^ {nominalValue} {K.unwrap nominalValue}}]
[# inject {^ {U A value} {K.inject U A value}}]
[# project {^ {unionValue A} {K.projectAlternative unionValue A}}]
[# IsType {^ {value T} {K.hasType value T}}]
```

`wrap` checks the nominal representation type; `unwrap` returns that representation. `inject` requires static
union U and an exact normalized alternative A, checks `value:A`, and returns U. A singleton union injection is
identity. `project` requires static alternative A and returns `Option A`; a different variant yields None.
Union values retain their alternative tag. No automatic Int/Float conversions, union injections, or downcasts.
`K.wrap`/`K.unwrap` validate and construct/project nominal values. `K.inject` validates the static union U,
alternative A, and payload type. `K.projectAlternative` checks whether the selected alternative is A and returns
Some(payload) or None. The primitive's dependent result scheme supplies `Option A`; an unrefined generic payload
cannot be passed to `Some` by an ordinary type-blind lambda. The checker admits `project` only when A is an
alternative of the known union. A singleton project returns Some(value) without a wrapper tag. Malformed host
values fail validation before use. `K.hasType` implements the exact-tag/signature predicate described next.
`IsType` is a value-level predicate over checked value tags/signatures: exact type match or a tagged union's selected
alternative; it does not execute function bodies or infer a live expression's type during compilation. It requires
a static expected Type, returns Bool, and does not refine the static type of a value by itself. Use `project` to
obtain a narrower value. Generic function equality is still unsupported.

Meta Type equality compares these finite normalized type descriptions. Types contain a dedicated `Type` node;
arbitrary runtime values/functions cannot be inserted into a checked signature. All type constructors are static
capabilities in v1; the runtime may carry compiled type/tag constants but cannot change checked signatures.

## 6. Meta forms and type ascription

These are compiler syntax, not ordinary runtime function bindings. The source language does not yet expose
compiler Syntax/environment values or a macro system; writing compiler pseudocode in lambda notation would not
make these forms definable by an ordinary source lambda.

| Form | Compiler meaning |
|---|---|
| `[# name e]` | Check initializer, establish a lexical binding in the enclosing definition context; no content/result expression. |
| `[Import p]`, `[Import* p]`, `[ImportAs p a]` | Resolve a package's public interface and introduce lexical namespaces/bindings as specified in the [package design](language-packages.md); no expression result. |
| `[% e]` | Check static capabilities, evaluate e during preparation, and embed the typed constant. A text/document host decides its presentation. |
| `[Assert a b]` | Evaluate both operands statically once, compare supported same-type values, return the first constant or fail. |
| `{: e T}` | Resolve static expected Type, check e without running a live value, and preserve e's value at its original stage. |

For `[% e]`, infer/check e, check static capabilities, evaluate it once in its lexical static environment, and embed
the typed value as a constant. Capability checking does not prove termination; evaluation can exhaust its budget.

The compiler implements `[Assert a b]` in this order:

1. Infer the first operand's type and check the second against the same type.
2. Require defined equality and static capabilities for both operands before evaluating either.
3. Evaluate the first operand once, then the second once, in their lexical static environment.
4. Apply the following private helper to those values and embed the returned first value as a typed constant.

```text
[# eqAssert {^ {a b}
    {ifThunk {K.equal a b} {^ {} a} {^ {} {error "assertion failed"}}}
}]
```

This helper is ordinary source, but is private to compiler preparation and not an exported runtime `Assert`
binding. It does not implement the preceding syntax/type/stage checks. `[Assert false false]` returns false.
Unequal values, incompatible operand types, unsupported equality, stage errors, and failed operand evaluation
produce diagnostics. `[Assert liveValue 3]` is rejected even if a future live value might be three. It is not
silently moved to runtime. Types can be asserted equal using their normalized descriptions.

For `{: e T}`, resolve T statically, require a valid Type value, and check e against it without running e. After
checking, its computational behavior is `{{^ {value} value} e}`, or simply e after erasure. Its runtime value
evaluates once. It is not equivalent to
`[Assert value T]`: one operand there is an instance and the other is a type value. A compiler can use equality
assertion on inferred/expected type descriptions as part of checking; the returned type is not the expression's
value. Computed Type arguments should be explicitly staged with `[% ...]`; direct static Type names need no
additional wrapper.
An ascription constrains argument/result types without erasing the function's inferred stage/capability
requirements. A matching function signature does not make a live function eligible for static evaluation.

Definitions in a function body elaborate around its result expression. For example:

```text
{{^ {}
    [# x {+ 1 2}]
    {* x 4}
}}
```

For this nonrecursive binding, the equivalent application is:

```text
{{^ {x} {* x 4}} {+ 1 2}}
```

Multiple nonrecursive definitions elaborate to nested bindings in source order; later initializers see earlier
names. Definition contexts also admit the three import declarations; these resolve before execution and obey
source-order lexical visibility without becoming expression results. A module initializer follows the same lexical
binding rules. A lambda captures its lexical environment, so a caller's
same-named parameter cannot alter it. Self/mutual recursion requires `fix` or a separately specified recursive
declaration form; it does not arise from accidental use-before-initialization. `[% ...]` does not introduce a scope.

## 7. General strings and formatting

`str` formats String unchanged, Int as signed decimal, Float with deterministic round-trippable formatting
(canonical zero), Bool as `true`/`false`, and Unit as the empty string. It does not print functions, Type values,
arbitrary records, or host handles. String operations use Unicode scalar sequences, without locale-dependent
normalization. A later locale formatter can be a separate operation.

```text
[# str {^ {value} {K.scalarString value}}]
[# concat {K.variadic 0 0 {^ {strings} {fold strings "" K.stringAppend}}}]
```

`concat` is variadic String concatenation; `{concat}` is empty. `format` takes a String template and variadic scalar
arguments. `parseFormat` recognizes `{}` slots, `{{`/`}}` literal braces, and other literal text. An unmatched brace
or wrong slot/argument count errors. It never parses expressions in strings.

The private token type is itself constructed using the library: String means literal text, Unit means a slot.
The parser emits one literal token per scalar (an implementation may coalesce adjacent literal tokens):

```text
[# FormatToken [% {Union String Unit}]]
[# literalToken {^ {text} {inject FormatToken String text}}]
[# slotToken [% {inject FormatToken Unit K.unit}]]
[# parseFormatChars {fix {^ {self} {^ {chars}
    {K.caseList chars
        {^ {} K.nil}
        {^ {char rest}
            {if {K.equal char "{"}
                {K.caseList rest
                    {^ {} {error "unmatched format brace"}}
                    {^ {next remaining}
                        {if {K.equal next "{"}
                            {K.cons {literalToken "{"} {self remaining}}
                            {if {K.equal next "}"}
                                {K.cons slotToken {self remaining}}
                                {error "invalid format slot"}
                            }
                        }
                    }
                }
                {if {K.equal char "}"}
                    {K.caseList rest
                        {^ {} {error "unmatched format brace"}}
                        {^ {next remaining}
                            {if {K.equal next "}"}
                                {K.cons {literalToken "}"} {self remaining}}
                                {error "unmatched format brace"}
                            }
                        }
                    }
                    {K.cons {literalToken char} {self rest}}
                }
            }
        }
    }
}}}]
[# parseFormat {^ {template} {parseFormatChars {K.stringScalars template}}}]
[# renderFormat {fix {^ {self} {^ {tokens args}
    {K.caseList tokens
        {^ {}
            {K.caseList args
                {^ {} ""}
                {^ {unused remaining} {error "format arity"}}
            }
        }
        {^ {token rest}
            {K.caseOption {project token String}
                {^ {}
                    {K.caseList args
                        {^ {} {error "format arity"}}
                        {^ {arg remaining} {K.stringAppend arg {self rest remaining}}}
                    }
                }
                {^ {text} {K.stringAppend text {self rest args}}}
            }
        }
    }
}}}]
[# format {K.variadic 1 0 {^ {template args}
    {renderFormat {parseFormat template} {K.mapArgs args str}}
}}]
```

`format` therefore needs only scalar String operations, the explicit heterogeneous argument adapter, and the
types/lists/control functions already defined. The remaining `K.scalarString` primitive fixes numeric formatting;
this draft does not derive binary64-to-decimal conversion from integer operations.

## 8. General modules and host extensions

The distribution supplies a `std` package containing this library. It is independent of Render Lab's `rlstd`
package. A standalone compiler, evaluator, and semantic-test runner must work with no `rlstd`, renderer, or GPU.
The [package specification](language-packages.md) defines the import forms:

| Form | Available names |
|---|---|
| `[Import package-name]` | Qualified public exports, such as `package-name.content`. |
| `[Import* package-name]` | Public exports under their own names, plus the qualified namespace. |
| `[ImportAs package-name pn]` | Public exports qualified through `pn`, such as `pn.content`. |

These are compiler declarations with lexical, source-order visibility and no expression result. Import only
public exports; conflicting same-scope names are errors, while repeated imports of the same binding are idempotent.
Aliases preserve the exporting definition's identity, type, arity, and stage requirements. Importing a package
does not invoke its exported functions or turn runtime values into compile-time constants. Namespace lookup is
distinct from record field access. The package specification covers nested scopes, submodules, initialization,
dependency resolution, and visibility in detail.

Each package has a public root interface; its modules have lexical definition environments and explicit exports.
A generic resolver maps package/module IDs to source or compiled artifacts and checks import cycles, versions,
and exported names. Hosts may implement that
resolver using their own storage. Render Lab's adapter maps package resource keys into it; the core never searches
`packages/` or reads lab manifests itself. A general CLI may evaluate a selected entry function or final expression
and report its value; it does not interpret that value as document content.

Host functions register a signature, static/runtime requirements, optional capability names, and a callback through
a generic interface. An explicit-context adapter has the source shape `{^ {context arguments} {hostOperation context arguments}}`.
The evaluator supplies explicit context; a program cannot access ambient host state without a registered capability.
A host can supply IO and effects through explicit operations; the core language is not limited to rendering or to
pure numeric calculations. Core typing/evaluation and the pure STL remain usable with no host operations at all.
Specific IO libraries and an effect sequencing syntax need a later design; foreign function registration does not
implicitly turn arbitrary calls into mutation or execute them during type checking.

Code and compiled artifacts keep source maps, normalized type information, and module dependency hashes. Core
cache keys and fingerprints depend on the language/compiler and imported modules, not a renderer build. Generic
resource budgets, diagnostics, immutable values, and stage checking are core services. TTS anchors, frame seeking,
translation parity, formula layout, scene parameter lookup, and animation conflict policies are `rlstd` services.

## 9. Standalone source example

```text
[Import* std]
[# next
    [% {: {^ {x} {+ x 1}}
          [% {| Int Int}]}]
]
[# nums [% {map {' 1 2 3} next}]]
[# checkedNums [Assert nums {' 2 3 4}]]
[# total [% {fold checkedNums 0 {^ {sum x} {+ sum x}}}]]
[# checkedTotal [Assert total 9]]
{format "The total is {}." checkedTotal}
```

This returns the String `"The total is 9."` in code mode; it does not draw, speak, or print it implicitly. The host
chooses how to consume a result. Static creation of `next` does not run its body; the later static `map` call does.
Both assertions return values bound by definitions. This example requires only the general language and `std`.

## 10. Acceptance cases and implementation boundaries

| Case | Required result |
|---|---|
| `{+}`, `{*}`, `{- 7}`, `{- 10 3 2}` | `0`, `1`, `-7`, `5` in default Int domain. |
| `{+ 1 2.0}` | Type error; use an explicit conversion. |
| `{% -8 3}`, `{% 8 -3}`, `{% 4 0}` | `1`, `2`, error. |
| `{if true 7 {error "unused"}}` | `7`; unchosen branch never executes, but still type-checks. |
| `{if true 7 "wrong"}` | Type error despite literal true. |
| `{' {+ 1 2} 4}` | List `[3,4]`, not syntax trees. |
| `{fold {' 1 2 3} 0 {^ {a x} {- a x}}}` | `-6`, proving left-fold order. |
| `{union {' 2 1 2} {' 1 3}}` | `[2,1,3]`; stable first occurrence. |
| `[Assert false false]`, `[Assert 1 2]` | `false`, static error. |
| `[Assert {foreignRead} 3]`, with a test host's Runtime-only `foreignRead` | Stage error, no runtime fallback. |
| `{: "wrong" Int}` | Static type error, no conversion or attempted execution. |
| Two Struct calls with reversed equal field descriptors | Equal structural Type; explicit Nominal tags can distinguish types. |
| `{Union Int String}` versus `{Union String Int Int}` | Equal normalized Type. |
| Ordinary `{Assert 1 1}` | No runtime Assert binding; meta syntax is required. |
| `[Import std]` followed by `{std.+ 1 2}` | `3`; package exports remain qualified. |
| `[Import* std]` followed by `{+ 1 2}` | `3`; wildcard imports introduce public names directly. |
| `[ImportAs std s]` followed by `{s.+ 1 2}` | `3`; aliasing preserves function and type identity. |
| Wildcard imports introducing distinct same-scope `map` bindings | Compile-time name conflict; no import-order winner. |

Implementation can optimize lists, tagged Bool, and native operations instead of allocating their lambda models,
provided value, error, stage, and order behavior is preserved. The catalog does not require a macro system that
rewrites arbitrary future syntax. Generalized dependent typing, arbitrary dynamic reflection, and function equality
are outside this first STL. Render Lab-specific behavior belongs to the `rlstd` design.

Before implementation, ratify this draft's numeric limits, structural Struct choice, explicit Nominal tags, union
conventions, collection argument order, and static-only Assert. Add executable semantic tests when an evaluator
exists; this documentation change alone does not supply a running interpreter.

## References for the semantic choices

- Racket's [conditional specification](https://docs.racket-lang.org/guide/conditionals.html) evaluates only the
  selected branch. The lambda translation here implements that property through explicit thunks.
- Notre Dame's [Church encoding notes](https://academicweb.nd.edu/~dchiang/teaching/pl/2025/church.html) explain why
  an eager Church selector alone does not implement ordinary short-circuit `if`.
- Lean's [list reference](https://lean-lang.org/doc/reference/latest/Basic-Types/Linked-Lists/) gives established
  map/fold distinctions; the argument order and callback order above are explicit choices for this language.
- Lean's [core type theory](https://lean-lang.org/doc/reference/latest/The-Type-System/) separates type checking,
  definitional equality, and term computation. This draft likewise does not claim ordinary runtime assertions
  implement a static type checker.
