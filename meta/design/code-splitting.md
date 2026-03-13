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
```

**Key files:**

- `crates/rolldown/src/stages/generate_stage/code_splitting.rs` — pipeline orchestration
- `crates/rolldown/src/stages/generate_stage/chunk_optimizer.rs` — merge/optimization
- `crates/rolldown/src/chunk_graph.rs` — output data structure
- `crates/rolldown_utils/src/bitset.rs` — compact reachability representation

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
