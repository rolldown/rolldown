# Constructing AST

## Summary

Rolldown synthesizes oxc AST nodes in many places — module finalizers, the scanner's pre-processing, HMR, and plugins. Historically it did so through several competing idioms (a hand-maintained `AstSnippet` facade, raw `oxc::ast::AstBuilder`, construction-flavored extension traits, and `..Foo::dummy(alloc)` struct-update literals). oxc has since made `AstBuilder` the single sanctioned construction path (`#[non_exhaustive]` on every `NodeId`-bearing node, oxc 0.135 / [oxc#23046](https://github.com/oxc-project/oxc/pull/23046)), which deleted the struct-literal idiom outright, and then ([oxc#23043](https://github.com/oxc-project/oxc/issues/23043), oxc 0.138) moved construction onto the AST types themselves as per-type associated functions that take the builder/allocator as their **last** argument.

Rolldown now follows that same shape end to end:

- **Generic nodes** are built with oxc's per-type constructors directly — `Expression::new_identifier(SPAN, name, builder)`, `StaticMemberExpression::boxed(.., builder)`, `oxc::allocator::Vec::new_in(builder)` — passing an `AstBuilder` (or any `GetAstBuilder` + `GetAllocator` holder) as the last argument.
- **Rolldown's own recurring constructions** are `new_*` associated functions on rolldown-owned **extension traits**, one per node type they produce (`ExpressionFactoryExt`, `StatementFactoryExt`, `MemberExpressionFactoryExt`, `CallExpressionFactoryExt`, `BindingIdentifierFactoryExt`, `IdentifierNameFactoryExt`, `ObjectPropertyKindFactoryExt`, `ClassElementFactoryExt`). They too take the builder last: `Expression::new_id_ref_expr(SPAN, name, builder)`, `Statement::new_commonjs_wrapper_stmt(.., builder)`.

There is no rolldown wrapper type around `AstBuilder` anymore. Rolldown holds and passes oxc's `AstBuilder` directly. This document records that decision and the reasoning.

See also `internal-docs/ast-mutation/implementation.md` for the span/`NodeId`-as-identity contract that constrains synthesized nodes.

## Prior state

Before this convention, the same kind of node could be built four different ways, and the entry points overlapped:

- **`AstSnippet`** (`crates/rolldown_ecmascript_utils/src/ast_snippet.rs`, ~1030 lines, ~50 methods). A wrapper around `AstBuilder` that mixed two unrelated jobs: thin renames of single `AstBuilder` calls (`id_ref_expr`, the `call_expr_with_*` family, `string_literal_expr`, …) and genuine multi-node rolldown patterns (`wrap_with_to_esm`, `commonjs_wrapper_stmt`, the `.then` chains). Its naming was sprawling and undiscoverable — the call-expression family alone encoded arg-count/return-shape into a suffix matrix (`call_expr_with_arg_expr` vs `_with_arg_expr_expr` vs `_with_2arg_expr_expr` …). The author already flagged the type name as a compromise:

  ```rust
  // `AstBuilder` is more suitable name, but it's already used in oxc.
  pub struct AstSnippet<'ast> {
    pub builder: AstBuilder<'ast>,
  }
  ```

- **The `pub builder` escape hatch.** Because `AstSnippet::builder` was public, roughly half of all AstSnippet interactions bypassed the helpers and reached straight through to raw `AstBuilder` (~219 `snippet.builder.*` call sites vs. ~196 named-helper calls). The facade coexisted with the thing it wrapped instead of encapsulating it.

- **Ad-hoc `AstBuilder` access.** Construction-flavored extension traits (`crates/rolldown_ecmascript_utils/src/extensions/ast_ext/`) that only receive `&Allocator` built a fresh `AstBuilder::new(alloc)` inline — a third way to obtain a builder, alongside `self.ast`-style fields and `snippet.builder`.

- **`..Foo::dummy(alloc)` struct-update literals.** Previously the most direct way to spell a node. oxc 0.135's `#[non_exhaustive]` made this uncompilable for any `NodeId`-bearing node; [#9670](https://github.com/rolldown/rolldown/pull/9670) migrated the ~26 affected sites onto `AstBuilder` constructors. The remaining `::dummy()` sites are on non-node types (options/config) and are unaffected.

- **Parse-from-source-string.** Not all AST is built — some is authored as JS source and parsed via `EcmaCompiler::parse` (`crates/rolldown_ecmascript/src/ecma_compiler.rs`), which parses a source string into a standalone `EcmaAst` with its own allocator. On the output side this is essentially just the runtime module (`crates/rolldown/src/module_loader/runtime_module_task.rs`). The direct oxc `Parser::new` sites in plugins and scanner sub-analyzers parse _input_ source to analyze or transform it — a different activity from constructing rolldown's own AST.

The `AstSnippet` facade was first replaced (oxc#23043 cutover, oxc 0.138) by a thin rolldown newtype, `AstFactory`, wrapping `AstBuilder`. That newtype has since been removed too (see [Migration](#migration)); construction now lives on the AST types, matching oxc.

Two facts constrain every choice and are documented in [ast-mutation](../ast-mutation/implementation.md): synthesized nodes must carry the reserved synthetic span (`SPAN`, `0..0`) — the cross-pass side tables are `NodeId`-keyed now, so the span no longer prevents false matches, but `span.is_unspanned()` checks (such as the global-`require` rewrite guard in `crates/rolldown/src/module_finalizers/mod.rs`) still use it to tell synthesized nodes from scanned ones — and rolldown does not re-run semantic after finalize, so synthesized nodes keep a dummy `NodeId` for life; that dummy id is what keeps them from matching scan-time records.

## The convention

Pick the tool by what you are building. In both cases the thing you build _through_ is a value implementing oxc's `GetAstBuilder` + `GetAllocator` — an `AstBuilder`, or any context that holds one — passed as the **last** argument.

### Generic nodes → oxc's per-type constructors

Since oxc#23043 (oxc 0.138), construction lives on the AST types themselves as per-type associated functions: `Expression::new_call_expression(.., builder)`, `StaticMemberExpression::boxed(.., builder)`, `oxc::allocator::Vec::new_in(builder)`, `oxc::ast::ast::Str::from_str_in(s, builder)`.

```rust
// A member expression, built with oxc's per-type constructor, builder passed last.
let member = StaticMemberExpression::boxed(SPAN, object, property, false, builder);
```

The naming maps mechanically: `alloc_X` → `X::boxed`, a plain value constructor `x` → `X::new`, and an enum constructor → `Enum::new_<variant>` (e.g. `Expression::new_call_expression` builds `Expression::CallExpression`). oxc's constructors are positional; preface a verbose chunk with a comment showing the JS it produces, as oxc itself recommends.

### Rolldown-specific patterns → `new_*` associated functions on extension traits

For constructions that compose several nodes into a recurring rolldown convention (CJS/ESM interop wrappers, `__toESM` / `__toCommonJS` calls, `.then` chains, …), add a `new_*` associated function to the extension trait for the node type it produces, rather than open-coding it at the call site. All the traits live in one module (`crates/rolldown_ecmascript_utils/src/ast_factory.rs`) and are re-exported from the crate root, so a call site pulls in the ones it needs with a single `use`, imported `as _`:

```rust
use rolldown_ecmascript_utils::{ExpressionFactoryExt as _, StatementFactoryExt as _};

let stmt = Statement::new_commonjs_wrapper_stmt(binding_name, /* … */, builder);
let id_ref = Expression::new_id_ref_expr(SPAN, name, builder);
```

Rolldown can't add inherent methods to oxc's foreign types, so these live on traits. They're imported `as _` — the `new_*` methods are all a call site needs, the trait name never appears — which also keeps them from clashing with the same-typed inspection traits (`ExpressionExt`, …).

These functions:

- are prefixed **`new_`** and named after the **operation** (`new_to_esm_wrapper`), never after a bare AST node;
- are **associated functions** (no `self`) generic over `B: GetAstBuilder<'ast> + GetAllocator<'ast>`, taking the builder as the **last** argument, exactly like oxc's own per-type constructors. A caller-provided `span` comes first as in oxc, but most `new_*` patterns synthesize nodes with the reserved `SPAN` internally and take no span;
- live on the extension trait for the node type they **return** (`Expression::new_*` on `ExpressionFactoryExt`, `Statement::new_*` on `StatementFactoryExt`, …). Private multi-step helpers that don't warrant a public entry point are free functions in the same module.

A function earns a place here only if it encodes a multi-step rolldown convention that is wrong-by-default when open-coded — not merely to shorten one oxc call.

### Build programmatically by default; parsing source is an exception

Construct nodes programmatically (oxc per-type constructors, rolldown patterns via `new_*`). This is the default for **all** node construction, including code rolldown emits, because direct construction has no runtime cost whereas parsing a source string pays lexing + parsing overhead on every build.

Authoring code as JS source and parsing it (`EcmaCompiler::parse`) is reserved for a large, fixed body of code where maintaining it as real JS clearly outweighs the one-time parse cost. In practice that is the **runtime module** (`crates/rolldown/src/module_loader/runtime_module_task.rs`) and essentially nothing else on the output side. Never parse for nodes that splice into an existing AST and need a synthetic `SPAN` + dummy `NodeId` — build those programmatically, per the constraint above.

### Read-only inspection → `as_*` / `is_*`

Keep read-only inspection helpers (on `ExpressionExt`, `CallExpressionExt`, `StatementExt`, …) separate from construction. Construction traits are named `*FactoryExt` and carry only `new_*`; inspection traits carry only `as_*` / `is_*`.

## Naming: `new_` + operation, never variant

rolldown's constructors share oxc's `new_` prefix — they read as constructors and match oxc's own convention (the oxc maintainer who proposed this shape uses it too). Two rules keep them coherent:

- **Name after the operation, not the node.** An oxc per-type constructor is named after the node it produces (`Expression::new_call_expression`, a noun); a rolldown convention is named after the operation it performs (`Expression::new_to_esm_wrapper`, a verb). A reader tells them apart by the noun-vs-verb name — and rolldown's are the ones reached through an imported `*FactoryExt` trait.
- **The operation names are what keep the two disjoint.** rolldown's functions are trait associated functions on oxc's types, so a name that collided with an oxc _inherent_ constructor would be silently shadowed by oxc's. Sharing the `new_` prefix, rolldown relies on the operation-vs-variant split to stay clear: no rolldown operation name (`new_keep_name_call`, `new_re_export_call`, …) matches an oxc variant name. This is a discipline, not the guarantee the old `make_` prefix gave — when adding a `new_*` helper pick a name oxc won't use; a signature-incompatible clash fails to compile, and the snapshot suite catches anything subtler.

## Why methods-on-types, and no wrapper

The earlier design funnelled all construction through a single rolldown newtype (`AstFactory`) wrapping `AstBuilder`, on the theory that it would be an insulation boundary around oxc's construction API. In practice that insulation was thin — generic-node construction already names oxc's types at every call site (`Expression::new_call_expression`, `StaticMemberExpression::boxed`, …), so the newtype only ever centralized the `new_*` patterns and the handle _type_. Adopting oxc's own methods-on-types shape gives that up and buys more than it costs:

- **It matches oxc.** Construction lives in the same place for oxc and rolldown nodes — on the AST types, builder passed last. The only rolldown-visible difference is that rolldown's helpers are operation-named and reached through `use … as _` trait imports. The `new_*` patterns still live in one module, so the "single place to absorb an oxc construction-API change" property is preserved for the part that actually needs it.
- **Builder-last unblocks `&mut` construction.** oxc intends to switch builder methods to take `&mut AstBuilder` (to drop the `Cell`s from `Allocator` — a hot-path win — and to enable stateful builders that assign unique `NodeId`s automatically). Passing the builder as an argument rather than as a `self` receiver is what makes that ergonomic: `Expression::new_id_ref_expr(SPAN, self.gen_name(), self)` borrow-checks where the old receiver form `self.ast_factory.make_id_ref_expr(SPAN, self.gen_name())` would not, because the argument is evaluated before the trailing `self` borrow. The old newtype-as-receiver design was the specific blocker here.
- **Statefulness is deferred, not lost.** If rolldown later needs a stateful builder (e.g. to auto-assign `NodeId`s), it reintroduces a wrapper type that implements `GetAstBuilder` + `GetAllocator` and threads it as the builder argument. Because every `new_*` and every oxc constructor is generic over the builder, that swap costs **zero** call-site changes — strictly more flexible than the `&self`-newtype it replaced.

So the move is toward oxc's shape, keeping the one genuinely useful property of the old design (rolldown patterns in one module) while shedding the property that actively blocked oxc's next steps.

## Migration

The convention arrived in two steps:

1. **`AstSnippet` → `AstFactory` newtype (oxc#23043 cutover, oxc 0.138).** `AstSnippet` became a newtype over `AstBuilder`: its `pub builder` field became the wrapped builder, the thin renames were dropped in favor of oxc's per-type constructors, and the genuine patterns became inherent `make_*` methods. Every generic-node call site moved to the per-type constructors, and oxc_ast's **`disable_old_builder`** cargo feature was enabled, removing the deprecated `AstBuilder` methods (and dropping `AstBuilder`'s `Clone`/`Copy`, and the top-level `oxc::ast::{AstBuilder, NONE}` re-exports — import from `oxc::ast::builder::` instead).

2. **`AstFactory` newtype → `new_*` on extension traits (this change).** The inherent `make_*` methods on `AstFactory` became `new_*` associated functions on per-node-type `*FactoryExt` traits, generic over `B: GetAstBuilder + GetAllocator`, taking the builder last; the private multi-step helpers became free functions in the same module. The two construction-flavored binding ext traits (`BindingPatternExt`, `BindingPropertyExt`) were made generic over the builder the same way, decoupling them from the deleted type. Every holder struct's `ast_factory: AstFactory` field became `ast_builder: AstBuilder`, and every `self.ast_factory.make_x(..)` call site became `Type::new_x(.., &self.ast_builder)`. The `AstFactory` newtype was deleted.

`disable_old_builder` remains enabled and pinned via a direct `oxc_ast` dependency in `crates/rolldown_ecmascript/Cargo.toml` (cargo-shear-ignored; feature unification applies it to the copy the umbrella re-exports). Keep that pin in lockstep with the `oxc` version when upgrading.

## Related

- [ast-mutation](../ast-mutation/implementation.md) — the span/`NodeId`-as-identity contract that constrains synthesized nodes
- [runtime-helpers](../runtime-helpers/implementation.md) — the runtime functions that `new_*` interop constructors emit calls to
