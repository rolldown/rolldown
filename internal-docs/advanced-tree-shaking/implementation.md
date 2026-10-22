# Advanced Tree Shaking — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

The analysis lives in `crates/rolldown/src/stages/link_stage/tree_shaking/advanced/`. The
inclusion engine in `include_statements.rs` owns it as `IncludeContext::advanced` and calls it at
five hooks. After the inclusion fixpoint, the link stage writes the decisions back into the
statement infos, and the module finalizer applies the AST edits.

| File                                         | Contents                                                                                                                                                   |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `advanced/mod.rs`                            | `AdvancedTreeShaking`: candidates, walked statements, passes, gated writes, the reference filter, and the output                                           |
| `advanced/env.rs`                            | `Env` (the inputs and the option flags) and `ModuleFacts` (declarations, hoisted functions, first reach, cycles), init safety, namespaces, the oxc context |
| `advanced/facts.rs`                          | `State` (the facts and the per-pass caches), `PathTree`, `BindingFacts`, `EntityFacts`, call records, parameters, and locals                               |
| `advanced/value.rs`                          | The value evaluator: `Value`, `Lit`, `ObjValue`, `ArrValue`, `FnValue`, `ClassValue`, slots, prototypes, parameter values, and return values               |
| `advanced/effects.rs`                        | The effect checker (`Cx`, calls, `new`, reads, writes, `delete`) and the write deopts                                                                      |
| `advanced/classes.rs`                        | Classes, constructors, function properties, and `this`: class slots, static and instance member effects, `prototype_owner`, and `construct_effects`        |
| `advanced/builtins.rs`                       | Calls of builtin methods of known values, `Symbol()`, and `Object.freeze`                                                                                  |
| `advanced/walk.rs`                           | `Walker` (demand, exclusions, rewrites), `EscapeVisitor`, and `on_direct_eval`                                                                             |
| `advanced/collect.rs`                        | `RefCollector` for excluded parts, and syntactic finders (`this`, `arguments`, `return`, `var`, class opacity)                                             |
| `types/tree_shaking_rewrites.rs`             | `TreeShakingRewrites`, stored in `LinkingMetadata::tree_shaking_rewrites`                                                                                  |
| `module_finalizers/tree_shaking_rewrites.rs` | The finalizer edits                                                                                                                                        |
| `tree_shaking/include_statements.rs`         | The hooks, `run_advanced_passes`, and `apply_advanced_tree_shaking`                                                                                        |
| `tree_shaking/dynamic_entries.rs`            | An `import()` in an excluded part does not keep its dynamic entry alive                                                                                    |
| `refine_stmt_side_effects.rs`                | `ImportCycles::is_in_cycle`                                                                                                                                |

## Option plumbing

`InputOptions.experimental.advancedTreeshaking` (TS, validator, `bindingify-input-options.ts`) →
`BindingExperimentalOptions::advanced_treeshaking` → `ExperimentalOptions::advanced_treeshaking`,
read through `is_advanced_treeshaking_enabled()` (`unwrap_or(false)`). `prepare_build_context`
sets it to `false` in dev mode. `include_statements` creates the analysis only when
`treeshake` is on, the option is on, and dev mode is off. The schema in
`crates/rolldown_testing/_config.schema.json` lists the option for fixtures.

`treeshake.propertyReadSideEffects` has the values `true` (the default), `'always'`, and `false`
(see design.md). The path: `module-side-effects.ts` and the validator →
`BindingPropertyReadSideEffects` → `PropertyReadSideEffects::{True, Always, False}`. The Rust
deserializer also takes the strings `"true"`, `"always"`, and `"false"`, which the fixtures use.
`FlatOptions::PropertyReadSideEffects` and the oxc options get the same value for `True` and
`Always`, so only `Env::property_reads_always` tells them apart.

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

`Env` also holds `property_reads_always` and `gated_writes`: with
`propertyWriteSideEffects: false`, the statements of analyzed modules that the engine includes
only with the binding that they reference.

`State` (`facts.rs`) holds the facts. Each fact only grows, and each change increases
`generation` and sets `facts_changed`. A "local" is a binding inside a function, keyed by its own
`SymbolRef`. A top-level binding is keyed by its canonical ref.

| Fact                                                   | Key                   | Set by                                                                                                    | Used by                                                           |
| ------------------------------------------------------ | --------------------- | --------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `BindingFacts::demand`                                 | binding               | `use_ident` and `add_demand_to` in the walker; `on_symbol_included` without `AnalyzedReference` (`whole`) | `Walker::binding_demand`, every escape check                      |
| `BindingFacts::reassigned`                             | binding               | writes in walked code and in effect checks, `on_direct_eval`                                              | `binding_value`, `local_value`, `binding_demand`                  |
| `BindingFacts::included`                               | canonical `SymbolRef` | `on_symbol_included`                                                                                      | write and `delete` effects                                        |
| `EntityFacts::escaped`                                 | module and node id    | a walk with `whole`, `EscapeVisitor`, a deopt with an unknown key or `__proto__`, a callback of a builtin | `is_object_escaped`, `is_array_escaped`, `is_fn_escaped`, classes |
| `EntityFacts::demand`                                  | module and node id    | each walk of the literal                                                                                  | writes and `delete`s of a literal without a holder                |
| `EntityFacts::deopted_keys`, `all_keys_deopted`        | module and node id    | the write deopts                                                                                          | values, `walk_object`, accessor checks                            |
| `EntityFacts::deleted_keys`                            | module and node id    | `deopt_member_delete`                                                                                     | `is_key_deleted`: a read can reach the prototype                  |
| `EntityFacts::frozen`                                  | module and node id    | `freeze_effects`                                                                                          | write and `delete` effects                                        |
| `calls`                                                | function              | `record_args`, for each call or `new` that included code or an effect check reaches                       | `arg_join`, `deopt_args`                                          |
| `return_demands`                                       | function              | `walk_call`: the demand on the call                                                                       | `Walker::return_demand`                                           |
| `arg_deopts`                                           | function              | `deopt_local` of a parameter                                                                              | `record_args`, so a later call gets the deopt                     |
| `params`, `local_inits`, `local_fns`, `registered_fns` | local or function     | `register_params`, `register_local_decl`, `register_local_fns`                                            | `local_value`, `deopt_local`                                      |
| `used_locals`                                          | module and `SymbolId` | reads of locals and parameters in the walker, `on_direct_eval`                                            | `is_param_unused`, `is_pattern_read`                              |
| `live_evals`                                           | module                | `on_direct_eval`                                                                                          | `stmt_has_effects` keeps each declaration                         |
| `all_objects_deopted`                                  | the build             | a deopt chain deeper than `MAX_DEPTH`                                                                     | every escape check                                                |

`begin_pass` clears the per-pass caches `call_effects`, `construct_effects`, `return_values`, and
`param_values`, because facts that change during a pass can make a cached result too optimistic.
`syntax` caches results that only depend on the AST (`uses_this`, `uses_arguments`,
`class_opaque`), for the whole build.

`PathTree` is the demand type. `whole` covers every path. `children` holds the demanded keys.
`every_key` says that code reads each own key but uses no value at a key that `children` does
not name. A rest element of an object pattern that no code reads gives it, so `walk_object`
keeps each getter, and a data property can still go.

## The three evaluators

All three are methods of `Analyzer { env, state }`. `Site` says where code runs: the module, the
top-level statement, and whether the code is in a function body.

- **Values** (`value.rs`): `value(expr, site)` gives a literal, a symbol, an object literal
  (`ObjValue`), an array literal (`ArrValue`), a function (`FnValue`), a class (`ClassValue`), a
  namespace of an analyzed module, or `Unknown`. An identifier has the value of its initializer
  only when the binding is unique, not reassigned, and init safe at the site
  (`Env::is_init_safe`). `holder` is the top-level binding whose initializer contains a literal.
  Only a literal with a holder runs one time, so only such literals have an identity for `===`
  and for merging values.
- **Effects** (`effects.rs`): `stmt_effects` and `expr_effects` with a `Cx`. `Cx::removal` asks
  whether code can go. `Cx::this` is the value of `this` in the checked code. `invoke_effects`
  asks whether a call or a `new` has effects outside the call: there, a write to a `var` or a
  parameter of the callee is not visible, and a recursive call assumes no effects
  (`used_assumption` keeps such a result out of the cache). Code that the checker does not model
  goes to oxc's `may_have_side_effects` through `OxcCtx`.
- **Walk** (`walk.rs`): `Walker::walk_top` visits the included parts of one top-level statement
  with a demand. It records reads, writes, calls, and escapes as facts. It removes object
  properties that are not demanded and have no effect, arguments for unused parameters, nested
  statements with no effect, and the dead parts of `if`, `?:`, `&&`, `||`, `??`, and `'k' in ns`.
  Each removal goes into `excluded` (for the reference filter) and into `rewrites` (for the
  finalizer).

## What the analysis models

- **Calls and parameters** (G1, G2). `walk_call` and the effect checks record each call of a known
  function (`record_args`). `param_value` joins the arguments at the index over the records
  (`arg_join`). The join is unknown when the function escaped (`is_fn_escaped`: an entity escape,
  or a demand on the holder that is `whole` or has `apply`, `arguments`, `bind`, `call`, or
  `caller`), reads
  `arguments`, or has a spread argument at or before the index. A rest parameter and a parameter
  with a default are unknown. The walker gives each argument the demand of its parameter
  (`param_demand`). A write through a parameter goes into `arg_deopts`, and `deopt_args` applies
  it to the argument of each record, also of a record that comes later.
- **Locals and closures** (G3). A function declaration in a function has a value
  (`register_local_fns`). Other locals are unknown, but `local_inits` lets a write through a local
  alias deopt the initializer.
- **Returns** (T1, T2). `return_value` merges the values of all `return` statements, and adds
  `undefined` when the body can end without one. Async functions and generators are unknown. A
  write below a returned value deopts each returned expression (`deopt_returns`).
- **Patterns** (G4). `pattern_demand` demands the keys of an object pattern. An array pattern
  demands `whole`, because the iterator protocol can read anything. A rest element that code
  reads demands `whole`, and one that no code reads sets `every_key`. `pattern_effects` checks
  the getters that a pattern runs, and `pattern_binding_value` follows the keys of a pattern.
- **Getters and setters** (G5, G6). `own_slot` finds the own property of a key. The last
  definition wins, and a data property between a getter and a setter of one key splits them. A
  getter that does not read `this` has its return value. A read runs the getter and a write runs
  the setter, so their effects are the effects of the call. The walker escapes each accessor
  function, because the analysis does not record its calls. A write below a getter value deopts
  what the getter returns.
- **Prototypes** (G7). `proto_of` reads `__proto__: value` of an object literal. A missing key
  follows a known prototype. On the default prototype, a missing key is unknown, apart from an
  array index (design.md principle 11). A `delete` makes later reads of the key reach the
  prototype (`deleted_keys`).
- **Arrays** (G8, G9). An array literal has indexes, holes, and `length`, and a spread element
  makes the later indexes unknown. A write to an array deopts all its keys. `builtins.rs` models
  the read methods, the callback methods, `join`, and `push`, `pop`, `shift`, and `unshift`. A
  callback of a builtin escapes, because the builtin calls it with arguments that the analysis
  does not record. A method that creates its result through `constructor` (`filter`, `map`,
  `slice`) needs an array that no write changed, because a write can set `constructor`.
- **Other builtins** (G10, G16). `builtin_call_effects` handles strings, numbers, booleans,
  regular expression literals, `Object.prototype` methods of an object literal that does not
  define them, `Symbol()`, and `Object.freeze`. Each needs primitive arguments, so that no
  conversion runs user code. Number methods also need arguments in the range that does not throw.
  `Value::Symbol` is a primitive key, and a key of unknown identity.
- **`Object.freeze`** (G10). `freeze_effects` marks the literal `frozen`. Then a write or a
  `delete` of it throws, so it has an effect. The call has an effect only when the object is
  included.
- **Function properties** (G11). A function object that did not escape has data properties only,
  apart from `caller` and `arguments`, which throw (`is_fn_object_escaped`). `deopt_fn` applies a
  write to one key. A write of `X.prototype.k` goes to `prototype_owner`.
- **Classes, constructors, and `this`** (G14). `classes.rs`. `class_slot` finds a static or an
  instance member, and `class_parent` follows `extends`. `static_member_effects` and
  `instance_member_effects` check accessors and the prototype chain. `class_effects` checks the
  definition: decorators, `extends`, computed keys, static fields, and static blocks with `this`
  as the class. `construct_effects` checks `new` of an ES5 function declaration or a base class,
  with `this` as the new instance when the prototype is known. `class_opaque` makes a class
  escape when its static code reads `this` or `super`, it has an `accessor` property, or a class
  expression refers to its own name.
- **Control flow** (G12, G13). An optional read or call of a nullish value has no effect. The
  statements after a jump that always runs never run (`always_jumps`). `for_in_effects` lets a
  `for...in` over a known object with a fresh `let` or `const` binding go when its body does.
- **`delete` and overwritten keys** (G17, G18). `delete_effects` applies the deopt first. A
  `delete` has no effect when the root and the holder are not included, and the object is not
  frozen and did not escape. `is_overwritten` removes a property that a later definition of the
  key replaces. When unknown code can list the keys, the property goes only when the definitions
  of the key follow it directly, because the first definition decides the key order.
- **Direct `eval`** (G15). `on_direct_eval` runs when the walker reaches a direct `eval` in
  included code. Each binding of the module becomes reassigned (apart from imports) and gets the
  demand `whole`, and `stmt_has_effects` keeps each declaration of the module.
- **Gated writes** (O1). With `propertyWriteSideEffects: false`, `run_pass` first applies
  `deopt_writes` to each statement of `Env::gated_writes` that the engine did not include.

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
- **A deeper write deopts every object that the written value can be.** `deopt_path` follows
  bindings, locals, parameters (through `arg_deopts`), object and array literals, prototypes,
  getters and calls (`deopt_returns`), class members, and conditional and logical operands. A
  value that it cannot follow escaped where it came from.
- **The walker gives `whole` to each value that it does not follow**: arguments of unknown
  callees, arguments after a spread, thrown values, values that unknown code can see, the right
  side of an assignment, spread arguments, template parts, and code in classes (through
  `EscapeVisitor`).
- **A method call keeps the receiver whole** when the callee reads `this` or `super`, or is not
  known. `collect::function_uses_this` also looks into arrow functions and classes in the
  function, because `extends` and computed keys read the outer `this`.
- **A written property stays.** A write adds the demand `prefixed(key, empty)` on its object, so
  a setter or a getter-only property at the key stays.
- **A key name is the name that JavaScript gives.** oxc's `static_name` formats a number with
  Rust's rules, so `number_key` gives a name only to an integer below 1e15, where the two agree.
  Other numeric keys and BigInt keys are unknown keys.

## Testing

- Fixtures: `crates/rolldown/tests/rolldown/tree_shaking/advanced_treeshaking/`. Each fixture
  turns the option on. `_test.mjs` checks the runtime behavior, and checks that no `DEAD_` marker
  is in `dist/*.js` where the code must go. The fixtures cover the 13 gaps, the chain, the
  dilemma, and the preserved semantics (live bindings, TDZ, cycles, escapes, setters, getters
  and `__proto__` on functions, `this`, `in` with a proxy prototype, object identity, numeric
  keys, effects in arguments and properties). Each supplement group has a removal fixture and a
  `_preserved` fixture, for example `arrays` and `arrays_preserved`. A `_preserved` fixture
  checks the exact log of its `LIVE_` markers.
- Soundness: set the default to `unwrap_or(true)` in `is_advanced_treeshaking_enabled`, run
  `INSTA_FORCE_PASS=1 INSTA_UPDATE=no cargo test -p rolldown --test integration --no-fail-fast -- --skip test262`,
  and set the default back to `false`. Every runtime test of every fixture must pass. Result on
  2026-10-23: 2218 passed, 0 failed.
- The rollup comparison of the RFC supplement: 177 cases, with 0 gaps, 0 gaps with `minify: true`,
  and 0 runtime differences against `rollup@4.64.0` and `rollup@4.62.4` on 2026-10-23.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- `../linking/reference-needed-symbols/implementation.md` — the references that the link stage
  adds to statements
- `../linking/module-execution-order/implementation.md` — `exec_order`, which `is_init_safe` uses
