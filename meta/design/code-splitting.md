# Code Splitting

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
    └─ split_chunks()
         │
         ├─ determine_reachable_modules_for_entry()   BFS per entry, set bits on reachable modules
         │
         ├─ apply_manual_code_splitting()             User-defined chunk groups (manualChunks)
         │
         ├─ Module assignment         Group modules by identical BitSet → chunks
         │
         └─ ChunkOptimizer           Merge common chunks into entry chunks, remove empty facades
              │
              ▼
         ChunkGraph                   Final module-to-chunk assignment

Post-ChunkGraph processing (in generate()):

ChunkGraph
    │
    ├─ compute_cross_chunk_links()                    Determine cross-chunk imports/exports
    │
    ├─ ensure_lazy_module_initialization_order()      Reorder wrapped module init calls
    │
    ├─ on_demand_wrapping()                           Strip unnecessary wrappers
    │
    └─ merge_cjs_namespace()                          Merge CJS namespace objects
```

**Key files:**

- `crates/rolldown/src/stages/generate_stage/code_splitting.rs` — pipeline orchestration, `generate_chunks()`, `ensure_lazy_module_initialization_order()`
- `crates/rolldown/src/stages/generate_stage/chunk_optimizer.rs` — merge/optimization
- `crates/rolldown/src/chunk_graph.rs` — output data structure
- `crates/rolldown_utils/src/bitset.rs` — compact reachability representation
- `crates/rolldown/src/types/linking_metadata.rs` — `original_wrap_kind()` used for init order analysis

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

## Reachability Propagation

`determine_reachable_modules_for_entry()` runs BFS from each entry module, setting `splitting_info[module].bits.set_bit(entry_index)` on every reachable module. External modules are skipped during traversal (they're not `Module::Normal`).

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

The trade-off of merging: entry chunks may include modules that not all consumers of that entry need. This adds a small amount of unnecessary code loading but significantly reduces chunk count and HTTP requests.

### Facade Elimination (`optimize_facade_entry_chunks`)

Dynamic/emitted entries can become empty facades when all their modules are pulled into other chunks by the optimizer. The optimizer identifies these and either:

- Merges the facade into its target chunk
- Marks it as `Removed` in `post_chunk_optimization_operations`

### Runtime Module Placement

Facade elimination can introduce **new runtime-helper consumers** after the merge phase has already placed the runtime module. Eliminating a dynamic-import facade runs two independent `wrap_kind`-gated branches on the target chunk, and either branch adds the chunk to `runtime_dependent_chunks`:

- `WrapKind::Esm | WrapKind::None` — `include_symbol(module.namespace_object_ref)` materializes the simulated namespace and explicitly inserts `RuntimeHelper::ExportAll` into the target chunk's `depended_runtime_helper` (emitted JS symbol: `__exportAll`).
- `WrapKind::Cjs | WrapKind::Esm` — `include_symbol(wrapper_ref)` pulls in the `require_xxx` symbol, which transitively drags whatever helpers the wrapper depends on (`RuntimeHelper::ToEsm`, `RuntimeHelper::CommonJsMin`, etc., emitted as `__toESM`, `__commonJSMin`, …) via the existing inclusion-propagation machinery.

`WrapKind::Esm` hits both branches, so ESM facades can add `ExportAll` _and_ wrapper-driven helpers to the same chunk.

The danger is that the runtime module may already be **co-located** with other modules in some host chunk from the merge phase (the chunker placed it there because the host's bitset matched the runtime's bitset). If the new helper-import edge points from a facade-elim consumer back to that host, and the host has any forward path back to the consumer, the dependency graph closes a cycle. See [#8989](https://github.com/rolldown/rolldown/issues/8989) for the canonical reproduction:

```
chunk(node2) ──forward──> chunk(node3) ──forward──> chunk(node4)
     ▲                                                   │
     └──────── helper edge after facade elim ────────────┘
```

The placement logic lives in `rehome_runtime_module`, called from `optimize_facade_entry_chunks` whenever `runtime_dependent_chunks` is non-empty. It is a **two-step decision** driven by static-import reachability between chunks:

**Step 1 — Peel decision (cycle risk only)**

Peel the runtime out of its current host chunk only when the host has a **static forward path** to some facade-elim consumer that is not the host. That is the precondition for a back-edge cycle: without such a path, the new helper import cannot close a cycle no matter where we place the runtime, so the most compact layout is to leave it where the merge phase already put it. Reachability is computed by `chunk_reaches_via_static_import`, a BFS that follows only `ImportKind::Import` edges through still-live target chunks.

When cycle risk is present and the host has other modules, the implementation removes `runtime_module_idx` from the host's `modules` vec via `swap_remove` (ordering doesn't matter — `sort_chunk_modules` re-establishes it later) and sets `module_to_chunk[runtime_module_idx] = None`. If runtime is alone in its host chunk, it stays there — that chunk is already a leaf and cannot participate in a cycle, and peeling would leave an empty chunk that downstream code expecting `chunk.modules[0]` would choke on.

**Step 2 — Placement decision (dominator search)**

When the runtime is unplaced (either because Step 1 peeled it, or because the merge phase never placed it), compute the full consumer set:

```
consumer_chunks = (non-removed chunks with non-empty depended_runtime_helper)
                ∪ runtime_dependent_chunks
                ∪ ({original_host} if original_host is not marked Removed)
```

The first term picks up chunks that already required helpers from the linking stage; the second term picks up chunks that facade elimination just announced; the third term re-adds the original host — the merge phase placed the runtime there because its bitset required it, making it an implicit consumer. The "not Removed" gate is defensive: `apply_common_chunk_merges` already retargets `module_to_chunk` when a host is merged into another chunk, so in practice `original_host` resolves to a still-live chunk. Deduplication is automatic via `FxHashSet`.

Then find a **dominator** — a member C such that every other consumer statically reaches C via forward edges. `find_consumer_dominator` checks each candidate with `chunk_reaches_via_static_import`. A dominator, if any, is a downstream sink of the consumer set: placing the runtime there means every other consumer's helper import rides an existing forward edge, so no back-edge is added and no cycle can form.

- **Dominator found** → runtime moves into that chunk. No extra chunk is created.
- **No dominator** (consumers sit in parallel sub-graphs or form a more complex shape) → runtime is placed in a fresh `rolldown-runtime.js` chunk created with the runtime's bitset. Every consumer imports from it. This chunk is structurally a leaf — not because being freshly created prevents outgoing edges, but because the runtime module itself contains no `import` statements, so the only module assigned to the chunk has no dependencies for the cross-chunk linker to translate into outgoing imports. Cycles are therefore impossible.

**Why this shape**

Relying on `runtime_dependent_chunks.len()` alone undercounts — it ignores chunks that already required helpers from the linking stage and the original host. Relying on consumer count alone (splitting the "single consumer" case from the "many consumers" case) over-triggers and under-triggers both: a sole consumer can still sit in the middle of the graph and create a cycle via back-edges from other implicit consumers (fuzz-discovered case in [#8920](https://github.com/rolldown/rolldown/issues/8920)), and a set of multiple consumers may have a natural downstream sink that hosts the runtime at zero extra cost ([#8989](https://github.com/rolldown/rolldown/issues/8989)).

The dominator search unifies both by asking the right question directly: "is there a chunk every consumer already reaches forward?". If yes, reuse it; if no, add a leaf.

**Regression coverage**

- `crates/rolldown/tests/rolldown/issues/8989/` — original cycle. Four entries with `node3` dynamically importing `node4` and `node1` namespace-importing `node2`. The merge phase drops the runtime into `entry2` (which already forward-reaches `node4` via `entry3`). Cycle risk → peel. Dominator search picks `node4` (leaf, all consumers reach it). Assertions cover the leaf invariant, the `entry2 → node4` direction, and that `node4` hosts the runtime.
- `crates/rolldown/tests/rolldown/issues/8920_2/` — fuzz-discovered shape where the previous single-consumer rule silently produced a cycle. Two entries with only a dynamic edge between them; `node1` is the shared common chunk. The merge phase places the runtime in `entry-2`, but `entry-2` has no static outbound edges — no cycle risk. Runtime stays in `entry-2`, the dominator of `{entry-2, node1}` by virtue of `node1 → entry-2` already being a forward static edge. Three chunks, no `rolldown-runtime.js` emitted.

Both fixtures assert structural invariants in `_test.mjs`, so any regression (e.g. reverting to the single-consumer-picks-itself rule, or over-peeling when no cycle risk exists) fails immediately rather than only showing up as a snapshot diff.

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

When `strict_execution_order` **is** enabled, all modules are already wrapped and execute in the correct order, so this pass is skipped entirely.

### Algorithm

The function iterates over every chunk in the `ChunkGraph` and performs six steps:

**Step 1 — Find DFS roots.** Entry chunks use the entry module as root. Common chunks have no single entry module, so roots are computed as modules not imported (via `ImportKind::Import`) by any other module _within the same chunk_ — i.e., the "top" of the chunk-local import graph. These are the modules that would execute first when the chunk loads, making them the correct starting points for the DFS that determines eager initialization order. Roots are sorted by execution order to ensure deterministic traversal.

**Step 2 — Build execution order map.** Collects execution order for all modules in the chunk, plus any wrapped modules from other chunks whose symbols are imported. This cross-chunk awareness is needed because a wrapped module in another chunk still requires its init call to run before dependents in this chunk.

**Step 3 — Classify modules via DFS (`js_import_order`).** Runs iterative DFS from roots, following only `ImportKind::Import` edges (skipping `require()` and `import()` since those are inherently lazy). Each visited module is classified:

- `WrapKind::Cjs` or `WrapKind::Esm` → pushed onto a `wrapped_modules` list
- `WrapKind::None` → records how many wrapped modules appeared before it in DFS order (its "wrapped dependency count")

Uses `original_wrap_kind()` from `LinkingMetadata`, which preserves the pre-`strictExecutionOrder` wrap kind.

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

- [rust-bundler](./rust-bundler.md) — Build lifecycle
- `crates/rolldown/src/stages/generate_stage/mod.rs` — Generate stage entry point
- `crates/rolldown/src/stages/generate_stage/manual_code_splitting.rs` — User-defined chunk groups
- #8595 — Bug caused by bit position / chunk index mismatch when external entries exist
