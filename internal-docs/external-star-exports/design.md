# External star re-exports — Design & Principles

## Summary

`export * from '<external>'` does two jobs:

- It requests a module. The external must load before the body of the importer runs, in source order.
- It re-exports names that rolldown cannot list.

An entry chunk re-exports an external **at entry level** when an unbroken chain of `export *` leads from the entry module to the external. Such a chunk prints the re-export itself:

- In ESM, it prints `export * from '<external>'`.
- In CJS, IIFE, and UMD, it binds the external with `require` and merges the keys into `exports`.

Other code that reads these names reads them from the namespace object of the importer. That object merges the external with a runtime `__reExport` call. [implementation.md](./implementation.md) describes the implementation.

## Design principles

- **Each record is first an ordinary import of its own chunk.** The chunk that holds the importer imports the external at the exec-order position of the external, as for `import 'x'`. A printer can upgrade that import, but it never drops the import.

  The old rule (#2946) left every external star record out of the chunk imports. It assumed that another path printed the import. In #11092, and for a module in a shared chunk, no path printed it.

- **Entry level is a fact about a chunk.** One walk computes the two facts below, and each new walk replaces both:
  - `Chunk::entry_level_externals`: the externals that this chunk re-exports at entry level.
  - `ChunkGraph::entry_level_star_records`: the records that at least one live entry chunk re-exports at entry level.

  No import record stores the fact. The old `ImportRecordMeta::EntryLevelExternal` flag was global, and nothing cleared it. But rolldown rendered the re-exports per chunk. Thus the readers of the flag and the readers of the chunk list could disagree.

- **Printers upgrade in place.** An entry-level external is part of the chunk imports. The printers upgrade its import where the import is:
  - ESM prints `export * from` in the place of the bare import.
  - CJS, IIFE, and UMD bind the external in the import section. They merge its keys in the exports section.

  The old printers caused two bugs:
  - ESM printed the re-export after all imports. This changed the evaluation order (`import "ext2"; export * from "ext1"`).
  - CJS printed the `require` next to the merge. Thus the `require` ran after the body, and the code could skip the binding (`ReferenceError`).

## Trade-offs

- **Redundant loads.** When a facade and its implementation chunk both load the external, it is imported twice. Module caching makes this harmless. The alternative is the cross-chunk "someone else imports it" reasoning that caused #11092.
- **One `with` clause per import line.** A chunk prints one import line per external, so conflicting attributes for one external in one chunk (a pathological input) keep the first clause.
- **Eager under strict execution order.** As for every external, the chunk-level import loads at chunk load, even when the importer's body is wrapped.

## Rejected alternatives

- **Skip the import when every entry chunk in `chunk.bits` re-exports the external.** Chunk bits under-count loaders (a manual group takes its first module's bits, an order-wrap implementation chunk copies its entry's bits), and it keeps the cross-chunk assumption.
- **Keep a per-record flag.** See the principle "Entry level is a fact about a chunk" above.

## Unresolved questions

- Rolldown prints imports of other chunks before external imports. An external that the source loads before a sibling chunk still loads after it. This applies to every external import, not only to star records.
- The namespace path prints its `import * as` inside the module body, so in ESM it loads after the chunk-level imports.

## Related

- [implementation.md](./implementation.md) — the implementation of these principles
- `../code-splitting/implementation.md` — the place where the walk runs, and the runtime sweep that uses its result
