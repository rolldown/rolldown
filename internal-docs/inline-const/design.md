# Inline Const — Design & Principles

## Summary

`optimization.inlineConst` replaces reads of an exported binding with the binding's value when the exporting module gives it a primitive value once and never writes it again. Reads in importers and in the exporting module both change, so a condition such as `if (DEBUG)` folds and its dead branch can be removed, and an import that only carried the value can disappear. Constants that are never exported are left to the minifier ([#5719](https://github.com/rolldown/rolldown/issues/5719)).

The rewrite assumes that every read runs after the declaration. This is a known trade-off, described in [Reads before the declaration runs](#reads-before-the-declaration-runs).

## Reads before the declaration runs

The scanner checks one kind of early read: a module-level read of a `var` before its declaration, which source order shows. Such a `var` is not inlined. These reads are not checked and get the inlined value:

- a module-level read of a `let` or `const` before its declaration;
- a read inside a function that is called before the declaration runs;
- a read from another module in an import cycle, directly or through a function, before the declaring module's body runs.

In ESM, an early read of a `let`, a `const` or an `export default` binding throws a `ReferenceError`, so inlining only changes programs that already fail. An early read of a `var` is legal and gives `undefined`, so a working program can see a different value.

Checking reads inside functions and across cycles requires following execution order through calls and modules, and the scanner sees one module at a time. Without that, the simple rule that removes the `var` difference is to never inline a `var` read inside a function or from another module, which gives up folding `export var` flags, as in ES5-transpiled ESM packages. Rollup makes the same choice by default: it treats a `var` read in an import cycle as initialized, and its [`treeshake.correctVarValueBeforeDeclaration`](https://rollupjs.org/configuration-options/#treeshake-correctvarvaluebeforedeclaration) option, off by default, makes it stop assuming the value of any `var`. Rolldown inlines these reads, and the troubleshooting guide documents the difference for users.

Dropping a call to a side-effect-free exported function relies on the same assumption: a call that runs before the function's declaration can be dropped too, also with `optimization.inlineConst: false`.

With `optimization.inlineConst: false`, a `var` read before its declaration gives `undefined` as in ESM. When `minify` is enabled, the minifier can still inline a `const` read in the chunk that declares it.

## Related

- `docs/guide/troubleshooting.md` — "Avoid relying on Temporal Dead Zone (TDZ) errors", the user-facing description of these cases
- [../linking/module-execution-order/implementation.md](../linking/module-execution-order/implementation.md) — execution order and the import-cycle rule for link-time constant lookups
