# Import defer — Design

> How this is realized in code lives in [implementation.md](./implementation.md).

## Summary

`experimental.importDefer` keeps the deferred evaluation of `import defer` in the output. The support grows in steps. Each step supports more shapes of a build, and a build with another shape falls back to an eager import. A build with the option off must give the same output as before the option existed.

## Whole-build fallback

### Goal

A build with a shape that the deferral does not support yet gives the output of the build with the option off, and a warning for each deferred import. Thus each step can add the deferral for some shapes, and all other shapes keep the eager result that they had before.

### Decisions

- The decision is for the whole build, not for each import. The conditions (one chunk, no cycle, no top-level `await`, only ES modules in the closures) are properties of the module graph. A decision for each import needs rules for deferred and eager closures across chunks, cycles and async modules.
- The decision runs after the scan and before the link stage, in `scan_modules_impl`. With `experimental.incrementalBuild`, the scan sees only the changed modules, and the normalized scan output has all modules. An error (a write to a deferred namespace in a build that falls back) then fails the build as a scan error does: `buildEnd` and `closeBundle` get the error, and no link step adds more errors.
- The scanner keeps the eager results and saves the deferred results apart. Thus a fallback gives the scan results of the option-off build exactly. A second analysis at link time cannot give the same results, because the scan reads a partial `constant_export_map` that the link stage does not have.
- The conditions are conservative. For example, a static import cycle anywhere in the graph, or top-level `await` in any module, makes the build fall back.

### Rejected alternatives

- A decision at the start of the link stage. `buildEnd` then gets no error for a write to a deferred namespace, and the build fails later than with the option off.
- An analysis of the statements at link time with the eager rules, instead of the saved scan results. The results can differ from the option-off build (see the `constant_export_map` above).

## Unresolved questions

- Which steps remove which conditions: CommonJS and JSON modules, top-level `await`, cycles, more than one chunk, `import.defer()`, external modules and the other output formats.

## Related

- [implementation.md](./implementation.md) — the scan, the fallback, the link stage and the finalizer
