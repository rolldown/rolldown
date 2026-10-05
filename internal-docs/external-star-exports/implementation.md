# External star re-exports — Implementation

> [design.md](./design.md) gives the reasons and principles behind this implementation.

## Summary

One walk in the generate stage decides which entry chunks re-export which externals at entry level. Each entry-level external is part of the imports of its chunk. The printers upgrade its import in place.

## Terms

- **External star record**: an `export * from '<external>'` import record.
- **At entry level**: an entry chunk re-exports an external at entry level when an unbroken chain of `export *` leads from the entry module to the external.
- **Entry-level star record**: an external star record that at least one live entry chunk re-exports at entry level.
- **Live chunk**: a chunk that post-chunk optimization did not remove (`ChunkGraph::chunk_is_live`).

## Facts

- `Chunk::entry_level_externals: Vec<EntryLevelExternal>` (`rolldown_common/src/chunk/types/entry_level_external.rs`): the externals that this entry chunk re-exports at entry level, sorted by exec order. `attribute_record` is the first record with a `with` clause, in the order of the walk.
- `ChunkGraph::entry_level_star_records`: the entry-level star records, as `(module, record)` pairs. Code reads the set through `ChunkGraph::is_entry_level_star_record`.

## Producer

`find_entry_level_external_module` (`stages/generate_stage/code_splitting.rs`) runs in `finalize_chunk_plan` before each cross-chunk analysis. It runs up to three times. `finalized_module_namespace_ref_usage` runs after it each time.

1. Clear both facts.
2. Start a walk at the entry module of each live `EntryPoint` chunk. The walk follows the `export *` edges in breadth-first order.
3. Add each external star record that the walk finds to the set. Add its external to the list of the chunk.
4. Re-propagate `has_dynamic_exports`:
   - The seeds are the modules with an entry-level star record from this walk or from the previous walk. Thus a module whose record is no longer at entry level gets its dynamic exports back.
   - A module with an observed namespace (`Unknown`) is not a seed.
   - The function also re-propagates the flag to the transitive importers of the seeds.
   - An entry-level star record does not count as a dynamic export (`propagate_has_dynamic_exports`).

## Consumers

- **Namespace emission.** `LinkingMetadata::ns_star_external_re_export_emitted(is_entry_level, format)` decides if the namespace declaration merges the external at runtime. In ESM, the merge is `import * as` plus `__reExport`. In other formats, the merge is `__reExport(ns, require(...))`. Two parts call this function: the finalizer (`generate_declaration_of_module_namespace_object`) and the runtime sweep (`runtime_helpers_still_demanded`).
- **Chunk imports** (`compute_cross_chunk_links.rs`). `collect_depended_symbols` adds each entry-level external of a chunk to the external imports of that chunk.
- **ESM** (`render_esm_chunk_imports`). `render_esm_chunk_imports` prints no bare import for an entry-level external. It prints `export * from` after the named imports of the external, with the `with` clause of `attribute_record`.
- **CJS, IIFE, UMD.**
  - `render_cjs_chunk_imports` and `render_chunk_external_imports` bind an entry-level external, also when its namespace symbol is unused.
  - `render_chunk_exports` prints only the key merge.
  - `determine_export_mode` resolves `auto` to `named` for a chunk with entry-level externals.
  - IIFE and UMD count these externals in `has_exports`.
- **Guards.** The order-wrap facade collapse and the selection of inline common chunks skip chunks with entry-level externals.

## Tests

These fixtures check the position and the binding at runtime:

- `function/external/export_star_keeps_import_order`
- `function/external/cjs_export_star_loads_before_body`
- `function/external/cjs_export_star_with_bare_import`

These fixtures cover the walk-back, the runtime sweep, and the facades:

- `issues/9374`
- `topics/runtime/sweep_shared_chunk_exec_order`
- `function/experimental/strict_execution_order/*external_star*`

## Related

- [design.md](./design.md) — the principles and trade-offs behind this implementation
- `../code-splitting/implementation.md` — the chunk plan and the runtime sweep
