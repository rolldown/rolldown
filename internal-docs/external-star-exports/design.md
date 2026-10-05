# External star re-exports — Design & Principles

## Summary

`export * from '<external>'` is two things at once: a module request (the external must load, in source order, before the importer's body runs) and a re-export of names rolldown cannot list. An entry chunk that reaches the record through an unbroken `export *` chain re-exports the external at entry level: `export * from` in ESM, a `require` plus a key merge elsewhere. Any other observer of the names gets a runtime `__reExport` through the importer's namespace object. The machinery is in [implementation.md](./implementation.md).

## Design principles

- **Entry level is a per-chunk fact.** `Chunk::entry_level_externals` (what this chunk re-exports) and `ChunkGraph::entry_level_star_records` (records that some live entry chunk flattens) come from one walk and are rebuilt together. No import record stores the fact. The old `ImportRecordMeta::EntryLevelExternal` flag was global and never cleared, while the rendering was per chunk, so its readers and the readers of the chunk list could disagree.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../code-splitting/implementation.md` — where the walk runs, and the runtime sweep that depends on it
