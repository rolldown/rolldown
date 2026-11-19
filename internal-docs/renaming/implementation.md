# Renaming — Implementation

> [design.md](./design.md) gives the reasons and the principles for this code.

## Summary

`deconflict_chunk_symbols` (`crates/rolldown/src/utils/chunk/deconflict_chunk_symbols.rs`) uses a `Renamer` (`crates/rolldown/src/utils/renamer.rs`) to give names to the bindings of one chunk. It runs once for each chunk in the generate stage, after the chunks and the links between chunks are final. It writes `chunk.canonical_names`. The finalizer reads these names. If a symbol has no entry, the finalizer prints the symbol with its source name.

## Components

Paths are relative to `crates/rolldown/src/`.

| Piece                | Where                                     | Role                                                                                                                                                       |
| -------------------- | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ConflictResolver`   | `utils/chunk/conflict_resolver.rs`        | The flat top-level namespace of the chunk. It resolves a name to itself or to the first free `name$N`. It asks the caller if each candidate is acceptable. |
| `Renamer`            | `utils/renamer.rs`                        | Owns the resolver and `canonical_names`. Gives names to top-level bindings and renames inner bindings.                                                     |
| `RootBindingKind`    | `utils/renamer.rs`                        | `Authored` or `Synthesized`. Decides which inner names a top-level binding must avoid.                                                                     |
| `InnerBindingNames`  | `utils/renamer.rs`                        | The names that the inner scopes of the chunk bind. The renamer builds it for the first synthesized binding.                                                |
| `root_binding_kind`  | `utils/chunk/deconflict_chunk_symbols.rs` | Returns the kind of a top-level symbol.                                                                                                                    |
| `NestedScopeRenamer` | `utils/renamer.rs`                        | The passes for one module that rename the inner bindings that capture a reference.                                                                         |

## Naming order

`deconflict_chunk_symbols` gives names in this order. An earlier binding gets its original name first, so the order sets the priority.

1. Reserve the fixed names:
   - `Renamer::new` reserves the fixed names that the format prints: `require`, `module`, `exports`, `__filename`, `__dirname` and `Symbol` for CJS, and `exports` and `Symbol` for IIFE/UMD. It also reserves `Object`, `Promise`, the JS keywords and the global objects.
   - `reserve_fixed_names` reserves each unresolved reference of the modules of the chunk (for example `console` and `window`). It also reserves the fixed names that the bodies of these modules print, and the fixed parameters of their CJS closures (`cjs_wrapper_fixed_params`).
2. IIFE/UMD/CJS: external module namespaces (factory parameters, `require()` bindings). Authored.
3. Entry chunks: the symbols that the entry exports (`referenced_symbols_by_entry_point_chunk`). Authored.
4. The included top-level declarations of each module, entry module first (descending execution order). HMR references and the `this` binding of a CJS-wrapped module (`cjs_this_ref`) are synthesized. All other bindings go through `root_binding_kind`. In a CJS-wrapped module, this step gives names only to two kinds of binding:
   - facades;
   - bindings of an external `import` declaration, because rolldown moves that declaration out of the CJS closure.

   Each other root binding is a local of the CJS closure, and this step skips it. This includes a binding that an external `require()` initializes.

5. Order-wrap synthetic declarations. Synthesized.
6. ESM: the external import bindings that the chunk uses. Authored. They come after the declarations of the modules, so a top-level declaration keeps its source name and the import binding gets the `$N`, as in Rollup (#11035).
7. Import bindings from other chunks (`imports_from_other_chunks`), through `root_binding_kind`.
8. Names with no symbol, through `create_conflictless_name` (synthesized): cross-chunk `require_<chunk>` bindings, external namespaces in node mode, and inline-common-chunk bridges.
9. `rename_shadowing_symbols_in_nested_scopes`: the inner-binding passes, entry module first.

## Top-level names

`Renamer::add_symbol_in_root_scope(symbol_ref, kind)` asks the resolver for the original name of the symbol:

- `Authored`: the renamer accepts the original name if the resolver has it free. A `$N` candidate must also not be bound in any scope of the module that owns the symbol (`is_name_available_with`). In a nested scope, the candidate would capture the references of the renamed binding itself. In the root scope of a CJS-wrapped module, the resolver does not see the binding, so the candidate would declare the same name two times.
- `Synthesized`: no candidate can be in `InnerBindingNames`, original or not.

`root_binding_kind` returns `Synthesized` for runtime-module symbols and for the facades that a normal module owns. A facade that represents a binding with a source name is an exception: a re-export (in `named_imports`), `default_export_ref`, and `shimmed_missing_exports`. The symbols of external modules are `Authored`, because their names come from the importing source.

`InnerBindingNames` reads the symbol table of each module once. It keeps each symbol that is not declared in the root scope. It also keeps the root-scope symbols of a CJS-wrapped module that the module really binds there. Facades are in the root scope, but no declaration binds them, so `InnerBindingNames` does not keep them.

## Inner bindings

`NestedScopeRenamer` runs three passes for each module:

- `rename_bindings_shadowing_star_imports`: for each resolved `ns.foo` member access, it checks the name that the finalizer prints for `foo`.
- `rename_bindings_shadowing_named_imports`: for each reference to a named import, it checks the name that the finalizer prints for the import.
- `rename_bindings_shadowing_fixed_names`: it renames the inner bindings that have a fixed name that the body of the module can print. `fixed_names_in_module_body` is the list. Each entry tells the rewrite and the module contents that cause it.

The factory parameter of an external module (IIFE/UMD) is a top-level binding that the renamer gives a name to. The finalizer prints each reference to it for an import binding. Thus the named-import pass covers it through `printed_binding`.

The first two passes use `Renamer::printed_binding`. This function follows a namespace alias to its namespace binding, as `finalized_expr_for_symbol_ref` does. The passes also use `rename_bindings_on_path`, which goes through the scope ancestors of the reference. It stops at the root scope, unless `root_scope_is_inner` is true (the module is CJS-wrapped). It renames each binding with that name, except the binding that the reference resolves to.

`Renamer::rename_inner_binding` takes the first `name$N` that is not in the resolver and that no scope of the module binds. Then it reserves that name. It skips a binding that already has a name. It also skips a binding that is linked to another symbol (an import binding), because the finalizer prints that binding with the name of the other symbol.

## How to add a binding

- A new kind of synthesized top-level binding that has a symbol: pass `RootBindingKind::Synthesized`, or make sure that `root_binding_kind` returns `Synthesized` for it. For a binding without a symbol, use `create_conflictless_name`. In both cases, the binding needs no shadowing pass.
- A new facade that represents a binding with a source name: exclude it in `root_binding_kind`. If you do not exclude it, it avoids inner names without a reason and takes unnecessary `$N` suffixes.
- A new rewrite that prints a global or host name (for example `Promise` or `require`): add the name to `fixed_names_in_module_body`, with the module contents that cause the rewrite. If only the top level of the chunk prints the name, add it to the list in `Renamer::new`. The name needs no pass of its own.

## Catch/var names

In `catch (e) { var e = 1 }`, oxc keeps two symbols (oxc-project/oxc#28739): the catch parameter `C`, and the binding `O` that the `var` declares in the enclosing var scope. The `var` identifier carries `C`, because its initializer writes `C`. `Scoping::catch_var_bindings` gives the `(C, O)` links. `O` has no identifier of its own, so `C` and `O` must print the same name. A sloppy-mode block function named `e` (Annex B.3.3) writes `O` when its block runs, so it must print the name of `O` too.

The rule is the one of oxc's mangler: a linked catch keeps its name, and everything else steps around it. `Renamer::add_catch_var_bindings` runs before any name is given:

- `O` is an inner binding (in a function, or in the root scope of a CJS-wrapped module): the renamer reserves the name of `C`. No top-level binding or import can print that name, so no nested-scope pass renames `C` or `O`, and the Annex B functions keep their names too. The cost: a top-level binding of another module with the same name takes a `$N`.
- `O` is a top-level binding: `O` gets its name like any other top-level binding. `C`, and each Annex B block function that writes `O`, print that name (`top_level_catch_vars`; `get_canonical_name` and `into_canonical_names` read it). `annex_b_function_var` finds these functions: a plain block function in sloppy code whose nearest var scope binds `O`'s name, with no other binding of that name in between (such a binding stops the write; a catch parameter does not). Only sloppy modules look for them, and they read the symbol table once.

Two cases still rename a linked catch:

- **A self-import read inside the catch.** `import { e as outer } from './self.js'; export var e; try {} catch (e) { var e = 2; log(outer) }`: `outer` prints `O`'s name, and `C` would capture it. `rename_bindings_on_path` looks up `C` by `O`'s source name when the printed symbol is a top-level `O` of the module, and `Renamer::split_catch_from_var` gives `C` a name of its own. A binding found by the searched name that prints a top-level `O`'s name is skipped: it prints another name, because top-level names are unique. After the split, the `var` inside the catch declares `C`'s name. If `O`'s declaring statement is included, `ScopeHoistingFinalizer::preserve_renamed_catch_vars` declares `O` again (`var e;`, or the wrapped-ESM hoist set), which also covers an `O` declared only by the catch. Annex B functions never split: they need sloppy code, which has no imports.
- **A fixed name.** If the linked name is a fixed name that the module body prints (`fixed_names_in_module_body`, for example `require` in CJS output), reserving cannot help. `rename_bindings_shadowing_fixed_names` renames `O`, then gives `C` and the Annex B functions that write `O` the same name (`Renamer::rename_inner_catch_var`). A top-level `O` never has a fixed name, because the resolver reserved the fixed names.

The scanner declares a top-level `O` when it visits the `var` identifier (`AstScanner::visit_binding_identifier`), so the statement counts as `O`'s declaration. When the module is wrapped ESM, the finalizer hoists `O` and turns the `var` into an assignment to `C`.

`rolldown#11736` first handled this with name groups: every same-named binding of the var scope printed `O`'s name, and a group was split when it would capture an import. The rule above replaces the groups, except for the self-import split, which keeps one catch parameter apart from its `var`.

## Tests

The fixtures are in `crates/rolldown/tests/rolldown/topics/deconflict/`. Each `_test.mjs` runs the output and checks the values. When a binding captures a reference, the output usually runs and reads the wrong binding. It does not fail to parse.

Catch/var fixtures (each runs the output and checks values against Node's behavior on the source):

- `catch_var_binding_groups`: imports read inside function-level catches, nested catches, destructuring and loop `var`s.
- `catch_var_corpus_esm`: the same, plus two modules with a top-level linked `e`, self-imports, cycles, and the entry's own `e`; ESM, CJS, minified and wrapped output.
- `catch_var_corpus_cjs`: sloppy CJS-wrapped modules with Annex B functions at the module level and in functions, nested catches, `require` and `__dirname` as linked names, and a wrapped ESM module.
- `catch_var_fixed_name_promise`: a linked `Promise` around a lowered `import()`.
- `catch_var_self_import`: self-imports and re-export cycles read inside a catch, including an `O` declared only by the catch.
- `catch_var_lexical_names`: block functions and classes named like a top-level linked catch keep their `.name`.
- `misc/catch_var_annex_b`, `misc/wrapped_esm_catch_param_redeclared`.

## Related

- [design.md](./design.md): the principles and the trade-offs of this code
- [../inline-common-chunks/implementation.md](../inline-common-chunks/implementation.md): carriers give names to the modules of their records in one renamer
