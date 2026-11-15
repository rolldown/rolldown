# Constant branches — implementation

## Summary

`LinkStage::prune_constant_branches` runs after cross-module constant discovery and before
statement inclusion. It removes references confined to unreachable branches of retained
statements, including function bodies. This prevents dead helpers from becoming cross-chunk
exports, which otherwise become roots for the per-chunk minifier.

## Shared decision, not independent optimizations

The linker records each decision in `LinkingMetadata::constant_branches`, keyed by the original
post-semantic `NodeId`. The scope-hoisting finalizer consumes those decisions **before** visiting
children, so removed references never reach symbol rewriting. This is required even with
`minify: false`; pruning dependencies without removing their AST references produces invalid
output. See [AST mutation](../ast-mutation/implementation.md) for the identity contract.

- For `if` and ternaries, the value chooses the consequent or alternate.
- For logical expressions, only a known-unreachable RHS is recorded; the left operand remains
  the expression's result.
- Condition evaluation is retained, including effects and exceptions. A literal `if` condition
  may be omitted after finalization substitutes constants.
- Replacements remain value expressions rather than references. In particular, a sequence
  preserves indirect `eval` and unbound member calls when simplifying expression callees.
- An `if` replacement keeps the live statement's block scope and its surrounding statement
  position, avoiding dangling-`else` and lexical-declaration hazards.

## Conservative boundaries

Only statements referencing a known constant are revisited. The pass is parallel per module,
and is disabled when tree shaking or constant inlining is disabled. It does not use CommonJS
property constants or reassigned bindings. A binding whose writes cannot run counts as not
reassigned (see [Constants whose writers cannot run](#constants-whose-writers-cannot-run)).
Modules containing direct `eval` are left untouched.

A condition that contains an array, object, or regexp literal is not decided. oxc converts such a
literal with the default `toString` and keeps the source order of regexp flags, but a program can
patch a prototype, and the runtime prints regexp flags in a fixed order. With `minify: false`,
nothing else removes such a branch, so a wrong decision here changes behavior. The pass uses the
same rule as `try_extract_const_literal` (`contains_object_literal`).

Statements containing import records are skipped: import lowering adds synthetic symbol and
runtime-helper dependencies, and these cannot be removed by merely revisiting identifier
references. Namespace helper references in dead branches are pruned by their semantic reference
IDs, while direct-symbol references are kept if any reachable use of that symbol remains.
Dependencies without a corresponding AST reference are preserved; the pass only removes
references explicitly observed in deleted subtrees, never reconstructs the entire dependency list.

An `if` whose dead branch contains `var` or function declarations is not simplified: hoisted
declarations must survive even when their initializers never execute. Supporting these cases
needs explicit hoist reconstruction. Dynamic imports and namespace-member branch conditions
are also outside this pass's current scope.

## Constants whose writers cannot run

An exported `let` with a literal value is a constant when no included code can run a write to it
(#9698). Svelte sets its runtime flags this way; a flag module that the compiler adds only when an
app needs the flag calls the writer:

```js
export let custom_render = false;
export function enable_custom_render() {
  custom_render = true;
}
```

The scanner keeps such a binding in `EcmaView::deferred_write_constants`
(`collect_deferred_write_constants`) when:

- each write is inside a function, and the visitor recorded each write that oxc reports;
- no write is in a statement that declares the binding;
- the binding is a `let`. In an import cycle, code can read a binding before its module runs. A
  `var` then gives `undefined`. A `let` throws, and the inlining of a never-written `let` already
  accepts that case.

`LinkStage::promote_deferred_write_constants` runs before `cross_module_optimization`, so all the
constant passes and the finalizer see one constant map. It adds the binding to the map and sets
`IsNotReassigned` when no statement with a write is reachable:

- The module is not an entry, and no `import()` or `import.meta.hot.accept()` loads it. Each of
  these makes every export usable.
- No importer uses `require()`, `new URL()`, `import * as` or `export * from` on the module.
- No importer imports or re-exports a writer by name. When each candidate of a module has such an
  import, the pass does not walk the module.
- The module has no `export *` of a module with dynamic exports
  (`LinkingMetadata::has_dynamic_exports`). For such an `export *`, `reference_needed_symbols` runs
  after this pass and adds a reference to the namespace object of the module. The namespace object
  keeps each export, also the writers.
- A walk inside the module, from the statements with side effects and from the symbols that
  importers use, does not reach a writer statement. The walk follows the same edges as tree
  shaking: from a symbol to its declarations, and with `propertyWriteSideEffects: false` to the
  statements that write a property of the symbol (`symbol_ref_to_referenced_stmt_idx`). Tree
  shaking finds the property writes from the symbol that the code references, so the walk uses
  that symbol and its canonical symbol.

The pass reads only the importers of the modules with candidates (`EcmaView::importers_idx`). It
reads each importer one time, also when the importer imports many of these modules. So its work
grows with the imports of these importers, not with the module graph.

These checks must cover each way that `include_statements` can include a statement, also through
the references that later passes add. After tree shaking, debug builds check that no writer
statement is included (`assert_promoted_writers_excluded`).

Limits:

- The walk does not follow references into other modules. A writer that another module imports
  stays reachable, also when nothing calls the importing function.
- `import * as ns` counts as a use of each export, also when the code reads only static members.
- A barrel that re-exports a writer by name counts as a use, also when nothing imports the name
  from the barrel.
- `export * from 'external'` also sets `has_dynamic_exports`, so the module keeps its candidates
  live, although this `export *` adds no reference to the namespace object.

## Files and tests

- `crates/rolldown/src/stages/link_stage/prune_constant_branches.rs`
- `crates/rolldown/src/stages/link_stage/promote_deferred_write_constants.rs`
- `crates/rolldown/src/module_finalizers/impl_visit_mut.rs`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_writer_shared_chunk`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_writer_property_write`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_writer_dynamic_reexport`
- `crates/rolldown/tests/rolldown/optimization/inline_const/issue_9698_multi_entry_dead_writer`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_function_shared_chunk`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_function_branch_safety`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_function_object_coercion` (only
  `minify: false`: the oxc minifier makes the same `toString` assumption)
