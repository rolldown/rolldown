# Inline Const — Design & Principles

## Summary

`optimization.inlineConst` replaces reads of an exported binding with the binding's value when the exporting module gives it a primitive value once and never writes it again. Reads in importers and in the exporting module both change, so a condition such as `if (DEBUG)` folds and its dead branch can be removed. Constants that are never exported are left to the minifier ([#5719](https://github.com/rolldown/rolldown/issues/5719)).

Inlining assumes that a read runs after the declaration. [Reads before the declaration runs](#reads-before-the-declaration-runs) describes where Rolldown checks this.

## Reads before the declaration runs

In ESM, reading a `let`, a `const` or an `export default` binding before its declaration runs throws a `ReferenceError`. Inlining such a read only changes programs that already fail, so these bindings are inlined without checking when they are read.

Reading a `var` before its declarator runs is legal and gives `undefined`. The scanner finds such reads in the declaring module (`ast_scanner/early_var_reads.rs`), and Rolldown does not inline that `var`:

- Module-level code reads it before its declarator ends, including code in blocks and `catch` clauses.
- The body of a root function (`function f() {}` or `const f = () => {}`) reads it, and module-level code references `f` before the declarator ends, directly or from another such function. Any reference counts as a call, since `f` can be passed on and called right away.
- Other nested code reads it, such as a callback, an IIFE, or an object or class member, in a module-level statement that starts before the declarator ends.

Such a `var` gets `SymbolRefFlags::ReadBeforeAssigned`. `SymbolRef::is_not_reassigned` then returns `false`, so the `var` is not inlined and a call to it is not dropped as side-effect-free. CommonJS export rendering ignores `ReadBeforeAssigned` and exports the `var` as a plain value, since a read before the declaration does not change the value the `var` ends up with. This keeps plain exports for downleveled ES5 classes that an earlier class's constructor reads. Export rendering keeps a getter when direct `eval` in the module can assign the binding, because `IsNotReassigned` does not see such an assignment.

The checks are conservative: an object or class member declared before the `var` keeps the `var` from being inlined even if the member only runs later.

At link time, exported `var` bindings in a static-import cycle also get `ReadBeforeAssigned` and are removed from the constant map. The same import-cycle components used by link-time constant lookups identify these modules. This keeps named imports, namespace reads, and re-exports from assuming the initializer ran, and the flag prevents later optimization passes from adding the binding back to the constant map. The check is conservative even when a cyclic importer only reads the binding after initialization.

With tree-shaking on, the dead-code removal that runs on each module when it is parsed still folds a condition such as `if (flag)` in the declaring module when nothing in that module reads the `var` early, even with `optimization.inlineConst: false`.

## Related

- `docs/guide/troubleshooting.md` — "Avoid relying on Temporal Dead Zone (TDZ) errors", the user-facing description of inlined `let` and `const` reads
- [../linking/module-execution-order/implementation.md](../linking/module-execution-order/implementation.md) — execution order and the import-cycle rule for link-time constant lookups
