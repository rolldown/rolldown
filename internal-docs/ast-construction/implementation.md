# Constructing AST

## Summary

Rolldown synthesizes oxc AST nodes in many places — module finalizers, the scanner's pre-processing, HMR, and plugins. Historically it did so through several competing idioms (a hand-maintained `AstSnippet` facade, raw `oxc::ast::AstBuilder`, construction-flavored extension traits, and `..Foo::dummy(alloc)` struct-update literals). oxc has since made `AstBuilder` the single sanctioned construction path (`#[non_exhaustive]` on every `NodeId`-bearing node, oxc 0.135 / [oxc#23046](https://github.com/oxc-project/oxc/pull/23046)), which deleted the struct-literal idiom outright.

Going forward rolldown routes **all** construction through a single rolldown-owned newtype, **`AstFactory`**, which wraps oxc's `AstBuilder` (implementing `GetAstBuilder` / `GetAllocator` so it can be passed to oxc's per-type node constructors) and adds rolldown's own recurring constructions as inherent `make_*` methods. Funnelling everything through one rolldown type — rather than calling oxc's `AstBuilder` directly at each site — is what let rolldown absorb the oxc `AstBuilder` redesign ([oxc#23043](https://github.com/oxc-project/oxc/issues/23043), oxc 0.138) at a single point. This document records that decision and the reasoning.

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

Two facts constrain every choice and are documented in [ast-mutation](../ast-mutation/implementation.md): synthesized nodes must carry the reserved synthetic span (`SPAN`, `0..0`) — the cross-pass side tables are `NodeId`-keyed now, so the span no longer prevents false matches, but `span.is_unspanned()` checks (such as the global-`require` rewrite guard in `crates/rolldown/src/module_finalizers/mod.rs`) still use it to tell synthesized nodes from scanned ones — and rolldown does not re-run semantic after finalize, so synthesized nodes keep a dummy `NodeId` for life; that dummy id is what keeps them from matching scan-time records.

## The convention

Everything goes through one handle, `ast_factory: AstFactory<'a>` — rolldown's newtype over `oxc::ast::AstBuilder`. Pick the tool by what you are building:

### Generic nodes → oxc's per-type constructors, passing the `ast_factory` handle

Since oxc#23043 (oxc 0.138), construction lives on the AST types themselves as per-type associated functions that take the builder/allocator as the **last** argument: `Expression::new_call_expression(.., gen)`, `StaticMemberExpression::boxed(.., gen)`, `oxc::allocator::Vec::new_in(gen)`, `oxc::ast::ast::Str::from_str_in(s, gen)`. `AstFactory` implements oxc's `GetAstBuilder` and `GetAllocator`, so the `ast_factory` handle _is_ the generator you pass:

```rust
// before (pre-0.138): a method on the builder, reached through Deref
let member = ast_factory.alloc_static_member_expression(SPAN, object, property, false);

// after: the per-type constructor, with the handle passed last
let member = StaticMemberExpression::boxed(SPAN, object, property, false, &ast_factory);
```

Inside an `&self` method that holds the handle, pass `self` directly (it implements the traits); from a value handle pass `&ast_factory` / `&self.ast_factory`. The naming maps mechanically: `alloc_X` → `X::boxed`, a plain value constructor `x` → `X::new`, and an enum constructor → `Enum::new_<variant>` (e.g. `expression_call` built `Expression::CallExpression`, so it is `Expression::new_call_expression`). Don't construct an `AstFactory` / `AstBuilder` ad hoc when a handle is already in scope. oxc's constructors are positional; preface a verbose chunk with a comment showing the JS it produces, as oxc itself recommends.

### Rolldown-specific patterns → inherent `make_*` methods on `AstFactory`

For constructions that compose several nodes into a recurring rolldown convention (CJS/ESM interop wrappers, `__toESM` / `__toCommonJS` calls, `.then` chains, …), add an inherent `make_*` method to the `AstFactory` newtype rather than open-coding it at the call site:

```rust
#[derive(Clone, Copy)]
pub struct AstFactory<'a>(oxc::ast::AstBuilder<'a>);

// generic oxc constructors reach the handle through these traits
impl<'a> GetAllocator<'a> for AstFactory<'a> {
  fn allocator(&self) -> &'a Allocator { self.0.allocator() }
}
impl<'a> GetAstBuilder<'a> for AstFactory<'a> {
  type Builder = AstBuilder<'a>;
  fn builder(&self) -> &AstBuilder<'a> { &self.0 }
}

impl<'a> AstFactory<'a> {                     // rolldown's own patterns
  pub fn make_to_esm_wrapper(&self, namespace: Expression<'a>) -> Expression<'a> { /* ... */ }
  pub fn make_commonjs_wrapper(&self, /* ... */) -> Statement<'a> { /* ... */ }
}
```

These methods:

- are prefixed **`make_`** and named after the **operation** (`make_to_esm_wrapper`), never after a bare AST node;
- mirror oxc's builder signature style: positional args, `make_<x>` returns a value and `make_alloc_<x>` returns a boxed node. A caller-provided `span` comes first as in oxc, but most `make_*` patterns synthesize nodes with the reserved `SPAN` internally and take no span. They take **`&self`** and pass `self` as the generator to oxc's per-type constructors (`self` implements `GetAstBuilder` / `GetAllocator`). `&self` keeps the **call sites** independent of `Copy` — the handle is borrowed, never moved, so reusing it after a `make_*` call always compiles.

A method earns a place here only if it encodes a multi-step rolldown convention that is wrong-by-default when open-coded — not merely to shorten one oxc call.

### Build programmatically by default; parsing source is an exception

Construct nodes through the `ast_factory` handle (oxc constructors via `Deref`, rolldown patterns via `make_*`). This is the default for **all** node construction, including code rolldown emits, because direct construction has no runtime cost whereas parsing a source string pays lexing + parsing overhead on every build.

Authoring code as JS source and parsing it (`EcmaCompiler::parse`) is reserved for a large, fixed body of code where maintaining it as real JS clearly outweighs the one-time parse cost. In practice that is the **runtime module** (`crates/rolldown/src/module_loader/runtime_module_task.rs:226`) and essentially nothing else on the output side — treat it as a special case, not a tool to reach for. Never parse for nodes that splice into an existing AST and need a synthetic `SPAN` + dummy `NodeId` — build those programmatically, per the constraint above.

### Read-only inspection → `as_*` / `is_*`

Keep read-only inspection helpers separate from construction; they are not methods on `AstFactory`.

## Why `make_` + operation names

The prefix is not decoration — it does two jobs:

- **Every call site self-identifies.** A generic node is an oxc per-type constructor (`Expression::new_call_expression(.., &ast_factory)`); a `make_*` name (`ast_factory.make_to_esm_wrapper(..)`) is a rolldown method on `AstFactory`. The distinction is carried by naming: oxc constructors are named after the node they produce (nouns), rolldown's after the operation they perform (verbs).
- **It keeps rolldown's additions in their own namespace.** The `make_` prefix means an inherent `AstFactory` method never collides with an oxc constructor name, and a reader always knows whether a construction is a generic oxc node or a rolldown convention.

The handle is spelled out as `ast_factory` rather than a bare `ast`: it reads unambiguously as an instance of `AstFactory`, and isn't visually confused with oxc's `ast` module that some files import.

## Forward compatibility: one chokepoint for oxc's construction API

The deeper reason to do this — independent of any specific oxc change — is that funnelling all construction through a single rolldown-owned newtype (`AstFactory`, wrapping oxc's `AstBuilder`) turns that type into an **insulation boundary** around oxc's construction API. oxc's construction surface has moved repeatedly: `#[non_exhaustive]` landed in 0.135 (oxc#23046, itself part of a stack of AST-macro reorganizations), and oxc#23043 redesigned `AstBuilder` wholesale in 0.138. Whatever oxc does next, the blast radius is confined to that one layer instead of being smeared across hundreds of call sites. (This insulates the _construction API_ — method names, signatures, the builder type — not oxc's AST node types themselves, which flow through rolldown everywhere and can't be wrapped away.)

Concretely:

- **oxc#23043 landed cheaply (oxc 0.138).** It moved construction from `builder.alloc_foo(span, …)` to per-type constructors taking the generator last (`Foo::boxed(span, …, gen)`), with automatic `NodeId` assignment — explicitly citing rolldown [#9609](https://github.com/rolldown/rolldown/pull/9609). Because one rolldown newtype was already threaded everywhere, adopting it was a localized change: `AstFactory` implements `GetAstBuilder` + `GetAllocator`, the per-type `Foo::new(.., &ast_factory)` / `Foo::boxed(..)` constructors take it directly, the `make_*` bodies pass `self`, and the `Deref` to oxc's `AstBuilder` was dropped. (The generic-node call sites did change spelling — `ast_factory.alloc_foo(..)` → `Foo::boxed(.., &ast_factory)` — but each is the same node, and the `make_*` call sites were untouched.)
- **Even removing `AstBuilder` stays contained.** If oxc ever drops or reshapes the builder entirely, rolldown re-hosts construction at this single point — `AstFactory` provides the surface itself (or impls whatever new generator trait oxc introduces) — and the call sites, all typed through `AstFactory`, stay put. The unification is precisely what makes that possible: you cannot absorb an upstream change at one point when construction is spread across four idioms and hundreds of direct sites bound to oxc's type.

So the unification is what limited the cost of oxc's flux. The one ergonomic problem oxc's redesign does **not** solve is the verbosity of positional arguments, which is the remaining justification for a thin local layer: kept to genuine rolldown patterns and aligned with oxc's style rather than diverging into its own taxonomy.

## Migration

The convention arrived incrementally, but the oxc#23043 cutover (oxc 0.138) was a single sweep:

- `AstSnippet` became the `AstFactory` newtype: its `pub builder` field became the wrapped `AstBuilder`; the thin renames were dropped in favor of oxc constructors; the genuine patterns became inherent `make_*` methods. The awkward `AstSnippet` name disappears — rolldown now owns a properly-named builder.
- The oxc 0.138 builder redesign was migrated in one pass: every generic-node call site moved from the (then-deprecated) `ast_factory.<builder_method>(..)` form to the per-type constructors (`Foo::new(.., &ast_factory)` / `Foo::boxed(..)` / `oxc::allocator::Vec::new_in(..)` / `Str::from_str_in(..)`), `AstFactory` gained the `GetAstBuilder` / `GetAllocator` impls, and the `Deref` was removed. New code follows the per-type convention directly.

## Related

- [ast-mutation](../ast-mutation/implementation.md) — the span/`NodeId`-as-identity contract that constrains synthesized nodes
- [runtime-helpers](../runtime-helpers/implementation.md) — the runtime functions that `make_*` interop constructors emit calls to
