# Top-Level Await Propagation

## Summary

`compute_tla` answers two questions for every module, in a single pass:

1. **"Does this module directly contain top-level `await`, or does it statically import — transitively — any module that does?"** The answer is stored on `LinkingMetadata::is_tla_or_contains_tla_dependency` and drives later `await init_foo()` codegen, ESM-chunk optimization bailouts, and export synthesis.
2. **"Is any `require(...)` in the graph pointed at such a module?"** If so, push a `RequireTla` build error, because CommonJS `require` is synchronous and cannot cross the async boundary that TLA introduces.

The pass runs as the second step of `LinkStage::link`, immediately after `sort_modules`. Source: `crates/rolldown/src/stages/link_stage/compute_tla.rs`.

## Why It Matters

TLA turns a module's initialization into an async operation. Any construct that expects synchronous module evaluation has to be adjusted, or rejected:

| Consumer                                 | Use                                                                                                                         |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `module_finalizers/mod.rs:296`           | Emits `await init_foo()` instead of `init_foo()` when the importee has TLA transitively                                     |
| `utils/chunk/render_chunk_exports.rs:33` | Wrapped ESM entries emit `await init_entry()` so `import` from a host can itself await                                      |
| `generate_stage/code_splitting.rs:865`   | Chunk optimization is disabled when any module has TLA — chunk merging can reorder initialization and reintroduce deadlocks |
| `compute_tla.rs` itself (this pass)      | `require(tla_module)` becomes a `RequireTla` diagnostic with a full import-chain note                                       |

Because downstream stages read only the boolean flag, the pass's real job is computing it once, correctly, and cheaply.

## Guarantees

- **Short-circuit on a TLA-free graph.** If `tla_module_count == 0` (the scan stage's running count of modules whose AST contains top-level `await`), the pass is a no-op. No `metas` writes, no allocations, no DFS.
- **Flag is set to `true` iff the module's static-import closure reaches a TLA module.** Dynamic imports (`import('…')`) and `require(…)` do **not** propagate the flag; only `ImportKind::Import` edges are traversed. This matches the semantics of TLA propagation — a dynamic import of a TLA module returns a promise to the caller explicitly.
- **One `RequireTla` diagnostic per offending `require` call site.** If multiple `require`s in a module hit TLA subgraphs, each gets its own error; the chain walker prefers direct edges to keep the note short.

## Algorithm

Memoized DFS over the static-import graph, one call per normal module, sharing a single `visited: IndexVec<ModuleIdx, TlaVisitState>` across the whole pass.

### The three-state memo

```rust
enum TlaVisitState {
  NotVisited,
  Visiting,                      // on the current DFS path
  Visited(Option<ModuleIdx>),    // Some(src) = first TLA source found; None = no TLA
}
```

The memoized value is not a boolean — it's the `ModuleIdx` of the first TLA source reached, or `None`. Recording the source (rather than a `bool`) is what lets the error path reconstruct an import chain from the `require`r down to the TLA keyword without re-running DFS.

### `find_tla_source`

For `module_idx`:

1. If already `Visited(result)`, return `result`.
2. If `Visiting`, return `None` — we're on the current DFS path; a back edge contributes no new TLA information here.
3. Otherwise mark `Visiting`, check the module's own `ast_usage.contains(TopLevelAwait)` — if set, memoize `Visited(Some(module_idx))` and return.
4. Else recurse into each static `Import` record's resolved module, take the first `Some(source)` via `find_map`, memoize it, return.

Step 2's "back edge returns `None`" is the eager cycle shortcut, and also the source of the known limitation below.

### The driver loop

For each normal module `m`:

1. Call `find_tla_source(m.idx, ...)`. Set `metas[m.idx].is_tla_or_contains_tla_dependency = tla_source.is_some()`.
2. For each `ImportKind::Require` record on `m`, call `find_tla_source` on the required module. If it returns `Some(tla_source_idx)`, build an `ImportChainNote` list via `build_import_chain`, look up `tla_keyword_span_map[tla_source_idx]`, and push a `RequireTla` diagnostic into `self.errors`.

Step 2 reuses the same `visited` memo — so an already-computed TLA status on the required subgraph is free.

### `build_import_chain`

Produces the `importer → importee` sequence shown in the error note. Starts at `start_idx` (the directly `require`-d module) and walks toward `tla_source_idx`:

- At each step, scan the current module's static `Import` records.
  - **Direct hit** — if any record resolves to `tla_source_idx`, prefer it and stop this hop.
  - **Indirect hit** — otherwise, take the first record whose resolved module is memoized as `Visited(Some(source))` with `source == tla_source_idx`.
- Seed a `seen: FxHashSet<ModuleIdx>` to guard against cycles: because cycle back edges are memoized with `Some(tla_source_idx)` just like forward edges, a naive first-match would walk back into the cycle instead of toward the keyword.

The `direct.or(indirect)` precedence is a readability choice — users get the shortest path the memo permits.

### `import_span_for`

Linear scan over `module.imports` to recover the source span for a given `ImportRecordIdx`. Called only on error paths, so the `O(imports)` cost is fine. A `debug_assert!` catches a scanner-invariant violation (every `Import`/`Require` record must be in the `imports` map).

## Known Limitation: Eager SCC Memoization

Documented in the source comment at the top of `compute_tla`:

> On a cycle hit we return `None` for the visiting edge and then memoize modules along the current DFS path even though a later branch of the same parent might still discover TLA through them. If a subsequent `require(...)` lookup lands on one of those prematurely memoized siblings we silently miss the error.

Concretely: in `A → B → A` with TLA buried behind a _different_ edge of `B` discovered later, `B`'s first recursion returns `None` because `A` is still `Visiting`. If we memoize `B` as `Visited(None)` before trying `B`'s other edges… we don't, actually — `find_map` short-circuits only on `Some`, so all of `B`'s other `Import` edges are explored before memoization. The limitation bites when the **parent** of the cycle memoizes its result before all cycle members have been re-entered from a non-cycle root.

A proper fix would use Tarjan-style SCC memoization: delay writing `Visited(..)` for any module in a pending SCC until the SCC fully resolves. Not implemented because (a) the miss only produces a missing _error_, never a wrong compilation, and (b) the incidence in practice is low.

## Invariants

- `tla_keyword_span_map[tla_source_idx]` must exist whenever `find_tla_source` returns `Some(tla_source_idx)`. `find_tla_source` only returns modules with `ast_usage::TopLevelAwait`, and the scanner always records a span for those modules. Asserted via `debug_assert!`.
- Every traversed `Import`/`Require` record has a span entry in `module.imports`. Asserted in `import_span_for`.
- `tla_module_count` tracks the exact number of TLA-containing modules (used as the fast-path gate). Underflow is asserted in the scan-stage cache.
- `metas[module.idx].is_tla_or_contains_tla_dependency` defaults to `false` and is only ever set here. A module not in `module_table` as `Normal` keeps the default.

## Unresolved Questions

- **Delayed SCC memoization.** Would catch the documented edge case but adds an SCC discovery pass. Worth the complexity only if users report missing `RequireTla` errors in practice.
- **Dynamic imports of TLA modules.** Currently not tracked. A dynamic `import('./tla-mod.js')` is fine (it returns a `Promise`), but we still don't propagate the flag through dynamic edges — which is the right choice for codegen. Worth documenting explicitly in user docs if/when TLA-in-dynamic-entry support matures.

## Related

- [module-execution-order](./module-execution-order.md) — `compute_tla` runs immediately after `sort_modules` in `LinkStage::link`
- `crates/rolldown/src/stages/link_stage/compute_tla.rs` — implementation
- `crates/rolldown/src/types/linking_metadata.rs` — `is_tla_or_contains_tla_dependency` field
- `crates/rolldown_error/src/build_diagnostic/events/require_tla.rs` — `RequireTla` / `ImportChainNote` payload
- `crates/rolldown_common/src/ecmascript/ecma_view.rs` — `EcmaModuleAstUsage::TopLevelAwait` flag
