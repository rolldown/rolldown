# Constructing AST

## Summary

Rolldown synthesizes oxc AST nodes in many places — module finalizers, the scanner's pre-processing, HMR, and plugins. Historically it did so through several competing idioms (a hand-maintained `AstSnippet` facade, raw `oxc::ast::AstBuilder`, construction-flavored extension traits, and `..Foo::dummy(alloc)` struct-update literals). oxc has since made `AstBuilder` the single sanctioned construction path (`#[non_exhaustive]` on every `NodeId`-bearing node, oxc 0.135 / [oxc#23046](https://github.com/oxc-project/oxc/pull/23046)), which deleted the struct-literal idiom outright.

Going forward rolldown routes **all** construction through a single rolldown-owned newtype, **`AstFactory`**, which wraps oxc's `AstBuilder` (deref-ing to it for the generic node constructors) and adds rolldown's own recurring constructions as inherent `make_*` methods. Funnelling everything through one rolldown type — rather than calling oxc's `AstBuilder` directly at each site — is also what lets rolldown absorb future oxc construction-API changes at a single point. This document records that decision and the reasoning, so future work (and the upcoming oxc `AstBuilder` redesign, [oxc#23043](https://github.com/oxc-project/oxc/issues/23043)) has a baseline.

## Prior state

Before this convention, the same kind of node could be built four different ways, and the entry points overlapped:

- **`AstSnippet`** (`crates/rolldown_ecmascript_utils/src/ast_snippet.rs`, ~1030 lines, ~50 methods). A wrapper around `AstBuilder` that mixes two unrelated jobs: thin renames of single `AstBuilder` calls (`id_ref_expr`, the `call_expr_with_*` family, `string_literal_expr`, …) and genuine multi-node rolldown patterns (`wrap_with_to_esm`, `commonjs_wrapper_stmt`, the `.then` chains). Its naming is sprawling and undiscoverable — the call-expression family alone encodes arg-count/return-shape into a suffix matrix (`call_expr_with_arg_expr` vs `_with_arg_expr_expr` vs `_with_2arg_expr_expr` …). The author already flagged the type name as a compromise:

  ```rust
  // crates/rolldown_ecmascript_utils/src/ast_snippet.rs
  // `AstBuilder` is more suitable name, but it's already used in oxc.
  pub struct AstSnippet<'ast> {
    pub builder: AstBuilder<'ast>,
  }
  ```

- **The `pub builder` escape hatch.** Because `AstSnippet::builder` is public, roughly half of all AstSnippet interactions bypass the helpers and reach straight through to raw `AstBuilder` (~219 `snippet.builder.*` call sites vs. ~196 named-helper calls). The facade coexists with the thing it wraps instead of encapsulating it.

- **Ad-hoc `AstBuilder` access.** Construction-flavored extension traits (`crates/rolldown_ecmascript_utils/src/extensions/ast_ext/`) that only receive `&Allocator` build a fresh `AstBuilder::new(alloc)` inline — a third way to obtain a builder, alongside `self.ast`-style fields and `snippet.builder`.

- **`..Foo::dummy(alloc)` struct-update literals.** Previously the most direct way to spell a node. oxc 0.135's `#[non_exhaustive]` makes this uncompilable for any `NodeId`-bearing node; [#9670](https://github.com/rolldown/rolldown/pull/9670) migrated the ~26 affected sites (in `module_finalizers/` and the `ast_ext` traits) onto `AstBuilder` constructors. The remaining `::dummy()` sites are on non-node types (options/config) and are unaffected.

- **Parse-from-source-string.** Not all AST is built — some is authored as JS source and parsed via `EcmaCompiler::parse` (`crates/rolldown_ecmascript/src/ecma_compiler.rs`), which parses a source string into a standalone `EcmaAst` with its own allocator. On the output side this is essentially just the runtime module (`crates/rolldown/src/module_loader/runtime_module_task.rs:226`). The ~35 direct oxc `Parser::new` sites in plugins and scanner sub-analyzers parse _input_ source to analyze or transform it — a different activity from constructing rolldown's own AST.

Two facts constrain every choice and are documented in [ast-mutation](./ast-mutation.md): synthesized nodes must carry the reserved synthetic span (`SPAN`, `0..0`) — the cross-pass side tables are `NodeId`-keyed now, so the span no longer prevents false matches, but `span.is_unspanned()` checks (such as the global-`require` rewrite guard in `crates/rolldown/src/module_finalizers/mod.rs`) still use it to tell synthesized nodes from scanned ones — and rolldown does not re-run semantic after finalize, so synthesized nodes keep a dummy `NodeId` for life; that dummy id is what keeps them from matching scan-time records.

## The convention

Everything goes through one handle, `ast_factory: AstFactory<'a>` — rolldown's newtype over `oxc::ast::AstBuilder`. Pick the tool by what you are building:

### Generic nodes → the `ast_factory` handle (oxc's builder, via `Deref`)

`AstFactory` derefs to the wrapped builder, so every oxc constructor is callable directly on `ast_factory`. The thin `AstSnippet` renames collapse to those oxc calls:

```rust
// before: an AstSnippet wrapper method
let member = self.snippet.builder.alloc_static_member_expression(SPAN, object, property, false);

// after: the same oxc constructor, on the `ast_factory` handle (resolved through Deref)
let member = ast_factory.alloc_static_member_expression(SPAN, object, property, false);
```

Don't construct an `AstFactory` / `AstBuilder` ad hoc when a handle is already in scope, and don't reach for raw `oxc::allocator::Vec` / `Box` when the builder already offers `ast_factory.vec*` / `ast_factory.alloc_*`. oxc's constructors are positional; preface a verbose chunk with a comment showing the JS it produces, as oxc itself recommends.

### Rolldown-specific patterns → inherent `make_*` methods on `AstFactory`

For constructions that compose several nodes into a recurring rolldown convention (CJS/ESM interop wrappers, `__toESM` / `__toCommonJS` calls, `.then` chains, …), add an inherent `make_*` method to the `AstFactory` newtype rather than open-coding it at the call site:

```rust
#[derive(Clone, Copy)]
pub struct AstFactory<'a>(oxc::ast::AstBuilder<'a>);

impl<'a> Deref for AstFactory<'a> {          // generic oxc constructors, no boilerplate
  type Target = oxc::ast::AstBuilder<'a>;
  fn deref(&self) -> &Self::Target { &self.0 }
}

impl<'a> AstFactory<'a> {                     // rolldown's own patterns
  pub fn make_to_esm_wrapper(&self, namespace: Expression<'a>) -> Expression<'a> { /* ... */ }
  pub fn make_commonjs_wrapper(&self, /* ... */) -> Statement<'a> { /* ... */ }
}
```

These methods:

- are prefixed **`make_`** and named after the **operation** (`make_to_esm_wrapper`), never after a bare AST node;
- mirror oxc's builder signature style: positional args, `make_<x>` returns a value and `make_alloc_<x>` returns a boxed node. A caller-provided `span` comes first as in oxc, but most `make_*` patterns synthesize nodes with the reserved `SPAN` internally and take no span. They take **`&self`** and reach the wrapped builder through `Deref` (`self.foo()`, never `self.0.foo()`). `&self` keeps the **call sites** independent of `Copy` — the handle is borrowed, never moved, so reusing it after a `make_*` call always compiles. The method **bodies** still lean on today's `Copy` builder (Deref yields `&AstBuilder` and oxc's value-taking constructors copy it back out); when oxc#23043 lands — per-type constructors taking the generator by reference, `AstFactory` impl `AstGenerator`, the `Deref` dropped — the bodies move onto that API while the `&self` call sites stay unchanged.

A method earns a place here only if it encodes a multi-step rolldown convention that is wrong-by-default when open-coded — not merely to shorten one oxc call.

### Build programmatically by default; parsing source is an exception

Construct nodes through the `ast_factory` handle (oxc constructors via `Deref`, rolldown patterns via `make_*`). This is the default for **all** node construction, including code rolldown emits, because direct construction has no runtime cost whereas parsing a source string pays lexing + parsing overhead on every build.

Authoring code as JS source and parsing it (`EcmaCompiler::parse`) is reserved for a large, fixed body of code where maintaining it as real JS clearly outweighs the one-time parse cost. In practice that is the **runtime module** (`crates/rolldown/src/module_loader/runtime_module_task.rs:226`) and essentially nothing else on the output side — treat it as a special case, not a tool to reach for. Never parse for nodes that splice into an existing AST and need a synthetic `SPAN` + dummy `NodeId` — build those programmatically, per the constraint above.

### Read-only inspection → `as_*` / `is_*`

Keep read-only inspection helpers separate from construction; they are not methods on `AstFactory`.

## Why `make_` + operation names

The prefix is not decoration — it does two jobs:

- **Every call site self-identifies.** A bare node name (`ast_factory.call_expression(..)`) reaches oxc's builder through `Deref`; a `make_*` name (`ast_factory.make_to_esm_wrapper(..)`) is a rolldown method on `AstFactory`. Rust doesn't mark the two differently at the call site, so the distinction is carried by naming: oxc methods are named after the node they produce (nouns), rolldown's after the operation they perform (verbs).
- **It prevents accidental shadowing.** Inherent methods on `AstFactory` take priority over the oxc methods reached through `Deref`. Naming a rolldown method after a bare node (e.g. `call_expression`) would silently override oxc's — occasionally that is the deliberate way to absorb an upstream change, but as an accident it's a trap. The `make_` prefix keeps rolldown's additions in their own namespace, so any override is intentional.

The handle is spelled out as `ast_factory` rather than a bare `ast`: it reads unambiguously as an instance of `AstFactory`, and isn't visually confused with oxc's `ast` module that some files import.

## Forward compatibility: one chokepoint for oxc's construction API

The deeper reason to do this now — independent of any specific oxc change — is that funnelling all construction through a single rolldown-owned newtype (`AstFactory`, wrapping oxc's `AstBuilder`) turns that type into an **insulation boundary** around oxc's construction API. oxc's construction surface is still actively moving: `#[non_exhaustive]` landed in 0.135 (oxc#23046, itself part of a stack of AST-macro reorganizations), and oxc#23043 will redesign `AstBuilder` wholesale. Whatever oxc does next, the blast radius is confined to that one layer instead of being smeared across hundreds of call sites. (This insulates the _construction API_ — method names, signatures, the builder type — not oxc's AST node types themselves, which flow through rolldown everywhere and can't be wrapped away.)

Concretely:

- **oxc#23043 drops in cheaply.** It moves construction from `builder.alloc_foo(span, …)` to per-type constructors taking the generator last (`Foo::boxed(span, …, gen)`), behind an `AstGenerator` trait, with automatic `NodeId` assignment — explicitly citing rolldown [#9609](https://github.com/rolldown/rolldown/pull/9609). With one rolldown newtype already threaded everywhere, adopting it is a localized change: `AstFactory` implements `AstGenerator`, the per-type `Foo::new(.., ast)` constructors work on it directly, and the `Deref` to today's `AstBuilder` is simply dropped — call sites unchanged.
- **Even removing `AstBuilder` stays contained.** If oxc ever drops or reshapes the builder entirely, rolldown re-hosts construction at this single point — `AstFactory` stops deref-ing to oxc's builder and provides the surface itself (or impls `AstGenerator`) — and the call sites, all typed through `AstFactory`, are untouched. The unification is precisely what makes that possible: you cannot absorb an upstream change at one point when construction is spread across four idioms and hundreds of direct sites bound to oxc's type.

So the work is worth doing now _even though_ oxc is still in flux — the unification is what limits the cost of that flux. The one ergonomic problem oxc's own redesign does **not** solve is the verbosity of positional arguments, which is the remaining justification for a thin local layer: kept to genuine rolldown patterns and aligned with oxc's style rather than diverging into its own taxonomy.

## Migration

This is an incremental convention, not a big-bang refactor:

- `AstSnippet` becomes the `AstFactory` newtype: its `pub builder` field becomes the wrapped `AstBuilder` exposed via `Deref`; the thin renames are dropped in favor of the deref'd oxc constructors; the genuine patterns become inherent `make_*` methods. The awkward `AstSnippet` name disappears — rolldown now owns a properly-named builder.
- New code follows the convention immediately; existing sites are migrated opportunistically (the `..::dummy()` cluster was already forced over by #9670).

## Related

- [ast-mutation](./ast-mutation.md) — the span/`NodeId`-as-identity contract that constrains synthesized nodes
- [runtime-helpers](./runtime-helpers.md) — the runtime functions that `make_*` interop constructors emit calls to
