# AST Mutation Between Passes

## Summary

Rolldown threads per-AST-node metadata between compiler passes via side tables. The cross-pass identity key is now oxc's post-semantic `NodeId`, while `Span` remains source-location metadata for diagnostics, comments, source maps, and generated replacement spans.

The public oxc type is `oxc::semantic::NodeId`. It is the implementation behind the `node_id()` / `set_node_id()` accessors on AST nodes after semantic analysis; there is not a separate public `AstNodeId` type in the version Rolldown currently uses.

## Pass Overview

Rolldown's bundling pipeline has three stages that interact with the AST:

- **Scan** - `ScanStage::scan` parses each module, runs Rolldown's pre-scan AST tweaks, then rebuilds semantic/scoping information. This final rebuild is what assigns every node — including the nodes the tweaks created — its `NodeId`, so the subsequent read-only walk via `AstScanner` sees stable ids while populating `EcmaView` side tables.
- **Link** - `LinkStage::link` performs cross-module work such as symbol binding, export resolution, tree shaking, and cross-module optimization. It still does not mutate the AST, but it can derive additional side tables from scan-time records.
- **Generate / Finalize** - `ScopeHoistingFinalizer`, driven from `GenerateStage::generate`, is the main stage that mutates the AST in place. It visits interesting nodes, calls `node_id()`, and queries the side tables to decide what to rewrite.

Between passes, Rolldown does not hold direct references to AST nodes. Lifetimes and parallel cross-module work make that impractical. The durable identity for a node within one module AST is therefore its `NodeId`.

## NodeId Contract

The shared invariant across passes:

- **Insertion**: scan/link write side-table entries using the `NodeId` of the AST node being recorded.
- **Lookup**: finalizer or another later AST walker reads `node_id()` from the current node and queries the table.
- **Required guarantees**: the node comes from the same post-semantic AST, and the side table is scoped to the module unless the key also includes `ModuleIdx`.

Important constraints:

- `NodeId` is only unique within a single AST. Any table that combines records from multiple modules must key by `(ModuleIdx, NodeId)`.
- `NodeId` is meaningful only after semantic analysis has assigned ids. Rolldown's normal scan path is post-semantic, so scan-created records are valid.
- Synthetic/default nodes use `NodeId::DUMMY` unless ids are assigned later. Do not insert cross-pass side-table records for synthetic `DUMMY` nodes.
- `NodeId::DUMMY` equals `NodeId::ROOT` (both are `0`, the `Program` node's id). `DUMMY` probes from synthesized nodes only miss because no side table records a `Program`-level entry — never add a `Program`-keyed entry to a per-module `NodeId` table.
- Cloned post-semantic nodes can preserve the original node id unless the clone is reset or semantic information is rebuilt. Treat cloned nodes as identity-sensitive.

Two paths finalize a _clone_ of the scanned AST, produced by `EcmaAst::clone_with_another_arena` into a fresh allocator, and they satisfy the "same post-semantic AST" guarantee through different mechanisms:

- **Cache path — id preservation.** The incremental-build cache (`NormalizedScanStageOutput::make_copy`, `ScanStageCache::create_output`) hands its clones to the link stage and `ScopeHoistingFinalizer`, which reuse scan-time scoping and never re-run semantic. The clone itself must carry the scan-time ids — this is why `clone_with_another_arena` uses oxc's `clone_in_with_semantic_ids` rather than plain `clone_in`, which would reset every id to `NodeId::DUMMY` and silently break every lookup.
- **HMR path — deterministic re-derivation.** The HMR renderers in `crates/rolldown/src/hmr/hmr_stage.rs` clone and then immediately run `EcmaAst::make_semantic` on the clone, which re-stamps every `NodeId`; the ids the clone preserved are overwritten before any lookup. Lookups still hit because `SemanticBuilder` numbers nodes purely by traversal order, so an unmutated clone of the same tree shape re-derives exactly the scan-time ids. Two invariants keep this true: nothing may mutate the clone before `make_semantic` runs, and oxc's numbering must remain a pure function of tree shape (true as of oxc 0.135 — builder options such as `with_cfg` / `with_enum_eval` do not affect numbering). Breaking either shifts ids silently: the indexing lookups (`module.imports[&…]`) panic, the `.get()` lookups silently skip rewrites.

## Current NodeId-Keyed Tables

The main cross-pass side tables keyed by `NodeId` are:

- `EcmaView::imports` - import declarations, export-from declarations, dynamic `import()` expressions, and recognized `require()` call expressions.
- `EcmaView::dummy_record_set` - `require` identifier references that need the runtime helper rewrite.
- `EcmaView::new_url_references` - `new URL('...', import.meta.url)` nodes mapped to asset import records.
- `EcmaView::this_expr_replace_map` - top-level `this` expressions that should become `exports` or `undefined`.
- `MemberExprRef::node_id` and `LinkingMetadata::resolved_member_expr_refs` - namespace/member-expression resolution from scan through link to finalization.
- `DynamicImportExprInfo::node_id` records the dynamic `import()` node within its own module; `EntryPoint::related_stmt_infos` then carries `(ModuleIdx, …, NodeId, …)` tuples so a dynamic-import entry can be traced back across the module graph.
- Cross-module optimization state, which comes in two shapes: a per-module set of side-effect-free call expressions (bare `NodeId`, only consumed within the same module's traversal) and a graph-wide set of unreachable dynamic imports keyed by `(ModuleIdx, NodeId)` because it aggregates records from every module.

This means finalizer-generated nodes that keep the default `NodeId::DUMMY` do not accidentally match scan-time records. `Span` no longer needs to double as the key for these rewrite decisions.

## Where Span Still Belongs

`Span` remains the right representation for source positions. It is still used for:

- diagnostics and warnings that point at user source;
- comments, source-map ranges, directive/hashbang ranges, and TLA keyword locations;
- generated replacement spans where codegen should preserve a useful source location;
- import-record source locations, including the raw module-request span for resolver diagnostics and resolved `importer_span` for diagnostics that need to point at the full import site.

For import records, the module-request span belongs to `ImportRecordStateInit`: dependency
resolution diagnostics still need to underline the original specifier, but the span is not
carried into `ImportRecordStateResolved`. Resolved records keep `importer_span` because later
passes, such as TLA import-chain diagnostics, need a location for the resolved import edge.

For member expressions, `NodeId` is the cross-pass lookup key, but spans remain necessary as
source locations: `MemberExprRef::span` points diagnostics at the original expression, and the
finalizer applies the current member expression span to generated replacements so source-map and
diagnostic locations stay tied to the rewritten source range.

Do not add a cross-pass node side table keyed only by `Span`. If a later pass needs to identify the same AST node, prefer `NodeId`; if records from more than one module can share a table, include `ModuleIdx`.

## Address Use

Oxc `Address` is still acceptable for scratch state inside one live AST traversal, where producer and consumer operate before the traversal returns and no data survives as cross-pass metadata. The current example is:

- `PreProcessor`'s `statement_stack` / `statement_replace_map` in `crates/rolldown/src/utils/tweak_ast_for_scanning.rs`.

`PreProcessor` specifically _cannot_ use `NodeId`: it runs before the final semantic rebuild (`recreate_scoping` in `crates/rolldown/src/utils/pre_process_ecma_ast.rs`), so node ids are not yet assigned to the nodes it creates or moves. `Address` is the only stable per-node identity available at that point, and it is safe because the table never outlives the traversal.

Do not store `Address` in module metadata, entry metadata, or link-stage tables that outlive the traversal that produced it. In the post-semantic scanner, prefer `NodeId` even for same-traversal node identity checks when the compared nodes already have semantic IDs.

## Pre-Scan Span Handling

`PreProcessor` does not rewrite spans for identity anymore. Pairwise span uniqueness does not back any identity table after the `NodeId` migration, so ordinary duplicate spans are left alone, and nodes created during pre-scan rewrites can keep the reserved synthetic span (`SPAN`, `0..0`).

Later passes must not use `span.is_unspanned()` to decide whether a scanner-visible node has a cross-pass record. For example, finalizing a `require()` call now relies on `EcmaView::imports.get(call_expr.node_id())`: pre-scan-created calls have semantic `NodeId`s and can hit, while finalizer-created calls keep `NodeId::DUMMY` and miss.

The practical rule is simple: treat `Span` as location, `NodeId` as same-AST node identity, and `(ModuleIdx, NodeId)` as cross-module node identity.

## Related

- [ast-construction](./ast-construction.md) — how rolldown builds the nodes whose identity this contract tracks; the synthetic-`SPAN` / dummy-`NodeId` discipline for synthesized nodes is shared with that doc
- [bundler-data-lifecycle](./bundler-data-lifecycle.md)
- [module-id](./module-id.md)
