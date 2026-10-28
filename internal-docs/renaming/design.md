# Renaming — Design & Principles

## Summary

A chunk contains the top-level code of many modules in one file. Thus two modules can each declare a top-level `foo`, and the two declarations collide. Also, a nested binding that keeps its source name can capture a reference that rolldown prints in its scope.

The renamer gives a final name to each binding. It has two goals:

1. Each printed reference resolves to the same binding as in the source.
2. Each generated reference resolves to the binding that rolldown intends.

The renamer runs once for each chunk, in `deconflict_chunk_symbols`, before the finalizer prints the chunk. Its output is `chunk.canonical_names`. If a binding has no entry, the finalizer prints the binding with its source name.

[implementation.md](./implementation.md) describes the code.

## Terms

- **Binding**: a name that a declaration adds to a scope.
- **Capture**: a binding captures a reference when the reference resolves to that binding, and not to the binding that it must resolve to.
- **Top level**: the root scope of the output file of a chunk.
- **Inner scope**: a scope below the top level in the output. See "The two kinds of scope".
- **Facade**: a symbol that rolldown makes for a module, for example a wrapper binding or a namespace object. The source does not declare it.
- **Synthesized binding**: a binding that rolldown makes. The source does not contain it.
- **Generated reference**: a reference that rolldown prints. The source does not contain it.
- **Fixed name**: a name that rolldown prints but cannot change, for example `Promise` or `require`. See principle 6.
- **Wrapper**: a function that rolldown puts around the code of a module.
- **CJS closure**: the wrapper of a CJS-wrapped module, `__commonJS((exports, module) => { ... })`.

## The two kinds of scope

The renamer divides the output into two levels:

- **The top level.** All modules of the chunk share this one flat namespace. It contains the top-level bindings of ESM modules, the import bindings from other chunks and from externals, and each synthesized binding. The `ConflictResolver` owns this namespace and adds the `$N` suffixes.
- **Inner scopes.** These are the nested scopes of each module, and the root scope of each CJS-wrapped module. The CJS closure contains that root scope as its body. An inner binding keeps its source name. The renamer renames it only when it can capture a reference.

#7425 made the root scope of a CJS-wrapped module an inner scope. Thus the locals of a CJS-wrapped module stopped taking top-level names, and they stopped adding `$N` suffixes to unrelated top-level bindings. But other parts of the renamer continued to treat these locals as top-level bindings in some places and as nested bindings in other places. Most CJS renaming bugs came from this difference (#9055, #9375, #9630, #9882, #10970).

Thus one place decides the level of a binding: the top-level naming loop. It decides from the location where the finalizer prints the binding. In a CJS-wrapped module, only two kinds of binding are at the top level:

- a facade;
- a binding of an external `import` declaration, because rolldown moves that declaration out of the CJS closure.

A binding that a `require()` call initializes stays a local of the CJS closure, also when the required module is external. Each inner-binding pass skips the bindings that the top-level naming loop gives a name to.

## Design principles

1. **The renamer renames an inner binding only when necessary.** A renamed binding makes the output more difficult to read and debug. Thus an inner binding keeps its name unless a reference in its scope must resolve to a binding outside it. Top-level bindings always get unique names, because they share one namespace.

2. **A synthesized top-level name is never equal to an inner binding name (invariant S).** There are two kinds of top-level binding (`RootBindingKind`):
   - _Authored_: source code declares the binding, or the binding gets its name from the source. Examples: the top-level bindings of a module, re-exports, the default export binding, shims for missing exports, external import bindings and external namespaces.
   - _Synthesized_: rolldown makes the binding. Examples: `require_x` and `init_x` wrapper bindings, `x_exports` namespace objects, `import_x` interop bindings, runtime helpers (for example `__toESM`), cross-chunk `require_<chunk>` bindings, inline-common-chunk bridges, HMR references, order-wrap synthetic bindings, and the `this$N` parameter for the top-level `this` of a CJS-wrapped module.

   The finalizer prints references to synthesized bindings at any location in the body of a module. For example, `require('./x')` becomes `require_x()` at the call, and an import from a CJS module becomes `import_x.foo` at each read. These references have no source reference, so the renamer cannot check them one kind at a time. Instead, a synthesized binding never takes a name that an inner binding of the chunk uses. Thus no source binding can capture a reference to it.

   This principle has two known costs. Synthesized names take a `$N` suffix more frequently: `require_dup$3`, not a rename of the `require_dup` of the user. Also, the renamer collects the inner names of the chunk into a set when it gives a name to the first synthesized binding.

3. **The renamer checks authored references at their locations.** An authored top-level binding keeps its source name when that name is free at the top level. Its references are real references in the scope tree of a module. The inner-binding passes go from each such reference up to the top level. They rename each inner binding with the same name on that path. For a CJS-wrapped module, the path includes the root scope of the CJS closure.

4. **An inner rename never causes a second rename.** A renamed inner binding takes the first `name$N` that no top-level binding uses and that no binding of the same module uses, in any scope. Such a name cannot capture a reference, and it cannot collide with a sibling binding. Thus the renamer never checks again the bindings that it renamed. #10970 had three variants of one bug: a rename took the name of a sibling binding that the renamer did not check.

5. **The renamer checks the name that the finalizer prints.** The finalizer prints an import that the code reads through a namespace binding as `node_path.join` or `import_x.default`. Thus a local with the name `node_path` captures the import, although the name of the import is `join`.

6. **A fixed name always resolves to its global or host binding.** Some names that the finalizer prints are not bindings that the renamer gives names to. They are global or host bindings that the output uses:
   - `Promise` and `Object`, for a lowered `import()`;
   - `URL`, for `import.meta.ROLLDOWN_FILE_URL_*`;
   - `Symbol` and `Object`, in the export code;
   - `require`, `__filename` and `__dirname`, under CJS output.

   The renamer cannot change a fixed name. Invariant S changes the synthesized name, but for a fixed name the renamer changes the bindings:
   - A chunk that can print a fixed name reserves it at its top level.
   - A module whose body can print a fixed name renames its inner bindings with that name.

   One list tells which rewrite prints each fixed name, and which module contents cause the rewrite. Both levels use this list. The renamer renames a binding only in a module that can print the fixed name in the scope of the binding. Thus `var Promise = require('bluebird')` keeps its name in all other modules.

   For `Promise` and `Object`, the list counts each module that has an `import()`, also when the finalizer keeps that `import()` native. The finalizer decides from the chunk graph and the options if it lowers an `import()`, and a copy of these conditions in the renamer could become different from the finalizer. Thus a `Promise` or `Object` binding in such a module can get a `$N` suffix that it does not need. This cost is deliberate, like the costs of principle 2.

   Rolldown prints no other name that it cannot change. The finalizer prints the top-level `this` of a CJS-wrapped module as a parameter of the CJS closure (`this$1`). The renamer gives this parameter its name, and the runtime passes the initial exports object of the module to it. The parameter is a synthesized binding, so invariant S keeps all bindings of the chunk off its name.

7. **A wrapper binds only the parameters that rolldown prints.** A wrapper adds a scope that is not in the source. Thus each binding of the wrapper is between the references of the module and their targets. The CJS closure is an arrow function, and an arrow function binds no implicit names. A `function` also binds `this`, `arguments` and `new.target`, so it changes the bindings that these references of the module resolve to.

   One function, `cjs_wrapper_fixed_params`, gives the fixed parameters of the CJS closure: `exports` and `module`. The printer and the top-level reservation both use this function, so no top-level binding has the name of a fixed parameter. After the fixed parameters, the finalizer (`cjs_wrapper_params`) adds the `this` binding, which is synthesized, so invariant S keeps all bindings of the chunk off its name. Thus a parameter cannot capture a reference to a top-level binding.

## Rejected alternatives

- **One shadowing pass for each kind of generated reference.** The previous design used this method. `collect_chunk_scope_captured_names` listed the names that CJS closures captured: wrapper bindings, IIFE factory parameters, order-wrap symbols and cross-chunk wrapper bindings. `rename_cjs_locals_shadowing_referenced_chunk_bindings` renamed the locals of CJS closures, one channel at a time: named imports, star imports and `require()`. Each new kind of generated reference needed one more case. The passes used five different methods to decide if a name is free. When a case was missing, the output ran but read the wrong binding, frequently with no error.
- **A rename of the source local, not of the generated name.** The previous design did this for CJS closures. It changes names that the user wrote. Also, the new local name must avoid all sibling bindings, and it did not always do that (#10970).
- **CJS closure locals in the top-level namespace again** (the state before #7425). This method is correct, but each closure local then competes for top-level names, and the output gets many `$N` suffixes.
- **Only the inner names of the modules that read a synthesized binding.** This method is more precise. But it must know each location where the finalizer prints a reference, and principle 2 removes exactly this per-channel knowledge. One set for the full chunk is simple and safe.
- **A name that the module can write to, for the top-level `this` of a CJS module.** `exports` is no longer the initial exports object after the module binds it again, with `var exports` or with `exports = ...`. (`var exports` declares the binding of the closure parameter itself.) `module.exports` is no longer the initial exports object after the module replaces it. A rename cannot help, because a top-level `var exports` is the binding that the name refers to.
- **A `function` CJS closure that the runtime calls with the exports object as `this`.** This method keeps the native `this`, but the function also binds `arguments`. A module that read `arguments[1]` got `module`, not the value from the code around the closure (`require`, under CJS output). Principle 7 does not allow this method.
- **Names from the scope tree of the output.** This model is the most precise. But the output AST does not exist before the finalizer runs, and the finalizer needs the names first.

## Unresolved questions

- Direct `eval` reads bindings by their source names. The renamer does not protect these reads.
- Under CJS output, an unwrapped CJS entry is at the top level of the output file. There, the top-level `var exports` of the entry is the `exports` parameter of Node itself. But the top-level reservation renames it to `exports$1`. Thus a read before the declaration (`exports.a = 1; var exports = {}`) throws an error. To fix this, the reservation must keep the name of a binding that is a host parameter itself. This is the same reason why the finalizer does not print the `this` of a CJS module as `exports`.
- The top-level `arguments` and `new.target` of a CJS module resolve past its arrow closure to the code around the chunk. They do not resolve to the module wrapper of Node. Under ESM output, `arguments` is not defined there. Principle 7 keeps this behavior. To give the arguments of the Node wrapper, rolldown must pass `require`, `__filename` and `__dirname` into each closure.

## Related

- [implementation.md](./implementation.md): the code for this design
- [../inline-common-chunks/implementation.md](../inline-common-chunks/implementation.md): how carriers give names to the modules and bridges of their records
