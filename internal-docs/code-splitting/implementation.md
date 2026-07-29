# Code Splitting

The rationale and target architecture are documented in [design.md](./design.md). This file describes the current implementation, where generate-stage order lowering owns wrappers and importer overlays in `OrderWrapState` without changing link-stage `WrapKind` or user statement inclusion, and final interop/order init facts are sealed separately in `FinalEsmInitMetadata`. One strict-mode exception stays at the link stage: `wrap_modules` forces `WrapKind::Cjs` onto every CommonJS module when `strictExecutionOrder` is enabled and `codeSplitting` groups are configured, because order lowering only wraps ESM modules while the interop rules leave a CommonJS module nobody imports (a CommonJS entry in cjs output) eager — a co-locating group would otherwise run one entry's top level while loading another. The wrap is gated on groups being present because only a group can move such a module out of its own entry chunk; without groups the raw body is safe where it is and preserves the entry's Node module contract (`module.filename`, `require.main === module`, the `exports` object shape), which a wrapper's synthetic `module`/`exports` parameters cannot reproduce.

## Summary

Code splitting determines which modules go into which output chunks. Rolldown uses a BitSet-based reachability model — the same fundamental approach as esbuild and Rollup. Each entry point gets a bit position, modules are marked with the set of entries that can reach them, and modules with identical reachability patterns are grouped into the same chunk.

## Why BitSet-Based Reachability?

All three approaches to code splitting in the ecosystem solve the same problem: given N entry points and M modules, assign each module to exactly one chunk such that no module is duplicated and every entry loads exactly the modules it needs.

**Webpack's approach (constraint-based heuristics):** Uses `SplitChunksPlugin` with configurable rules — `minSize`, `minChunks`, `maxAsyncRequests`, cache group priorities. This gives users maximum control but accepts code duplication as a trade-off for fewer HTTP requests. The rules-based system can't guarantee zero duplication.

**Rollup's approach (entry set coloring):** Builds a `Set<entryIndex>` per module, groups modules with identical sets. Uses `BigInt` bitmasks for efficient set operations. Guarantees zero duplication. Supports `experimentalMinChunkSize` for merging small chunks.

**esbuild's approach (BitSet reachability):** Assigns each entry a bit position, propagates through the graph, groups by identical `BitSet`. Conceptually identical to Rollup's coloring but implemented with compact bitwise operations at file level. Guarantees zero duplication. Minimal user configuration.

Rolldown follows the esbuild/Rollup model because:

1. **Zero duplication guarantee** — Every module appears in exactly one chunk. No user configuration needed to avoid duplication pitfalls.
2. **Deterministic output** — Same input always produces same chunks. No heuristic thresholds to tune.
3. **Performance** — BitSet operations (union, intersection, equality) are O(entries/64) per operation, making the algorithm O(modules × entries) overall. This is critical for large codebases.
4. **Rollup compatibility** — As a Rollup successor, matching Rollup's splitting semantics minimizes migration friction.

The trade-off is that this approach can produce many small chunks when there are many entry points with different reachability patterns. The chunk optimizer (see below) mitigates this by merging small common chunks back into entry chunks when safe.

## How Other Bundlers Handle Key Problems

| Problem                    | Rollup                                                  | esbuild                                                 | Rolldown                                                                         |
| -------------------------- | ------------------------------------------------------- | ------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Shared module detection    | `Set<entryIndex>` per module                            | `BitSet` per file                                       | `BitSet` per module                                                              |
| Separate chunk vs. inline? | Always separate; `experimentalMinChunkSize` for merging | Always separate; no merging                             | Separate by default; optimizer merges into entry chunks                          |
| Circular chunk deps        | Warns; allows cyclic reexports                          | Enforces acyclic static chunk graph                     | Enforces acyclic via `would_create_circular_dependency` check before every merge |
| Dynamic imports            | New entry points; computes "already loaded" atoms       | New entry points; rewrites to chunk unique keys         | New entry points; facade elimination for empty dynamic entries                   |
| External modules           | Excluded from chunk graph                               | Excluded from bundling                                  | Filtered from entry list at source (never get bit positions)                     |
| Granularity                | Module level                                            | File level (was statement-level, backed off due to TLA) | Module level                                                                     |

## Pipeline

The entry point is `generate_chunks()` in `code_splitting.rs`, called from `GenerateStage::generate()`.

```
generate_chunks()
    │
    ├─ init_entry_point()             Assign bit positions, create entry chunks
    │
    ├─ split_chunks()
    │    │
    │    ├─ pre_chunk_order_state()                   Discover consumer-local barrels/carriers
    │    ├─ determine_reachable_modules_for_entry()   BFS + importer-local route bits per entry
    │    │
    │    ├─ apply_manual_code_splitting()             User-defined chunk groups (manualChunks)
    │    │
    │    ├─ Module assignment         Group modules by identical BitSet → chunks
    │    │
    │    ├─ ChunkOptimizer           Merge common chunks into entry chunks, remove empty facades
    │    └─ try_merge_runtime_chunk() Optionally merge the standalone runtime into a safe host
    │
    └─ Chunk exec-order assignment    → ChunkGraph    Provisional module-to-chunk assignment

Post-ChunkGraph processing (in generate()):

ChunkGraph
    │
    ├─ finalize_chunk_plan()
    │    ├─ finalize provisional namespace/external facts
    │    ├─ analyze_execution_order()                Build an OrderWrapPlan with per-module reasons
    │    ├─ apply_order_wraps()                      Lower the plan into wrappers and topology edits
    │    │    └─ ensure_runtime_module_for_order_wraps()   Re-place the runtime, then sole-consumer fold
    │    ├─ recompute metadata if topology changed
    │    ├─ sweep_unused_runtime_module()            Drop a runtime with no source or order-state demand
    │    └─ validate final output shape
    │
    ├─ used_symbol_refs.seal()                        Freeze liveness; the sweep is the last writer
    │
    ├─ compute_wrapped_esm_init_metadata()            Produce Sealed<FinalEsmInitMetadata>
    │
    ├─ compute_cross_chunk_links()                    Determine cross-chunk imports/exports
    │
    ├─ ensure_lazy_module_initialization_order()      Reorder wrapped module init calls
    │
    └─ merge_cjs_namespace()                          Merge CJS namespace objects
```

**Key files:**

- `crates/rolldown/src/stages/generate_stage/code_splitting.rs` — pipeline orchestration, `generate_chunks()`, `ensure_lazy_module_initialization_order()`
- `crates/rolldown/src/stages/generate_stage/order_analysis.rs` — `strictExecutionOrder` analysis and the reasoned `OrderWrapPlan`
- `crates/rolldown/src/stages/generate_stage/order_wrapping.rs` — applies order wrappers after chunk assignment
- `crates/rolldown/src/stages/generate_stage/order_wrap_state.rs` — owns synthetic wrapper/carrier declarations, importer overlays, namespace/runtime requirements, and re-export routing evidence
- `crates/rolldown/src/stages/generate_stage/compute_wrapped_esm_init_metadata.rs` — derives the sealed final no-op and excluded-statement init facts shared by interop and order wrappers
- `crates/rolldown/src/stages/generate_stage/finalize_chunk_plan.rs` — final topology boundary before output metadata and validation
- `crates/rolldown/src/stages/generate_stage/dynamic_already_loaded.rs` — Rollup-style dynamic import already-loaded atom reduction
- `crates/rolldown/src/stages/generate_stage/chunk_optimizer.rs` — merge/optimization
- `crates/rolldown/src/stages/generate_stage/runtime_module_sweep.rs` — post-optimization runtime-demand sweep
- `crates/rolldown/src/chunk_graph.rs` — output data structure
- `crates/rolldown_utils/src/bitset.rs` — compact reachability representation
- `crates/rolldown/src/types/linking_metadata.rs` — immutable link-stage `wrap_kind()`

Wrap-all (the default strict mode) seeds the plan from the expected orders alone and skips prediction; `experimental.onDemandWrapping` enables the selective analysis. `finalize_chunk_plan` may run two metadata passes. Namespace usage and entry-level external re-exports are first finalized on the provisional graph. They are recomputed when order wrapping or strict entry facades change topology.

`create_order_wrap_entry_facades` and `restore_order_wrap_entry_facades` share one statement-aware scan of live dynamic importers. When a pure dynamic entry has a synchronous activation target and every surviving call site can be rewritten, the create path avoids minting a strict-only facade (or the restore path leaves an optimizer-removed facade dead): the implementation chunk is marked common, `common_chunk_exported_facade_chunk_namespace` requests namespace extraction, and `rewrite_dynamic_import_for_merged_entry` carries the trigger at each call site. An ordinary entry carries its single ESM init target; a consumer-local entry carries its complete cached namespace target list, so the intentionally empty shared barrel wrapper is never used as activation. Same-chunk rewrites call the list directly. Cross-chunk rewrites make the host chunk depend on and export every leaf/carrier wrapper alongside the simulated namespace, then invoke those exported wrappers in list order before returning the namespace. TLA-tainted entries or targets, callable-`then` host namespaces, emitted/user entries, and entries with any entry-level external star retain a real facade. Converting an external-star entry to a common chunk removes the facade's format-specific merge. ESM loses its chunk-level `export *`; CJS-like formats replace a deduplicated `Object.keys` merge with per-record `__reExport` calls, which also differ for primitive module values, and a transitive star is not available from the entry module's direct records at all. The chunk's `entry_level_external_module_idx` covers direct and transitive chains, so one format-independent guard preserves all of these interfaces. This external-star guard is create-path-only; the restore path relies on the chunk optimizer's simulated-namespace handling, whose external-star preservation is handled separately. If the restore path has already revived an empty facade because its own collapse proof failed, the create path cannot reclassify that facade as an implementation chunk.

Both collapse paths register a synthetic namespace declaration and its runtime demand in `OrderWrapState`, then recompute the runtime-symbol closure: the create path does so when it avoids minting a facade, and the restore path does so when it leaves an optimizer-eliminated facade dead. The state stores the exact export names selected by `DynamicImportExportsUsage` and the non-inlined bindings behind them. The names keep finalization identical to the removed facade even when another alias of the same canonical binding is live elsewhere; the binding references let normal cross-chunk linking import every getter target after the entry becomes a common chunk. A real semantic namespace consumer overrides this narrowing and keeps the complete interface. Runtime demand includes `ExportAll`; entries that would require an external-star merge retain their facade instead. Simulated-facade namespace demand remains separate from semantic namespace demand, so this does not turn the narrowed dynamic-import interface into an opaque namespace read or reopen link-stage user-code liveness.

Once final topology and liveness are fixed, `compute_wrapped_esm_init_metadata` produces a sparse `Sealed<FinalEsmInitMetadata>`: absence of a module entry means the default no-op/empty-target values, not absence of a wrapper. `Sealed<T>` has a private field and constructor, exposes only `Deref`, and offers neither `DerefMut` nor an unwrap operation. Final cross-chunk linking and module finalization require that sealed type, so re-owning the artifact cannot reopen mutation and an unsealed result cannot reach either consumer. The earlier `predicted_static_import_edges` pass explicitly marks final metadata unavailable instead of manufacturing an empty sealed artifact; its Project-mode obligation traversal remains separate and conservative.

Both modes consume the same frozen tree-shaking facts. `order_wrapper_is_reexport_transparent` identifies pure ESM order wrappers that are only init-routing waypoints. `consumer_local_reexport_plan` extends that category to direct re-export-only barrels whose generated CJS interop can be extracted safely. It creates one `OrderCjsCarrierKey { importer, record }` per retained CJS record and maps each affected export/facade symbol to that carrier. The original re-export statement no longer emits `namespace = __toESM(require_cjs())` in a shared `init_barrel()`; the CJS importee's finalizer declares a memoized carrier and namespace in the importee's chunk.

`collect_wrapped_esm_init_targets_for_import_record` then resolves obligations per consumer into `WrappedEsmInitTarget::{Module, CjsCarrier}`: it filters named specifiers by local facade liveness, follows consumed re-export facades recorded in `OrderWrapState`, resolves static namespace-member reads from included `StmtInfo` references, expands all exports only for opaque namespace use, and applies the same constant-inline bypass as the inclusion pass. It also recursively includes effectful carriers and sorts them with selected leaves by the full nested record path. Emit, cross-chunk Register, on-demand Project, and pre-chunk placement consume this same target model. This lets wrap-all emit more wrappers without retaining or executing more user code than on-demand.

Lowering also caches the complete namespace targets for every consumer-local route. Entry prologues and collapsed dynamic-entry call-site triggers use that list when the route itself is an entry. An included import/re-export record always uses the importer-local resolver when its target is a consumer-local route, even if link-stage interop gave that target `WrapKind::Esm`; this prevents the finalizer's legacy direct-wrapper path from bypassing carrier initialization. For excluded `export *` records whose namespace is materialized, final ESM init metadata uses the cached complete list. In both cases the calls stay inside the consuming wrapper's memoized guard and at the re-export record position. Final metadata filters cached targets against final live chunk placement before Emit and Register consume them.

`consumer_local_reexport_plan` is a monotone fixpoint so a carrier-free outer barrel can become a routing waypoint after its inner carrier-owning barrel is accepted. It rejects tree-shaking-disabled builds, synchronous `import`/`require` SCC members, modules targeted by an opaque CommonJS `require()`, TLA-tainted modules, dynamic exports, CJS `export *`, shimmed exports, concatenated wrappers, and any source body that is not entirely direct re-exports. These cases retain the existing monolithic init path. A direct require target stays monolithic; if its re-export record exposes a downstream consumer-local namespace, final metadata initializes that route's complete cached target list inside the monolithic guard. The SCC fallback keeps one module-level evaluating guard around mixed ESM/CJS cycles. A carrier is eager only when both its source record is side-effectful and the barrel's own side-effect contract retains that record unconditionally, so `moduleSideEffects: false` on the barrel can keep an otherwise effectful CJS route lazy. Recursive eager-carrier discovery applies this side-effect gate at every forwarding hop, so an outer `moduleSideEffects: false` barrel also blocks a deeper carrier from leaking into unrelated consumers. Carrier-owning barrels are forced into the on-demand plan even when provisional source order alone would not select them; otherwise their ordinary CJS lowering would reappear after placement.

Excluded re-exports with a recorded retained path use the same boundary rule. `collect_order_wrap_esm_init_targets` computes which retained-path modules are reachable through an entirely side-effect-retaining chain before adding any off-path eager carrier; an explicitly selected carrier is still retained because that is binding demand, not eagerness. Computing the permission as an any-path reachability fact also makes converging retained paths independent of DFS order. A non-empty retained-path overlay drops its direct wrapper reference after recording the path: sealed init metadata registers exactly the resolver's targets, and finalization emits exactly those targets, so the otherwise-unused direct wrapper cannot create a bare cross-chunk import into a carrier-hosting chunk.

Order planning closes over sensitive suffixes, dependent importers/readers, and eligible sensitive modules in any static chunk SCC already touched by the plan. Under `onDemandWrapping` it then runs the emergent-cycle fixpoint in `order_analysis.rs` (`post_lowering_import_edges`): each round projects the plan's post-lowering `init_*` forwarding edges onto the pre-lowering baseline from `predicted_static_import_edges`, wraps every eligible module in a chunk cycle those edges close, and repeats until the at-risk set stops growing (see the design doc for the projected/omitted edge-source inventory). Set `ROLLDOWN_ORDER_DEBUG=1` for a stderr trace of the per-round SCC counts and final wrap delta.

The scanner derives a module's intrinsic order sensitivity without changing its tree-shaking result. The ordinary `StmtEvalAnalyzer` remains the only producer of `tree_shaking_flags`. For strict on-demand builds only, a statement in a module not already known to be sensitive gets a separate eager-evaluation reason walk when ordinary analysis has not classified that statement as order-sensitive. That walk follows top-level expressions, binding defaults and computed keys, immediately invoked function literals (including conditional/logical/assignment callees, directly invoked literal template tags, and the construction-time positions of a directly `new`ed class literal), and calls/new/tagged templates covered by `manualPureFunctions`; it stops at ordinary function and method bodies and at unconstructed instance initializers. Like `TopLevelImportReadDetector`, it also honors `propertyReadSideEffects: false` as that option's documented contract: a getter or `Proxy` trap invoked through a certified property read stays invisible to order analysis. This separation prevents Oxc's valid tree-shaking early exits for pure calls or property operations from hiding an observable global read from order wrapping, while default builds, wrap-all builds, and cross-module tree-shaking re-analysis keep their existing hot paths and flags.

## Bit Positions and Entry Points

`init_entry_point()` iterates `link_output.entries` (an `FxIndexMap<ModuleIdx, Vec<EntryPoint>>`), assigning each entry a sequential bit position via `.enumerate()`:

```
entry_index 0  →  entry-a.js      →  bit 0  →  ChunkIdx(0)
entry_index 1  →  entry-b.js      →  bit 1  →  ChunkIdx(1)
entry_index 2  →  plugin.js       →  bit 2  →  ChunkIdx(2)
```

Dynamic imports are treated as entry points — they get bit positions and entry chunks just like static entries. This matches Rollup and esbuild behavior: a dynamic `import()` creates a new loading boundary, so the imported module needs its own chunk (or must be merged into an existing one).

External modules are filtered out at the source — they never appear in `link_output.entries`. This is done in `module_loader.rs` where dynamic imports are collected as entry points: external modules are excluded from `dynamic_import_entry_ids`. User-defined and emitted entries are also safe because `load_entry_module()` rejects external resolutions with `entry_cannot_be_external`. This matches esbuild's approach where external modules never enter the entry list, and ensures that **bit positions directly equal chunk indices** — `ChunkIdx::from_raw(bit_position)` is always valid.

See #8595 for the bug that motivated this filtering.

### Dynamic Already-Loaded Analysis

Before chunk materialization, Rolldown can reduce dynamic-entry bits for modules that are guaranteed to be loaded by every importer of that dynamic entry. This pass is controlled separately from common-chunk merging via `experimental.chunkOptimization: { mergeCommonChunks, avoidRedundantChunkLoads }`. The boolean form remains a compatibility alias for enabling or disabling both optimizers, while the object form allows each pass to be controlled independently. For example, when `main` statically imports `shared`, dynamically imports `route`, and `route` also imports `shared`, the `route` bit is removed from `shared` before modules are grouped into chunks.

The pass groups modules into temporary atoms by their current dependent-entry bitset, computes the static atoms loaded by each entry, then runs a fixed-point propagation over dynamic imports. A dynamic entry's already-loaded atoms are the intersection of the static and already-loaded atoms of all entries that can reach its included dynamic importers. Any atom already loaded for a dynamic entry can drop that dynamic entry bit, and modules are then regrouped by the reduced bitsets during normal chunk creation.

When the reduced bitset would put an atom into a single dynamic-entry chunk, the pass preserves that dynamic entry's observable namespace. The reduction is accepted only if the atom has no extra exports, its exports are already part of the dynamic entry's signature, it is runtime-only, or it is the removed dynamic-entry module itself. Otherwise the atom stays separate so `import("./entry.js")` does not expose helper exports needed only by other chunks.

Every accepted reduction must also keep the regrouped static atom graph acyclic. "Already loaded" does not always mean "already initialized": if a reduced atom is moved into a chunk that statically imports one of its consumers, an ES module cycle can expose uninitialized bindings, including CJS wrapper functions.

Runtime may participate in this bit reduction, but only as placement metadata. It is extracted into a standalone runtime chunk before manual and normal chunk materialization, so this pass does not assign runtime code to user chunks and does not need runtime-specific cycle handling.

Top-level-await refinements are intentionally not modeled here yet. The existing chunk optimizer still bails out globally when any included module is TLA or contains a TLA dependency, so the awaited-dynamic-import safety path remains future work.

## Reachability Propagation

`determine_reachable_modules_for_entry()` runs BFS from each entry module, setting `splitting_info[module].bits.set_bit(entry_index)` on every reachable module. External modules are skipped during traversal (they're not `Module::Normal`).

Strict builds first construct a discovery-only wrap-all `OrderWrapState`. This is required before chunk assignment: a re-export barrel's ordinary `load_dependencies` is a bundle-wide union, so using it would give an eager entry the leaves that only a dynamic entry consumes before the real per-record carriers exist. When an incoming record targets a consumer-local barrel, the shared resolver adds only that record's selected leaf modules and CJS carrier importees to the queue. Processing a non-entry consumer-local barrel marks the waypoint reachable, queues any directly nested consumer-local waypoints, skips its module-wide dependency union, and recursively queues only effectful carriers. A user or dynamic entry whose module is itself such a barrel keeps full traversal because importing that entry observes its complete namespace.

The resulting entry bits place each carrier in the same chunk as its CJS importee. Later order lowering assigns the carrier's synthetic statement to that chunk, while the carrier symbol still belongs to the re-exporting module for binding identity. Symbol assignment and deconfliction therefore use the synthetic statement's explicit chunk rather than assuming that every declared symbol is hosted by its owner module. CJS namespace merging and lazy-init transfer skip these carrier records because moving or coalescing their require sites would destroy the per-record route boundary.

After all entries are processed, each module's `bits` encodes which entries can reach it:

```
shared.js:    bits = 1111  (reachable from all 4 entries)
parser-a.js:  bits = 1010  (reachable from entries 1 and 3)
entry-a.js:   bits = 0001  (only reachable from entry 0)
```

This is equivalent to Rollup's "dependent entry set" and esbuild's `EntryBits`. The key insight is that modules with identical `bits` have identical loading requirements — they're always needed together, never separately — so they belong in the same chunk.

## Chunk Creation

After reachability propagation, `split_chunks()` assigns modules to chunks by their `bits` pattern:

1. Entry chunks already exist from `init_entry_point()` with their single-bit patterns
2. For each non-entry module (iterated in `sorted_modules` order), look up `bits_to_chunk[module.bits]`
3. If a chunk exists for that pattern, add the module to it
4. Otherwise, create a new `Common` chunk

Modules with the same reachability pattern always land in the same chunk. This is the core invariant that guarantees zero code duplication — a module is emitted exactly once, in the chunk matching its reachability fingerprint.

## Chunk Optimizer

Without optimization, the BitSet approach can produce many small common chunks (one per unique reachability pattern). For example, 10 entry points with varied sharing patterns could produce dozens of tiny chunks. This is the main drawback of the pure BitSet approach that webpack's heuristic system avoids.

The chunk optimizer reduces chunk count by merging common chunks back into entry chunks when safe. It operates on a temporary `ChunkOptimizationGraph` to test merges without modifying the real chunk graph.

### Common Module Merging (`try_insert_common_module_to_exist_chunk`)

For each common chunk, translates its `bits` to chunk indices (bit positions directly map to `ChunkIdx`), then tries to merge it into one of those entry chunks. Merging is skipped if it would:

- **Create a circular dependency between chunks** — checked via BFS in `would_create_circular_dependency()`. This is stricter than Rollup (which warns but allows cycles) and matches esbuild's enforcement of acyclic static chunk graphs.
- **Change an entry's export signature** — when `preserveEntrySignatures: 'strict'`, adding modules to an entry chunk would expose symbols that the original entry didn't export.
- **Pollute dynamic-import namespaces** — dynamic entries that form static cycles are not merged asymmetrically when a non-target pending entry module has observable exports. The check uses linked export metadata, so re-export-only entries (`export * from ...`) are treated the same as direct named exports.

The trade-off of merging: entry chunks may include modules that not all consumers of that entry need. This adds a small amount of unnecessary code loading but significantly reduces chunk count and HTTP requests.

For chunks shared only by dynamic entries, the optimizer does not infer a merge target from dynamic-import reachability alone. Sibling dynamic imports from the same loaded entry can be requested independently, so "the entry can reach both chunks" does not prove either dynamic chunk is already loaded before the other. In that case, Rolldown keeps a separate common chunk unless the existing static-import merge-target check proves a safe target.

### Facade Elimination (`optimize_facade_entry_chunks`)

Dynamic/emitted entries can become empty facades when all their modules are pulled into other chunks by the optimizer. The optimizer identifies these and either:

- Merges the facade into its target chunk
- Marks it as `Removed` in `post_chunk_optimization_operations`

A **user-defined** entry can likewise become an empty facade when manual code splitting (a `codeSplitting` group, possibly via `entriesAware` subgroup merging) places its module into a common chunk. Folding that common chunk back into the entry chunk is only safe when the chunk does not also hold **another user-defined entry's module**. Otherwise the sibling entry would be forced to import this entry chunk just to reach its own module, and loading it would eagerly run this entry's top-level `init_*` — leaking its side effects into the sibling (visible under `strictExecutionOrder`). In that case the facade is kept so each entry imports the shared (wrapped) chunk and runs only its own `init_*`. See [#9463](https://github.com/rolldown/rolldown/issues/9463).

### Runtime Module Placement

When code splitting is enabled, the runtime module is assigned before manual and normal module chunking into a dedicated common chunk. This chunk uses normal chunk naming and is not registered in `bits_to_chunk`, so other modules with the same reachability bits cannot be grouped into it. Manual chunking also treats the runtime as already assigned, including during recursive dependency collection. The normal chunking and common-chunk merging passes therefore operate on user modules without carrying runtime-specific exceptions.

When code splitting is disabled, Rolldown does not extract a standalone runtime chunk. The runtime remains in the single output chunk, which preserves the single-file formats such as IIFE and UMD.

This standalone-first placement is the correctness baseline. Runtime helper consumers import helper symbols such as `__exportAll` from whichever chunk contains the runtime. If the runtime is co-located with a chunk that already has a forward static path to one of those consumers, the helper import can close a static cycle. See [#8989](https://github.com/rolldown/rolldown/issues/8989) for the canonical shape:

```
chunk(node2) ──forward──> chunk(node3) ──forward──> chunk(node4)
     ▲                                                   │
     └──────── helper edge after facade elim ────────────┘
```

After chunk optimization, `try_merge_runtime_chunk()` can fold the standalone runtime chunk into an existing live chunk when that is proven safe. It computes the runtime consumer set from:

```
consumer_chunks = (non-removed chunks with non-empty depended_runtime_helper)
                ∪ chunks whose included statements reference runtime-owned symbols
                ∪ chunks containing modules that depend on the runtime module
                ∪ chunks containing wrapped modules or side-effectful runtime dependencies
                ∪ caller-supplied additional consumers
```

The additional-consumers channel carries demand that only the caller can see: facade-elimination consumers during chunk optimization, and order-introduced consumers during the post-order-lowering fold below.

Most runtime-helper dependencies are known at link time, before chunking, so this consumer set is usually complete as soon as chunks exist. Facade elimination is the exception: collapsing a facade into its target chunk can add a helper edge that did not exist during early chunking, through one of two `wrap_kind`-gated paths in `optimize_facade_entry_chunks`:

- **`WrapKind::Esm` / `WrapKind::None`** — the dynamic `import()` site still expects a namespace object, so the eliminated module's simulated namespace is materialized (`include_symbol(namespace_object_ref)`) and `__exportAll` (`RuntimeHelper::ExportAll`) is inserted into the target chunk's `depended_runtime_helper`.
- **`WrapKind::Cjs` / `WrapKind::Esm`** — the `require_xxx` wrapper (`wrapper_ref`) is included, which transitively pulls whatever helpers the wrapper needs — commonly `__toESM` (`RuntimeHelper::ToEsm`) and `__commonJSMin` (`RuntimeHelper::CommonJsMin`) — via normal inclusion propagation.

A `WrapKind::Esm` facade hits both paths.

A chunk only _consumes_ the runtime once it carries a non-empty `depended_runtime_helper` (or references a runtime-owned symbol). So in a build that needed no helpers at link time, _no_ chunk consumed the runtime — and one of the paths above can create the very first consumer _after_ early chunking has already placed the runtime standalone. To keep that placement decision sound, the pass restores the inclusion metadata it just mutated, (re)materializes the standalone runtime chunk, and re-runs `try_merge_runtime_chunk` with these newly added consumers (the final bullet of the consumer set above).

The merge target must not create a static cycle or force unrelated entry chunks to execute just to access helpers. Candidate targets are tried in compactness-preserving order: a sole runtime consumer, a sole live chunk with the runtime bitset, a live common chunk with the same runtime bitset, then a consumer-set dominator. The caller selects how deep this cascade may go (`RuntimeMergeCascade`): both merge attempts during chunk optimization use the full cascade, while the post-order-lowering fold below stops after the sole-consumer step. Manual code splitting / advanced chunk group chunks may host runtime only when that chunk is the sole runtime consumer; otherwise their contents are user-directed grouping output, and absorbing the runtime would make unrelated chunks load the group for helpers. Safety is checked by following static chunk-loading edges through still-live chunks. Dynamic imports are not followed; static imports and `require()` records are both considered because either can become a static chunk import in generated output. Chunks containing top-level await, or a dependency on top-level await, are runtime hosts only when they are the sole runtime consumer; otherwise a dynamically imported chunk that statically imports its awaiting importer for helpers can produce an unsettled async module cycle.

- **Safe target found** → runtime moves into that chunk, and the empty standalone runtime chunk is marked removed.
- **No safe target** → keep the standalone runtime chunk. Runtime imports that resolve only to externals are ignored for chunk-cycle checks; live internal runtime imports keep the runtime standalone instead.

### Post-Order-Lowering Runtime Fold (`ensure_runtime_module_for_order_wraps`)

Order lowering can invalidate the placement the merge above decided. Synthetic wrappers and importer overlays add helper demand that lives only in `OrderWrapState`, never in link-stage metadata, so a chunk with no pre-lowering demand can become a consumer — and that new consumer may sit in a static cycle with the runtime's current host. `ensure_runtime_module_for_order_wraps` therefore restores the standalone-first baseline at the end of `apply_order_wraps()` when code splitting is enabled: a runtime co-hosted in a user chunk is evicted into a fresh standalone chunk, and a runtime that was never placed is materialized standalone. (With code splitting disabled, the single-chunk placement above stands.) These (re)minted chunks carry synthetic all-live-union bits so they sort and name as a universal source; the bits are not reachability facts.

Evicting unconditionally would permanently split layouts the pre-lowering merge had already proven safe (#10294). So every exit of the pass — standalone runtime kept, co-hosted runtime evicted, runtime newly materialized — re-runs `try_merge_runtime_chunk` against the post-lowering consumer set (`fold_runtime_chunk_after_order_lowering`). The order-introduced consumers come from `OrderWrapState::runtime_helper_consumer_chunks`: a synthetic `init_*` declaration consumes helpers in its assigned chunk, and an importer overlay consumes them in the importer's chunk, where its lowered import glue renders. They are passed through the additional-consumers channel; pre-lowering demand is re-scanned by the merge itself, so the proof sees the full post-lowering consumer set. The standalone-kept exit re-runs the proof too, because entry-facade restoration during lowering can tombstone chunks the pre-lowering merge counted as consumers — a sole consumer may exist only now.

The fold is deliberately narrower than the optimization-time merge:

- **Sole-consumer host only (`RuntimeMergeCascade::SingleConsumerOnly`).** Exec order and the order-wrap plan are already fixed. Any other host would create or reorder chunk-evaluation edges the order analysis never modeled: a dominator merge turns transitive reachability into a direct helper import whose exec-order sort position can hoist the host past sibling imports, and a bitset host can gain a brand-new consumer edge. Merging into the sole consumer only removes that consumer's edge to a runtime-only chunk — no new edges, no reordering.
- **ESM output only.** Under CJS output, `compute_cross_chunk_links` later gives every ESM-exports entry chunk — including zero-module facades minted during lowering — a speculative `__toCommonJS` demand that is invisible at fold time, so a chunk with no visible demand could be handed a brand-new require edge into a user chunk. Other formats keep the standalone or evicted layout.
- **No bits union.** The full cascade unions the runtime chunk's bits into the host so bits stay an exact reachability record. Here the runtime chunk's bits are the synthetic all-live union above; folding them in would widen the host's bits, which is user-visible through bits-derived chunk names. The sole-consumer host keeps its own bits.

**Regression coverage:** `crates/rolldown/tests/rolldown/issues/9463/` and `issues/9463_plain_group/` — the runtime folds into the sole consumer `common~a~b~shared` instead of shipping as a helper-only chunk, in both config cells; `issues/10265/` and `optimization/chunk_merging/dynamic_entry_merged_in_user_defined_entry/` — the only demand is order-introduced (synthetic `init_*` wrappers), and the runtime folds into the chunk hosting them, while the latter's cjs cells pin the ESM-only gate by keeping the standalone `rolldown-runtime.js`; the `function/experimental/strict_execution_order/` snapshots pin the folded layouts under wrap-all and on-demand wrapping (`top_level_await_syntax` pins the TLA sole-consumer case).

### Unused-Runtime Sweep (`sweep_unused_runtime_module`)

Tree-shaking includes runtime helpers at link time, before chunks exist, and some of its reasons are pessimistic: a star re-export chain ending at an external module registers `__reExport`/`__exportAll` demand that only chunking can invalidate. `find_entry_level_external_module` performs that walk-back (flattening the chain to chunk-level `export * from '<external>'` statements and re-propagating `has_dynamic_exports` to `false` through transitive star importers), and `finalized_module_namespace_ref_usage` then drops the namespace objects that only served the chain. The finalizer consequently emits no helper call — but the runtime module was already included and placed, so it used to ship as a dead chunk plus bare imports (#9374, #7233).

`sweep_unused_runtime_module` (in `runtime_module_sweep.rs`) closes that gap. It runs near the tail of `finalize_chunk_plan()`, after `try_merge_runtime_chunk`, order lowering, and the final namespace/external walk-back, but before liveness is sealed and cross-chunk links are derived. It re-derives runtime demand from the same post-walk-back facts the module finalizer renders from, through four source-module channels: per-module `depended_runtime_helper` flags (with `ReExport` discounted unless some included `export * from './normal'` importee still has `has_dynamic_exports` — the exact condition the finalizer checks; CommonJS importees always keep it), the namespace-object channel gated on `namespace_included` (sharing `LinkingMetadata::ns_star_external_re_export_emitted` with the finalizer so the prediction cannot diverge from the emission), runtime-owned symbols referenced by included statements, and `referenced_symbols_by_entry_point_chunk`. If `OrderWrapState` reports synthetic runtime-helper demand, the sweep is skipped conservatively because that demand does not live in link-stage metadata.

The sweep is **all-or-nothing and conservative**: any remaining demand — or any bail-out condition (tree-shaking disabled, runtime not included, runtime has side effects as in dev/HMR mode) — leaves everything exactly as tree-shaking decided. Only a runtime with zero demand is un-included: its statement/module inclusion is cleared, its symbols are purged from `used_symbol_refs` via `remove_owned_by`, it is removed from its chunk, and a now-empty chunk is tombstoned with `PostChunkOptimizationOperation::Removed`.

**Liveness invariants the sweep relies on.** Stale references to runtime symbols survive the sweep by design — namespace statements that will not render keep their `__exportAll` reference, chunk-level `depended_runtime_helper` bits keep their flags, and `compute_cross_chunk_links` speculatively inserts `__toCommonJS` for CJS-format ESM entries. All of these are filtered against `used_symbol_refs` before becoming imports or exports, so purging the runtime's symbols is what actually severs every cross-chunk edge. Every bare-import emitter tolerates `module_to_chunk == None`, and naming/rendering already skip tombstoned chunks (the same lifecycle the chunk optimizer's removals use). If the sweep removes the runtime after the provisional execution-order assignment, `finalize_chunk_plan()` re-derives chunk `exec_order`s and then rebuilds the live sorted-chunk list. Re-sorting the existing values is not enough when the runtime merely co-hosted a still-live `Common` chunk: a `Common` chunk keys on `modules[0]`, which the runtime occupied, so that chunk would otherwise keep an `exec_order` derived from the module it just lost and sort ahead of chunks it should follow. An entry chunk keys on its entry module instead, so losing the runtime does not move it. The sweep remains the **last writer** of `used_symbol_refs`: `generate()` seals the builder immediately after `finalize_chunk_plan()` returns.

**Regression coverage:** `crates/rolldown/tests/rolldown/issues/9374/` (snapshot-asserted: no runtime chunk for a multi-entry star-to-external chain, `minify: false` so vestigial namespace declarations would also surface); issues `6992`, `7115`, `7233`, `7233_chain` cover the same class under `preserveModules` for both ESM and CJS output; `crates/rolldown/tests/rolldown/topics/runtime/sweep_shared_chunk_exec_order/` pins the exec-order re-derivation above (the order of the two imports emitted at the top of `entry_a.js` in `artifacts.snap` is the whole assertion — swapping them means the regression is back).

**Why this shape**

Runtime used to be placed by normal bitset grouping and later peeled out when a cycle was detected. That made every optimizer responsible for understanding runtime-host edge cases. Standalone-first flips the default: the initial layout is always cycle-safe, and runtime-specific processing is confined to three final passes — the optional merge into a proven host after chunk optimization, the post-order-lowering re-placement and sole-consumer fold, then the unused-runtime sweep once the entry-level-external walk-back has settled what the finalizer will actually emit.

**Regression coverage**

- `crates/rolldown/tests/rolldown/issues/9401/` — `avoidRedundantChunkLoads` must not move the runtime into a user entry and create a helper cycle.
- `crates/rolldown/tests/rolldown/issues/8989/` — facade elimination introduces helper consumers; runtime may merge only into a downstream dominator.
- `crates/rolldown/tests/rolldown/issues/8920_2/` — fuzz-discovered shape where consumer-count heuristics produced a cycle.
- `crates/rolldown/tests/rolldown/issues/9597/` — a static + dynamic import cycle that previously placed the runtime chunk inside the cycle, throwing `__exportAll is not a function`; standalone-first keeps the runtime out of the cycle.

These fixtures assert structural invariants in `_test.mjs`, so runtime-placement regressions fail immediately rather than only showing up as snapshot drift.

## Lazy Module Initialization Order

`ensure_lazy_module_initialization_order()` runs after chunk creation as a post-processing step on the `ChunkGraph`. It fixes a correctness issue with lazy evaluation of wrapped modules.

### The Problem

When `strict_execution_order` is **not** enabled, CJS modules are wrapped in `__commonJSMin()` and their body doesn't execute until the wrapper's init function (`require_xxx()`) is explicitly called. Some ESM modules may also be wrapped in `__esm()` (e.g., those with circular dependencies or TLA), but most ESM modules remain unwrapped — their top-level code executes eagerly in the order it appears in the bundle.

During scope hoisting, each `require_xxx()` init call is placed at the point where the CJS module is first referenced. This default placement can produce incorrect initialization order when unwrapped ESM modules reference different wrapped CJS modules that have a dependency between them.

The root cause is how modules are laid out in the bundle. The link stage's `sort_modules()` (in `sort_modules.rs`) computes a global execution order via DFS of the import graph — in that analysis, `require()` is treated as an implicit static import so that required modules are ordered before their requirers. Modules are then emitted in this order. For **wrapped** modules (CJS/ESM), only the wrapper definition is placed at that position; the actual init call (`require_xxx()`) is placed wherever the module is first referenced by an **unwrapped** module. When two wrapped modules are referenced by different unwrapped modules, the init calls can end up in the wrong relative order.

Note: `sort_modules()` and `js_import_order()` (described below) are two different DFS analyses with different traversal rules. `sort_modules()` follows both `import` and `require()` edges to determine global execution order. `js_import_order()` only follows `import` edges because it specifically analyzes **eager** initialization — `require()` calls produce lazy wrappers that don't contribute to eager init order.

Consider this example (based on [#5531](https://github.com/rolldown/rolldown/issues/5531)):

```js
// leaflet.js (CJS → wrapped)
global.L = exports;
exports.foo = 'foo';

// leaflet-toolbar.js (CJS → wrapped, reads global.L)
global.bar = global.L.foo;

// lib.js (ESM → unwrapped, uses require internally)
require('./leaflet-toolbar.js');

// main.js (ESM → unwrapped)
import './leaflet.js';
import './lib.js';
assert.equal(bar, 'foo');
```

`sort_modules()` DFS from `main.js` produces: `leaflet(1) < leaflet-toolbar(2) < lib(3) < main(4)`. The execution order correctly puts `leaflet` before `leaflet-toolbar`. But in the bundled output, since both are **wrapped**, their wrapper definitions are just inert function declarations — what matters is where the init calls land:

- `lib.js` (exec_order 3, unwrapped) references `leaflet-toolbar` via `require()` → `require_leaflet_toolbar()` is placed here
- `main.js` (exec_order 4, unwrapped) references `leaflet` via `import` → `require_leaflet()` is placed here

Since `lib.js` appears before `main.js` in the bundle, `require_leaflet_toolbar()` runs first — but it needs `global.L` which `require_leaflet()` hasn't set yet:

```js
// ❌ Wrong output: require_leaflet_toolbar() runs before require_leaflet()
//#region lib.js
require_leaflet_toolbar(); // 💥 global.L is undefined here
//#endregion
//#region main.js
var import_leaflet = require_leaflet(); // too late — toolbar already failed
assert.equal(bar, 'foo');
//#endregion
```

Note: if `main.js` imported `leaflet-toolbar.js` directly (without `lib.js` as intermediary), both init calls would land in the same module region and rolldown would order them correctly. The problem only arises when init calls are split across different unwrapped modules.

**With** this pass, `require_leaflet()` is transferred from `main.js` to before `lib.js`'s region:

```js
// ✅ Correct output: require_leaflet() runs before require_leaflet_toolbar()
//#region lib.js
require_leaflet(); // ← transferred here by insert_map
require_leaflet_toolbar();
//#endregion
//#region main.js
assert.equal(bar, 'foo'); // require_leaflet() removed from here by remove_map
//#endregion
```

The function builds `insert_map` and `remove_map` on each chunk to move init calls from their default position to the correct one. `remove_map` suppresses the init call at the original location; `insert_map` prepends it before the module that needs it.

When `strict_execution_order` **is** enabled, the order plan wraps the eager carriers that need scheduling and closes over their dependent readers/importers. The plan owns lazy-init ordering for that output, so this pass is skipped entirely.

### Algorithm

The function iterates over every chunk in the `ChunkGraph` and performs six steps:

**Step 1 — Find DFS roots.** Entry chunks use the entry module as root. Common chunks have no single entry module, so roots are computed as modules not imported (via `ImportKind::Import`) by any other module _within the same chunk_ — i.e., the "top" of the chunk-local import graph. These are the modules that would execute first when the chunk loads, making them the correct starting points for the DFS that determines eager initialization order. Roots are sorted by execution order to ensure deterministic traversal.

**Step 2 — Build execution order map.** Collects execution order for all modules in the chunk, plus any wrapped modules from other chunks whose symbols are imported. This cross-chunk awareness is needed because a wrapped module in another chunk still requires its init call to run before dependents in this chunk.

**Step 3 — Classify modules via DFS (`js_import_order`).** Runs iterative DFS from roots, following only `ImportKind::Import` edges (skipping `require()` and `import()` since those are inherently lazy). Each visited module is classified:

- `WrapKind::Cjs` or `WrapKind::Esm` → pushed onto a `wrapped_modules` list
- `WrapKind::None` → records how many wrapped modules appeared before it in DFS order (its "wrapped dependency count")

Uses the immutable link-stage `wrap_kind()` from `LinkingMetadata`.

**Step 4 — Determine modules to check.** Collects all unwrapped modules that have wrapped dependencies, plus the wrapped modules they depend on (up to the maximum dependency count). If this set is empty, no reordering is needed and the function returns early.

**Step 5 — Find first init position.** Walks chunk modules in order, scanning import records. For each module in the check set, records the first `(importer, import_record_idx)` that imports it. Stops early once all positions are found.

**Step 6 — Build transfer maps.** Sorts init positions by execution order, then iterates:

- **Wrapped module** → added to `pending_transfer`
- **Unwrapped module** → pulls matching wrapped modules from `pending_transfer` and builds:
  - `insert_map[module_idx]` → init calls to prepend before this module's output
  - `remove_map[importer_idx]` → init calls to remove from their original location

A guard prevents transferring init calls from a lower-exec-order module to a higher one, which would incorrectly reorder execution.

### Helper: `js_import_order()`

Iterative DFS from the chunk's roots. Only follows `ImportKind::Import` edges — `require()` and `import()` are inherently lazy so they don't contribute to eager initialization order. Returns modules in DFS visit order.

### Output: `insert_map` and `remove_map`

These maps are stored on each `Chunk` and consumed during module finalization:

- **`remove_map`** — Read in `finalizer_context.rs`. The `ScopeHoistingFinalizer` checks whether any of the current module's import records should have their init calls suppressed (the init call is being moved elsewhere).
- **`insert_map`** — Read in `finalize_modules.rs`. For each target module, the rendered init call strings from the original locations are prepended to the module's output via `PrependRenderedImport` mutations.

```rust
// On Chunk (in rolldown_common::chunk)
pub insert_map: FxHashMap<ModuleIdx, Vec<(ModuleIdx, ImportRecordIdx)>>,
pub remove_map: FxHashMap<ModuleIdx, Vec<ImportRecordIdx>>,
```

## ChunkGraph

```rust
pub struct ChunkGraph {
    pub chunk_table: ChunkTable,                    // IndexVec<ChunkIdx, Chunk>
    pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
    pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
    pub post_chunk_optimization_operations: FxHashMap<ChunkIdx, PostChunkOptimizationOperation>,
    // ...
}
```

- `chunk_table` — All chunks, indexed by `ChunkIdx`. May contain removed chunks (marked in `post_chunk_optimization_operations`) since re-indexing would be expensive.
- `module_to_chunk` — Which chunk each module belongs to. O(1) lookup.

## Related

- [rust-bundler](../rust-bundler/implementation.md) — Build lifecycle
- `crates/rolldown/src/stages/generate_stage/mod.rs` — Generate stage entry point
- `crates/rolldown/src/stages/generate_stage/manual_code_splitting.rs` — User-defined chunk groups
- #8595 — Bug caused by bit position / chunk index mismatch when external entries exist
