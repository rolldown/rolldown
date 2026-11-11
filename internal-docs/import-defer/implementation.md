# Import defer — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

With `experimental.importDefer`, each ESM module of an `import defer` closure gets a lazy init wrapper (`function init_x() { return (init_x = __esmMin(() => { ... }))(); }`). The first access to a deferred namespace calls the wrapper of the target, and that wrapper calls the wrappers of the modules that the target imports.

This doc describes only one part of the machinery: the deferred groups. The other parts (the closures, the deferred namespaces, the state records of `__esmState`, the top-level await rules) are not described here yet.

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

- `print_deferred_group_bodies` (`render_chunk_to_assets.rs`) prints the body of each root and member as its own `Program` (`NormalModule::render_group_body`), in parallel, before the chunk loop prints the declarations. The body print has no text mutations. The only text mutation (`PrependRenderedImport`) adds code at the start of a module, which is in the declarations.
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

## Related

- [design.md](./design.md) — why the groups exist, and the measurements
- `../code-splitting/implementation.md` — two proofs that reject concatenated wrappers. They run before the groups exist.
- `../runtime-helpers/implementation.md` — `__esmMin`, `__esmState` and the other helpers
