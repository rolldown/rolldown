# Import defer — Design

> How this is realized in code lives in [implementation.md](./implementation.md).

## Summary

`experimental.importDefer` keeps the deferred evaluation of `import defer` in the output. The support grows in steps. Each step supports more shapes of a build, and a build with another shape falls back to an eager import. A build with the option off must give the same output as before the option existed.

## Whole-build fallback

### Goal

A build with a shape that the deferral does not support yet gives the output of the build with the option off, and a warning for each deferred import. Thus each step can add the deferral for some shapes, and all other shapes keep the eager result that they had before.

### Decisions

- The decision is for the whole build, not for each import. The conditions (one chunk, no cycle, no external module in the closures, the order rule of top-level `await`) are properties of the module graph. A decision for each import needs rules for deferred and eager closures across chunks, cycles and async modules.
- The decision runs after the scan and before the link stage, in `scan_modules_impl`. With `experimental.incrementalBuild`, the scan sees only the changed modules, and the normalized scan output has all modules. An error (a write to a deferred namespace in a build that falls back) then fails the build as a scan error does: `buildEnd` and `closeBundle` get the error, and no link step adds more errors.
- The scanner keeps the eager results and saves the deferred results apart. Thus a fallback gives the scan results of the option-off build exactly. A second analysis at link time cannot give the same results, because the scan reads a partial `constant_export_map` that the link stage does not have.
- The conditions are conservative. For example, a static import cycle anywhere in the graph makes the build fall back.

### Rejected alternatives

- A decision at the start of the link stage. `buildEnd` then gets no error for a write to a deferred namespace, and the build fails later than with the option off.
- An analysis of the statements at link time with the eager rules, instead of the saved scan results. The results can differ from the option-off build (see the `constant_export_map` above).

## Shared CommonJS namespaces

### Goal

The proposal gives one deferred namespace for each module. Without a merge, each `import defer` of a CommonJS module has its own `__deferNs` object and its own `__toESM` namespace, also for two imports in one file. An eager import shares one namespace for the importers with no wrapper in one chunk (`merge_cjs_namespace`). The deferred form has the same scope.

### Decisions

- The deferred records have their own groups, apart from the groups of `merge_cjs_namespace`. An eager import assigns `import_x` at the import statement, and a deferred access assigns it at the first access. One shared binding would get two `__toESM` objects in one run.
- The interop mode is part of the key. The eager merge keys by module only, so its merged binding has the mode of the first importer. A deferred access assigns `import_x` with the mode of its own importer, so a merge by module only would make the mode depend on the access that runs first.
- Only importers with no wrapper take part, as in the eager merge. A wrapped importer declares its symbols inside its wrapper, where other modules cannot read them.
- Nothing merges with `strictExecutionOrder`, as in the eager merge. The order wraps of the generate stage give the importers wrappers that `wrap_kind()` does not show. A merge then puts the declaration in the wrapper of one importer, and another importer can read it before that wrapper runs. The strict variant of the fixture `commonjs_shared_namespace` checks this rule.

### Rejected alternatives

- A declaration at the top of the chunk, which also covers the importers with a wrapper. `__deferNs` evaluates nothing, so the declaration can move there. This needs new placement code, and the eager namespace has the same gap.

## Top-level await

### Goal

A deferred import of a module with top-level `await` keeps the steps of native Node where a bundle can: the importer waits for the modules with top-level `await` first, a later access evaluates the rest synchronously, and an access to a module that is still evaluating throws a `TypeError`.

### Decisions

- Only the tracked modules get a state record (`__esmState`). The other modules of a closure keep `__esmMin`, so a closure with no top-level `await` costs nothing more.
- A module with top-level `await` ends before the next import starts, as in a build without `import defer`. Thus the preparation waits for each module in the order of its walk. Native Node starts all of them first, and then waits. The fixture `tla_gather_order` checks the order of the walk.
- A build falls back when a module with no wrapper has an import after a deferred import of a module that reaches a top-level `await`. In the output, the code of that import runs before the modules with top-level `await` start, but native Node starts them first. The fallback gives the eager order, in which the modules with top-level `await` also run first. A build with `strictExecutionOrder` also falls back, because the fallback runs before the link stage and does not know the wrappers.

### Rejected alternatives

- A fallback for each build with top-level `await`. The preparation and the state records give the native result for the other shapes, also when an import comes before the deferred import.

## Unresolved questions

- Which steps remove which conditions: cycles, more than one chunk, `import.defer()`, external modules and the other output formats.
- Can the preparation run at the position of the deferred import in a module with no wrapper? It must run before the code of the first later import, at the top level of the chunk, as `ensure_lazy_module_initialization_order` moves an init call (`Chunk::insert_map`). Then the order rule can go away.

## Related

- [implementation.md](./implementation.md) — the scan, the fallback, the link stage, the finalizer and the CommonJS targets
