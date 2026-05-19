# AST Mutation Between Passes

## Summary

Rolldown threads per-AST-node metadata between compiler passes via side tables keyed by the oxc `Span` of the node. Scan populates them, Link adds more, and the Finalizer mutates the AST in place by calling `node.span()` and looking the result up. This works today, but it rests on an implicit "spans are stable and unique within a module" contract that has no validation layer and is easy to break. This document describes the current behavior so that a future migration to oxc's upcoming `AstNodeId` has a baseline to compare against — it is not itself a proposal for that migration.

## Pass overview

Rolldown's bundling pipeline has three stages that interact with the AST:

- **Scan** — `ScanStage::scan` (`crates/rolldown/src/stages/scan_stage.rs:159`). Per module: parse, then walk the AST read-only via `AstScanner` to populate `EcmaView` side tables (imports, this-expressions, `new URL(...)` references, etc.). The AST itself is not mutated.
- **Link** — `LinkStage::link` (`crates/rolldown/src/stages/link_stage/mod.rs:229`). Cross-module work — symbol binding, export resolution, tree shaking. Still no AST mutation. Computes additional side tables (most notably `resolved_member_expr_refs`) keyed by spans collected during scan.
- **Generate / Finalize** — `ScopeHoistingFinalizer` (`crates/rolldown/src/module_finalizers/impl_visit_mut.rs:26`), driven from `GenerateStage::generate` (`crates/rolldown/src/stages/generate_stage/mod.rs:82`). The only stage that mutates the AST. Implemented as a `VisitMut` traversal; at each interesting node it calls `node.span()` and queries the side tables to decide what to rewrite.

Between passes, rolldown does **not** hold direct references to AST nodes (lifetimes wouldn't allow it across the parallel + cross-module boundaries anyway). The only thing that survives across passes is the `Span`.

## The Span-as-identity contract

The shared invariant across all passes:

- **Insertion**: scan/link write side-table entries using the `Span` of the AST node being recorded.
- **Lookup**: the finalizer (or other AST walkers) read `node.span()` and query the table.
- **Required guarantees**: each recorded node has a `Span` that is (a) unique within its module and (b) preserved unchanged from the time of insertion to the time of lookup.

There is no `AstNodeId` or other stable identity. The span doubles as both source-position metadata and as a primary key.

### Pre-scan span normalization

The contract above would be unsafe if held over the raw post-parse AST: oxc gives every node a span derived from source position, but identical-looking source can yield identical spans (most often empty / synthetic spans inside the parser's output). Rolldown therefore runs a pre-scan pass — `PreProcessor` in `crates/rolldown/src/utils/tweak_ast_for_scanning.rs` — that walks the AST and, for the node kinds it cares about, rewrites any duplicate span to a fresh empty span (`start == end`, allocated upward from `program.span.end + 1`).

The deduplication is **a targeted subset, not an exhaustive list of all span-keyed node kinds**. `PreProcessor` visits the node kinds where the parser is known to produce duplicate or synthetic spans (e.g., empty spans from desugaring, or kinds that can legitimately overlap):

- `ModuleDeclaration` (import/export decls)
- `ImportExpression` (dynamic `import()`)
- `ThisExpression`
- `CallExpression` whose callee is `require` (and `IdentifierReference`s named `require`)
- `NewExpression`

See `tweak_ast_for_scanning.rs:208-240`. Other node kinds keep whatever span the parser gave them — including `StaticMemberExpression`, which `resolved_member_expr_refs` uses as a key. Member expressions are not deduplicated because their spans cover real source ranges and don't collide in practice. The synthetic `SPAN` (`0..0`) is pre-seeded into the visited set so the deduplicator never produces it as a "unique" replacement — synthetic spans remain reserved for finalizer-generated nodes.

The practical guarantee is narrower than it might appear: **for the node kinds `PreProcessor` visits, spans are unique within a module by the time scan runs.** For any other side-table key — including the member-expression case — uniqueness relies on the parser's own behavior. Adding a new side table whose key is prone to collisions or synthetic spans would need either an entry added to `PreProcessor` or a different identity strategy.

## Address-as-identity (alternative key)

`Span` isn't the only node identity rolldown threads between passes. Some side tables key off oxc's `Address` (the arena pointer obtained via `GetAddress::address` / `UnstableAddress::unstable_address`). Unlike spans, an `Address` is **unique by construction within a live AST** — two distinct allocator-resident nodes can never share one, with no `PreProcessor` involvement needed — but it is only valid for the lifetime of the `Allocator` that owns the AST and is therefore unusable across reparses or after the AST is dropped.

Rolldown uses `Address` where the producer and consumer hold references into the same arena and the table doesn't need to survive the AST going away:

- **`DynamicImportExprInfo.address`** (`crates/rolldown_common/src/types/import_record.rs:22`) — scan records the `ImportExpression`'s address on each dynamic-import record (`ast_scanner/mod.rs:509-510`), and link looks it up during cross-module optimization (`stages/link_stage/cross_module_optimization.rs:328-329`).
- **`side_effect_free_call_expr_addr`** (`stages/link_stage/cross_module_optimization.rs:376`) — link populates a `FxHashSet<Address>` of pure call expressions; `SideEffectDetector` consults it when re-evaluating side effects (`ast_scanner/side_effect_detector/mod.rs:48`).
- **`unreachable_import_expression_addresses`** (`stages/link_stage/cross_module_optimization.rs:340`) — link flags dynamic imports inside lazy paths so the tree-shaker can skip them (`stages/link_stage/tree_shaking/include_statements.rs:213`).
- **`EntryPoint.related_stmt_infos`** (`crates/rolldown_common/src/types/entry_point.rs:16`) — tuples carry an `Address` alongside `(ModuleIdx, StmtInfoIdx, ImportRecordIdx)` to point at the originating AST node.
- **`PreProcessor`'s `statement_stack` / `statement_replace_map`** (`crates/rolldown/src/utils/tweak_ast_for_scanning.rs:17-18`) — scratch state used within a single traversal, where the AST is obviously still alive.

Why both mechanisms exist side by side: `Span` is what survives all the way into the finalizer, which re-derives identity by calling `node.span()` after the AST has been threaded through scan → link → finalize with no direct node references kept. `Address` is the natural choice when the consumer can keep a reference into the same arena and would otherwise have to extend `PreProcessor` (or fight synthetic-span collisions) just to make `Span` work. A future `AstNodeId` migration would in principle subsume both, but the fragilities listed below are specifically about the `Span` contract — `Address`-keyed sites aren't subject to them.

## Classical patterns

The side tables differ in detail, but they all instantiate one of two patterns. The pattern dictates which passes touch the entry, not the shape of the data.

### Pattern A — Scan → Finalize

Scan records the span together with whatever rewriting decision it can make locally. The link stage doesn't modify the entry. The finalizer reads it back and mutates the AST.

Example: `EcmaView::new_url_references`.

1. During scan, `ast_scanner/new_url.rs:69` sees a `new URL('./img.png', import.meta.url)`, resolves the path into an import record, and inserts `(NewExpression.span → ImportRecordIdx)` into the side table.
2. During finalize, `module_finalizers/mod.rs:961` visits each `NewExpression`, looks up its span, and on a hit rewrites the expression to emit the resolved asset URL.

The same shape recurs in:

- `EcmaView::this_expr_replace_map` — scan picks the replacement (`exports` vs `undefined`), finalizer applies it at `impl_visit_mut.rs:460`.
- `EcmaView::imports` — scan records the import-site span, finalizer rewrites the site.
- `EcmaView::dummy_record_set` — scan flags `require` identifier references (`ast_scanner/impl_visit.rs:591`), finalizer at `module_finalizers/rename.rs:86` consults the set on each `IdentifierReference` and rewrites the call to use the runtime `__require` helper. Here the span functions as a per-node boolean rather than a key for richer data, but the round-trip is still scan→finalize.

### Pattern B — Scan → Link → Finalize

Scan collects the span with only the local information available to it; link uses those records to populate a resolution table keyed by the same spans once cross-module facts are known; the finalizer applies the final decision.

Example: `LinkingMetadata::resolved_member_expr_refs`.

1. **Scan** sees a chain like `ns.foo.bar` and records the `StaticMemberExpression.span` together with the local symbol reference.
2. **Link**, in `link_stage/bind_imports_and_exports.rs:445-447, 477-479`, resolves each recorded span to the actual exported binding it points at across the module graph and writes the result into a `FxHashMap<Span, MemberExprRefResolution>` (committed to `LinkingMetadata` at `link_stage/bind_imports_and_exports.rs:700`).
3. **Finalize**, in `module_finalizers/mod.rs:1006`, visits each `StaticMemberExpression`, calls `.span()`, and on a hit replaces the chain with a direct reference to the linked symbol.

This is the fullest expression of the contract: three passes communicate about the same AST node entirely through its span.

## Known fragilities

- **Cloned nodes carry the original span.** If any pass clones an AST node without resetting its span, the clone falsely matches the side-table entry that belonged to the original. Future lookups can fire against the wrong node.
- **Synthetic spans collide with each other.** When the finalizer constructs new AST nodes (helpers, member expressions, etc.) it must give them a span that won't accidentally match a recorded one. The convention is the synthetic `SPAN` (`0..0`) — see `module_finalizers/mod.rs:1088-1090`, where the comment explicitly notes the workaround:

  ```rust
  // IMPORTANT: Use SPAN (0-0) for the new member expression to avoid being
  // matched by resolved_member_expr_refs lookup which uses span as key
  let ns_id_ref = self.snippet.id_ref_expr(ns_name, SPAN);
  ```

  This is fragile in the other direction: every synthetic node shares the same key, so adding side tables that look up by `SPAN` itself would immediately collide.

- **Uniqueness coverage is hand-maintained.** `PreProcessor` only deduplicates the node kinds it has been taught about (see "Pre-scan span normalization"). Other span-keyed kinds (e.g. `StaticMemberExpression`) rely on the parser to produce unique spans on its own. Adding a new side table on a kind prone to collisions or synthetic spans silently exits the safety net unless `PreProcessor` is extended — there's no compile-time check linking the two.
- **No post-`PreProcessor` validation.** Nothing checks at runtime that the uniqueness invariant still holds once `PreProcessor` has finished — the contract is enforced by construction, not by assertion. (Plugin `transform` hooks operate on source code before parsing, so they're not a concern; the AST being keyed wasn't built yet when they ran.) When the invariant breaks anyway — a cloned span, a new side-table kind not covered by `PreProcessor`, etc. — the regression manifests as a silent miss in the finalizer (a rewrite that should have happened didn't) rather than as a crash.
- **Existing FIXME.** `crates/rolldown_common/src/types/member_expr_ref.rs:23-24` already calls this out:

  ```rust
  /// FIXME: use `AstNodeId` to identify the MemberExpr instead of `Span`
  /// related discussion: https://github.com/rolldown/rolldown/pull/1818#discussion_r1699374441
  pub span: Span,
  ```

## Why this is on the radar

Oxc is introducing an `AstNodeId` — a true per-tree node identity, independent of source position. Switching the side tables to be keyed by `AstNodeId` instead of `Span` would dissolve the structural problems above: no uniqueness assumption to maintain, no synthetic-span workaround for finalizer-generated nodes (synthesized nodes get fresh ids that can't collide with anything), and no need for `PreProcessor` to keep a hand-curated list of node kinds in sync with the side-table set. This document captures the current state so that migration can be evaluated and planned against a concrete description of what's there today.

## Related

- [bundler-data-lifecycle](./bundler-data-lifecycle.md)
- [module-id](./module-id.md)
