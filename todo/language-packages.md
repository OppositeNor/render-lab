# Language packages and imports (design draft)

Packages belong to the general language, independently of Render Lab. This specifies the import forms requested
for the [language plan](story-language.md) and [STL](language-stl.md). It does not implement a loader or package manager.

## Import forms

| Form | Names introduced in the current lexical scope |
|---|---|
| `[Import package-name]` | The namespace `package-name`; access an exported binding as `package-name.content`. |
| `[Import* package-name]` | The namespace `package-name` and direct bindings for its public exports; both `content` and `package-name.content` work. |
| `[ImportAs package-name pn]` | The namespace alias `pn`; access an exported binding as `pn.content`. The original prefix is not additionally introduced. |

The three forms are alternatives; each example below is a separate importing body. Assume `package-name` exports
a String named `content` and a function named `transform`:

```text
[Import package-name]
{package-name.transform package-name.content}
```

```text
[Import* package-name]
{transform content}
```

```text
[ImportAs package-name pn]
{pn.transform pn.content}
```

Importing a binding does not call it. A bare `pn.content` reads a value; `{pn.transform pn.content}` invokes a
function. `{pn.content}` is a zero-argument call and fails if content is not a zero-argument function.

Package names and aliases are literal identifiers in these forms, not evaluated expressions or String values.
Hyphens are allowed in package names, as in `package-name`. Names are case-sensitive. Dots separate namespace
components: `package-name.content` names an export of the imported package. Symbolic exports are also qualified,
for example `{std.+ 1 2}`. An alias is one namespace identifier. Registry/path syntax and additional package-ID
characters remain resolver decisions; a source import is not a filesystem path or a version-selection expression.
Reject missing/extra arguments and non-identifier package/alias operands during parsing/checking. In `rlstd`
interpolation, `$pn.content` reads the same exported binding; a dot belongs to the name only when followed by a
valid next namespace component, so sentence-ending punctuation remains text.

## Public interfaces and namespaces

A package supplies a public root export table. It maps names to immutable bindings or exported submodule
namespaces. Bindings carry their types, static/runtime requirements, and stable definition identities. Private
implementation names are unavailable through all three import forms. Importing one package does not automatically
expose its dependencies; a package must deliberately re-export a dependency's binding or namespace.

For example, if `graphics` publicly exports a `color` submodule, `[Import graphics]` permits
`graphics.color.rgb`. `[Import* graphics]` also introduces `color`, allowing `color.rgb`; it imports the root
export table once, rather than recursively flattening every submodule's exports. `[ImportAs graphics g]` permits
`g.color.rgb`. A private submodule remains inaccessible even when its underlying artifact is installed.

A namespace is compiler name-resolution information, not an ordinary record or callable value. `pn.content`
selects an exported binding; record fields still use `{get record "field"}`. Import names do not create runtime
reflection, dynamically selected package names, or a mutable global environment. The source syntax or descriptor
format by which package authors declare exports and re-exports remains a Phase 0 decision; importing always uses
the explicit public interface, never an accidental list of every internal definition.

## Lexical scope and conflicts

Imports are compiler declarations, like `[# ...]`. They have no expression result and contribute no text or
speech. Permit them in definition contexts: a module, a lambda body before its result expression, or an `rlstd`
document body. Their names are visible only to subsequent source in that body and nested scopes. They do not
change a caller's scope, leak out of a function, or alter a closure's previously resolved bindings.

An import inside a lambda is still resolved while compiling the enclosing program, regardless of whether the
lambda is ever called. It does not load a new package on every invocation. Reject imports as ordinary call
arguments or in an expression-only branch of `if`; they are not conditional runtime operations.

Use these conflict rules in v1:

- Two different bindings cannot acquire the same name in one lexical scope. Report an error at the conflicting
  import/definition and identify both origins; import order must not silently select a winner.
- Repeating an import of the same resolved binding under the same name is idempotent. A second alias for the
  same package is allowed and preserves binding identity.
- The namespace prefix itself is a binding for conflict checks. A prefix cannot replace a same-scope value,
  parameter, or different package alias. `Import*` checks both its namespace and every introduced export name.
- A nested scope may shadow an outer binding, consistently with ordinary lexical definitions. A same-body
  `[# ...]` cannot silently replace an imported name. Use `Import` or `ImportAs` when unqualified exports collide.

Thus two wildcard imports with distinct exports named `map` fail, even if the program never uses `map`. Explicit
namespace imports allow both `first-package.map` and `second-package.map` without ambiguity. An import either
introduces its complete checked set of names or fails; there is no partial wildcard import after a collision.

## Compiler meaning and stages

Let P be the package resolved from the source package name, and E its public export table. These compiler
operations describe the forms; they are not ordinary source lambdas:

| Form | Elaboration |
|---|---|
| `[Import p]` | Resolve/check P, then bind namespace p to E. |
| `[Import* p]` | Resolve/check P, bind namespace p to E, then introduce each root export under its own name. |
| `[ImportAs p a]` | Resolve/check P, then bind namespace a to E. |

Every reference resolves to the exporting package's definition identity, independent of its spelling in the
importing body. Qualified, wildcard, aliased, and deliberately re-exported references to the same definition have
the same type/value semantics. Aliasing does not reconstruct types, generate fresh nominal tags, or change a
function's arity, generic constraints, captured environment, or capability requirements.

Resolving an import does not make every exported value static and does not execute arbitrary runtime code.
Compile-time preparation may use exports with available static values and capabilities. Runtime-only functions
remain runtime-only under every spelling: `[% {pn.liveValue}]` is a stage error if liveValue requires Runtime.
Static type constructors, `[% ...]`, and `[Assert ... ...]` use imported bindings under the usual checking rules.

If a package needs runtime initializers, the VM initializes it once per execution context before dependent code
uses its exports, in dependency order. Multiple imports or aliases do not repeat initialization. Static
preparation and runtime execution have separate environments; an import does not transfer mutable host state
between them. Any initialization effects require explicit host capabilities and follow their stage requirements.
Import forms themselves are declarations, not effect-sequencing instructions.

## Resolution and package identity

The general compiler asks an injected resolver for a package interface and its source/compiled modules. It knows
no Render Lab manifests, content keys, renderer types, or GPU objects. The host/build configuration supplies the
available dependencies; `[Import ...]` neither installs packages nor silently downloads them.

Resolve dependencies to canonical package identities and exact versions/content hashes before execution. All
references to a package in one program must agree on that resolved identity; incompatible version requirements
are an error in v1. Version-selection syntax, package manifests, lockfile schema, registry/distribution tooling,
and side-by-side versions are separate package-manager work. Source aliases do not select a different version.

Compile each resolved module once per compilation and initialize each package once per relevant execution
context, including when reached through a diamond dependency graph. Cache keys include canonical package/module
identities, source hashes, compiler version, public type/capability interfaces, and imported dependencies. Reject
missing packages, unknown/private exports, incompatible artifacts, and import cycles with source diagnostics.
V1 rejects cyclic package/module dependencies rather than exposing partially initialized exports.

## std and Render Lab

`std` is an ordinary language package. Standalone consumer examples explicitly use `[Import* std]` to put the STL
names in scope, or `[Import std]` to use qualified names. Library implementation blocks within `std` refer directly
to their own definitions. An automatic prelude, if provided later, must be an explicit language/host policy.

Render Lab provides `rlstd` through the same interface:

```text
[ImportAs rlstd rl]
{rl.colored "Hello" {rl.rgb 255 128 0}}
```

The proposed `rlstd` public root API re-exports its presentation constructors and types; it can also expose
submodule namespaces. Its lab adapter maps built resource keys to package/module artifacts and enforces the lab's
manifest imports/exports. Lab resource permission checks and language symbol visibility are separate layers;
neither bypasses the other. A standalone host may resolve another package using entirely different storage.
See the [rlstd library design](rlstd-language.md) for the adapter boundary.

## Implementation and validation

Add these forms to the grammar and definition-context checker, then implement package resolution, public export
tables, namespace/alias bindings, wildcard conflict checking, and qualified lookup before runtime bytecode binding.
Preserve stable imported definition identities in source maps and compiled artifacts. This is language-core work;
the Render Lab resolver adapter comes afterward.

Required cases include all three import forms; hyphenated names and qualified operators; private/unknown exports;
submodule visibility; wildcard conflicts; duplicate imports; multiple aliases; lexical shadowing and source-order
visibility; unchanged closure capture; imported function/type identity and stages; runtime initialization once
through aliases and diamond dependencies; incompatible versions/artifacts; and cycle rejection. Resolver tests
use in-memory packages without Render Lab, network access, GPU, or TTS. No interpreter or package-manager
implementation is created by this document.
