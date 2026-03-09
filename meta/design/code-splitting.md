# Code Splitting

## Summary

Code splitting determines which modules go into which output chunks. It uses a BitSet-based reachability model: each entry point gets a bit position, modules are marked with the set of entries that can reach them, and modules with identical reachability patterns are grouped into the same chunk. An optimization pass then merges small common chunks into entry chunks when safe.

## Pipeline

```
LinkStageOutput.entries
    │
    ▼
init_entry_point()          Assign bit positions, create entry chunks
    │
    ▼
determine_reachable_modules_for_entry()   BFS per entry, set bits on reachable modules
    │
    ▼
split_chunks()              Group modules by identical BitSet → chunks
    │
    ▼
ChunkOptimizer              Merge common chunks into entry chunks, remove empty facades
    │
    ▼
ChunkGraph                  Final module-to-chunk assignment
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
entry_index 2  →  @optional/ext   →  bit 2  →  (external, no chunk)
entry_index 3  →  plugin.js       →  bit 3  →  ChunkIdx(2)
```

External modules get bit positions but no chunks. This means **bit positions do not equal chunk indices**. The mapping is stored in `chunk_graph.bit_to_chunk_idx: Vec<Option<ChunkIdx>>` — `None` for external entries, `Some(idx)` for real chunks.

### Invariant

Any code converting a bit position to a `ChunkIdx` **must** use `bit_to_chunk_idx`, not `ChunkIdx::from_raw(bit_position)`. Violating this produces wrong chunk assignments when external entries exist.

## Reachability Propagation

`determine_reachable_modules_for_entry()` runs BFS from each entry module, setting `splitting_info[module].bits.set_bit(entry_index)` on every reachable module.

After all entries are processed, each module's `bits` encodes which entries can reach it:

```
shared.js:    bits = 1111  (reachable from all 4 entries)
parser-a.js:  bits = 1010  (reachable from entries 1 and 3)
entry-a.js:   bits = 0001  (only reachable from entry 0)
```

## Chunk Creation

`split_chunks()` groups modules by identical `bits` patterns:

1. Entry chunks already exist from `init_entry_point()` with their single-bit patterns
2. For each non-entry module, look up `bits_to_chunk[module.bits]`
3. If a chunk exists for that pattern, add the module to it
4. Otherwise, create a new `Common` chunk

Modules with the same reachability pattern always land in the same chunk. This is the core invariant that makes code splitting correct.

## Chunk Optimizer

When enabled, the optimizer reduces chunk count by merging common chunks into entry chunks. It operates on a temporary `ChunkOptimizationGraph`.

### Common Module Merging (`try_insert_common_module_to_exist_chunk`)

For each common chunk, translates its `bits` to chunk indices via `bit_to_chunk_idx`, then tries to merge it into one of those entry chunks. Merging is skipped if it would:

- Create a circular dependency between chunks
- Change an entry's export signature (when `preserveEntrySignatures: 'strict'`)

### Facade Elimination (`optimize_facade_entry_chunks`)

Dynamic/emitted entries can become empty facades when all their modules are pulled into other chunks. The optimizer identifies these and either:

- Merges the facade into its target chunk
- Marks it as `Removed` in `post_chunk_optimization_operations`

Circular dependency checks (`would_create_circular_dependency`) run before every merge to preserve acyclicity.

## ChunkGraph

```rust
pub struct ChunkGraph {
    pub chunk_table: ChunkTable,                    // IndexVec<ChunkIdx, Chunk>
    pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
    pub bit_to_chunk_idx: Vec<Option<ChunkIdx>>,    // bit position → chunk index
    pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
    pub post_chunk_optimization_operations: FxHashMap<ChunkIdx, PostChunkOptimizationOperation>,
    // ...
}
```

- `chunk_table` — All chunks, indexed by `ChunkIdx`. May contain removed chunks (marked in `post_chunk_optimization_operations`).
- `module_to_chunk` — Which chunk each module belongs to. O(1) lookup.
- `bit_to_chunk_idx` — Translates entry bit positions to chunk indices. `None` for external entries.

## Related

- [rust-bundler](./rust-bundler.md) — Build lifecycle
- `crates/rolldown/src/stages/generate_stage/mod.rs` — Generate stage entry point
- `crates/rolldown/src/stages/generate_stage/manual_code_splitting.rs` — User-defined chunk groups
- #8595 — Bug caused by bit position / chunk index mismatch
