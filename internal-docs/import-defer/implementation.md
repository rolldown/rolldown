# Import defer — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

With `experimental.importDefer`, a build keeps the deferred evaluation of `import defer * as ns from './x.js'` when the build has a shape that the deferral supports (see "Whole-build fallback"). Each module of a deferred closure gets a lazy wrapper: `init_x` for an ES module, `require_x` for a CommonJS module. An ES module of a closure that reaches a top-level `await` also gets a state record (see "Top-level await"). The first access to the deferred namespace calls the wrapper of the target, and that wrapper calls the wrappers of the modules that the target imports. Any other build lowers each deferred import to a normal import, with a warning, and gives the output of the build with the option off.

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
3. No module of a deferred closure (a target and its dependencies) is an external module.
4. A module outside the closures has no import of a module outside the closures after a deferred import of a module that reaches a top-level `await` (see "Top-level await"). An import of an external module does not count.

When the build keeps the deferral, the saved `eval_flags` go into `stmt_infos`. The new flags are the union of both runs, because the deferred run only adds side effects. A module with `DeterminedSideEffects::Analyzed(false)` gets `Analyzed(true)` when one of its statements gets `UnknownSideEffect`.

When the build falls back, `DeferPhase` goes away from each record. If the build has a write to a deferred namespace, the saved errors fail the build with the hooks of a scan error (`buildEnd` and `closeBundle` get the errors). Else each deferred import gets an `UNSUPPORTED_FEATURE` warning that names the reason. All other scan results are already the results of an eager import, so the output is the output of the build with the option off.

## Link stage

- `sort_modules` visits the deferred records of a module after its eager records. A module that only a deferred record reaches thus gets a position after the eager dependencies of the importer and before the importer.
- `wrap_import_defer_closures` (`crates/rolldown/src/stages/link_stage/wrapping.rs`) gives `WrapKind::Esm` to each ES module of each closure, `WrapKind::Cjs` to each CommonJS module, and sets `wrapped_by_import_defer`. The loop of `wrap_modules` then wraps the rest of their graph, as for any wrapped module. A closure module that a `require()` loads uses the same wrapper, so the module evaluates one time, at the first call or access.
- `create_deferred_namespaces` adds a facade statement that declares `x_deferred` for each ES module target. `advance_import_tracker` (`bind_imports_and_exports.rs`) binds each deferred star import to `x_deferred`. Thus all deferred imports of a module read one object, also through `export { ns }`. A CommonJS target gets its symbols from `create_deferred_cjs_namespaces` (see "CommonJS targets").
- A JSON module has no wrapper: `generate_lazy_export` removes it (`unwrap_json_module`), and `json_object_expr_to_esm` makes new symbols and statements for the module. Thus `create_deferred_namespaces` runs after `generate_lazy_export`. A deferred access to a JSON module calls nothing, because the module has no visible evaluation.
- `resolve_member_expr_refs` records each access to `x_deferred` (`ns.foo`, `'foo' in ns`) in `deferred_access_targets`. The access resolves as for an eager namespace, so `ns.foo` reads the binding `foo` or its inlined value. Two exceptions:
  - A write (`ns.foo = v`, `ns.a.b++`, `delete ns.foo`) keeps `x_deferred`, whose traps reject the write.
  - The key `then` is always absent, and a read of it evaluates nothing: `ns.then` gives `undefined`, and `'then' in ns` gives `false`.
- `reference_needed_symbols` adds the wrapper of the target to each statement with a deferred access.
- A deferred record is not an init obligation (`crates/rolldown/src/esm_init_obligations.rs`), and the init walks of the generate stage do not follow it (`compute_wrapped_esm_init_metadata.rs`). The link-time passes do not treat a read of a deferred namespace as pure (`refine_stmt_side_effects.rs`, `cross_module_optimization.rs`).

## Generate stage

- The finalizer removes the statement of a deferred import, because the import evaluates nothing.
- A deferred access becomes `(init_x(), <access>)` (`wrap_deferred_access` in `crates/rolldown/src/module_finalizers/impl_visit_mut.rs`). A target with no wrapper (a JSON module) keeps the plain access.
- The init wrapper of a closure module is the hoisted form `function init_x()` (`esm_wrapper_decl_kind`).
- When tree shaking keeps the facade statement, `x_deferred` gets its declaration after the namespace object of the target: `var x_deferred = __deferNs(() => (init_x(), x_exports), x_exports)`. A kept `x_deferred` also keeps the namespace object (`module_namespace_is_referenced` in `code_splitting.rs`). A module with dynamic exports (`export *` from CommonJS) gets no second argument: the keys come from the first evaluation.
- `__deferNs` (`crates/rolldown/src/runtime/runtime-defer.js`) returns a `Proxy` that evaluates the module for each operation with a string key other than `then`. The runtime module has it only with the option on (see `../runtime-helpers/implementation.md`).

## CommonJS targets

`import defer * as ns from './x.cjs'` gives the importer two symbols for the import record (`create_deferred_cjs_namespaces` in `crates/rolldown/src/stages/link_stage/wrapping.rs`):

- `import_x`, the namespace. A static access assigns it at the first access: `(import_x || (import_x = __toESM(require_x()))).foo`. The evaluation is the object of the member chain, so a call `ns.fn()` keeps `import_x` as `this`, as the eager form `import_x.fn()` does. An access with no such chain (`'foo' in ns`, an inlined export) uses `(import_x || (import_x = __toESM(require_x())), <access>)`. For `'foo' in ns`, the right side reads `import_x` (`prepare_deferred_access`).
- `x_deferred`, the deferred namespace object: `__deferNs(() => import_x || (import_x = __toESM(require_x())))`. Only an escape (`f(ns)`, `export { ns }`) reads it. The keys of the object come from the first evaluation.

The import statement declares `import_x` and references `require_x` (`crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs`). Two rules differ from an eager import:

- The statement keeps the side-effect flag of the target, though it runs nothing. Tree shaking includes a deferred target with side effects as a dependency of its importer, because `meta.dependencies` has the deferred records. When nothing reads the namespace, this statement is the only reference to `require_x`. Without the flag, the module stays but its wrapper goes: the finalizer writes the body at the top level, so the module runs at startup, and two such modules can declare one top-level name twice (a `SyntaxError` in `esm` output).
- The statement does not reference `__toESM`. Each static access to a bundled CommonJS module references it, and so does the statement of `x_deferred`. Thus an unread deferred import adds no `__toESM`. These statements, and the finalizer when it writes `__toESM(require_x())` (`cjs_deferred_namespace_expr`), do not check the record with `import_record_needs_interop`. That check searches all named imports of the importer, and here its answer is always true: `wrap_import_defer_closures` adds only `import defer * as ns` records to `deferred_cjs_namespace_imports`, and a namespace import always needs interop. The `needs_interop` of `safely_merge_cjs_ns_map` does not apply either: `determine_safely_merge_cjs_ns` skips the deferred records, so that value describes only the eager imports. It is `false` when no eager import needs interop, for example `import './x.cjs'` or `import { foo } from './x.cjs'`. When the deferred import used that value, no statement referenced the `__toESM` of a read: an error at the first access in `esm` output, and a panic in `cjs` output.

An unread deferred import still keeps `require_x` and `var import_x;`, and nothing calls them. The wrapper is `/* @__PURE__ */`, so a minifier can drop both. An unread `import defer` of an ES module with side effects keeps `init_y` in the same way.

### Shared CommonJS namespaces

`merge_deferred_cjs_namespaces` (`crates/rolldown/src/stages/generate_stage/code_splitting.rs`) runs after `merge_cjs_namespace`:

1. It groups the records by chunk, CommonJS module and interop mode. Only importers with no wrapper (`wrap_kind()` is `None`) take part. Nothing merges with `strictExecutionOrder`, because its order wraps are not in `wrap_kind()`.
2. It sorts each group by the execution order of the importer, then by the record.
3. It links the included `import_x` symbols of a group to one root, and the included `x_deferred` symbols to another root. A record with only static accesses keeps `import_x` and drops `x_deferred`, so the two links are separate. The root is the first symbol whose canonical symbol is a key of the chunk's `exports_to_other_chunks`: another chunk imports it, or the entry chunk exports it. With no such symbol, the root is the first symbol.
4. It puts each symbol except the first into `ChunkGraph::merged_deferred_cjs_namespace_refs`. `deferred_cjs_import_decl` (`crates/rolldown/src/module_finalizers/mod.rs`) does not declare these symbols. The first symbol keeps the declaration, under the name of the root. A record that declares neither symbol loses its statement.

Invariants:

- The declarations stay in the first importer by execution order. That importer runs before the other importers, so a read of `x_deferred` gets its value at the same time as before the merge, or earlier.
- The canonical symbol of each key of `exports_to_other_chunks` stays a key. `commit_cross_chunk_links` builds the table before the merge, and a read in `cjs` output looks it up by the canonical symbol after the merge (`finalized_expr_for_cross_chunk_symbol` in `crates/rolldown/src/module_finalizers/mod.rs`, and `crates/rolldown/src/types/generator.rs`). A key that links to a symbol that is not a key makes that lookup panic. Two keys can share the root: the chunk then exports one object under two names.
- Each access assigns `import_x` with the interop mode of its own importer. Thus all records of a group have one mode.

Tests: `commonjs_shared_namespace` (two files, two imports in one file, a `.mjs` importer, a wrapped importer, two objects that the entry exports, a `strictExecutionOrder` variant with no merge, and a `cjs` variant), `commonjs_eager_import_no_interop` (an eager import with no interop and a deferred import of each module, with no other user of `__toESM`, and a `cjs` variant), `commonjs_unread_namespace_keeps_wrapper` (unread deferred imports of two CommonJS modules that both declare `snapshot`: the entry parses, runs neither module and has no `__toESM`), `commonjs_escape`, `commonjs_target`, `commonjs_traps_before_evaluation`, `commonjs_write_and_call` and `named_commonjs_reexport`. `json_target` checks a JSON target.

## Top-level await

A deferred import of a module that has a top-level `await`, or that reaches one through static imports, has a preparation (spec `GatherAsynchronousTransitiveDependencies`): the importer waits for the modules with top-level `await` first. A later access then evaluates the rest of the closure synchronously. The access throws a `TypeError`, and evaluates nothing, when a module that it needs is still evaluating (spec `ReadyForSyncExecution`).

- The early flag. `wrap_modules` and `reference_needed_symbols` run before tree shaking, and `compute_tla` runs after it. Thus `mark_tla_before_tree_shaking` (`crates/rolldown/src/stages/link_stage/compute_tla.rs`) sets `is_tla_or_contains_tla_dependency` before tree shaking: for each module with `await` at the top level, and for each module that reaches one through `import` records. `compute_tla` then sets the final flag, which is a subset.
- Tracked modules. `track_import_defer_modules` (`crates/rolldown/src/stages/link_stage/wrapping.rs`) takes the graph of the requested modules (`import` and `import defer` records) between the ES modules of the closures. A module is tracked when its strongly connected component has a module with the early flag. `import_defer_tracked_deps` holds the tracked requested modules of a tracked module, and is `None` for each other module.
- State records. The init wrapper of a tracked module is `__esmState(body, deps, flags)`, not `__esmMin` (`EsmWrapperStmtOptions::state` in `crates/rolldown_ecmascript_utils/src/ast_factory.rs`). The record has the state of spec `InnerModuleEvaluation` (evaluating, evaluating-async, evaluated, error), the stack indexes and the error. `deps` returns the init functions of the tracked requested modules. An async body calls `__esmEnd(init_x)` at its end and `__esmFail(init_x, e)` for an error. The comments in `crates/rolldown/src/runtime/runtime-defer.js` describe each field and helper.
- The import forms in the body of a tracked module `bar` (`import_stmt_init_expr` in `crates/rolldown/src/module_finalizers/mod.rs`):
  - `init_foo(init_bar)` when `foo` is also tracked. The record of `foo` gets the importer, so the walk follows the evaluation order.
  - `__esmDepDone(init_foo(init_bar), init_foo, init_bar) || await init_foo(init_bar)` when `foo` also has the early flag. The body waits only when `foo` did not finish, so the body of `bar` runs synchronously after a preparation.
- The preparation. The statement of a deferred import of a module with the early flag becomes `__esmGather(init_foo, init_bar) || await __esmGather();` (`deferred_import_preparation_stmt`). `init_bar` is absent when the importer is not tracked. The first call walks the records from `foo` and returns `true` when nothing is pending. Else the second call waits: first for each module with its own top-level `await`, then for each started module that waits. `reference_needed_symbols` keeps the statement also when `foo` has no side effects.
- The access. A deferred access to a tracked module calls `__esmDefer(init_foo)`, not `init_foo()` (`deferred_init_call_expr`).

The order rule (condition 4 of the fallback): the preparation is a statement in the body of the importer. A module outside the closures has no wrapper, so the code of its imports runs before its body. An import after the deferred import then runs before the modules with top-level `await` start, but native Node starts them first. A module of a closure has a wrapper that runs its import statements in order. A later import of a module of a closure is an init call in the body of the importer, so it runs after the preparation. The check does not know the wrappers of the link stage, so it also applies with `strictExecutionOrder`. The reasons are in [design.md](./design.md#top-level-await).

Tests: `tla_transitive`, `tla_gather_order`, `tla_diamond_evaluates_once`, `tla_error_after_preparation`, `tla_defer_in_closure` and `tla_restored_reexport_helper`. The Node test `packages/rolldown/tests/import-defer-fallback.test.ts` checks the fallback of the order rule.

## Init order

In one chunk, `ensure_lazy_module_initialization_order` (`crates/rolldown/src/stages/generate_stage/code_splitting.rs`) moves the init call of a wrapped module before the code of a later module with no wrapper. Without a kept deferral, the position of the call is the first import of the module in chunk order. A build that keeps a deferral uses the start record of the module instead (`eager_import_order`): the import through which a walk of the eager imports from the entry first reaches the module, as the native evaluation does.

- The walk skips the deferred records, because such a record evaluates nothing. Thus a module that only a deferred record reaches before a later module is not a dependency of that later module (fixture `cycle_through_defer_later_import`).
- A start record in a wrapped importer is not a position. It runs inside the start of its importer, so the module needs no move of its own. In a cycle of wrapped modules, only the module that the evaluation enters first has a start record in a module with no wrapper. Its init call runs the other modules of the cycle in native order.
- The first import in chunk order is not always the start. A deferred module comes early in chunk order through its deferred record (`sort_modules`), but its wrapper runs late (fixture `deferred_importer_first_in_chunk`). In a cycle, the first import can be in another module of the cycle (fixture `cycle_eager_import_order`), or in a later module that imports a module of the cycle other than the module that the evaluation enters first (fixture `cycle_member_later_import`).
- A moved call can come just before a call of the same module at the start of the later module (fixture `eager_and_deferred`). The second call does nothing.
- The rule applies only when the build keeps a deferral. A build without `import defer` can also wrap modules, for example an ES module that a `require()` loads and its dependencies. Its output does not change.

Example (fixture `cycle_eager_import_order`): `main.js` imports `b.js`, defers `a.js` and then imports `z.js`, and `a.js` and `b.js` import each other. The call `init_b()` moves from the body of `main.js` to the start of `z.js`. Thus the cycle runs before `z.js`, as in native order.

Tests: `deferred_importer_first_in_chunk`, `cycle_eager_import_order`, `cycle_eager_import_order_long`, `cycle_member_later_import`, `cycle_entry_after_deferred_import` and `cycle_through_defer_later_import`.

## Cycles

A static import cycle can contain a deferred target, also through the deferred record itself: `a.js` defers `d.js`, and `d.js` imports `a.js`.

- Tracked modules. `track_import_defer_modules` also tracks each module of a strongly connected component that contains a deferred ES module target, when the component has more than one module or a module that imports itself. Only these modules can still be evaluating at a deferred access to the target. `__esmDefer` then throws a `TypeError`, and evaluates nothing, when the target or a module that it needs is in a cycle that did not finish (spec `ReadyForSyncExecution`). The records keep the cycle root and the ancestor index of spec `InnerModuleEvaluation`, so an error of one member goes to each member of its cycle.
- The init order of a cycle follows the start records (see "Init order").

Tests: `cycle_deferred_read`, `cycle_root_error`, `escape_cycle`, `empty_star_state`, `mutual_defer_failed`, `mutual_defer_read_after`, `mutual_defer_read_in_body` and `placement_cycle`. With top-level `await`: `tla_ancestor_wait`, `tla_cycle_child_wait`, `tla_cycle_cross_edge_wait`, `tla_cycle_export_star_wait`, `tla_cycle_in_sync_part` and `tla_on_stack`. The Node test `packages/rolldown/tests/import-defer-fallback.test.ts` runs a cycle through the deferred record and a cycle that only the deferred import reaches.

## Known gaps

- The warnings of a fallback come again on each incremental rebuild.
- A preparation waits for each module with top-level `await` in the order of its walk, and each module ends before the next one starts. Native Node starts all of them first, and then waits. A build without `import defer` has the same limit for eager imports.
- In `cjs` output with `strictExecutionOrder`, the runtime module goes into its own chunk. The link stage wraps the closure modules, and the order lowering then moves the runtime out of the entry chunk. It moves the runtime back only for `esm` output (`fold_runtime_chunk_after_order_lowering`). A build with the option off has the same layout when the link stage wraps a module, for example a CommonJS module.
- In a closure module, a top-level `arguments` is the arguments object of `init_x`, and direct `eval` code runs in `init_x` (see the JSDoc of `experimental.importDefer`).
- A call `ns.fn()` of a CommonJS target gets `import_x` as `this`, not the deferred namespace object. Thus `this === ns` gives `false` in `fn` when `ns` escapes (`ns` is then `x_deferred`). Native Node and the eager import give `true`. A call through `x_deferred` gives the native result, but each call then goes through the `Proxy`.
- The deferred namespace object of a CommonJS module, or of a module with dynamic exports, gets its keys at the first evaluation. Before that, `Object.isExtensible(ns)` gives `true`, and after `Object.preventExtensions(ns)`, the first evaluation throws a `TypeError` when the namespace has keys (see `__deferNs`).

## Related

- [design.md](./design.md) — why one fallback for the whole build
- `../runtime-helpers/implementation.md` — the runtime helpers of `import defer`
