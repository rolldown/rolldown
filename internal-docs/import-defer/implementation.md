# Import defer — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

With `experimental.importDefer`, each ESM module of an `import defer` closure gets a lazy init wrapper (`function init_x() { return (init_x = __esmMin(() => { ... }))(); }`). The first access to a deferred namespace calls the wrapper of the target, and that wrapper calls the wrappers of the modules that the target imports.

This doc describes three parts of the machinery: the deferred groups, the lazy hosts, and the shared namespaces of a CommonJS defer target. The other parts (the closures, the other deferred namespaces, the state records of `__esmState`, the top-level await rules) are not described here yet.

## Deferred groups

A deferred group is a root and its private members. Only the root keeps its init wrapper. The body of each member runs in the wrapper of the root, in the order of evaluation. Each member keeps its top-level declarations (hoisted functions, the namespace object and the `var` list) at the top level of the chunk.

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

`GenerateStage::group_deferred_modules` runs after `commit_cross_chunk_links` (which writes the chunk exports) and `ensure_lazy_module_initialization_order`, and before `finalize_modules`.

1. Guards. No groups if `importDefer` is off, or with these conditions: a format other than ESM, strict execution order, dev mode, `profilerNames`, inline common chunks. Only `render_esm` renders the groups.
2. Groupable module: in a chunk, included, `wrapped_by_import_defer`, `WrapKind::Esm`, has a `wrapper_ref`, no top-level await, no state record (`import_defer_tracked_deps`), not a `dynamic_defer_target`, not the runtime.
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

8. The result: `concatenated_wrapped_module_kind` is `Root` or `Inner`, and `chunk.module_groups` gets one `ModuleGroup` for each group. A CommonJS module of a group keeps the kind `None` and its `require_*` wrapper.

### Finalizer: `crates/rolldown/src/module_finalizers`

- `impl_visit_mut.rs`: a root or a member puts its declarations, then its body, at the top level of its program. It has no wrapper in the program. `RenderedConcatenatedModuleParts::body_start` is the index of the first statement of the body. The finalizer records it after the last change to the statements (the removal of the empty statements of the JSON inline).
- A root also records the text of its wrapper (`group_root_wrapper_text`). The finalizer prints the wrapper with an empty closure and splits the text at its only `{}`. Thus the text has the hoisted form, the canonical names and the PIFE setting of a wrapper that has no group.
- `mod.rs`: the finalizer records the steps of each body in `RenderedConcatenatedModuleParts::group_events`, in source order:
  - `Member(idx)`: an import of a member. The finalizer removes the init call.
  - `Call(text)`: the text of an init call of a module outside the group.
  - `Init(text)`: a statement that writes a binding or a namespace object at the place of an import or a re-export: `var import_x = __toESM(require_x())`, `var import_x, x_deferred = __deferNs(...)`, `__reExport(p_exports, __toESM(require_x()))` or `__reExport(p_exports, x_exports)`.
- `move_group_init_stmts` runs at the end of `remove_unused_top_level_stmt`. The statement of each `Init` step with a later `Member` step moves out of the body, and the step gets its text. A `var` declaration becomes an assignment, and its names go into the hoisted `var` list of the module. The other statements stay in the body, and their steps go away. The positions that the loop recorded before the move (the end of the imports, and the place of the `__name()` statement of `export default function () {}` with `keepNames`) become smaller by the number of moved statements before them.
- Why the move: a member body runs before the whole body of its importer. A wrapper runs the import statements in source order, so the binding of an import is ready before the init call of each later import. Without the move, a member body can read a binding before its statement runs. A cycle is not necessary: in `group_cjs_exports_snapshot`, a member adds a property to the exports of a CommonJS module, and the root must make its namespace before that.
- Invariant: no code refers to the `wrapper_ref` of a member (a `debug_assert` in `finalized_expr_for_symbol_ref`), and no chunk exports it (`modules_that_keep_wrapper`).

### Rendering

- `print_deferred_group_bodies` (`render_chunk_to_assets.rs`) prints the body of each root and member as its own `Program` (`render_normal_module_group_body` in `render_ecma_module.rs`), in parallel, before the chunk loop prints the declarations.
- `ensure_lazy_module_initialization_order` moves some import statements to an earlier module of the chunk. `finalize_modules` adds them at the start of that program after all finalizers ran. The earlier module always has `WrapKind::None` (a `debug_assert` in `finalize_modules`). Thus no root or member gets these statements, and its `body_start` stays correct.
- `take_group_body_comments` gives each comment to the part where a print of the whole program puts it. A comment goes to the declarations if it is at position 0, if it is in the span of a declaration statement, or if it is a legal or a coverage comment before the last declaration statement (Oxc prints these as orphan comments before the next statement). All other comments go to the body.
- `render_grouped_ecma_module` (`crates/rolldown/src/utils/render_ecma_module.rs`) joins the two outputs, each with its own sourcemap. `RenderedModuleSource::group_split` is the number of sources of the declarations.
- `render_deferred_groups` (`crates/rolldown/src/ecmascript/format/esm.rs`) appends the declarations of each module in chunk order. For a root, it then appends the start of the wrapper text and the whole wrapper declaration (`var require_x = __commonJSMin(...)`) of each CommonJS module of the group. Then `append_group_member_bodies` appends the member bodies, and the root appends its pending steps, its own body and the end of the wrapper text. The member bodies and the pre-calls can thus call each moved `require_x`. The engine compiles a moved factory with the root wrapper, not at startup.
- `append_group_member_bodies` walks the steps depth first, like the spec `InnerModuleEvaluation`. A member body comes after the bodies of the members that it imports. The `Call` and `Init` steps of a module are pending, in their order, until the walk adds the next member body. Then the walk writes them before that member body:
  - A `Call` step becomes a pre-call: a copy of the call, so that the module outside the group still evaluates first. A pre-call that an earlier pre-call of the same group made is not added again.
  - An `Init` step is written one time, because it is pending only one time.
- A `Member` step of a member that already ran adds nothing, and the pending steps stay pending. At the end of the steps of a module, the walk writes the pending steps up to the last `Init` step, before the body of the module. The body makes the calls after it (`group_init_visited_member`).
- The debug comment (`// path`) of a member body comes before that body in the root wrapper. The root gets a second comment only if a CommonJS wrapper or a member body comes before its own code. A root with no declarations gets its comment before the start of its wrapper.

### Known gaps

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
- `group_chunk_cycle`: a chunk cycle, where a deferred access calls the root before its chunk runs.
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

## Lazy hosts

A lazy host is the entry chunk of an ES module of a deferred closure, when that chunk is not a user-defined or an emitted entry (`ChunkGraph::lazy_dynamic_defer_host` in `crates/rolldown/src/chunk_graph.rs`). With `preserveModules`, each such module gets one. Without `preserveModules`, the dynamic entry chunk of an `import()` or `import.defer()` of the module can be one. Another chunk can load a lazy host for a deferred access, so the load must not run the module:

- `render_wrapped_entry_chunk` (`crates/rolldown/src/utils/chunk/render_chunk_exports.rs`) writes no `init_x()` at the end of the chunk. The importer calls `init_x()` at the first access.
- In `cjs` output, each export of a lazy host is a getter (`render_chunk_exports`), as in a common chunk. `init_x()` assigns the bindings after the load, so a copy (`exports.foo = foo`) keeps `undefined`, also for a `const`. An eager importer reads the bindings through the same exports: it calls `init_x()`, then reads them. The mode of a lazy host stays `Named`, because other chunks import `init_x`, which is not a declared export (`export_mode_module` in `crates/rolldown/src/stages/generate_stage/mod.rs`).
- The chunk exports internal bindings (`init_x`, `x_exports`, `x_deferred`), but no code reads its namespace: `import()` reads `init_x` and `x_exports`, and `import.defer()` reads `x_deferred`. So the rule that a dynamic entry chunk exports only the exports of its entry does not apply to a lazy host (`dynamic_entry_reads_defer_bindings` in `crates/rolldown/src/stages/generate_stage/dynamic_entry_namespace.rs`). The same is true for the chunk of a JSON module that `import.defer()` loads. A JSON module has no interop wrapper and is not a lazy host, but its chunk also exports `x_exports` and `x_deferred`. So `import()` of it in another chunk reads only `x_exports`: `import('./chunk.js').then((n) => n.x_exports)` (`rewrite_dynamic_import_of_defer_target`, `topics/import_defer/dynamic_json_target_eager_and_deferred`). With `strictExecutionOrder`, the JSON module can have an execution-order wrapper, and its chunk does not always call `init_x()` at load: an entry facade calls it instead (`preserve_strict_entry_signatures`), and a deferred import in the same chunk does not call it. So each `import()` of it also calls `init_x()`, and the chunk exports `init_x` to the other chunks.

The load of a lazy host runs nothing, also for code outside the bundle. With `preserveModules`, a direct import of the file of a deferred module that is not an entry does not run the module, and its exports stay uninitialized. Native ESM runs the module. This is an accepted difference. The docs of `input` already recommend an entry for each file whose export signatures must stay, instead of `preserveModules` (`packages/rolldown/src/options/docs/input.md`), and a user-defined entry chunk is not a lazy host.

### Init after the import of a host

The host of a deferred ES module is the chunk that holds the module and does not run it at load (`ChunkGraph::deferred_init_host` in `crates/rolldown/src/chunk_graph.rs`): the lazy host of the module, or a common chunk. A common chunk has no tail, so only an init function runs its wrapped modules. An eager importer in another chunk calls `init_x()` in its body.

The imports of a chunk run before its body, so this call runs the module after the later static imports of the importer: `import './x.js'; import './y.js'` runs `y` first. In one chunk, `ensure_lazy_module_initialization_order` moves the call to an earlier module. Across chunks, `plan_lazy_host_inits` (`crates/rolldown/src/stages/generate_stage/lazy_host_inits.rs`) moves the init to the chunk of the importer, right after one of its imports: the import of the host, or a later import of an earlier module of the importer. The reasons are in [design.md](./design.md#init-after-the-import-of-a-lazy-host).

- `esm` output: `add_lazy_host_init_chunks` adds an init chunk for each deferred module that a chunk initializes. A common chunk can hold more than one deferred module, so an init chunk calls the init function of one module. The init chunk is a common chunk with no modules. It imports `init_x` from the host and calls it: `import { init_x } from "./x.js"; init_x();`, with `await` when the module has top-level await (`render_lazy_host_init_call` in `crates/rolldown/src/ecmascript/format/esm.rs`). Each chunk that initializes the module imports the init chunk right after the import that the init follows (`LazyHostInit::after`), in `static_imports`, `cross_chunk_imports` and `imports_from_other_chunks`. The init chunks after one import keep the order of the imports of the importer. Two chunks that initialize the same module share its init chunk.
  - The plan runs after the export modes and before the chunk names. Thus the names, `renderChunk`, `augmentChunkHash`, the hashes and the minifier handle the init chunk as any other chunk, and the `imports` of each importer list it. The init chunk has no source map, because no part of its code has one (`SourceJoiner`). A chunk that only re-exports also has no map.
  - The name of the init chunk is the name of the host with `.init` (`name_lazy_host_init_chunks`): `x.init.js`, or `x.init-[hash].js`. A common chunk takes its name from one of its modules, so the init chunk of a common host takes the name of its own module instead (the naming in `crates/rolldown/src/stages/generate_stage/mod.rs`). Without `preserveModules`, the init chunk uses the file name template of the host, so its file is next to the host. With `preserveModules`, the init chunk has no module, so its file is in the folder of the generated chunks: `_virtual/lib/x.init.js` for `lib/x.js`.
  - The init chunk is last in `sorted_chunk_idx_vec`. Thus the chunk of a source file `x.init.js` keeps its name, and the init chunk gets a number: `x2.init.js`. The init chunk also does not take the name of an emitted asset, an emitted prebuilt chunk or a chunk with a `fileName`, in any letter case (`lazy_host_init_reserved_names`, `avoid_reserved_names`). A name with a hash does not change.
- `cjs` output: the chunk calls the init of each module right after the `require()` that the init follows: `require_x.init_x();` (`LazyHostInits::inits_after` and `init_export_name`). No init chunk exists.
- The importer then does not call `init_x()` in its body (`LazyHostInits::moved_calls` in `crates/rolldown/src/chunk_graph.rs`).

The init now runs before the body of the chunk. Thus the plan takes an importer only when each step that native ESM runs before the module also runs before the init:

- Only the first module of the chunk body that runs code takes part. A wrapped importer calls `init_x()` in its own init function, and a deferred-only importer has no call.
- Each earlier import of the importer must run while the chunk loads (`import_load_step`). The init follows the import of the host. When the import of an earlier module is a chunk after the host in `static_imports`, the init follows the last such import (`lazy_host_to_init`). For example, `a1.js` and `a2.js` are deferred modules of one common host, and `w.js` is in another chunk. In `import './a1.js'; import './w.js'; import './a2.js'`, the init of `a2` follows the import of `w`. When that last import is an external module, the plan stops. An external module, a module with no wrapper in another chunk, and a wrapped module whose entry chunk runs it at load (`runs_by_tail`) run at load. An import that runs code in the body of the chunk stops the plan for the later hosts of the importer:
  - a module of the same chunk;
  - in `cjs` output, a CommonJS module whose file exports a `default` getter: the first read of `default` in the body runs the module;
  - a module that the importer initializes through a module with no wrapper, for example an import of `foo` from a barrel with no side effects (`has_init_obligation_targets`);
  - a deferred import of a module with top-level await: the body prepares it with `__esmGather` (`deferred_import_has_preparation`);
  - an excluded statement whose init function calls the init functions that it forwards to (`transitive_init_targets`).
- A static import of the chunk after the init can run code at load (`chunk_runs_code_at_load`). Otherwise the call in the body runs in the same order.
- The host does not import the chunk of the importer, directly or through other chunks (`chunk_imports_chunk`). In such a cycle, the module can read bindings of the chunk before its body runs.
- With `strictExecutionOrder`, the plan runs only with `preserveModules`. Without `preserveModules`, a module of a common chunk runs only when an init function calls it, so the order wrappers already call `init_x()` in the order of the imports. With `preserveModules`, each file calls the init function of its own module at load, so an importer with an order wrapper also runs when its chunk loads. That importer keeps its call: in a cycle, another module can call its init function before its chunk imports the init chunk.
- With `strictExecutionOrder`, a common chunk that holds an entry module is not a host (`LazyHostInitPlanner::init_host`). With `preserveModules`, the order wrappers move such an entry into a common chunk, for example a dynamic entry that another file also imports statically (`collapse_carries_trigger`). That chunk does not run the module at load, also without `import defer` (see the known gaps). An init after its import would fix the order only with the option on, and it adds an init chunk also where the order wrappers already give the native order.
- The plan does not run in dev mode, with `experimentalInlineCommonChunks`, or for `iife` and `umd` output. With `experimentalInlineCommonChunks`, a file can run the modules of the records that it carries. `iife` and `umd` output have no lazy host.

Known gaps:

- A CommonJS module of a deferred closure has no lazy host. With `preserveModules`, its `esm` file ends with `export default require_x();`, so the module runs when a deferred importer loads that file. Its `cjs` file is correct: it exports a `default` getter, so the module runs at the first read.
- When the plan does not take an importer, the importer still calls `init_x()` in its body, after its later static imports. Without `preserveModules`, a chunk often has more than one module, so this occurs often. The split before a deferred import (below) removes only one of the causes. For example, in `import './x.js'; import './a.js'; import './y.js'`, the output still runs `y` before `a` when `x.js` is in the chunk of the importer and imports a CommonJS module (`x.js` then uses a runtime helper), or when `x.js` or `y.js` has top-level await. A fallback must keep the order and the async steps of the other imports.
- The rules do not check if the module of an earlier import already ran. Such an import still stops the plan, for example a barrel import of a module that another chunk already initialized.
- The split of a common host (below) moves only the modules after its first deferred module in execution order. When the first importer in execution order imports `w.js` before `a.js`, `w.js` stays in the host and runs at its load, also for a later importer that imports `a.js` first. The build without `import defer` has the same order, because a common chunk runs its modules in one order for all importers.
- With `strictExecutionOrder` and `preserveModules`, the order wrappers can turn a dynamic entry into a common chunk (`collapse_carries_trigger` in `crates/rolldown/src/stages/generate_stage/order_wrapping.rs`). That chunk does not run the module at load, so its eager importers in other files run it after their later static imports. This also occurs without `import defer`, so the plan does not take such a host.
- An asset that a plugin emits after the chunks get their names, for example in `renderChunk`, can get the name of the init chunk, as for any other chunk.
- The importer keeps its import of `init_x` from the host, though it does not call `init_x`. The default `minify: 'dce-only'` and the minifier remove the import. With `minify: false`, the import stays.

Tests:

- `preserve_modules` (`esm` output: a chunk load does not run a deferred module) and `preserve_modules_cjs_exports` (`cjs` output: the getters of a lazy host, and of a module in its closure that an eager importer also imports).
- `lazy_host_init_order`: the init right after the import of the host, with a `cjs` variant.
- `lazy_host_init_shared_host`: without `preserveModules`, two chunks share one init chunk.
- `lazy_host_init_name_collision`: the chunk of the source file `a.init.js` keeps its name, and the init chunk is `a2.init.js`.
- `lazy_host_init_earlier_module`: without `preserveModules`, an earlier import of a module of the same chunk moves that module to a new common chunk, and the plan takes the importer. With `preserveModules`, the plan takes the importer with no split. With a `cjs` variant.
- `lazy_host_init_common_chunk`: a deferred module in a common chunk, with a `cjs` variant.
- `lazy_host_init_mixed_common_chunk`: two deferred modules and a module that runs at load in one common chunk. The host splits, and the init of the second deferred module follows the import of the moved module. The test also loads the chunk of a deferred-only importer first, in a new process. With a `cjs` variant.
- `lazy_host_init_earlier_commonjs`: an earlier import of a CommonJS module. In `esm` output, its file runs it at load, so the plan takes the importer. In `cjs` output, a `default` getter runs it in the body, so the plan stops.
- `lazy_host_init_earlier_deferred_tla` and `lazy_host_init_earlier_barrel`: an earlier deferred import of a module with top-level await, and an earlier import through a barrel with no side effects, stop the plan.
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

- No module that stays imports a moved module. Else the host loads the new chunk.
- No dynamic import goes from one part to the other.
- The moved modules have no top-level await, and the chunk does not keep their exports for a facade (`common_chunk_preserve_export_names_modules`, `common_chunk_exported_facade_chunk_namespace`).

A second deferred module of the host can come after `w.js` in the imports of `b.js`: `import './a.js'; import './w.js'; import './a2.js'`. Its init then follows the import of the new chunk (see "Init after the import of a host").

#### Limits of both splits

`move_modules_to_new_common_chunk` does both splits. The old chunk keeps the runtime helpers of its other modules, and it also keeps `ExportAll`, which the chunk optimizer adds for the namespace of an eliminated facade.

The splits do not run with `strictExecutionOrder`, `preserveModules`, dev mode, `experimentalInlineCommonChunks`, `codeSplitting: false` or `experimental.chunkModulesOrder: 'module-id'`, or for `iife` and `umd` output. With `preserveModules`, each module already has its own file. Neither split changes a manual group chunk. The split of an importer chunk also skips a chunk that holds the runtime module, and an entry facade, whose entry module is in another chunk. Each new chunk adds a file to the output, only in a build with `import defer`.

## Shared CommonJS namespaces

`import defer * as ns from './x.cjs'` gives the importer two symbols for the import record (`create_deferred_cjs_namespaces` in `crates/rolldown/src/stages/link_stage/wrapping.rs`):

- `import_x`, the namespace. A static access assigns it at the first access: `(import_x || (import_x = __toESM(require_x()))).foo`.
- `x_deferred`, the deferred namespace object: `__deferNs(() => import_x || (import_x = __toESM(require_x())))`. Only an escape (`f(ns)`, `export { ns }`) reads it.

The import statement declares `import_x` and references `require_x` (`crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs`). Two rules differ from an eager import:

- The statement keeps the side-effect flag of the target, though it runs nothing. Tree shaking includes a deferred target with side effects as a dependency of its importer, because `meta.dependencies` has the deferred records. When nothing reads the namespace, this statement is the only reference to `require_x`. Without the flag, the module stays but its wrapper goes: the finalizer writes the body at the top level, so the module runs at startup, and two such modules can declare one top-level name twice (a `SyntaxError` in `esm` output).
- The statement does not reference `__toESM`. Each static access to a bundled CommonJS module references it, and so does the statement of `x_deferred` (`create_deferred_cjs_namespaces` in `crates/rolldown/src/stages/link_stage/wrapping.rs`). Thus an unread deferred import adds no `__toESM`. These statements, and the finalizer when it writes `__toESM(require_x())` (`cjs_deferred_namespace_expr`), do not check the record with `import_record_needs_interop`. That check searches all named imports of the importer, and here its answer is always true: `wrap_import_defer_closures` adds only `import defer * as ns` records to `deferred_cjs_namespace_imports`, and a namespace import always needs interop. The `needs_interop` of `safely_merge_cjs_ns_map` does not apply either: `determine_safely_merge_cjs_ns` skips the deferred records, so that value describes only the eager imports. It is `false` when no eager import needs interop, for example `import './x.cjs'` or `import { foo } from './x.cjs'`. When the deferred import used that value, no statement referenced the `__toESM` of a read: an error at the first access in `esm` output, and a panic in `cjs` output.

An unread deferred import still keeps `require_x` and `var import_x;`, and nothing calls them. The wrapper is `/* @__PURE__ */`, so a minifier can drop both in one chunk. In a split build, the deferred entry keeps the import of `require_x`. To drop the module, tree shaking must skip the edges of deferred-only imports. An unread `import defer` of an ES module with side effects keeps `init_y` in the same way.

`merge_deferred_cjs_namespaces` (`crates/rolldown/src/stages/generate_stage/code_splitting.rs`) runs after `merge_cjs_namespace`:

1. It groups the records by chunk, CommonJS module and interop mode. Only importers with no wrapper (`wrap_kind()` is `None`) take part. Nothing merges with `strictExecutionOrder`, because its order wraps are not in `wrap_kind()`. An external module in CommonJS output is not in a group: `__deferNsCached` gives it one object for each interop mode, in all chunks (see `../runtime-helpers/implementation.md`).
2. It sorts each group by the execution order of the importer, then by the record.
3. It links the included `import_x` symbols of a group to one root, and the included `x_deferred` symbols to another root. A record with only static accesses keeps `import_x` and drops `x_deferred`, so the two links are separate. The root is the first symbol whose canonical symbol is a key of the chunk's `exports_to_other_chunks`: another chunk imports it, or the entry chunk exports it. With no such symbol, the root is the first symbol.
4. It puts each symbol except the first into `ChunkGraph::merged_deferred_cjs_namespace_refs`. `deferred_cjs_import_decl` (`crates/rolldown/src/module_finalizers/mod.rs`) does not declare these symbols. The first symbol keeps the declaration, under the name of the root. A record that declares neither symbol loses its statement.

Invariants:

- The declarations stay in the first importer by execution order. That importer runs before the other importers, so a read of `x_deferred` gets its value at the same time as before the merge, or earlier.
- The canonical symbol of each key of `exports_to_other_chunks` stays a key. `commit_cross_chunk_links` builds the table before the merge, and a read in `cjs` output looks it up by the canonical symbol after the merge (`finalized_expr_for_cross_chunk_symbol` in `crates/rolldown/src/module_finalizers/mod.rs`, and `crates/rolldown/src/types/generator.rs`). A key that links to a symbol that is not a key makes that lookup panic. Two keys can share the root: the chunk then exports one object under two names.
- Each access assigns `import_x` with the interop mode of its own importer. Thus all records of a group have one mode.

Tests: `commonjs_shared_namespace` (two files, two imports in one file, a `.mjs` importer, a wrapped importer, two objects that another chunk imports, a `strictExecutionOrder` variant with no merge, and a `cjs` variant), `commonjs_eager_import_no_interop` (an eager import with no interop and a deferred import of each module, with no other user of `__toESM`, and a `cjs` variant), `commonjs_unread_namespace` (two entries: an unread deferred import and an eager import with no interop; the deferred entry runs nothing, and no chunk has `__toESM`; `minify` is off, so that the helper keeps its name; a `cjs` variant), `commonjs_unread_namespace_keeps_wrapper` (one entry with unread deferred imports of two CommonJS modules that both declare `snapshot`: the entry parses, runs neither module and has no `__toESM`), `commonjs_escape` and `commonjs_target`.

## Related

- [design.md](./design.md) — why the groups exist, the measurements, why the shared CommonJS namespaces have their rules, and why a chunk initializes a lazy host after its import
- `../runtime-helpers/implementation.md` — `__esmMin`, `__esmState` and the other helpers
