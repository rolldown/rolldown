# Advanced Tree Shaking — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

The analysis lives in `crates/rolldown/src/stages/link_stage/tree_shaking/advanced/`. The
inclusion engine in `include_statements.rs` owns it as `IncludeContext::advanced` and calls it at
five hooks. After the inclusion fixpoint, the link stage writes the decisions back into the
statement infos, and the module finalizer applies the AST edits.

| File                                         | Contents                                                                                                                                                                                              |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `advanced/mod.rs`                            | `AdvancedTreeShaking`: candidates, walked statements, passes, gated writes, the reference filter, and the output                                                                                      |
| `advanced/env.rs`                            | `Env` (the inputs and the option flags) and `ModuleFacts` (declarations, hoisted functions, first reach, cycles), init safety, namespaces, the oxc context                                            |
| `advanced/facts.rs`                          | `State` (the facts and the per-pass caches), `PathTree`, `BindingFacts`, `EntityFacts`, call records (`CallArgs`), parameters, locals, and the summaries of the depth retry                           |
| `advanced/value.rs`                          | The value evaluator: `Value`, `Lit`, `ObjValue`, `ArrValue`, `FnValue`, `ClassValue`, `RegExpValue`, operators, slots, prototypes, parameter values, local values, return values, and the depth retry |
| `advanced/effects.rs`                        | The effect checker (`Cx`, calls, `new`, reads, writes, `delete`) and the write deopts                                                                                                                 |
| `advanced/classes.rs`                        | Classes, constructors, function properties, and `this`: class slots, static and instance member effects, `prototype_owner`, `construct_effects`, and `ctor_args_fn`                                   |
| `advanced/builtins.rs`                       | Calls of builtin methods of known values, `Symbol()`, `Object.freeze`, `Object.values`, `Object.entries`, and `new` of typed arrays                                                                   |
| `advanced/walk.rs`                           | `Walker` (demand, exclusions, rewrites, `catching`), `EscapeVisitor`, and `on_direct_eval`                                                                                                            |
| `advanced/collect.rs`                        | `RefCollector` for excluded parts, `LocalTdz`, `CaughtParams`, and syntactic finders (`this`, `arguments`, `return`, `var`, class opacity, methods, instance elements, catching `try` blocks)         |
| `types/tree_shaking_rewrites.rs`             | `TreeShakingRewrites`, stored in `LinkingMetadata::tree_shaking_rewrites`                                                                                                                             |
| `module_finalizers/tree_shaking_rewrites.rs` | The finalizer edits                                                                                                                                                                                   |
| `tree_shaking/include_statements.rs`         | The hooks, `run_advanced_passes`, and `apply_advanced_tree_shaking`                                                                                                                                   |
| `tree_shaking/dynamic_entries.rs`            | An `import()` in an excluded part does not keep its dynamic entry alive                                                                                                                               |
| `refine_stmt_side_effects.rs`                | `ImportCycles::is_in_cycle`                                                                                                                                                                           |

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
| `EntityFacts::new_elements`                            | module and node id    | a write of an index, a write of `length` that is not a reorder, a builtin that stores its arguments       | `is_array_values_known`                                           |
| `calls`                                                | function              | `record_args`, for each call or `new` that included code or an effect check reaches                       | `arg_join`, `deopt_args`                                          |
| `return_demands`                                       | function              | `walk_call`: the demand on the call                                                                       | `Walker::return_demand`                                           |
| `arg_deopts`                                           | function              | `deopt_local` of a parameter                                                                              | `record_args`, so a later call gets the deopt                     |
| `params`, `local_inits`, `local_fns`, `registered_fns` | local or function     | `register_params`, `register_local_decl`, `register_local_fns`                                            | `local_value`, `deopt_local`                                      |
| `used_locals`                                          | module and `SymbolId` | reads of locals and parameters in the walker, `on_direct_eval`                                            | `is_param_unused`, `is_pattern_read`                              |
| `live_evals`                                           | module                | `on_direct_eval`                                                                                          | `stmt_has_effects` keeps each declaration                         |
| `caught_fns`                                           | function              | `walk_arg`, for a function that a catching `try` block can call through a parameter                       | `walk_function` sets `catching`                                   |
| `all_objects_deopted`                                  | the build             | a deopt chain deeper than `MAX_DEPTH`                                                                     | every escape check                                                |

`begin_pass` clears the per-pass caches `call_effects`, `construct_effects`, `return_values`, and
`param_values`, because facts that change during a pass can make a cached result too optimistic.
`syntax` caches results that only depend on the AST (`uses_this`, `uses_arguments`,
`class_opaque`, `caught_params`, `local_tdz` for each top-level statement, and `methods` for each
module), for the whole build.

`PathTree` is the demand type. `whole` covers every path. `children` holds the demanded keys.
`every_key` says that code reads each own key but uses no value at a key that `children` does
not name. A rest element of an object pattern that no code reads gives it, so `walk_object`
keeps each getter, and a data property can still go.

## The three evaluators

All three are methods of `Analyzer { env, state }`. `Site` says where code runs: the module, the
top-level statement, and whether the code is in a function body.

- **Values** (`value.rs`): `value(expr, site)` gives a literal, `Prim` (a primitive with an
  unknown value, never a symbol or a BigInt), a symbol, an object literal (`ObjValue`), an array
  literal (`ArrValue`), a regular expression literal (`RegExpValue`), a function (`FnValue`), a
  class (`ClassValue`), a namespace of an analyzed module, or `Unknown`. A join of two primitives
  is `Prim`. `unary_value`, `binary_value`, `logical_value`, and `conditional_value` fold the
  operators on literals (`number_op`, `exponent`, `to_int32`), and an operator on primitives
  gives `Prim`. An identifier has the value of its initializer
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
- **Locals and closures** (G3, N2). A function declaration in a function has a value
  (`register_local_fns`). A `const` or a `let` with an initializer has the value of the
  initializer at a read that `LocalTdz::is_read_safe` accepts (design.md principle 8).
  `LocalTdz::collect` records, for one top-level statement, the end of each declarator, the start
  of each reference, and the start of each function. It leaves out a declaration directly in a
  `case` clause. `runs_after` walks the scopes from the read to the declaration: a `with` scope
  fails, and the outermost function between them must start after the declarator, or, for a
  function declaration, each reference to it from outside it must pass the same check.
  `local_inits` also lets a write through a local alias deopt the initializer.
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
- **Arrays** (G8, G9, N3, N5). An array literal has indexes, holes, and `length`, and a spread
  element makes the later indexes unknown. `deopt_array` applies a write of an index to the
  index, and to `length` when the index is past the end or the literal has a spread. A write of
  `length` deopts every key, and a write of another key deopts the key. `new_elements` says that
  a write or a builtin stored a value. A method that only moves or removes elements
  (`ARRAY_REORDER_METHODS`, and `splice` with two arguments at most) goes through
  `deopt_array_reorder`, which sets no `new_elements`. So the join of the elements stays known
  (`is_array_values_known`), but the value at each index does not. `builtins.rs` models the read
  methods, the callback methods, `join`, `concat`, `flat`, `flatMap`, `reduce`, `reduceRight`,
  `sort`, and the methods that change the array. A method key or `constructor` that a write set
  makes the call unknown (`is_array_key_written`).
- **Callbacks of builtins** (N6). A callback gets a call record with `CallArgs::Each`, `Reduce`,
  or `Compare`, so its parameters have values: the join of the elements, `Prim` for the index,
  and the array. `accumulator_value` gives `Prim` for the accumulator of a reducer when the start
  value is primitive and the callback, with `Prim` assumed for the parameter, returns a
  primitive. When it does not, `clear_caches` drops the results that the assumption gave. The
  walker still makes the callback of a kept call escape, because the builtin calls it in ways
  that the analysis does not follow.
- **Other builtins** (G10, G16, N7, N8, N9, N11). `builtin_call_effects` handles strings,
  numbers, booleans, regular expressions, `Object.prototype` methods of an object literal that
  does not define them, `Symbol()`, `Object.freeze`, `Object.values`, and `Object.entries`. Each
  needs primitive arguments, so that no conversion runs user code. `string_method_ok` checks
  `repeat`, `padStart`, and `padEnd` against `MAX_STRING_LENGTH`, `normalize` against the four
  forms, `match`, `matchAll`, and `search` against a plain pattern or a regular expression that
  did not escape, and the locale methods against a simple language tag. A number method checks
  only its first argument, against the range that does not throw. `own_values_effects` gives
  the getters of an object literal for `Object.values` and `Object.entries`.
  `builtin_new_effects` decides each `new` of a global typed array: no effect for no argument,
  `undefined`, `null`, a boolean, an integer length from 0 to 65536, or an array literal of at
  most 65536 primitive values with no new elements. A BigInt typed array takes only the lengths.
  `Value::Symbol` is a primitive key, and a key of unknown identity.
- **Regular expressions** (N10). A `RegExpValue` has entity facts. A write to it, and each place
  that the walker does not model, makes it escape. `exec` and `test` with primitive arguments
  have an effect only when the expression has the `g` or `y` flag and included code can read the
  object (`regexp_write_effects`): it escaped, its holder is included, or a walk demanded a key of
  a literal without a holder.
- **`Object.freeze`** (G10, N9). `freeze_effects` marks the literal `frozen`. Then a write or a
  `delete` of it throws, so it has an effect. The call has an effect only when the object is
  included. A new array, function, or regular expression literal in the argument has no other
  reference, so the call has no effect.
- **Function properties** (G11). A function object that did not escape has data properties only,
  apart from `caller` and `arguments`, which throw (`is_fn_object_escaped`). `deopt_fn` applies a
  write to one key. A write of `X.prototype.k` goes to `prototype_owner`.
- **Classes, constructors, and `this`** (G14). `classes.rs`. `class_slot` finds a static or an
  instance member, and `class_parent` follows `extends`. `static_member_effects` and
  `instance_member_effects` check accessors and the prototype chain. `class_effects` checks the
  definition: decorators, `extends`, computed keys, static fields, and static blocks with `this`
  as the class. `construct_effects` checks `new` of an ES5 constructor or a base class, with
  `this` as the new instance when the prototype is known. An ES5 constructor is a function
  declaration, or a function expression that is not a method, a getter, or a setter
  (`is_es5_ctor`, with `collect::method_functions`), and not async or a generator (N4).
  `run_ctor_effects` runs the default constructor of a derived class: the parent constructor with
  `this` as the new instance of the derived class, then the fields. `extends null` throws. When
  the class puts elements on the new object (`collect::installs_instance_elements`) and a
  constructor in the parent chain can return a value (`ctor_may_return_value`), the result is
  unknown: the returned object can reject new properties, or already have the private elements.
  The cache key of a construction holds the class of `this`. `class_opaque` makes a class
  escape when its static code reads `this` or `super`, it has an `accessor` property, or a class
  expression refers to its own name.
- **Control flow** (G12, G13). An optional read or call of a nullish value has no effect. The
  statements after a jump that always runs never run (`always_jumps`). `for_in_effects` lets a
  `for...in` over a known object with a fresh `let` or `const` binding go when its body does.
- **`try` statements** (design.md principle 13). `collect::discards_throw` decides whether the
  block of a `try` statement is a catching `try` block: the statement has a `catch` clause, or its
  `finally` block has a `return`, a `break`, or a `continue`. Such a block with code in it has an
  effect (`stmt_effects`). In such a block, the walker sets `catching`, and then it removes no
  code that runs: no statement, property, or argument goes, a fold that removes a test or an
  operand does not happen, and a folded `if` keeps its test. Dead branches and statements after
  a jump still go. The `catch` clause and the `finally` block are walked like other code.
- **Arguments that a catching `try` block can call** (design.md principle 13).
  `collect::caught_params` finds the parameters that a catching `try` block in a function reads.
  It compares names, also in nested functions, and a read of `arguments` in a function that is
  not an arrow counts for each parameter. `syntax.caught_params` caches the result for each
  function. `walk_call` and `walk_arguments` ask `is_arg_caught` for each argument of a known
  callee. For `new`, `ctor_args_fn` gives the function that gets the arguments: the function, the
  constructor of the class, or the constructor of a parent when the class has the default
  constructor. `walk_arg` walks such an argument with `catching`, and adds a function that the
  argument gives to `caught_fns`. `walk_function` sets `catching` for a function in `caught_fns`.
  A new entry is a fact change, so the next pass walks the function again, also when an earlier
  walk saw the function before the call.
- **Depth** (N12). At depth 0, `value` and `expr_effects` run `with_retry`. During a retry,
  `return_value` and `invoke_effects` at `DEFER_DEPTH` (48) or deeper call `defer_summary`: the
  result is pessimistic, `cut` is set, and the result does not go into a cache. Then
  `with_retry` warms each deferred summary from depth 0 (`warm`, which also warms the summaries
  that it defers) and evaluates again. A deferral of a summary that is warming is a cycle: its
  pessimistic result stays, and it sets no `cut`. `WARM_BUDGET` (32) limits the steps of one
  evaluation.
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
- **A `false` from the effect checker means that the code cannot throw**, apart from code that
  oxc treats as pure. The walker never removes such code from a catching `try` block, or from an
  argument or a function that such a block can call through a parameter (design.md principle 13).
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
  keys, effects in arguments and properties). Each supplement group and each group of the
  further comparison (N1–N12) has a removal fixture and a `_preserved` fixture, for example
  `arrays` and `arrays_preserved`, or `call_depth` and `call_depth_preserved`. Principle 13 has
  two pairs: `try_catch` for catching `try` blocks, and `try_catch_callbacks` for the arguments
  that such a block can call. A `_preserved` fixture checks the exact log of its `LIVE_` markers.
- A marker of a removal fixture must be in code that the option-off build keeps, so that the
  fixture checks the analysis. The DCE of the scan stage already removes some code, for example
  a call of a function that only returns its argument.
- A `_preserved` fixture puts code that must throw in a callback of `attempt(name, run)`. The
  helper calls the callback through an alias (`const call = run`), which principle 13 does not
  follow, so the walk of the callback checks the rule of the fixture. Code directly in a
  catching `try` block always stays, so such code checks only principle 13.
  `try_catch_callbacks` fails when principle 13 starts to follow aliases.
- Soundness: set the default to `unwrap_or(true)` in `is_advanced_treeshaking_enabled`, run
  `INSTA_FORCE_PASS=1 INSTA_UPDATE=no cargo test -p rolldown --test integration --no-fail-fast -- --skip test262`,
  and set the default back to `false`. Every runtime test of every fixture must pass. Result on
  2026-10-23: 2246 passed, 0 failed.
- The rollup comparison: the 177 cases of the RFC supplement and the 265 cases of the further
  comparison, against `rollup@4.64.0` and the saved verdicts of `rollup@4.62.4`. Result on
  2026-10-23: 0 gaps, and 0 gaps with `minify: true`, in both suites. The 177 cases have no
  runtime difference. The 265 cases have 9 runtime differences in 4 control cases, and the build
  before the further comparison has the same 9.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- `../linking/reference-needed-symbols/implementation.md` — the references that the link stage
  adds to statements
- `../linking/module-execution-order/implementation.md` — `exec_order`, which `is_init_safe` uses
