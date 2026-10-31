# General-purpose functional language STL: lambda semantics (design draft)

This is the proposed general-purpose language standard library, not an implementation or an existing package.
It accompanies the [language and integration plan](story-language.md). It has no Render Lab dependency: no text
renderer, narration, timeline, scene, GPU, or lab package loader is required to check or run it. Those features are
defined in the separate [`rlstd` language library](rlstd-language.md), owned by `packages/rlstd/`.

This draft fixes a coherent first set of operations, mathematical meanings, and errors. Library names are
case-sensitive. Source examples use the current syntax; mathematical definitions use lambda notation, not
additional source syntax. The choices here are proposals to ratify before implementation. General code modules
contain definitions and expressions; they do not implicitly print or collect a document. The optional text reader,
`$` interpolation, presentation, and `.story` profile belong to `rlstd`.

Source examples assume the listed `std` names are in scope. Whether a normal module imports a prelude explicitly
or receives one by default is a module-syntax decision, not a second evaluator.

`[Assert first second]` is a **compile-time meta form**. The former ordinary `{Assert ...}` operation is removed.
`[% expression]` evaluates statically; `{% a b}` is numeric modulo. They are different forms.

## 1. Foundation and notation

`lambda x. e`, abbreviated `λx. e`, constructs a function. Application associates left: `f x y = (f x) y`.
Mathematical `let x = e in body` abbreviates `(λx. body) e`, with lexical, capture-avoiding binding. Tuples in
these definitions describe mathematical arguments or results; they do not introduce tuple-literal source syntax.

Ordinary source calls evaluate the target and arguments **once, left to right**, then apply the function. An earlier
error stops later evaluation. Source functions have declared arity; no implicit partial application occurs.
`{f}` calls a zero-argument function, while bare `f` in code reads the function value. A zero-argument function has
type `Fn([], R)`, distinct from `R`; it lowers to a hidden Unit argument in the unary lambda model.

Signatures use `Fn([A,B],R)` for a two-argument function returning `R`. `A -> B` is mathematical function notation.
The lambda equations may be curried for readability; each source signature fixes where argument application stops
and a returned lambda begins. For instance, `compose(f,g)` accepts two functions and returns a one-argument function.

Variadic signatures explicitly pack their arguments into a list or heterogeneous argument vector before entering
the lambda definition. This is a calling convention, not a scope containing all document expressions. Arity/type
checking happens before execution; argument packing preserves source order. A variadic list of values is distinct
from a function that takes one list argument.

For a fixed-arity source function with n parameters, the compiler emits one closure with arity n; its mathematical
model is n nested unary lambdas. An exact n-argument call evaluates all n arguments first, then performs the n
internal applications. Intermediate curried closures are implementation artifacts, not implicit partial source
calls. A returned source lambda has its own arity metadata. For variadics, evaluate all source arguments once,
keep the fixed prefix, pack the remaining tail, then enter the equation with prefix plus one packed-tail argument:

| Source operations | Fixed prefix | Packed tail in the equations |
|---|---|---|
| `+`, `-`, `*`, `/`, comparisons, `min`, `max`, `list`, `'`, `union`, `Struct`, `Union`, `concat` | None | All arguments, subject to each operation's minimum arity. |
| `FuncType` | Return Type | Remaining argument Types, including an empty tail. |
| `Record` | Record Type | Remaining heterogeneous Member values. |
| `format` | Template String | Remaining heterogeneous formattable scalars. |

This adapter is part of the declared function signature and is preserved by aliases and higher-order use.
`map`, `fold`, `foldr`, `append`, `Field`, and `Member` are fixed arity and do not pack a tail. The table describes
the library's variadic native signatures; source syntax for declaring user variadic lambdas is a later decision.

Names include ordinary word identifiers and standalone symbolic identifiers such as `+`, `-`, `%`, `'`, and `|`.
They are valid binding names in `[# ...]`. Double quotes delimit String literals; apostrophe is not a quote reader
prefix. Signed numeric tokens such as `-8` are literals when the sign directly precedes digits; `-` alone is an
operator name, and `{- 8}` is unary application. Qualified module names can be resolved through the module system;
precise identifier/escape Unicode rules must be frozen with the lexer.

The semantic substrate supplies these primitives; definitions using them do not claim to derive their meaning
from lambdas alone:

| Primitive | Contract |
|---|---|
| `K.int`, `K.float`, numeric primitive operations | The numeric behavior in section 3; errors propagate. |
| `Nil`, `Cons(h,t)`, `caseL(xs,n,c)` | Immutable list constructors and elimination: `caseL(Nil,n,c)=n(unit)`; `caseL(Cons(h,t),n,c)=c(h,t)`. Branch handlers are lambdas. |
| `None`, `Some(x)`, `caseO(o,n,s)` | Option constructors with analogous elimination. |
| `K.equal`, `K.less` | Defined equality/ordering on supported types, never object addresses. |
| `K.typeNode`, `K.record`, `K.fieldDescriptor`, `K.recordMember`, `K.project`, `K.wrap`, `K.hasType` | Typed immutable data/type representation, tag checks, and the checks specified below. |
| `K.stringAppend`, `K.scalarString` | General String operations, with formatting specified below. |
| `EmptyArgs`, `Arg(h,t)`, `caseArgs` | Internal heterogeneous argument-vector representation/elimination, analogous to `caseL`. It is not a heterogeneous source List or an implicit `Any` type. |
| `K.fail(message)` | Abort the current evaluation with a diagnostic. No language value is returned. |
| `Infer`, `Check`, `Static`, `Eval`, environment extension | Compiler/evaluator operations on checked syntax and explicit environments, not ordinary calls on already evaluated values. `EvalStatic` is `Eval` after successful static checking. |
| Explicit host context | A caller-provided capability interface. No built-in domain-specific keys, clocks, resources, or effects. |

For heterogeneous variadics, use this internal helper. The checker specializes the constrained callback for each
argument's known type; this is not a call to homogeneous source `map` and does not expose dynamic duck typing:

```text
mapArgs = Fix (λself. λargs.λf.
  caseArgs args (λ_. Nil) (λh.λt. Cons(f h,self t f)))
```

`format` uses this adapter with `str`; `rlstd.content` uses it with its registered presentation conversion. Each
application is checked before evaluation, and callbacks run left to right. `Record` instead validates its Member
argument vector against the static record schema.

`Fix` supplies unrestricted recursion over functions, with the guarded unfolding rule:

```text
Fix F = F (λx. (Fix F) x)
fix   = λF. Fix F
```

This is an operational recursive-closure rule. The recursive argument is a lambda, so a strict evaluator does not
eagerly unfold it forever before a call. An untyped lambda model can use the call-by-value fixed-point combinator:

```text
Z = λf. (λx. f (λv. x x v)) (λx. f (λv. x x v))
```

The typed VM may implement `Fix` directly with signature `Fn([Fn([Fn([A],B)],Fn([A],B))],Fn([A],B))`.
This is a deliberate recursion primitive; the self-application in `Z` is not typable in a simply typed core.
All evaluation, including static recursion, has instruction, depth, and allocation budgets. A budget error is not a
normal result. Finite list recursions below terminate if their callbacks terminate and remain within the budget.

## 2. Control flow and basic functions

Booleans have the lambda model `True = λt.λf.t`, `False = λt.λf.f`. The VM may use Boolean tags with the same
elimination rules. Thunks are zero-argument source functions, modeled as `Unit -> T`.

```text
ifThunk = λb. λthenThunk. λelseThunk. (b thenThunk elseThunk) unit
not     = λb. ifThunk b (λ_. False) (λ_. True)
id      = λx. x
const   = λx. λignored. x
compose = λf. λg. λx. f (g x)
flip    = λf. λx. λy. f y x
error   = λmessage. K.fail(message)
```

| Source call | Signature and meaning |
|---|---|
| `{id x}` | `Fn([T],T)`; unchanged value. |
| `{const x y}` | `Fn([A,B],A)`; returns `x`; both arguments still evaluate. |
| `{compose f g}` | Given `g:A->B`, `f:B->C`, return `A->C`. |
| `{flip f}` | Given a two-argument `f`, return a two-argument function with argument order reversed. |
| `{fix f}` | The guarded fixed point above; no termination guarantee. |
| `{error message}` | `String -> Never`; can inhabit an expected result position because it never returns. |
| `{if c yes no}` | Syntactic conditional, not eager three-argument application. `c:Bool`; both branches must check against the same result type, except `Never`. |
| `{ifThunk c yesThunk noThunk}` | Ordinary first-class function; explicit delayed branches. |
| `{not b}` | `Bool -> Bool`. |

The conditional source form elaborates to:

```text
if c yes no  ==>  ifThunk c (λ_. yes) (λ_. no)
```

Only the selected branch runs. Both branches are type-checked. Static-capability checking conservatively includes
both branches, even if a literal condition would avoid a live dependency. `if` is reserved syntax and cannot be
aliased as an ordinary eager function; use `ifThunk` for higher-order use. No implicit truthiness is allowed.

`and` and `or` are variadic short-circuit forms, implemented using lists of thunks:

```text
allThunks = Fix (λself. λts.
  caseL ts (λ_. True)
           (λh.λt. ifThunk (h unit) (λ_. self t) (λ_. False)))
anyThunks = Fix (λself. λts.
  caseL ts (λ_. False)
           (λh.λt. ifThunk (h unit) (λ_. True) (λ_. self t)))

and e1 ... en  ==>  allThunks [λ_.e1, ..., λ_.en]
or  e1 ... en  ==>  anyThunks [λ_.e1, ..., λ_.en]
```

Zero operands yield `true` for `and`, `false` for `or`. Only required operands execute, in source order; all must
type-check as Bool. The square lists in these equations are mathematical notation, not source bracket forms.

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

`xs` in the variadic equations is the packed list of arguments. `fold` is defined in section 4; `addN`, `subN`,
`mulN`, `negN`, `divFloat`, and `checkInt` denote the specified numeric primitives.

```text
+ = λxs. fold xs zeroN (λacc.λx. addN acc x)
* = λxs. fold xs oneN  (λacc.λx. mulN acc x)
- = λxs. caseL xs (λ_. error "- needs an argument")
       (λh.λt. caseL t (λ_. negN h) (λ_. fold t h subN))
/ = λxs. caseL xs (λ_. error "/ needs an argument")
       (λh.λt. caseL t (λ_. divFloat 1.0 h) (λ_. fold t h divFloat))
abs = λx. ifThunk (K.less x zeroN) (λ_. negN x) (λ_. x)
float = λn. K.float(n)
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

div = λa.λb. ifThunk (K.equal b 0)
                (λ_. error "division by zero") (λ_. checkInt(q(a,b)))
%   = λa.λb. ifThunk (K.equal b 0)
                (λ_. error "modulo by zero") (λ_. checkInt(r(a,b)))
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
chain = Fix (λself. λcmp. λxs.
  caseL xs (λ_. True) (λa.λtail.
    caseL tail (λ_. True) (λb.λrest.
      ifThunk (cmp a b) (λ_. self cmp tail) (λ_. False))))
=  = λxs. chain K.equal xs
<  = λxs. chain K.less xs
<= = λxs. chain (λa.λb. not (K.less b a)) xs
>  = λxs. chain (λa.λb. K.less b a) xs
>= = λxs. chain (λa.λb. not (K.less a b)) xs
!= = λa.λb. not (K.equal a b)
nonemptyFold = λxs.λf. caseL xs (λ_. error "empty reduction") (λh.λt. fold t h f)
min = λxs. nonemptyFold xs (λa.λb. ifThunk (K.less b a) (λ_.b) (λ_.a))
max = λxs. nonemptyFold xs (λa.λb. ifThunk (K.less a b) (λ_.b) (λ_.a))
```

`!=` has exactly two operands; it does not mean variadic pairwise distinctness. `min`/`max` require one or more
ordered operands. `nonemptyFold xs f` extracts the first element and left-folds the tail, or errors on empty input.
All ordinary comparison operands evaluate before `chain`; callback short-circuiting does not make source argument
evaluation lazy.

For compatibility with the examples used during design, `add = +`, `subtract = -`, `multiply = *`, and `divide = /`
are ordinary aliases with identical arity, dispatch, and error rules. The aliases do not implement another evaluator.

## 4. Lists, apostrophe, map, folds, and collection union

Lists are immutable and homogeneous. Empty lists get an element-type variable resolved from context; require an
explicit type ascription if the compiler cannot resolve it. There is no implicit `Any` fallback.

`list` is variadic. Bind apostrophe as an ordinary alias, `[# ' list]`, so `{' 1 2 3}` constructs the evaluated list
`[1,2,3]`. `{' {+ 1 2} 4}` contains `3` and `4`. **This is not Lisp quotation**: it does not preserve identifiers or
expressions as unevaluated code. No separate `'expression` reader syntax is introduced.

```text
list = λpackedArguments. packedArguments
cons = λh.λt. Cons(h,t)
isEmpty = λxs. caseL xs (λ_. True) (λh.λt. False)
head = λxs. caseL xs (λ_. error "empty head") (λh.λt. h)
tail = λxs. caseL xs (λ_. error "empty tail") (λh.λt. t)
fold = Fix (λself. λxs.λz.λf.
  caseL xs (λ_. z) (λh.λt. self t (f z h) f))
foldr = Fix (λself. λxs.λz.λf.
  caseL xs (λ_. z) (λh.λt. f h (self t z f)))
map = Fix (λself. λxs.λf.
  caseL xs (λ_. Nil) (λh.λt. Cons(f h, self t f)))
filter = Fix (λself. λxs.λp.
  caseL xs (λ_. Nil) (λh.λt.
    ifThunk (p h) (λ_. Cons(h,self t p)) (λ_. self t p)))
length = λxs. fold xs 0 (λn.λx. addInt n 1)
reverse = λxs. fold xs Nil (λacc.λx. Cons(x,acc))
append = λxs.λys. foldr xs ys (λx.λacc. Cons(x,acc))
flatten = λxss. foldr xss Nil append
contains = Fix (λself. λxs.λx.
  caseL xs (λ_. False) (λh.λt.
    ifThunk (K.equal h x) (λ_. True) (λ_. self t x)))
unique = λxs. reverse (fold xs Nil (λseen.λx.
  ifThunk (contains seen x) (λ_. seen) (λ_. Cons(x,seen))))
union = λpackedLists. unique (flatten packedLists)
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

Operational lambda definitions use kernel type nodes and immutable field maps:

```text
List = λT. K.typeNode("List", [T])
Option = λT. K.typeNode("Option", [T])
Pair = λA.λB. K.typeNode("Pair", [A,B])
Field = λname.λT. K.fieldDescriptor(name,T)
Struct = λfields. K.typeNode("Record", canonicalFields(fields))
FuncType = λR.λargumentTypes. K.typeNode("Fn", [argumentTypes,R])
Nominal = λtag.λrepresentation. K.typeNode("Nominal", [tag,representation])
Union = λtypes. let alternatives = normalizeAlternatives(types) in
  caseL alternatives (λ_. Never) (λA.λrest.
    caseL rest (λ_. A) (λB.λtail. K.typeNode("Union", alternatives)))
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

For a pure deterministic `Struct`, choose **structural type identity**: `canonicalFields` validates unique names,
then sorts descriptors by exact name in Unicode scalar order. Two calls with equal field maps yield equal types,
regardless of field order or call site. Names are nonempty strings with exact spelling; no locale-dependent
normalization occurs. Aliases preserve identity. For distinct identity, use `Nominal` with an explicit qualified tag;
the same tag with incompatible representations is a static error. This replaces the earlier tentative proposal
of a fresh identity for every Struct declaration, which would require an implicit generation effect.

`normalizeAlternatives` produces a list: recursively flatten type unions, remove `Never` and duplicate normalized
alternatives, and sort by canonical type encoding. `Union` collapses zero alternatives to `Never`, one to that
alternative, and two or more to a tagged union. Normalization includes aliases and structural type construction; it does not try
to prove arbitrary functions equivalent. `Union A B` equals `Union B A`; collection `union` and type `Union` are
separate operations, distinguished by spelling and signatures.

Instance construction and field access use explicit functions:

```text
Member = λname.λvalue. K.recordMember(name,value)
Record = λT.λmembers. K.record(T, checkedMemberMap(T,members))
get = λrecord.λname. K.project(record,name)
withField = λrecord.λname.λvalue. K.record(typeOfRecord(record), updatedFields(record,name,value))
pair = λa.λb. K.record(Pair(typeOf(a),typeOf(b)), [a,b])
first = λp. K.project(p,0)
second = λp. K.project(p,1)
```

`typeOf` in these equations denotes compiler/runtime type metadata, not an exported reflection API. `Member` is
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
wrap = λN.λvalue. K.wrap(N,value)
unwrap = λnominalValue. K.project(nominalValue,"representation")
inject = λU.λA.λvalue. K.wrap(U,[A,value])
project = λUvalue.λA.
  ifThunk (K.equal (K.variantType Uvalue) A)
          (λ_. Some(K.variantValue Uvalue)) (λ_. None)
IsType = λvalue.λT. K.hasType(value,T)
```

`wrap` checks the nominal representation type; `unwrap` returns that representation. `inject` requires static
union U and an exact normalized alternative A, checks `value:A`, and returns U. A singleton union injection is
identity. `project` requires static alternative A and returns `Option A`; a different variant yields None.
Union values retain their alternative tag. No automatic Int/Float conversions, union injections, or downcasts.
`K.variantType` and `K.variantValue` are checked data projections: for a union value, return its alternative tag
and payload. For a singleton union they return the ordinary value's type and the value itself. The checker only
admits `project` when A is an alternative of the known union; malformed host values fail validation before use.
Thus singleton project returns Some for its sole alternative without requiring a wrapper tag.
`IsType` is a value-level predicate over checked value tags/signatures: exact type match or a tagged union's selected
alternative; it does not execute function bodies or infer a live expression's type during compilation. It requires
a static expected Type, returns Bool, and does not refine the static type of a value by itself. Use `project` to
obtain a narrower value. Generic function equality is still unsupported.

Meta Type equality compares these finite normalized type descriptions. Types contain a dedicated `Type` node;
arbitrary runtime values/functions cannot be inserted into a checked signature. All type constructors are static
capabilities in v1; the runtime may carry compiled type/tag constants but cannot change checked signatures.

## 6. Meta forms and type ascription

These are compiler syntax, not ordinary runtime function bindings. Giving a mathematical lambda for their
compiler implementation does not make their syntax operands already evaluated language values.

| Form | Compiler meaning |
|---|---|
| `[# name e]` | Check initializer, establish a lexical binding in the enclosing definition context; no content/result expression. |
| `[% e]` | Check static capabilities, evaluate e during preparation, and embed the typed constant. A text/document host decides its presentation. |
| `[Assert a b]` | Evaluate both operands statically once, compare supported same-type values, return the first constant or fail. |
| `{: e T}` | Resolve static expected Type, check e without running a live value, and preserve e's value at its original stage. |

Let `Gamma` hold lexical types, `rho` lexical values, and `Check(Gamma,e,T)` either produce a checked core term or
fail. `Static(Gamma,e)` checks capability requirements, not termination; evaluation can still exhaust its budget.

```text
prepare = λGamma.λrho.λsyntax.
  let (checked,T) = Infer(Gamma, syntax) in
  let allowed = Static(Gamma, checked) in
  Eval(checked,rho,staticContext)

eqAssert = λa.λb. ifThunk (K.equal a b) (λ_. a) (λ_. error "assertion failed")
compileAssert = λGamma.λrho.λaSyntax.λbSyntax.
  let (ca,T) = Infer(Gamma,aSyntax) in
  let cb = Check(Gamma,bSyntax,T) in
  let comparable = RequireEquality(T) in
  let sa = Static(Gamma,ca) in
  let sb = Static(Gamma,cb) in
  let a = Eval(ca,rho,staticContext) in
  let b = Eval(cb,rho,staticContext) in
  eqAssert a b

ascribe = λGamma.λrho.λeSyntax.λtypeSyntax.
  let T = prepare Gamma rho typeSyntax in
  Check(Gamma,eSyntax,T)
```

`RequireEquality` is the compiler's equality-capability check. Both operands are type/stage checked before either
is evaluated; then left evaluation precedes right evaluation. `eqAssert` is a semantic helper, not an exported
runtime `Assert` function. `[Assert false false]` returns false.
Unequal values, incompatible operand types, unsupported equality, stage errors, and failed operand evaluation
produce diagnostics. `[Assert liveValue 3]` is rejected even if a future live value might be three. It is not
silently moved to runtime. Types can be asserted equal using their normalized descriptions.

Ascription's successful computational behavior is the typed identity `I_T = λ(x:T).x` applied to the checked
expression, or simply the expression after erasure. Its runtime value evaluates once. It is not equivalent to
`[Assert value T]`: one operand there is an instance and the other is a type value. A compiler can use equality
assertion on inferred/expected type descriptions as part of checking; the returned type is not the expression's
value. Computed Type arguments should be explicitly staged with `[% ...]`; direct static Type names need no
additional wrapper.
An ascription constrains argument/result types without erasing the function's inferred stage/capability
requirements. A matching function signature does not make a live function eligible for static evaluation.

Definitions in a function body elaborate around its result expression:

```text
define(x,e,body) = (λx. body) e
```

Multiple nonrecursive definitions elaborate to nested bindings in source order; later initializers see earlier
names. A module initializer follows the same lexical binding rules. A lambda captures its lexical environment, so a caller's
same-named parameter cannot alter it. Self/mutual recursion requires `fix` or a separately specified recursive
declaration form; it does not arise from accidental use-before-initialization. `[% ...]` does not introduce a scope.

## 7. General strings and formatting

`str` formats String unchanged, Int as signed decimal, Float with deterministic round-trippable formatting
(canonical zero), Bool as `true`/`false`, and Unit as the empty string. It does not print functions, Type values,
arbitrary records, or host handles. String operations use Unicode scalar sequences, without locale-dependent
normalization. A later locale formatter can be a separate operation.

```text
str = λvalue. K.scalarString(value)
concat = λstrings. fold strings "" K.stringAppend
format = λtemplate.λpackedArgs. renderFormat(parseFormat(template), mapArgs packedArgs str)
```

`concat` is variadic String concatenation; `{concat}` is empty. `format` takes a String template and variadic scalar
arguments. `parseFormat` recognizes `{}` slots, `{{`/`}}` literal braces, and other literal text. An unmatched brace
or wrong slot/argument count errors. It never parses expressions in strings. Its private token datatype has two
constructors, `Literal(String)` and `Slot`; the parser returns a list of these tokens. These are implementation
data, not new source syntax. Its renderer is defined by:

```text
renderFormat([],[]) = ""
renderFormat(Literal(s)::tokens,args) = K.stringAppend(s,renderFormat(tokens,args))
renderFormat(Slot::tokens,a::args) = K.stringAppend(a,renderFormat(tokens,args))
renderFormat(other unmatched shape) = error "format arity"
```

## 8. General modules and host extensions

The distribution supplies a `std` module containing this library. It is independent of Render Lab's `rlstd`
package. A standalone compiler, evaluator, and semantic-test runner must work with no `rlstd`, renderer, or GPU.
Export/import spelling and a source-file extension remain to be chosen; dependency resolution and module scopes
are general language services, not calls to the lab's package loader.

Each module has a lexical definition environment and explicit exports. A generic module resolver maps module IDs
to source or compiled artifacts and checks import cycles, versions, and exported names. Hosts may implement that
resolver using their own storage. Render Lab's adapter maps package resource keys into it; the core never searches
`packages/` or reads lab manifests itself. A general CLI may evaluate a selected entry function or final expression
and report its value; it does not interpret that value as document content.

Host functions register a signature, static/runtime requirements, optional capability names, and a callback through
a generic interface. Their mathematical form is `lambda context. lambda arguments. hostOperation(context,arguments)`.
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
[# ' list]
[# | FuncType]
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
