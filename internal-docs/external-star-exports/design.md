# External star re-exports — Design & Principles

## Summary

`export * from '<external>'` does two jobs:

- It requests a module. The external must load before the body of the importer runs, in source order.
- It re-exports names that rolldown cannot list.

An entry chunk re-exports an external **at entry level** when an unbroken chain of `export *` leads from the entry module to the external. Such a chunk prints the re-export itself:

- In ESM, it prints `export * from '<external>'`.
- In CJS, IIFE, and UMD, it requires the external and merges the keys into `exports`.

Other code that reads these names reads them from the namespace object of the importer. That object merges the external with a runtime `__reExport` call. [implementation.md](./implementation.md) describes the implementation.

## Design principles

- **Entry level is a fact about a chunk.** One walk computes the two facts below, and each new walk replaces both:
  - `Chunk::entry_level_externals`: the externals that this chunk re-exports at entry level.
  - `ChunkGraph::entry_level_star_records`: the records that at least one live entry chunk re-exports at entry level.

  No import record stores the fact. The old `ImportRecordMeta::EntryLevelExternal` flag was global, and nothing cleared it. But rolldown rendered the re-exports per chunk. Thus the readers of the flag and the readers of the chunk list could disagree.

## Related

- [implementation.md](./implementation.md) — the implementation of these principles
- `../code-splitting/implementation.md` — the place where the walk runs, and the runtime sweep that uses its result
