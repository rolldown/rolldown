# External star re-exports — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

One walk in the generate stage decides which entry chunks re-export which externals.

## Facts

- `Chunk::entry_level_externals: Vec<EntryLevelExternal>` (`rolldown_common/src/chunk/types/entry_level_external.rs`): the externals that this entry chunk re-exports, sorted by exec order. `attribute_record` is the first record in the walk that has import attributes.
- `ChunkGraph::entry_level_star_records`: the `(module, record)` pairs that some live entry chunk flattens. Read through `ChunkGraph::is_entry_level_star_record`.

## Producer

`find_entry_level_external_module` (`stages/generate_stage/code_splitting.rs`) runs in `finalize_chunk_plan`, before each cross-chunk analysis (up to three times), and `finalized_module_namespace_ref_usage` follows it.

1. Clear both facts.
2. From the entry module of each live `EntryPoint` chunk, walk the `export *` edges breadth-first. Each external record is added to the set, and its external to the chunk list.
3. Re-propagate `has_dynamic_exports`. The seeds are the modules with flattened records from this walk or the previous one, so a record that is no longer flattened gets its dynamic exports back. Modules whose namespace is observed (`Unknown`) are not seeds. The transitive importers of the seeds are recomputed too. A flattened record does not count as a dynamic export (`propagate_has_dynamic_exports`).

## Consumers

- **Namespace emission.** `LinkingMetadata::ns_star_external_re_export_emitted(is_entry_level, format)` decides whether the namespace declaration prints `import * as` plus `__reExport` (ESM) or `__reExport(ns, require(...))`. The finalizer (`generate_declaration_of_module_namespace_object`) and the runtime sweep (`runtime_helpers_still_demanded`) call it.
- **ESM** (`render_esm`). After the imports, each entry-level external is printed as `export * from`, with the `with` clause of `attribute_record`.
- **CJS, IIFE, UMD** (`render_chunk_exports`). Each entry-level external is required, unless a direct import already covers it, and its keys are merged into `exports`.
- **Other readers.** Deconflicting, `OutputChunk.imports`, the order-wrap facade collapse, and inline common chunk selection read the chunk list.

## Tests

- `issues/9374`, `topics/runtime/sweep_shared_chunk_exec_order`, and `function/experimental/strict_execution_order/*external_star*`: the walk-back, the runtime sweep, and facades.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- `../code-splitting/implementation.md` — the chunk plan and the runtime sweep
