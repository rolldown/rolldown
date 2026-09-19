# Module Side Effects — Design & Principles

## Summary

Every normal module gets one `DeterminedSideEffects` verdict, which decides whether
tree-shaking may drop it. Three sources can supply it, and they are ranked. For the
machinery, see [implementation.md](./implementation.md).

## Design principles

1. **Ranked sources.** A `resolveId` or `load` hook override beats `package.json#sideEffects`,
   which beats statement analysis. An explicit answer always wins over an inferred one.
2. **The verdict does not depend on the specifier.** A file imported as `./unused.js` and the
   same file imported as `@unused` must get the same verdict. Resolution attaches the package
   metadata that principle 1 needs, so every resolution path has to attach the same metadata.

Principle 2 is the one that broke in [#10909](https://github.com/rolldown/rolldown/issues/10909).
A `resolveId` hook may return a bare id string. That is a supported return type, and it carries
no `packageJsonPath`, so the module arrived with no package policy while the same file reached
through a relative import arrived with one. Two things then went wrong at once:

- The module was kept even with a single importer, because statement analysis saw its
  `console.log`. That is a plain correctness bug, with no race involved.
- With two importers, `ModuleLoader::try_spawn_new_task` keeps the first `ResolvedId` for an id
  and discards the second, and the two carried different metadata. The winner followed task
  completion order, so the same input produced two different bundles.

## Rejected alternative

Merging the discarded `ResolvedId` into the module on the second visit. It cannot work: the
module may already be loaded, and a later merge would override an earlier hook's explicit
choice, which principle 1 forbids. Recovering the metadata at resolution time makes both
`ResolvedId`s equal instead, so there is nothing left to merge.

## Unresolved questions

- `determine_module_exports_kind` still walks the module table by `ModuleIdx`, and the first
  importer decides an `ExportsKind::None` importee's kind. That is a second, independent
  order dependence. #10909 reports it as an unstable `__toESM` flag. Nothing here addresses it.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../module-id/implementation.md` — what an id is, and when it is a real path
