# Import defer — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

With `experimental.importDefer`, a build keeps the deferred evaluation of `import defer * as ns from './x.js'` when the build has a shape that the deferral supports (see "Whole-build fallback"). Each module of a deferred closure gets a lazy init wrapper. The first access to the deferred namespace calls the wrapper of the target, and that wrapper calls the wrappers of the modules that the target imports. Any other build lowers each deferred import to a normal import, with a warning, and gives the output of the build with the option off.

`import.defer()` is not supported yet: the pre-processor always lowers it to `import()`, with a warning (`crates/rolldown/src/utils/tweak_ast_for_scanning.rs`). Dev mode turns the option off (`crates/rolldown/src/utils/prepare_build_context.rs`).

Example (fixture `topics/import_defer/basic`, output shortened):

```js
// main.js
import defer * as dep from './dep.js';
import { log } from './log.js';

export const before = [...log];
export const value = dep.value;
```

```js
function init_dep() {
  return (init_dep = __esmMin(() => {
    init_log();
    log.push('dep');
  }))();
}
// main.js
init_log();
const before = [...log];
const value = (init_dep(), 1);
```

## Scan

- The scanner sets `ImportRecordMeta::DeferPhase` on the record of each `import defer` (`scan_import_decl` in `crates/rolldown/src/ast_scanner/mod.rs`). With the option off, the pre-processor removes the phase before the scan, so no record has the flag.
- The scan results are the results of an eager import, as with the option off. A deferred namespace is in `namespace_object_symbol_ids`, so a read `ns.foo;` has no side effect, and a write `ns.foo = 1` gives the `assign_to_import` error.
- The scanner also saves the results of the deferred import in `EcmaView::import_defer_scan`:
  - `eval_flags`: the flags of each statement that change when a read of a deferred namespace has a side effect (the read evaluates the module). A second `StmtEvalAnalyzer` run (`with_deferred_namespaces`) gives them. Both runs read the same `constant_export_map` of the scan.
  - `namespace_writes`: the data of the `assign_to_import` error of each write to a deferred namespace. The scanner does not report these errors. A deferred namespace rejects a write at run time, so a build that keeps the deferral has no error.

## Whole-build fallback

`apply_import_defer_support` (`crates/rolldown/src/utils/import_defer_fallback.rs`) decides one time for each build. `scan_modules_impl` (`crates/rolldown/src/bundle/bundle.rs`) calls it after `normalize_scan_stage_output_and_update_cache`. Thus it sees all modules, also in an incremental build that scans only some of them. The cache keeps its own copy of the scan output, so the changes do not go into the cache.

The build keeps the deferral when all of these conditions are true:

1. The output format is `esm` or `cjs`.
2. The build makes one chunk: one entry record (each `EntryPoint`, also when one module has several), no `preserveModules`, no chunk groups, no `output.codeSplitting.experimentalInlineCommonChunks`, and no `import()`.
3. No module has top-level `await`.
4. The static import records make no cycle.
5. Each module of a deferred closure (a target and its static dependencies) is a JavaScript module of the build, not CommonJS, with only `import` statements as static imports, and no `require()` in the build loads it. A module with `ExportsKind::None` becomes CommonJS when a `require()` loads it.

When the build keeps the deferral, the saved `eval_flags` go into `stmt_infos`. The new flags are the union of both runs, because the deferred run only adds side effects. A module with `DeterminedSideEffects::Analyzed(false)` gets `Analyzed(true)` when one of its statements gets `UnknownSideEffect`.

When the build falls back, `DeferPhase` goes away from each record. If the build has a write to a deferred namespace, the saved errors fail the build with the hooks of a scan error (`buildEnd` and `closeBundle` get the errors). Else each deferred import gets an `UNSUPPORTED_FEATURE` warning that names the reason. All other scan results are already the results of an eager import, so the output is the output of the build with the option off.

## Link stage

- `sort_modules` visits the deferred records of a module after its eager records. A module that only a deferred record reaches thus gets a position after the eager dependencies of the importer and before the importer.
- `wrap_import_defer_closures` (`crates/rolldown/src/stages/link_stage/wrapping.rs`) gives `WrapKind::Esm` to each module of each closure and sets `wrapped_by_import_defer`. The loop of `wrap_modules` then wraps the rest of their graph, as for any wrapped module.
- `create_deferred_namespaces` adds a facade statement that declares `x_deferred` for each target. `advance_import_tracker` (`bind_imports_and_exports.rs`) binds each deferred star import to `x_deferred`. Thus all deferred imports of a module read one object, also through `export { ns }`.
- `resolve_member_expr_refs` records each access to `x_deferred` (`ns.foo`, `'foo' in ns`) in `deferred_access_targets`. The access resolves as for an eager namespace, so `ns.foo` reads the binding `foo` or its inlined value. Two exceptions:
  - A write (`ns.foo = v`, `ns.a.b++`, `delete ns.foo`) keeps `x_deferred`, whose traps reject the write.
  - The key `then` is always absent, and a read of it evaluates nothing: `ns.then` gives `undefined`, and `'then' in ns` gives `false`.
- `reference_needed_symbols` adds the wrapper of the target to each statement with a deferred access.
- A deferred record is not an init obligation (`crates/rolldown/src/esm_init_obligations.rs`), and the init walks of the generate stage do not follow it (`compute_wrapped_esm_init_metadata.rs`). The link-time passes do not treat a read of a deferred namespace as pure (`refine_stmt_side_effects.rs`, `cross_module_optimization.rs`).

## Generate stage

- The finalizer removes the statement of a deferred import, because the import evaluates nothing.
- A deferred access becomes `(init_x(), <access>)` (`wrap_deferred_access` in `crates/rolldown/src/module_finalizers/impl_visit_mut.rs`).
- The init wrapper of a closure module is the hoisted form `function init_x()` (`esm_wrapper_decl_kind`).
- When tree shaking keeps the facade statement, `x_deferred` gets its declaration after the namespace object of the target: `var x_deferred = __deferNs(() => (init_x(), x_exports), x_exports)`. A kept `x_deferred` also keeps the namespace object (`module_namespace_is_referenced` in `code_splitting.rs`).
- `__deferNs` (`crates/rolldown/src/runtime/runtime-defer.js`) returns a `Proxy` that evaluates the module for each operation with a string key other than `then`. The runtime module has it only with the option on (see `../runtime-helpers/implementation.md`).

## Known gaps

- The warnings of a fallback come again on each incremental rebuild.
- In `cjs` output with `strictExecutionOrder`, the runtime module goes into its own chunk. The link stage wraps the closure modules, and the order lowering then moves the runtime out of the entry chunk. It moves the runtime back only for `esm` output (`fold_runtime_chunk_after_order_lowering`). A build with the option off has the same layout when the link stage wraps a module, for example a CommonJS module.
- In a closure module, a top-level `arguments` is the arguments object of `init_x`, and direct `eval` code runs in `init_x` (see the JSDoc of `experimental.importDefer`).

## Related

- [design.md](./design.md) — why one fallback for the whole build
- `../runtime-helpers/implementation.md` — the runtime helpers of `import defer`
