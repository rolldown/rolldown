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
property constants or reassigned bindings. Modules containing direct `eval` are left untouched.

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

## Files and tests

- `crates/rolldown/src/stages/link_stage/prune_constant_branches.rs`
- `crates/rolldown/src/module_finalizers/impl_visit_mut.rs`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_function_shared_chunk`
- `crates/rolldown/tests/rolldown/optimization/inline_const/dead_function_branch_safety`
