# Advanced Tree Shaking — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

The analysis lives in `crates/rolldown/src/stages/link_stage/tree_shaking/advanced/`. The
inclusion engine in `include_statements.rs` owns it as `IncludeContext::advanced` and calls it at
five hooks. After the inclusion fixpoint, the link stage writes the decisions back into the
statement infos, and the module finalizer applies the AST edits.

| File                                         | Contents                                                                                                                                     |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `advanced/mod.rs`                            | `AdvancedTreeShaking`: candidates, walked statements, passes, the reference filter, and the output                                           |
| `advanced/env.rs`                            | `Env` (the inputs) and `ModuleFacts` (declarations, hoisted functions, first reach, cycles), init safety, namespace queries, the oxc context |
| `advanced/facts.rs`                          | `State` (the facts), `PathTree`, `BindingFacts`, `EntityFacts`                                                                               |
| `advanced/value.rs`                          | The value evaluator: `Value`, `Lit`, `ObjValue`, `FnValue`, return values                                                                    |
| `advanced/effects.rs`                        | The effect checker and the write deopts                                                                                                      |
| `advanced/walk.rs`                           | `Walker` (demand, exclusions, rewrites) and `EscapeVisitor`                                                                                  |
| `advanced/collect.rs`                        | `RefCollector` for excluded parts, and syntactic finders (`this`, `arguments`, `return`, `var`)                                              |
| `types/tree_shaking_rewrites.rs`             | `TreeShakingRewrites`, stored in `LinkingMetadata::tree_shaking_rewrites`                                                                    |
| `module_finalizers/tree_shaking_rewrites.rs` | The finalizer edits                                                                                                                          |
| `tree_shaking/include_statements.rs`         | The hooks, `run_advanced_passes`, and `apply_advanced_tree_shaking`                                                                          |
| `tree_shaking/dynamic_entries.rs`            | An `import()` in an excluded part does not keep its dynamic entry alive                                                                      |
| `refine_stmt_side_effects.rs`                | `ImportCycles::is_in_cycle`                                                                                                                  |

## Option plumbing

`InputOptions.experimental.advancedTreeshaking` (TS, validator, `bindingify-input-options.ts`) →
`BindingExperimentalOptions::advanced_treeshaking` → `ExperimentalOptions::advanced_treeshaking`,
read through `is_advanced_treeshaking_enabled()` (`unwrap_or(false)`). `prepare_build_context`
sets it to `false` in dev mode. `include_statements` creates the analysis only when
`treeshake` is on, the option is on, and dev mode is off. The schema in
`crates/rolldown_testing/_config.schema.json` lists the option for fixtures.

## Inputs and facts

`Env::new` builds `ModuleFacts` for each analyzed module (see design.md for the conditions):

- `decls`: each top-level binding with its statement, kind (`var`, `let`, `const`, function,
  class, `export default` expression, other) and initializer. `unique` is false when the binding
  has more than one declaration.
- `hoisted_fns`: the function that a top-level function declaration statement declares.
- `fn_first_reached`: for each top-level function, the first top-level statement that refers to
  it, directly or through the bodies of other top-level functions. Hoisted declarations and
  import or export lists reach nothing.
- `in_cycle`: the module is in a static-import cycle (a self-import counts).

`State` (`facts.rs`) holds the facts. Each fact only grows, and each change increases
`generation` and sets `facts_changed`.

| Fact                                            | Key                   | Set by                                                                                          | Used by                                       |
| ----------------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `BindingFacts::demand`                          | canonical `SymbolRef` | `Analyzer::use_ident` in the walker; `on_symbol_included` without `AnalyzedReference` (`whole`) | `Walker::binding_demand`, `is_object_escaped` |
| `BindingFacts::reassigned`                      | canonical `SymbolRef` | writes in walked code and in effect checks                                                      | `binding_value`, `binding_demand`             |
| `BindingFacts::included`                        | canonical `SymbolRef` | `on_symbol_included`                                                                            | write effects                                 |
| `EntityFacts::escaped`                          | module and node id    | `walk_object` with `whole`, `EscapeVisitor`, a deopt with an unknown key or `__proto__`         | `is_object_escaped`, `walk_object`            |
| `EntityFacts::deopted_keys`, `all_keys_deopted` | module and node id    | `deopt_member_write`                                                                            | `object_property_value`, `walk_object`        |
| `used_locals`                                   | module and `SymbolId` | reads of locals and parameters in the walker                                                    | `Walker::is_param_unused`                     |
| `all_objects_deopted`                           | the build             | a deopt chain deeper than `MAX_DEPTH`                                                           | `is_object_escaped`, `walk_object`            |

`begin_pass` clears the per-pass caches `call_effects` and `return_values`, because facts that
change during a pass can make a cached result too optimistic. `syntax` caches results that only
depend on the AST, for the whole build.

## The three evaluators

All three are methods of `Analyzer { env, state }`. `Site` says where code runs: the module, the
top-level statement, and whether the code is in a function body.

- **Values** (`value.rs`): `value(expr, site)` gives a literal, an object literal (`ObjValue`), a
  function (`FnValue`), a namespace of an analyzed module, or `Unknown`. An identifier has the
  value of its initializer only when the binding is unique, not reassigned, and init safe at the
  site (`Env::is_init_safe`). A member read finds the last definition of the key in an object
  literal, unless a spread, an unknown key, an accessor, a `__proto__` setter, an escape, or a
  deopt is in the way. A call of a known function gives the value that every `return` gives.
  `holder` is the top-level binding whose initializer contains a literal. Only a literal with a
  holder runs one time, so only such literals have an identity for `===` and for merging return
  values.
- **Effects** (`effects.rs`): `stmt_effects` and `expr_effects` with a `Cx`. `Cx::removal` asks
  whether code can go. `function_call_effects` asks whether a call has effects outside the call:
  there, a write to a `var` or a parameter of the callee is not visible, and a recursive call
  assumes no effects (`used_assumption` keeps such a result out of the cache). Code that the
  checker does not model goes to oxc's `may_have_side_effects` through `OxcCtx`.
- **Walk** (`walk.rs`): `Walker::walk_top` visits the included parts of one top-level statement
  with a demand. It records reads, writes, and escapes as facts. It removes object properties
  that are not demanded and have no effect, arguments for unused parameters, nested statements
  with no effect, and the dead parts of `if`, `?:`, `&&`, `||`, `??`, and `'k' in ns`. Each
  removal goes into `excluded` (for the reference filter) and into `rewrites` (for the
  finalizer).

## Hooks in the inclusion engine

1. `sweep_side_effect_statements` and `drain_body_demand_stmts` call `sweep_has_effects`. A
   walked statement with no effect does not go into the worklist. It becomes a candidate, and
   each pass checks it again.
2. `handle_include_symbol` calls `on_symbol_included` after the inlined-constant bypass. Without
   `SymbolIncludeReason::AnalyzedReference`, the symbol escapes.
3. `handle_include_statement` calls `walk_new_stmt` for a walked statement. The engine follows
   only the kept entries of `referenced_symbols`, with `analyzed_reason`. That function adds
   `AnalyzedReference` only to an entry whose root symbol belongs to the module, because the link
   stage adds entries with other owners that the walk did not see. The CommonJS bailout scan
   skips the excluded import records.
4. `is_dynamic_entry_alive` treats an `import()` in an excluded part as dead.
5. The driver loop: the existing dynamic-entry loop runs until it includes nothing more. Then
   `run_advanced_passes` runs passes until a pass changes nothing. New statements go into the
   worklist. For a grown statement, the engine follows the newly kept entries and scans its
   import records again. If the passes included code, the loop starts again, because new code can
   make more dynamic entries alive. After `preserve_reexported_interfaces`, the loop starts again
   when `generation` changed.

A walked statement is a statement of an analyzed module that maps to the AST and is not an import
or a re-export: `program.body[i]` is statement info `i + 1`. Statement info 0 (the namespace
object) and the statement infos that the link stage appends are not walked, so their references
make symbols escape.

## The reference filter

`AdvancedTreeShaking::walk` maps the excluded parts to the entries of `referenced_symbols`:

- The scanner adds one `Symbol` entry per identifier occurrence, or one `MemberExpr` entry for a
  static member chain on an imported binding, never both for one occurrence. `RefCollector` counts
  identifier references in the excluded parts (E) and in the whole statement (T). It does not
  count the root identifiers of `MemberExpr` entries (`member_roots`).
- A `Symbol` entry goes only when its owner is the module, T is more than 0, and E equals T.
  Other entries are ones that the link stage added (for example namespace objects and runtime
  symbols), and they stay.
- A `MemberExpr` entry goes when its node id is in an excluded part.
- An import record goes when all of its import nodes are in excluded parts.

The kept masks are the union over all walks of a statement, because the engine already followed
an entry that an earlier walk kept. The excluded records, excluded import nodes, and rewrites come
from the latest walk. Facts only grow, so the exclusions only shrink from pass to pass, and the
union is the same as the latest walk.

## Output

`finish` returns `AdvancedOutput`. `apply_advanced_tree_shaking` runs after the inclusion and:

- removes the dropped entries from `referenced_symbols` and the excluded records from
  `import_records`, so the generate stage sees only what the output contains;
- stores the merged `TreeShakingRewrites` of each module in `LinkingMetadata`;
- marks each excluded dynamic import `ImportRecordMeta::DeadDynamicImport`.

The finalizer applies the edits before it visits the children of a node, in `visit_statements`
(`remove_tree_shaken_statements`), `visit_statement` (`fold_if_statement`), and `visit_expression`
(`apply_tree_shaking_expr_rewrites`). `remove_unused_top_level_stmt` runs first, so the
statement indexes of the program body are not affected. A removed argument that is not the last
one becomes `void 0`, so the other arguments keep their positions. Synthetic nodes have
`NodeId::DUMMY` and never match an edit.

## Invariants

- **A `false` from the effect checker visited every write.** It applied the deopt of each write
  that the code can run, because the code goes and the walker never sees it. A `true` can stop
  early: the code stays, so the walker visits it in this pass or in a later one.
- **The walker gives `whole` to each value that it does not follow**: arguments of unknown
  callees, returned and thrown values, initializers of locals, array elements, the right side of
  an assignment, spread arguments, template parts, and code in classes (through `EscapeVisitor`).
- **A method call keeps the receiver whole** when the callee reads `this` or `super`, or is not
  known. `collect::function_uses_this` also looks at the `extends` clause and the computed keys
  of a class in the function.
- **A written property stays.** A write adds the demand `prefixed(key, empty)` on its object, so
  a setter or a getter-only property at the key stays.
- **Function objects are not entities.** Reads of their properties go to oxc, and a write of
  `__proto__` on one is an effect.
- **A key name is the name that JavaScript gives.** oxc's `static_name` formats a number with
  Rust's rules, so `number_key` gives a name only to an integer below 1e15, where the two agree.
  Other numeric keys and BigInt keys are unknown keys.

## Testing

- Fixtures: `crates/rolldown/tests/rolldown/tree_shaking/advanced_treeshaking/`. Each fixture
  turns the option on. `_test.mjs` checks the runtime behavior, and checks that no `DEAD_` marker
  is in `dist/*.js` where the code must go. The fixtures cover the 13 gaps, the chain, the
  dilemma, and the preserved semantics (live bindings, TDZ, cycles, escapes, setters, getters
  and `__proto__` on functions, `this`, `in` with a proxy prototype, object identity, numeric
  keys, effects in arguments and properties).
- Soundness: set the default to `unwrap_or(true)` in `is_advanced_treeshaking_enabled`, run
  `INSTA_FORCE_PASS=1 INSTA_UPDATE=no cargo test -p rolldown --test integration --no-fail-fast -- --skip test262`,
  and set the default back to `false`. Every runtime test of every fixture must pass. Result on
  2026-10-22: 2194 passed, 0 failed.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- `../linking/reference-needed-symbols/implementation.md` — the references that the link stage
  adds to statements
- `../linking/module-execution-order/implementation.md` — `exec_order`, which `is_init_safe` uses
