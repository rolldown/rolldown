# Import defer — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

With `experimental.importDefer`, a build keeps the deferred evaluation of `import defer * as ns from './x.js'` when the build has a shape that the deferral supports (see "Whole-build fallback"). Each module of a deferred closure gets a lazy wrapper: `init_x` for an ES module, `require_x` for a CommonJS module. An ES module of a closure that reaches a top-level `await` also gets a state record (see "Top-level await"). The first access to the deferred namespace calls the wrapper of the target, and that wrapper calls the wrappers of the modules that the target imports. Any other build lowers each deferred import to a normal import, with a warning, and gives the output of the build with the option off.

A build with more than one chunk also keeps the deferral. The chunk that holds a deferred module does not run the module when it loads. A chunk with an eager importer of the module initializes it after its import of that chunk (see "Lazy hosts"). When a common chunk cannot keep that order, it runs the module at load, as the build without `import defer` does (see "Run at load").

`import.defer('./x.js')` also keeps the deferral: its promise gives the deferred namespace object of `x.js`, and the call evaluates nothing (see "`import.defer()`"). Dev mode turns the option off (`crates/rolldown/src/utils/prepare_build_context.rs`).

All output formats keep the deferral of the modules in the bundle. For an external module, `cjs` output loads the module at the first access, and `esm` output keeps `import defer` and `import.defer()`. A build falls back when its output format evaluates an external module of a closure before the first access (see "External modules").

In `esm` output, the private modules of a deferred module run in its wrapper, with no wrapper of their own (see "Deferred groups").

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

- The scanner sets `ImportRecordMeta::DeferPhase` on the record of each `import defer` (`scan_import_decl` in `crates/rolldown/src/ast_scanner/mod.rs`), and on the `DynamicImport` record of each `import.defer()` (`crates/rolldown/src/ast_scanner/impl_visit.rs`). With the option off, the pre-processor removes the phase before the scan, so no record has the flag.
- The scan results are the results of an eager import, as with the option off. A deferred namespace is in `namespace_object_symbol_ids`, so a read `ns.foo;` has no side effect, and a write `ns.foo = 1` gives the `assign_to_import` error.
- The scanner also saves the results of the deferred import in `EcmaView::import_defer_scan`:
  - `eval_flags`: the flags of each statement that change when a read of a deferred namespace has a side effect (the read evaluates the module). A second `StmtEvalAnalyzer` run (`with_deferred_namespaces`) gives them. Both runs read the same `constant_export_map` of the scan.
  - `namespace_writes`: the data of the `assign_to_import` error of each write to a deferred namespace. The scanner does not report these errors. A deferred namespace rejects a write at run time, so a build that keeps the deferral has no error.

## Whole-build fallback

`apply_import_defer_support` (`crates/rolldown/src/utils/import_defer_fallback.rs`) decides one time for each build. `scan_modules_impl` (`crates/rolldown/src/bundle/bundle.rs`) calls it after `normalize_scan_stage_output_and_update_cache`. Thus it sees all modules, also in an incremental build that scans only some of them. The cache keeps its own copy of the scan output, so the changes do not go into the cache.

The build keeps the deferral when all of these conditions are true:

1. The build does not use `output.codeSplitting.experimentalInlineCommonChunks`. With this option, the init plan does not run, so each eager importer of a deferred module in another chunk calls `init_x()` in its body, after the later imports of its chunk (see "Init after the import of a host").
2. No code splitting group sets `includeDependenciesRecursively: false` (`has_non_recursive_dependency_capture`). Such a group can make two chunks import each other. In such a cycle, a chunk can run before the other chunk exports its init functions: `require_x.init_x is not a function` in `cjs` output. When the runtime is in the cycle, a chunk can also run before the runtime helpers exist: `__esmMin is not a function`. The build without `import defer` calls no init function or helper there.
3. No `import.defer()` call loads a CommonJS module (`dynamic_import_of_commonjs`). Native ESM defers the evaluation of a CommonJS module also for this form, but the deferred namespace of a CommonJS module exists only for the static form (see "CommonJS targets"). The check finds the CommonJS modules as `determine_module_exports_kind` does (`commonjs_modules`): the scan gives `ExportsKind::CommonJs`, or `ExportsKind::None` and a `require()` loads the module. With `codeSplitting: false`, an `import()` or an `import.defer()` also counts as a `require()`.
4. The output format does not evaluate an external module of a deferred closure (a target and its dependencies) before the first access (`deferred_closures`). A deferred import of an external module counts too. The rules for each format are in "External modules".
5. No output file of a CommonJS module of a closure runs the module when the file loads (`commonjs_module_run_at_load`). A deferred importer imports that file, so the module runs before the first access:
   - With `preserveModules` and `esm` output, the file of each CommonJS module ends with `export default require_x();`. In `cjs` output, the file exports a `default` getter, which runs the module at the first read.
   - In `cjs` output, the chunk of a user-defined entry resolves its own export mode (`export_mode_module`), so it can end with `module.exports = require_x();`. An emitted chunk keeps the `Named` mode and its getter. In `esm` output without `preserveModules`, the chunk of a user-defined entry gets a facade (see "Lazy hosts").

   The check finds the CommonJS modules in the same way as the check of `import.defer()` (`commonjs_modules`).

6. A module outside the closures has no import of a module outside the closures after a deferred import of a module that reaches a top-level `await` (see "Top-level await"). An import of an external module counts only when the build can make more than one chunk (`can_have_more_chunks`): code splitting is on, and the build has `preserveModules`, chunk groups, an `import()`, or more than one entry record (each user-defined or emitted `EntryPoint`, also when one module has several). In a build with one chunk, the chunk imports each external module at its top, so the external module also runs first without the deferral.

7. With `preserveModules`, a static deferred importer in an eager import or require cycle can expose a binding before its host or runtime helpers load. The build falls back if that importer exports a function declaration, re-exports a deferred binding, or has a synthetic default export (which can be an anonymous function declaration). Cycles reached only through deferred edges do not trigger this check.

8. In a build that can emit multiple chunks, an importer can deadlock if it waits for one asynchronous dependency before starting a sibling. The scan checks two cases: a later deferred import that requires async preparation, and a later eager import with side effects in a deferred closure module that has its own top-level `await`. The scan tracks which module bodies the earlier requests start. Deferred preparation starts a module with its own top-level `await` and evaluates its eager dependencies. Synchronous modules reached only during preparation stay lazy. A later request that starts no new body needs no fallback. A new body can also wait safely if it already depends on every earlier top-level `await` through acyclic static imports. An evaluating ancestor in a cycle can bypass the wait. The eager fallback lets the ESM loader start dependencies in separate chunks concurrently.

When the build keeps the deferral, the saved `eval_flags` go into `stmt_infos`. The new flags are the union of both runs, because the deferred run only adds side effects. A module with `DeterminedSideEffects::Analyzed(false)` gets `Analyzed(true)` when one of its statements gets `UnknownSideEffect`.

When the build falls back, `DeferPhase` goes away from each record. If the build has a write to a deferred namespace, the saved errors fail the build with the hooks of a scan error (`buildEnd` and `closeBundle` get the errors). Else each deferred import gets an `UNSUPPORTED_FEATURE` warning that names the reason. The warning of an `import.defer()` call says that the call becomes `import()` (`UnsupportedReason::messages`). All other scan results are already the results of an eager import, so the output is the output of the build with the option off.

## Link stage

- `sort_modules` places modules in source request order. Moving a deferred edge after an eager sibling can place the sibling first in their shared chunk, even when another entry imports the deferred target eagerly before that sibling. Chunk import ranks still visit eager requests first. Loading a shared chunk to obtain a deferred binding can run unrelated eager code in it.
- `wrap_import_defer_closures` (`crates/rolldown/src/stages/link_stage/wrapping.rs`) gives `WrapKind::Esm` to each ES module of each closure, `WrapKind::Cjs` to each CommonJS module, and sets `wrapped_by_import_defer`. The loop of `wrap_modules` then wraps the rest of their graph, as for any wrapped module. A closure module that a `require()` loads uses the same wrapper, so the module evaluates one time, at the first call or access.
- `create_deferred_namespaces` adds a facade statement that declares `x_deferred` for each ES module target. `advance_import_tracker` (`bind_imports_and_exports.rs`) binds each deferred star import to `x_deferred`. Thus all deferred imports of a module read one object, also through `export { ns }`. A CommonJS target gets its symbols from `create_deferred_cjs_namespaces` (see "CommonJS targets").
- A JSON module has no wrapper: `generate_lazy_export` removes it (`unwrap_json_module`), and `json_object_expr_to_esm` makes new symbols and statements for the module. Thus `create_deferred_namespaces` runs after `generate_lazy_export`. A deferred access to a JSON module calls nothing, because the module has no visible evaluation.
- `resolve_member_expr_refs` records each access to `x_deferred` (`ns.foo`, `'foo' in ns`) in `deferred_access_targets`. The access resolves as for an eager namespace, so `ns.foo` reads the binding `foo` or its inlined value. Two exceptions:
  - A write (`ns.foo = v`, `ns.a.b++`, `delete ns.foo`) keeps `x_deferred`, whose traps reject the write.
  - The key `then` is always absent, and a read of it evaluates nothing: `ns.then` gives `undefined`, and `'then' in ns` gives `false`.
- `reference_needed_symbols` adds the wrapper of the target to each statement with a deferred access.
- In a build with one chunk, the `export *` statement of a closure module follows the eager rule. Tree shaking keeps it only for a used binding or a side effect, so an `export *` target that no code uses leaves the output (fixture `treeshake_export_star`). With more chunks, the statement always stays. See `../linking/reference-needed-symbols/implementation.md`.
- A deferred record is not an init obligation (`crates/rolldown/src/esm_init_obligations.rs`), and the init walks of the generate stage do not follow it (`compute_wrapped_esm_init_metadata.rs`). The link-time passes do not treat a read of a deferred namespace as pure (`refine_stmt_side_effects.rs`, `cross_module_optimization.rs`).

## Generate stage

- The finalizer removes the statement of a deferred import, because the import evaluates nothing.
- A deferred access becomes `(init_x(), <access>)` (`wrap_deferred_access` in `crates/rolldown/src/module_finalizers/impl_visit_mut.rs`). A target with no wrapper (a JSON module) keeps the plain access.
- The init wrapper of a closure module is the hoisted form `function init_x()` (`esm_wrapper_decl_kind`).
- The closure of that wrapper has the PIFE hint also for `platform: 'neutral'`, unless `optimization.pifeForModuleWrappers` is `false` (`is_pife_for_deferred_module_wrappers_enabled`, fixture `pife_neutral`). See [design.md](./design.md#pife-hint-of-deferred-wrappers).
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
- The preparation. The statement of a deferred import of a module with the early flag becomes `__esmGather(init_foo, init_bar) || await __esmGather();` (`deferred_import_preparation_stmt`). `init_bar` is absent when the importer is not tracked. The first call walks the records from `foo` and returns `true` when nothing is pending. Else the second call starts the gathered modules in order, waits for them together, then waits for previously started modules. Each sibling gets an immediate rejection handler. Awaiting a thenable supplies the resolving functions for the group even if user code has replaced the global `Promise`. A rejection from any gathered module rejects the preparation. `reference_needed_symbols` keeps the statement also when `foo` has no side effects.
- The access. A deferred access to a tracked module calls `__esmDefer(init_foo)`, not `init_foo()` (`deferred_init_call_expr`).

A module whose top-level `await` already rejected is complete for the gather step. A later deferred importer can observe `typeof ns` as `"object"`. Reading `ns.x` rethrows the original error. [GatherAsynchronousTransitiveDependencies, step 6](https://tc39.es/proposal-defer-import-eval/#sec-GatherAsynchronousTransitiveDependencies).

The fallback for async deferred imports accounts for where the importer runs its preparation statement. A module outside the closures has no wrapper, so the code of its imports runs before its body. An import after the deferred import then runs before the modules with top-level `await` start, but native Node starts them first. A module of a closure has a wrapper that runs its import statements in order. A later import of a module of a closure is an init call in the body of the importer, so it runs after the preparation. The check does not know the wrappers of the link stage, so it also applies with `strictExecutionOrder`. In a build with more than one chunk, a chunk imports an external module at its place among the other imports of the chunk, so an import of an external module also counts there. The reasons are in [design.md](./design.md#top-level-await).

Tests: `tla_transitive`, `tla_gather_order`, `tla_diamond_evaluates_once`, `tla_error_after_preparation`, `tla_defer_in_closure` and `tla_restored_reexport_helper`. With an `import()` that starts a module before the preparation: `tla_started_before_preparation`, `tla_started_module_throws`, `tla_started_dependency_rejects` and `tla_failure_during_wait`. `cross_chunk_tla` prepares a deferred module in another chunk. `tla_external_after_deferred_import` checks an import of an external module after the deferred import: with `preserveModules`, the build falls back, and in one chunk, it keeps the deferral. The Node test `packages/rolldown/tests/import-defer-fallback.test.ts` checks the fallback of the order rule.

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

## Lazy hosts

A lazy host is the entry chunk of an ES module of a deferred closure, when that chunk is not a user-defined or an emitted entry (`ChunkGraph::lazy_dynamic_defer_host` in `crates/rolldown/src/chunk_graph.rs`). With `preserveModules`, each such module gets one. Without `preserveModules`, the dynamic entry chunk of an `import()` or an `import.defer()` of the module can be one. Another chunk can load a lazy host for a deferred access, so the load must not run the module:

- `render_wrapped_entry_chunk` (`crates/rolldown/src/utils/chunk/render_chunk_exports.rs`) writes no `init_x()` at the end of the chunk. The importer calls `init_x()` at the first access.
- In `cjs` output, each export of a lazy host is a getter (`render_chunk_exports`), as in a common chunk. `init_x()` assigns the bindings after the load, so a copy (`exports.foo = foo`) keeps `undefined`, also for a `const`. An eager importer reads the bindings through the same exports: it calls `init_x()`, then reads them. The mode of a lazy host stays `Named`, because other chunks import `init_x`, which is not a declared export (`export_mode_module` in `crates/rolldown/src/stages/generate_stage/mod.rs`).
- Each `import()` of the module calls `init_x()` (`rewrite_dynamic_import_of_lazy_module` in `crates/rolldown/src/module_finalizers/mod.rs`). From another chunk, the form is `import('./x.js').then((n) => (n.init_x(), n.x_exports))`. In the same chunk, the form is `Promise.resolve().then(() => (init_x(), x_exports))`. When the module reaches a top-level `await`, the form waits for `init_x()`.
- Thus the chunk exports `init_x` and `x_exports` to each chunk with an `import()` of the module, and `x_deferred` to each chunk with an `import.defer()` of the module (`dynamic_defer_target_exports` in `crates/rolldown/src/stages/generate_stage/compute_cross_chunk_links.rs`). No code reads the namespace of the chunk. The rule that a dynamic entry chunk exports only the exports of its entry does not apply to a lazy host (`dynamic_entry_reads_defer_bindings` in `crates/rolldown/src/stages/generate_stage/dynamic_entry_namespace.rs`). The rule also does not apply to the chunk of a JSON module that `import.defer()` loads. A JSON module has no wrapper and is not a lazy host, but its chunk exports `x_exports` and `x_deferred`. So an `import()` of it in another chunk reads only `x_exports`: `import('./chunk.js').then((n) => n.x_exports)` (fixture `dynamic_json_target_eager_and_deferred`). With `strictExecutionOrder`, the JSON module can have an order wrapper, and its chunk does not always call `init_x()` at load: an entry facade calls it, and a deferred import in the same chunk does not call it. So each `import()` of the module also calls `init_x()`, and the chunk exports `init_x` to the other chunks (fixture `dynamic_json_target_strict_user_entry`).
- No statement of the importer can reference `x_exports`, because that reference imports the chunk statically. Thus `init_entry_point_stmt_info` (`crates/rolldown/src/stages/link_stage/create_exports_for_ecma_modules.rs`) keeps the namespace of each ES module of a closure with an `import()`. `module_namespace_is_referenced` keeps it also for a module with no exports, because the closure wraps such a module as an ES module.

The load of a lazy host runs nothing, also for code outside the bundle. With `preserveModules`, a direct import of the file of a deferred module that is not an entry does not run the module, and its exports stay uninitialized. Native ESM runs the module. This is an accepted difference. The docs of `input` already recommend an entry for each file whose export signatures must stay, instead of `preserveModules` (`packages/rolldown/src/options/docs/input.md`), and a user-defined entry chunk is not a lazy host.

The chunk of a user-defined or an emitted entry runs its entry module when it loads, also when the module is in a closure. When such a chunk exports more than the public exports of its entry, it gets a facade (`preserve_strict_entry_signatures` in `crates/rolldown/src/stages/generate_stage/entry_facades.rs`). Another chunk can import the old chunk for a deferred access (`init_x`, `x_deferred`), and only the facade runs the module (fixture `user_entry_target`).

### Init after the import of a host

The host of a deferred ES module is the chunk that holds the module and does not run it at load (`ChunkGraph::deferred_init_host` in `crates/rolldown/src/chunk_graph.rs`): the lazy host of the module, or a common chunk. A common chunk has no tail, so only an init function runs its wrapped modules. An eager importer in another chunk calls `init_x()` in its body.

The imports of a chunk run before its body, so this call runs the module after the later static imports of the importer: `import './x.js'; import './y.js'` runs `y` first. In one chunk, `ensure_lazy_module_initialization_order` moves the call to an earlier module (see "Init order"). Across chunks, `plan_lazy_host_inits` (`crates/rolldown/src/stages/generate_stage/lazy_host_inits.rs`) moves the init to the chunk of the importer, right after one of its imports: the import of the host, or a later import of an earlier module of the importer. The reasons are in [design.md](./design.md#init-after-the-import-of-a-lazy-host).

- `esm` output: `add_lazy_host_init_chunks` adds an init chunk for each deferred module that a chunk initializes. A common chunk can hold more than one deferred module, so an init chunk calls the init function of one module. The init chunk is a common chunk with no modules. It imports `init_x` from the host and calls it: `import { init_x } from "./x.js"; init_x();`, with `await` when the module has top-level await (`render_lazy_host_init_call` in `crates/rolldown/src/ecmascript/format/esm.rs`). Each chunk that initializes the module imports the init chunk right after the import that the init follows (`LazyHostInit::after`), in `static_imports`, `cross_chunk_imports` and `imports_from_other_chunks`. The init chunks after one import keep the order of the imports of the importer. Two chunks that initialize the same module share its init chunk.
  - The plan runs after the export modes and before the chunk names. Thus the names, `renderChunk`, `augmentChunkHash`, the hashes and the minifier handle the init chunk as any other chunk, and the `imports` of each importer list it. The init chunk has no source map, because no part of its code has one (`SourceJoiner`). A chunk that only re-exports also has no map.
  - The name of the init chunk is the name of the host with `.init` (`name_lazy_host_init_chunks`): `x.init.js`, or `x.init-[hash].js`. A common chunk takes its name from one of its modules, so the init chunk of a common host takes the name of its own module instead (the naming in `crates/rolldown/src/stages/generate_stage/mod.rs`). Without `preserveModules`, the init chunk uses the file name template of the host, so its file is next to the host. With `preserveModules`, the init chunk has no module, so its file is in the folder of the generated chunks: `_virtual/lib/x.init.js` for `lib/x.js`.
  - The init chunk is last in `sorted_chunk_idx_vec`. Thus the chunk of a source file `x.init.js` keeps its name, and the init chunk gets a number: `x2.init.js`. The init chunk also does not take the name of an emitted asset, an emitted prebuilt chunk or a chunk with a `fileName`, in any letter case (`lazy_host_init_reserved_names`, `avoid_reserved_names`). A name with a hash does not change.
- `cjs` output: the chunk calls the init of each module right after the `require()` that the init follows: `require_x.init_x();` (`LazyHostInits::inits_after` and `init_export_name`). No init chunk exists.
- The importer then does not call `init_x()` in its body (`LazyHostInits::moved_calls` in `crates/rolldown/src/chunk_graph.rs`).

The init now runs before the body of the chunk. Thus the plan takes an importer only when each step that native ESM runs before the module also runs before the init:

- Only the first module of the chunk body that runs code takes part. A wrapped importer calls `init_x()` in its own init function, and a deferred-only importer has no call.
- Each earlier import of the importer must run while the chunk loads (`import_load_step`). The init follows the import of the host. When the import of an earlier module is a chunk after the host in `static_imports`, the init follows the last such import (`lazy_host_to_init`). For example, `a1.js` and `a2.js` are deferred modules of one common host, and `w.js` is in another chunk. In `import './a1.js'; import './w.js'; import './a2.js'`, the init of `a2` follows the import of `w`. When that last import is an external module, the plan stops. An external module, a module with no wrapper in another chunk, a wrapped module whose entry chunk runs it at load (`runs_by_tail`), and a deferred module that its common chunk runs at load (see "Run at load") run at load. A bare `import './w.js'` keeps no statement. The walk still counts such an import when its module is in `load_dependencies` and has no wrapper (`unkept_import_load_step`), in the plan and in the declined inits. An import that runs code in the body of the chunk stops the plan for the later hosts of the importer:
  - a module of the same chunk;
  - in `cjs` output, a CommonJS module whose file exports a `default` getter: the first read of `default` in the body runs the module;
  - a module that the importer initializes through a module with no wrapper, for example an import of `foo` from a barrel with no side effects (`init_obligation_targets`);
  - a deferred import of a module with top-level await: the body prepares it with `__esmGather` (`deferred_import_has_preparation`);
  - an excluded statement whose init function calls the init functions that it forwards to (`transitive_init_targets`).
- A static import of the chunk after the init can run code at load (`chunk_runs_code_at_load`). Otherwise the call in the body runs in the same order.
- The host does not import the chunk of the importer, directly or through other chunks (`chunk_imports_chunk`). In such a cycle, the module can read bindings of the chunk before its body runs.
- With `strictExecutionOrder`, the plan runs only with `preserveModules`. Without `preserveModules`, a module of a common chunk runs only when an init function calls it, so the order wrappers already call `init_x()` in the order of the imports. With `preserveModules`, each file calls the init function of its own module at load, so an importer with an order wrapper also runs when its chunk loads. That importer keeps its call: in a cycle, another module can call its init function before its chunk imports the init chunk.
- With `strictExecutionOrder`, a common chunk that holds an entry module is not a host (`LazyHostInitPlanner::init_host`). With `preserveModules`, the order wrappers move such an entry into a common chunk, for example a dynamic entry that another file also imports statically (`collapse_carries_trigger`). That chunk does not run the module at load, also without `import defer` (see the known gaps below). An init after its import would fix the order only with the option on, and it adds an init chunk also where the order wrappers already give the native order.
- The plan does not run in dev mode, with `experimentalInlineCommonChunks`, or for `iife` and `umd` output. With `experimentalInlineCommonChunks`, a file can run the modules of the records that it carries, and the build falls back (see "Whole-build fallback"). `iife` and `umd` output have no lazy host.

#### Declined inits

When the first two rules decline an importer, its call stays in the body, after each import of the chunk. When a static import of the chunk after the place of the init runs code at load, that code then runs before the module, and native ESM runs it after the module. The build without `import defer` runs the module when the chunk loads the chunk of the module, so before that code. Thus the chunk initializes such a module at the place of the plan anyway (`LazyHostInitPlanner::add_declined_inits`): right after the import of the host, or after the last earlier import of the importer that runs at load. Where the plan cannot give the native order, this init keeps the order of the build without `import defer`:

- Each module of the body takes part, also after the first one.
- An import through a module with no wrapper (a barrel) initializes the modules behind the barrel (`init_obligation_targets`). Each of these modules takes part as a direct import does. The load step of such an import is the step of the chunk of the barrel (`chunk_load_step`).
- Some code stays in the body only because of `import defer`: the call of a deferred module in another chunk that gets no init, and the preparation of a deferred import (`__esmGather`). The build without `import defer` runs that module when the chunk loads its chunk. An init at or after the import of that chunk then runs the other module first, in the opposite order. Thus the chunk adds only the declined inits that come before the first such import (the limit). When the chunk does not import the chunk of such a module, the limit is the start of `static_imports`, and the chunk adds no declined init.
- An init of the plan, or an earlier declined init of the same module, already runs the module at load. Then the module gets no second init.

Example (fixture `lazy_host_init_earlier_barrel`): `b.js` imports `foo` from a barrel with no side effects, then the lazy host `a.js`, then `y.js`. The barrel re-exports `foo` from the deferred module `z.js`. The import of the barrel stops the plan, because `b.js` calls `init_z()` in its body. The chunk of `b.js` initializes `z.js` after its import of `z.js`, then `a.js` after its import of `a.js`: `z`, `a`, `y`, as native ESM and the build without `import defer` run them.

Known gaps:

- A CommonJS module of a deferred closure has no lazy host. With `preserveModules` and `esm` output, its file ends with `export default require_x();`, so the module runs when a deferred importer loads that file. The build falls back for this shape (see "Whole-build fallback"). Its `cjs` file is correct: it exports a `default` getter, so the module runs at the first read.
- A declined init gives the order of the build without `import defer`, not the native order: the code that stops the plan runs after the module. For example, in `cjs` output, a CommonJS module whose file exports a `default` getter runs in the body of its importer. In `import x from './x.cjs'; import './a.js'; import './y.js'`, native ESM runs `x`, `a`, `y`, and the output runs `a`, `y`, `x`, as the build without `import defer` does (fixture `lazy_host_init_earlier_commonjs`).
- When neither the plan nor a declined init takes an importer, the importer still calls `init_x()` in its body, after its later static imports. The declined inits stop at the limit (see "Declined inits"). A fallback must keep the order and the async steps of the other imports.
- The rules do not check if the module of an earlier import already ran. Such an import still stops the plan, for example a barrel import of a module that another chunk already initialized.
- The split of a common host (below) moves only the modules after its first deferred module in execution order. When the first importer in execution order imports `w.js` before `a.js`, `w.js` stays in the host and runs at its load, also for a later importer that imports `a.js` first. The build without `import defer` has the same order, because a common chunk runs its modules in one order for all importers.
- With `strictExecutionOrder` and `preserveModules`, the order wrappers can turn a dynamic entry into a common chunk (`collapse_carries_trigger` in `crates/rolldown/src/stages/generate_stage/order_wrapping.rs`). That chunk does not run the module at load, so its eager importers in other files run it after their later static imports. This also occurs without `import defer`, so the plan does not take such a host.
- An asset that a plugin emits after the chunks get their names, for example in `renderChunk`, can get the name of the init chunk, as for any other chunk.
- The importer keeps its import of `init_x` from the host, though it does not call `init_x`. The default `minify: 'dce-only'` and the minifier remove the import. With `minify: false`, the import stays.

Tests:

- `preserve_modules_cjs_exports` (`cjs` output: the getters of a lazy host, and of a module in its closure that an eager importer also imports).
- `lazy_host_init_order`: the init right after the import of the host, with a `cjs` variant.
- `lazy_host_init_shared_host`: without `preserveModules`, two chunks share one init chunk.
- `lazy_host_init_name_collision`: the chunk of the source file `a.init.js` keeps its name, and the init chunk is `a2.init.js`.
- `lazy_host_init_earlier_module`: without `preserveModules`, an earlier import of a module of the same chunk moves that module to a new common chunk, and the plan takes the importer. With `preserveModules`, the plan takes the importer with no split. With a `cjs` variant.
- `lazy_host_init_common_chunk`: a deferred module in a common chunk, with a `cjs` variant.
- `lazy_host_init_mixed_common_chunk`: two deferred modules and a module that runs at load in one common chunk. The host splits, and the init of the second deferred module follows the import of the moved module. The test also loads the chunk of a deferred-only importer first, in a new process. With a `cjs` variant.
- `lazy_host_init_earlier_commonjs`: an earlier import of a CommonJS module. In `esm` output, its file runs it at load, so the plan takes the importer. In `cjs` output, a `default` getter runs it in the body, so the plan stops, and the declined init runs `a` before `y`.
- `lazy_host_init_earlier_barrel`: an earlier import through a barrel with no side effects stops the plan, and the declined inits run `z`, then `a`.
- `lazy_host_init_earlier_deferred_tla`: an earlier deferred import of a module with top-level await. The build falls back because of the order rule of top-level `await`.
- `lazy_host_init_after_side_effect_import`: `import './w.js'` keeps no statement, and the init of a later deferred module still comes after the import of the chunk of `w.js`.
- `lazy_host_run_at_load_tla`, `lazy_host_run_at_load_group`, `lazy_host_run_at_load_interleaved` and `lazy_host_run_at_load_module_id`: a common host does not split, because its later module has top-level `await`, because it is the chunk of a code splitting group, because a second deferred module comes between two later modules, or because of `chunkModulesOrder: 'moduleId'`. The host runs the deferred modules at load. In `lazy_host_run_at_load_group`, the deferred module has top-level `await`, so the host waits for its init.
- `lazy_host_deferred_first`, `lazy_host_failed_first`, `dynamic_failed_then_deferred` and `failed_then_eager`: a deferred access and an `import()` of a lazy host that throws give the same error, in both orders. The load of the lazy host does not throw.
- `cross_chunk_static_access`, `cross_chunk_reexport_star` and `user_entry_target`: a deferred access to a module in another chunk, through a star barrel, and to a user-defined entry.
- The Node test `topics/import-defer/lazy-host-init-chunk`: a plugin asset with the name of the init chunk keeps its name and content, and `renderChunk`, `augmentChunkHash` and the `imports` of the importer show the init chunk.

### Split before a deferred import

Only the first module of the chunk body takes part in the plan, because an init before the body also runs before each earlier module of the body. In `import './x.js'; import './a.js'; import './y.js'`, native ESM runs `x`, `a`, `y`. When `x` is in the chunk of the importer and `a` is a deferred module in another chunk, the chunk runs `y` at load, then `x` and `init_a()` in its body: `y`, `x`, `a`. An init right after the import of the host gives `a`, `y`, `x`.

`split_modules_before_deferred_imports` (`crates/rolldown/src/stages/generate_stage/lazy_host_inits.rs`) moves the modules of the body before the importer to a new common chunk, with the bits of the old chunk. The chunk imports the new chunk at its first import of them. Thus the importer is the first module of the body, and the plan initializes `a` right after the import of its host: `x`, `a`, `y`. The split runs in `generate_chunks` (`crates/rolldown/src/stages/generate_stage/code_splitting.rs`), after the modules get their chunks. It runs before `merge_external_import_symbols`, which groups the symbols by chunk, and before the chunk modules are sorted.

The importer is the first module of the body, in execution order, with a kept eager import of a deferred module in another chunk (`first_deferred_import_host`). The moved modules are the modules of the body before the importer. A bare `import './x.js'` keeps no statement, so the scan reads the kept records from `load_dependencies`. A chunk splits only when the moved modules cannot read the chunk at load, and when they keep their order among the other imports of the chunk (`modules_before_deferred_import`):

- Each moved module has no top-level await and uses no runtime helper. It has no dynamic import of a module of the chunk, and it imports no module of the chunk that stays.
- No moved module is in `common_chunk_preserve_export_names_modules` or `common_chunk_exported_facade_chunk_namespace` of the chunk.
- The importer has no top-level await.
- Each import of the importer before the host runs at load after the split: a moved module, an external module, or a module with no wrapper in another chunk. A deferred import of a module with top-level await before the host stops the split, as it stops the plan.
- A later import of the importer runs code at load: an import of an external module, or of a chunk that is not the host. A later import of a module of the chunk needs no split, because `ensure_lazy_module_initialization_order` already moves `init_a()` before that module.
- The walk of the import ranks (`chunk_import_ranks`) keeps the order (`split_keeps_import_order`). The check walks the chunk with the moved modules in the new chunk. The walk must visit the moved modules as one block, and these modules must lead to all moved modules. When an earlier import of the importer moves, the new chunk must come before the host. In a common chunk, the walk must visit each module that stays, because the chunk otherwise sorts its imports only by execution order.

#### Split of a common host

A common host can also hold a module `w.js` that runs at load, for example an eager import of a deferred-only importer `r.js`. The host then runs `w` when it loads, before the init after the import of the host. In `import './a.js'; import './w.js'`, native ESM runs `a` first. Thus a common host first gives up the modules that run at load and come after its first deferred module in execution order (`modules_after_deferred_modules`). Only a deferred module with an eager importer in another chunk counts (`deferred_modules_with_eager_importers`). This split runs before the split of the importers, so that they see the final chunk of each module. The moved modules go to a new common chunk with the bits of the host. The importers load the new chunk at their first import of its modules, so `b.js` runs `a`, then `w`. A deferred-only importer loads both chunks and runs only `w`. The runtime module and the deferred modules stay. The earlier modules also stay: the host runs them at load, before `a`, in the order of the build without `import defer`. The host splits only when the move keeps the load of each part:

- The host is not the chunk of a code splitting group, and `experimental.chunkModulesOrder` is not `'module-id'`.
- Each later deferred module of the host comes before all moved modules or after all of them, in execution order. The importers load the new chunk at one place, so no init can run between two moved modules.
- No module that stays imports a moved module. Else the host loads the new chunk.
- No dynamic import goes from one part to the other.
- The moved modules have no top-level await, and the chunk does not keep their exports for a facade (`common_chunk_preserve_export_names_modules`, `common_chunk_exported_facade_chunk_namespace`).

A second deferred module of the host can come after `w.js` in the imports of `b.js`: `import './a.js'; import './w.js'; import './a2.js'`. Its init then follows the import of the new chunk (see "Init after the import of a host").

#### Run at load

A host that does not split still runs `w` at load, before the init of `a`. Thus the host runs `a` at load too, at its place: the finalizer adds `init_a();` right after the wrapper of `a`, or `await init_a();` when `a` reaches a top-level `await` (`ChunkGraph::deferred_modules_run_at_load`, and the finalizer in `crates/rolldown/src/module_finalizers/impl_visit_mut.rs`). The build without `import defer` runs `a` at the same place, so each importer gets the order of that build. In the shapes of the conditions above, that order is also the native order. A deferred-only importer that loads the host then also runs `a` at load, as the build without `import defer` does.

- For a manual group, `plan_grouped_run_at_load` walks the eager dependencies of each entry in evaluation order. If every walk that evaluates the deferred module has already evaluated all unwrapped members of the group, the eager importer can run its init. Entries that only defer it can then load the same group without evaluating it.
- Only a deferred module with an eager importer in another chunk, and with a later module of the host that runs at load in execution order, runs at load. A deferred module after the last such module keeps its init after the import of the host.
- Such a module has no host (`ChunkGraph::deferred_init_host`), so the plan adds no init for it. Its importers keep their calls, which do nothing after the first call. The plan counts its import as an import that runs at load (`chunk_load_step`).
- Only a common chunk takes part. The split of a common host does not change a lazy host either.
- The declarations of an `import.defer()` target can move before the other modules of the chunk. The init call of such a module then stays at its old place (see "Declarations before the first call").

#### Limits of both splits

`move_modules_to_new_common_chunk` does both splits. The old chunk keeps the runtime helpers of its other modules, and it also keeps `ExportAll`, which the chunk optimizer adds for the namespace of an eliminated facade.

The splits do not run with `strictExecutionOrder`, `preserveModules`, dev mode, `experimentalInlineCommonChunks` or `codeSplitting: false`, or for `iife` and `umd` output. With `preserveModules`, each module already has its own file. With `experimental.chunkModulesOrder: 'module-id'`, neither split moves a module, and a common host runs its deferred modules at load instead (see "Run at load"). Neither split changes the chunk of a code splitting group. The eager entry walks determine which of its deferred modules run at load. The split of an importer chunk also skips a chunk that holds the runtime module, and an entry facade, whose entry module is in another chunk. Each new chunk adds a file to the output, only in a build with `import defer`.

## `import.defer()`

`import.defer('./x.js')` gives a promise for the deferred namespace object of `x.js`. The call loads the chunk of `x.js` and evaluates nothing. When the closure of `x.js` reaches a top-level `await`, the promise first waits for the modules with top-level `await`, as a static deferred import does (see "Top-level await").

- The pre-processor lowers a call to `import()`, with a warning, when the specifier is not a string literal or a template literal with no expressions (`NonStaticSpecifier` in `crates/rolldown/src/utils/tweak_ast_for_scanning.rs`). The bundler does not know the target of such a call, so it cannot wrap a closure for it. The pre-processor also lowers a call with a `/* @vite-ignore */` comment, with a warning (`IgnoredByComment`). The scanner adds no import record for such a call (`is_import_expr_ignored_by_comment`), so the build cannot defer its module either.
- The finalizer removes the phase from each import expression (`crates/rolldown/src/module_finalizers/impl_visit_mut.rs`). A call that the build keeps gets a new form (see below). In a build that falls back, each call becomes `import()`.
- `wrap_import_defer_closures` (`crates/rolldown/src/stages/link_stage/wrapping.rs`) takes the ES module target of each call as the target of a closure, as for the static form, and sets `dynamic_defer_target` (`crates/rolldown/src/types/linking_metadata.rs`). `track_import_defer_modules` also follows these records. A JSON target loses its wrapper later (see "Link stage"), so the call gives its deferred namespace object with no init call. A CommonJS target makes the build fall back (see "Whole-build fallback").
- `create_exports_for_ecma_modules` keeps `x_deferred` for each target, because the form of the call reads it. `reference_needed_symbols` adds `__esmGather` when the target reaches a top-level `await`.
- `plan_grouped_dynamic_imports` (`crates/rolldown/src/stages/generate_stage/order_wrapping.rs`) reads no phase, so it can also take an `await import.defer()`. It takes only a target with no wrapper (`is_order_wrap_eligible`), and of the targets of a call, only a JSON target has no wrapper. The plan gives the JSON module an order wrapper and removes the facade chunk of the target. The call gets the form for the same chunk (`(async () => x_deferred)()`), so the wrapper runs at the first access, through `x_deferred`. A check of the phase gives the same results, but it keeps the facade chunk.
- The form of the call (`rewrite_dynamic_import_of_defer_target` in `crates/rolldown/src/module_finalizers/mod.rs`):
  - In the same chunk, and with `codeSplitting: false`: `(async () => x_deferred)()`.
  - In another chunk: `(async (n) => (n = await import('./x.js'), n.x_deferred))()`. In `cjs` output, `require()` loads the chunk: `(n = (await 0, require('./x.js')), n.x_deferred)`. The chunk of the target exports `x_deferred` (see "Lazy hosts").
  - With a preparation, the function first runs `__esmGather(init_x) || await __esmGather()`. A failed preparation rejects the promise.
- Each call and each static deferred import of `x.js` give the same object, `x_deferred` (fixture `dynamic_identity`). The key `then` of the object is absent (see "Link stage"), so the promise does not evaluate the module when it resolves with the object.

### Declarations before the first call

In the same chunk, the call reads `x_deferred` when it runs. An earlier module of the chunk can call `import.defer()` before the code of `x.js` runs. A callback can also read the namespace during a pause at a top-level `await`. Without a move, the call reads `undefined`, or the read reaches a namespace object of the closure that does not exist yet.

A static deferred namespace is also observable before its importer runs when another member of an eager cycle calls a hoisted function. Its namespace must exist when that call runs, while the target body stays lazy.

Thus `plan_defer_declarations` (`crates/rolldown/src/stages/generate_stage/place_defer_declarations.rs`) puts the code of each target, and of its static closure in that chunk, before the other modules of the chunk, after the runtime module:

- Only a module whose code has only declarations moves: a module with an init wrapper, a CommonJS module, or a JSON module (`unwrap_json_module`). Thus the move does not change when a module evaluates.
- The earliest call can come from a module that invokes a hoisted function in a cycle. The position of the module containing the `import.defer()` expression therefore gives no lower bound on the call.
- The plan runs before `finalize_modules`, and `place_defer_declarations` applies it after `finalize_modules`. The order analyses and the finalizers read the order of a chunk as the order of execution.

A module that its common chunk runs at load (see "Run at load") has an init call after its wrapper. When such a module moves, its call stays at its old place: the last module before it that keeps its place runs the call at the end of its code (`ChunkGraph::run_at_load_anchors`, and the finalizer in `crates/rolldown/src/module_finalizers/impl_visit_mut.rs`). When no such module exists, the module does not move. Without the anchor, the call moves with the declarations, and the module runs before the modules that came before it in the chunk (fixtures `dynamic_same_chunk_call_before_target` and `dynamic_same_chunk_top_level_await`).

Tests:

- `dynamic_same_chunk`, `dynamic_cross_chunk`, `dynamic_identity`, `dynamic_unused_result`, `dynamic_eager_and_deferred` and `dynamic_failed_before_defer`: the two forms of the call, the identity of the namespace, a call whose result no code reads, an eager import and a call of one module, and a call after a failed evaluation.
- `dynamic_same_chunk_call_before_target`, `dynamic_same_chunk_read_during_tla` and `dynamic_same_chunk_top_level_await`: the declarations before the first call.
- `dynamic_tla_same_chunk`, `dynamic_tla_cross_chunk` and `dynamic_tla_failed_before`: the preparation of a target that reaches a top-level `await`. `dynamic_eager_tla`: an eager `import()` of such a target, whose chunk does not run it at load.
- `dynamic_json_target`, `dynamic_json_target_eager_and_deferred` and `dynamic_json_target_strict_user_entry`: JSON targets.
- `preserve_modules` and `user_entry_dynamic_target`: a call with `preserveModules`, and a call of a user-defined entry.
- `non_literal_specifier` and `vite_ignore_specifier`: the warnings for a specifier that is not static and for a call with a `/* @vite-ignore */` comment.
- The Node test `packages/rolldown/tests/import-defer-fallback.test.ts`: the fallback for an external target in `iife` output, for a CommonJS target, and for a module with no module syntax with `codeSplitting: false`, and a kept call.
- The test262 tests in `language/expressions/dynamic-import/import-defer` (`crates/rolldown/tests/integration/test262.rs`).

## External modules

The rules depend on the output format. `deferred_closures` (`crates/rolldown/src/utils/import_defer_fallback.rs`) checks them before the link stage, and a build that breaks a rule falls back (see "Whole-build fallback"). Two kinds of external module do not count, because their early evaluation is not observable:

- a builtin module (`is_node_like_builtin` in `crates/rolldown_utils/src/node_builtin.rs`);
- an external module with no side effects, for example with `treeshake.moduleSideEffects: false`.

The reasons for the rules are in [design.md](./design.md#external-modules).

### `cjs` output

- A module of a closure loads each external module at the place of its import, in its init (`loads_externals_lazily` in `crates/rolldown/src/module_finalizers/mod.rs`):
  - An import becomes `var import_ext = require("ext")`, or `__toESMCached("ext", require("ext"))` when the import needs the interop helper (`lazy_external_import_stmt`). The link stage declares `import_ext` (`reference_needed_symbols`), and binds each import of the module to it (`match_external_import` in `crates/rolldown/src/stages/link_stage/bind_imports_and_exports.rs`).
  - `export * from 'ext'` becomes `__reExport(x_exports, require("ext"))` at its place (`cjs_external_re_export_stmt`).
  - The chunk has no `require()` of these external modules at its top: `is_lazy_external_import` (`crates/rolldown/src/stages/generate_stage/compute_cross_chunk_links.rs`) leaves them out of the imports of the chunk.
- `import defer * as ns from 'ext'` gets a deferred namespace over `__toESMCached("ext", require("ext"))` (`create_deferred_cjs_namespaces` in `crates/rolldown/src/stages/link_stage/wrapping.rs`, and `deferred_cjs_import_decl` in the finalizer). The first access loads the external module.
- `import.defer('ext')` becomes `(async () => __deferNsCached("ext", () => __toESMCached("ext", require("ext"))))()`, but only with `output.dynamicImportInCjs: false`. With `dynamicImportInCjs: true` (the default), the call keeps the native `import()` of the build with the option off, with an `UNSUPPORTED_FEATURE` warning (`lowered_external_dynamic_defer_imports`). A call with a second argument also keeps `import()`, because `require()` cannot carry import attributes. The module then evaluates when it loads.
- `__toESMCached` and `__deferNsCached` give one object for each external module and interop mode, also across chunks (see [runtime-helpers](../runtime-helpers/implementation.md)). `mark_lazy_namespace_externals` (`crates/rolldown/src/stages/link_stage/patch_module_dependencies.rs`) sets `has_lazy_namespace_load` on each external module that a module of a closure loads lazily with the interop helper. The binding of such an external module in the imports of a chunk then also uses `__toESMCached` (`render_cjs_chunk_imports` in `crates/rolldown/src/ecmascript/format/cjs.rs`). Thus an eager import and a lazy load give one namespace object.
- The output file of an entry module loads the external modules of its `export *` chain when the file loads (`render_chunk_exports`: `var ext = require("ext")` and a copy of the keys of `ext`). Thus the build falls back when a module of a closure can be such an entry module:
  - with `preserveModules`;
  - as an entry point: user-defined, emitted, or the target of an `import()` or `import.defer()` call.

  The check does not know the chunks, so it also counts an entry module that gets an entry facade.

### `esm` output

- The output keeps `import defer * as ns from 'ext'` and `import.defer('ext')`. `render_named_imports` (`crates/rolldown/src/ecmascript/format/esm.rs`) renders a deferred namespace import as its own `import defer` declaration. The chunk renders a bare `import "ext"` for the side effects of the external module only when a module of the chunk has an eager import of it (`has_evaluation_phase_import`). The finalizer sets the phase of an `import.defer()` call of an external module (`try_rewrite_import_expression`).
- The chunk imports each external module at its start. Thus the build falls back when a module of a closure has an eager import or re-export of an external module.

### `iife` and `umd` output

The output gets each external module as a global or as a wrapper argument, so it cannot delay the external module. The build falls back when a module of a closure imports an external module, and for each deferred import of an external module. A build with no such import keeps the deferral. `iife` and `umd` output make one chunk, so they have no lazy host.

### Limits

- In `esm` output, the check also counts an external module that a module outside the closures imports eagerly. The check runs before the chunks and tree shaking exist. Such a build falls back, also when the eager import of the other module evaluates the external module first in native Node.
- In `cjs` output, `require()` throws for an ES module with top-level `await` (`ERR_REQUIRE_ASYNC_MODULE`). Thus a static deferred import of such a module, and an import of it in a module of a closure, throw at the first access. The build with the option off also loads these modules with `require()`, and throws when the chunk loads.

Tests:

- `cjs` output: `external_cjs`, `external_cjs_cache_key`, `external_cjs_cross_chunk_identity`, `external_cjs_cross_chunk_observer`, `external_cjs_dynamic_import`, `external_cjs_eager_and_deferred`, `external_cjs_shared_namespace`, `transitive_cjs`, `transitive_cjs_barrel`, `transitive_cjs_dynamic`, `transitive_cjs_identity` and `transitive_cjs_namespace_reexport`.
- `esm` output: `external_esm_preserve`, `transitive_esm_builtin` and `transitive_esm_pure_external`.
- The fallbacks: `external_iife_umd`, `transitive_external_fallback`, and the Node test `packages/rolldown/tests/import-defer-fallback.test.ts` (each format, and the `export *` rule of `cjs` output).

## Deferred groups

In `esm` output, a deferred group is a root and its private members. Only the root keeps its init wrapper. The body of each member runs in the wrapper of the root, in the order of evaluation. Each member keeps its top-level declarations (hoisted functions, the namespace object and the `var` list) at the top level of the chunk. The reasons are in [design.md](./design.md#deferred-groups).

Before (fixture `topics/import_defer/timing_star_target`, shortened):

```js
// a.js
var a;
function init_a() {
  return (init_a = __esmMin(() => {
    a = { value: 1 };
  }))();
}
// barrel.js
function init_barrel() {
  return (init_barrel = __esmMin(() => {
    init_a();
    init_b();
  }))();
}
// first.js
function read() {
  return a.value;
}
function init_first() {
  return (init_first = __esmMin(() => {
    init_barrel();
  }))();
}
```

After:

```js
// a.js
var a;
// first.js
function read() {
  return a.value;
}
function init_first() {
  return (init_first = __esmMin(() => {
    // a.js
    a = { value: 1 };
    // barrel.js
    init_b();
  }))();
}
```

`b.js` keeps its wrapper, because the closure of another root also contains it.

### Membership: `crates/rolldown/src/stages/generate_stage/deferred_groups.rs`

`GenerateStage::group_deferred_modules` runs after `commit_cross_chunk_links` (which writes the chunk exports), `ensure_lazy_module_initialization_order` and `plan_defer_declarations` (which sets the anchors), and before `finalize_modules`.

1. Guards. No groups if `importDefer` is off, or with these conditions: a format other than ESM, strict execution order, dev mode, `profilerNames`, inline common chunks. Only `render_esm` renders the groups.
2. Groupable module: in a chunk, included, `wrapped_by_import_defer`, `WrapKind::Esm`, has a `wrapper_ref`, no top-level await, no state record (`import_defer_tracked_deps`), not a `dynamic_defer_target`, not the runtime. Also not in `ChunkGraph::deferred_modules_run_at_load`, and not an anchor (a value of `ChunkGraph::run_at_load_anchors`).
   - Why the last rule: the finalizer puts the `init_x();` call of such a module, or of the module after an anchor, at the top level of the program (see "Run at load" and "Declarations before the first call"). In a group module, that place is the body, which runs in the wrapper of the root. In `group_run_at_load_anchor`, the anchor `r.js` would get the member `m.js`, and the init call of `l.js` would run in the wrapper of `r.js`. Then `w.js` runs before `l.js`.
   - A module that its chunk runs at load has an eager importer, so the next step keeps the wrappers of the module and of its static imports. Thus the module gets no members, and its check is only a guard.
3. `modules_that_keep_wrapper`: entries, modules with a deferred namespace (`deferred_namespace_ref`), `DeferredAccess::Esm` targets, the modules that eager code reaches through static records, all modules of an inline common chunk record, the modules that `ensure_lazy_module_initialization_order` moves an init call to or from (`remove_map`, `insert_map`), and each module whose wrapper a chunk exports.
   - Why the last rule: the cross-chunk links add the wrapper of the owner of each symbol that a chunk imports (`add_depended_symbol_with_wrapped_esm_init`), also when the importing chunk does not call it. Without the rule, a member loses its declaration, but its chunk still exports it, and Node does not load the chunk (`SyntaxError: Export 'init_x' is not defined in module`).
   - The rule reads the keys of `exports_to_other_chunks` and compares canonical refs, because only one branch of `deconflict_exported_names` stores canonical keys. The table also has the entry exports.
   - The same set goes into `commonjs_modules_of_groups`, so an exported `require_x` stays at the top level.
   - A module that keeps its wrapper is a root. Thus the rule can split a group: the module keeps its own private members.
4. Callers. An import record of an included module, or a restored init call (`transitive_init_targets`), is a call. A call is plain if it is a non-defer `ImportKind::Import` record (or a restored call) from a module of the same chunk.
5. Candidate: a groupable module that does not keep its wrapper and has only plain callers. The other groupable modules are the first roots.
6. Fixed point (greatest):
   - A candidate that only one root reaches through candidates gets that root as its owner.
   - A member with a caller that is not its root or a member of the same root loses its owner. This repeats until no member changes.
   - The candidates with no owner are shared modules. The shared modules that a module outside the shared part calls become roots, and the loop starts again. Thus a shared module can have its own group. If each shared module has only shared callers (a cycle), all of them become roots.
   - A root with a no-op init (`init_is_noop`) gets no group.
7. CommonJS modules (`commonjs_modules_of_groups`): a CommonJS module of a deferred closure goes into the group of a root when each caller is in the same chunk and is one of these (least fixed point):
   - a plain `import` of a member or of the root. The finalizer puts the `require_x()` call of a static import in the body of the importer.
   - a `require()` of a CommonJS module that goes into the same group. The whole code of a CommonJS module is in its factory.

   A `require()` of an ESM module does not qualify, because it can be in a hoisted function at the top level. A root with no members can also get CommonJS modules. This is also true when the chunk has only one groupable module, or when no module is a candidate.

8. The result: `concatenated_wrapped_module_kind` is `Root` or `Inner`, and `chunk.module_groups` gets one `ModuleGroup` for each group. A root with no members gets no group, and keeps the kind `None`. A CommonJS module of a group keeps the kind `None` and its `require_*` wrapper.

`compute_wrapped_esm_init_metadata` runs before the groups exist, so `init_is_noop` reads each module as a module with no group. This is safe: a root with a no-op init gets no group (step 6), and no code calls the init of a member.

### Finalizer: `crates/rolldown/src/module_finalizers`

- `impl_visit_mut.rs`: a root or a member puts its declarations, then its body, at the top level of its program. It has no wrapper in the program. `RenderedConcatenatedModuleParts::body_start` is the index of the first statement of the body. The finalizer records it after the last change to the statements (the removal of the empty statements of the JSON inline).
- A root also records the text of its wrapper (`group_root_wrapper_text`). The finalizer prints the wrapper with an empty closure and splits the text at its only `{}`. Thus the text has the hoisted form, the canonical names and the PIFE setting of a wrapper that has no group.
- `mod.rs`: the finalizer records the steps of each body in `RenderedConcatenatedModuleParts::group_events`, in source order:
  - `Member(idx)`: an import of a member. The finalizer removes the init call.
  - `Call(text)`: the text of an init call of a module outside the group.
  - `Init(text)`: a statement that writes a binding or a namespace object at the place of an import or a re-export: `var import_x = __toESM(require_x())`, `var import_x, x_deferred = __deferNs(...)`, `__reExport(p_exports, __toESM(require_x()))` or `__reExport(p_exports, x_exports)`.
- `move_group_init_stmts` runs at the end of `remove_unused_top_level_stmt`. The statement of each `Init` step with a later `Member` step moves out of the body, and the step gets its text. A `var` declaration becomes an assignment, and its names go into the hoisted `var` list of the module. The other statements stay in the body, and their steps go away. The positions that the loop recorded before the move (the end of the imports, and the place of the `__name()` statement of `export default function () {}` with `keepNames`) become smaller by the number of moved statements before them.
- Why the move: a member body runs before the whole body of its importer. A wrapper runs the import statements in source order, so the binding of an import is ready before the init call of each later import. Without the move, a member body can read a binding before its statement runs. A cycle is not necessary: in `group_cjs_exports_snapshot`, a member adds a property to the exports of a CommonJS module, and the root must make its namespace before that.
- `finalize_modules` (`crates/rolldown/src/stages/generate_stage/finalize_modules.rs`) keeps the parts of each root and member in `Chunk::module_idx_to_render_concatenated_module`.
- Invariant: no code refers to the `wrapper_ref` of a member (a `debug_assert` in `finalized_expr_for_symbol_ref`), and no chunk exports it (`modules_that_keep_wrapper`).

### Rendering

- `print_deferred_group_bodies` (`render_chunk_to_assets.rs`) prints the body of each root and member as its own `Program` (`render_normal_module_group_body` in `render_ecma_module.rs`), in parallel, before the chunk loop prints the declarations.
- `ensure_lazy_module_initialization_order` moves some import statements to an earlier module of the chunk. `finalize_modules` adds them at the start of that program after all finalizers ran. The earlier module always has `WrapKind::None`, so it is not in a group (a `debug_assert` in `finalize_modules`). Thus no root or member gets these statements, and its `body_start` stays correct.
- `take_group_body_comments` gives each comment to the part where a print of the whole program puts it. A comment goes to the declarations if it is at position 0, if it is in the span of a declaration statement, or if it is a legal or a coverage comment before the last declaration statement (Oxc prints these as orphan comments before the next statement). All other comments go to the body.
- `render_grouped_ecma_module` (`crates/rolldown/src/utils/render_ecma_module.rs`) joins the two outputs, each with its own sourcemap. `RenderedModuleSource::group_split` is the number of sources of the declarations.
- `render_deferred_groups` (`crates/rolldown/src/ecmascript/format/esm.rs`) appends the declarations of each module in chunk order. For a root, it then appends the start of the wrapper text and the whole wrapper declaration (`var require_x = __commonJSMin(...)`) of each CommonJS module of the group. Then `append_group_member_bodies` appends the member bodies, and the root appends its pending steps, its own body and the end of the wrapper text. The member bodies and the pre-calls can thus call each moved `require_x`. The engine compiles a moved factory with the root wrapper, not at startup.
- `append_group_member_bodies` walks the steps depth first, like the spec `InnerModuleEvaluation`. A member body comes after the bodies of the members that it imports. The `Call` and `Init` steps of a module are pending, in their order, until the walk adds the next member body. Then the walk writes them before that member body:
  - A `Call` step becomes a pre-call: a copy of the call, so that the module outside the group still evaluates first. A pre-call that an earlier pre-call of the same group made is not added again.
  - An `Init` step is written one time, because it is pending only one time.
- A `Member` step of a member that already ran adds nothing, and the pending steps stay pending. At the end of the steps of a module, the walk writes the pending steps up to the last `Init` step, before the body of the module. The body makes the calls after it (`group_init_visited_member`).
- The debug comment (`// path`) of a member body comes before that body in the root wrapper. The root gets a second comment only if a CommonJS wrapper or a member body comes before its own code. A root with no declarations gets its comment before the start of its wrapper.

### Limits

- The sourcemap lists the source of a root or a member two times in `sources`, because `ConcatSourceMapBuilder` does not merge equal sources. The mappings are correct.
- The texts of the `Call` and `Init` steps have no mappings in the sourcemap.
- Without minify, the member bodies, the moved CommonJS wrappers, the code of the root and the end of the root wrapper have no indent in the root wrapper. The default `dce-only` minify prints the chunk again with the correct indent.

### Tests

The fixtures in `crates/rolldown/tests/rolldown/topics/import_defer/` with the prefix `group_`:

- `group_conditional_require`: a module that only a `require()` evaluates keeps its wrapper.
- `group_pre_call`: the pre-call of a shared module before a member body.
- `group_member_throws`: two accesses throw the same error, and the member body runs one time.
- `group_cycle`: a cycle in a group, and a cycle through shared modules with a group of a shared module.
- `group_nested_defer`: a deferred import in the closure stays lazy.
- `group_state_record`: modules with a state record keep their wrappers, with a pre-call.
- `group_commonjs`: CommonJS modules of a group (also a `require()` chain) move into the root wrapper. A CommonJS module that eager code imports, or that a hoisted function requires, stays at the top level.
- `group_cjs_one_root`, `group_cjs_two_roots`: roots that import only CommonJS modules get their CommonJS wrappers, with one groupable module and with no candidates.
- `group_cjs_exports_snapshot`: with no cycle, the namespace of a CommonJS module is made before a member body changes its exports.
- `group_cjs_cycle`, `group_deferred_cjs_cycle`, `group_star_cycle`: in a cycle, a member reads a CommonJS binding, a deferred CommonJS namespace or a key of an `export *` before the body of its importer runs.
- `group_cjs_import_stays`: an `Init` step with no later member stays in the body, after the init call before it.
- `group_init_visited_member`: each later member already ran, so the moved statement comes before the body of its module.
- `group_keep_names`: `keepNames` with a moved statement before `export default function () {}`.
- `group_marker_text`: user code that has the text of the old group marker does not change.
- `group_pife`: the root wrapper text with the default PIFE setting and with `pifeForModuleWrappers: false`.
- `group_sourcemap`: the mappings of the declarations and of the body of a root and a member.
- `group_comments`: each legal comment prints one time, in the part where a print of the whole module puts it.
- `group_exported_wrapper`: a chunk exports the wrapper of a module that the other chunk does not call. The module keeps its wrapper. The config sets `format`, so that the extended variant runs with `minifyInternalExports: false`. That branch of `deconflict_exported_names` stores the keys without `canonical_ref_for`.
- `group_run_at_load_anchor`: the anchor of a module that its chunk runs at load is not in a group.

A build with a code splitting group that sets `includeDependenciesRecursively: false` falls back (see "Whole-build fallback"), so no fixture has a chunk cycle with a group.

## Known gaps

- The warnings of a fallback come again on each incremental rebuild.
- With the option on, the devtools event `ChunkGraphReady` comes after the export modes and the init plan, so that it lists the init chunks. Thus a build whose export modes fail sends no such event. With the option off, the event keeps its place.
- In `cjs` output with `strictExecutionOrder`, the runtime module goes into its own chunk. The link stage wraps the closure modules, and the order lowering then moves the runtime out of the entry chunk. It moves the runtime back only for `esm` output (`fold_runtime_chunk_after_order_lowering`). A build with the option off has the same layout when the link stage wraps a module, for example a CommonJS module.
- In a closure module, a top-level `arguments` is the arguments object of `init_x`, and direct `eval` code runs in `init_x` (see the JSDoc of `experimental.importDefer`).
- A call `ns.fn()` of a CommonJS target gets `import_x` as `this`, not the deferred namespace object. Thus `this === ns` gives `false` in `fn` when `ns` escapes (`ns` is then `x_deferred`). Native Node and the eager import give `true`. A call through `x_deferred` gives the native result, but each call then goes through the `Proxy`.
- The deferred namespace object of a CommonJS module, or of a module with dynamic exports, gets its keys at the first evaluation. Before that, `Object.isExtensible(ns)` gives `true`, and after `Object.preventExtensions(ns)`, the first evaluation throws a `TypeError` when the namespace has keys (see `__deferNs`).
- With `preserveModules`, an entry chunk gets no facade (`preserve_strict_entry_signatures` skips `preserveModules`). Thus the file of a user-defined or an emitted entry runs its module when it loads, also when the module is in a closure. A deferred import or an `import.defer()` of the module in another file loads that file, so the module runs before the first access. This is the order of the build without `import defer`, and the build gives no warning. With `strictExecutionOrder`, the order wrappers move the module into a common chunk, and the order is native.
- The test262 test `promise-prototype-then-not-called` fails: the output loads the chunk of the target with a native `import()`, and the `import()` of Node.js calls `Promise.prototype.then`.

## Related

- [design.md](./design.md) — why one fallback for the whole build, why a chunk initializes a lazy host after its import, the decisions for `import.defer()`, for external modules, for the deferred groups and for the PIFE hint
- `../runtime-helpers/implementation.md` — the runtime helpers of `import defer`
