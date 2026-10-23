# Advanced Tree Shaking — Design & Principles

## Summary

`experimental.advancedTreeshaking` (default `false`) adds a path-keyed analysis to the inclusion
step of the link stage. The top-level statement stays the unit that the inclusion engine includes.
For each included statement of an analyzed module, the analysis decides which parts of it the
output keeps: object properties, call arguments, branches, and statements in function bodies and
blocks. It also stops the side-effect sweep from including a statement that has no effect under
its facts.

The option closes the 13 gaps of the RFC (H1–H3, E1–E4, T1–T3, W1–W3), which were measured
against `rollup@4.62.4`. It also closes the 74 cases of the RFC supplement of 2026-10-22 (groups
G1–G18 and O1), which were measured against `rollup@4.62.4` and `rollup@4.64.0`.

The contract:

- **Option off:** no change. The engine creates no analysis, and every new branch in the engine
  checks `IncludeContext::advanced` first.
- **Option on:** the output contains a subset of the code of the option-off output. The only new
  code is a folded form: `true` or `false` for `'k' in ns`, the live branch of an `if`, a `?:`, or
  a `&&`/`||`/`??`, an empty statement for an `if` with no live branch, and `void 0` for a removed
  argument that is not the last one.
- **Behavior stays the same:** evaluation order, effects, getters and setters that run, thrown
  errors (TDZ errors too), and live bindings. `moduleSideEffects`, `annotations`, and
  `manualPureFunctions` keep their meanings and still win.
- **One assumption, like rollup:** no code replaces a builtin method or adds an array index to a
  builtin prototype. Code can add other keys to a builtin prototype, so a read of a key that an
  object literal does not define is unknown (see principle 11).

The machinery is in [implementation.md](./implementation.md).

## Design principles

1. **One engine.** The analysis is a part of the worklist engine in `include_statements.rs`, not
   a second tree shaker. The engine still resolves symbols, follows member expressions into
   namespaces, does the CommonJS bailout, includes dynamic entries and namespace objects, and
   feeds the generate stage. The analysis only removes: it vetoes a sweep inclusion, and it
   excludes parts of included statements with the references and import records in them. So the
   generate stage needs no change, and a build with the option off runs no new code.
2. **Before chunking.** W3 decides the placement. A fact in one chunk (nothing writes
   `custom_render`) must fold a branch in another chunk. The minifier runs per chunk, so it
   cannot do this. The chain example of the RFC shows that a dimension left to the minifier stops
   the other dimensions, because its results never go back into the fixpoint.
3. **An optimistic fixpoint, like rollup.** The analysis first assumes that nothing is included,
   read, written, called, or escaped. It gives up an assumption only when code that it must keep
   breaks the assumption. A pessimistic analysis cannot prove W2: each of the mutually guarded
   `if` statements keeps the other alive. Facts only grow, so the fixpoint ends. Each pass walks
   every walked statement again with all facts so far, and the last pass decides.
4. **The old verdict AND the new verdict.** The sweep includes a statement only when the existing
   side-effect flags say that it has effects and the analysis agrees. The existing flags carry
   decisions that the analysis must not override: `moduleSideEffects`, pure annotations,
   `manualPureFunctions`, `propertyReadSideEffects: false`, and on-demand side effects. The
   analysis never keeps a statement that the option-off build removes.
5. **Escape is the default.** A value that flows to a place that the analysis does not model gets
   the demand `whole`. Then every property of it stays, and every property value is unknown. A
   symbol that the engine includes for a reason other than a reference in a walked statement
   escapes: entry exports, namespace objects, references from modules that the analysis does not
   cover, CommonJS interop. The modeled places are parameters and returns of known functions,
   elements of array literals, patterns, and members of classes. The analysis aims at the common
   shapes (configuration objects, flags, small helpers, unused arguments) and gives up early on
   the rest.
6. **Value and effect are separate questions.** What `o.b` holds and whether a read of `o.b` runs
   code have different answers. A read of a key with a getter is the call of the getter. A read
   of a different key of the same object is not (E2, and the dilemma of the RFC). So "give up on
   every object with a getter" is not used, and `propertyReadSideEffects: false` is not needed.
7. **A removed write still changes facts.** The effect check of a write applies the deopt of the
   write before it decides whether the write has an effect. So the analysis never folds a
   property that a removed write would change. A write has no effect only when nothing can read
   what it writes: the binding or the object is not included, and no setter runs (E4, W1). A
   write below a value that a getter or a call returns changes the objects that it can return.
8. **Initialization order is a part of the value.** A value is known only at a read that cannot
   see the binding uninitialized. Import cycles and hoisted functions make many reads in function
   bodies unsafe. The rules are syntactic and conservative, and they also decide TDZ effects. A
   `const` or a `let` in a function has the value of its initializer only at a read that runs
   after the initialization in the same run of its scope: a read later in the source with no
   function between, or a read in a function that code creates after the initialization. A
   function declaration is hoisted, so a read in it counts only when each reference to the
   declaration from outside it counts. A declaration in a `case` clause, and a read through a
   `with` scope, never count, because a jump or an object can skip the initialization.
9. **Widening.** The RFC leaves the widening policy to rolldown. The current settings: a path
   deeper than 8 keys becomes `whole`. An evaluation deeper than 64 levels is unknown. A deopt
   chain deeper than 64 levels makes every object literal unknown. A summary of a function (its
   return value or its effects) that an evaluation reaches 48 levels deep or more waits: the
   evaluation gives the pessimistic result and does not cache it, the analysis computes the
   summary from depth 0, and then it evaluates again. One evaluation does at most 32 such steps,
   so a chain of 80 calls still folds, and a deeper chain gives the pessimistic result. Rollup
   widens a path to top on the first unknown write. This analysis widens one key of one object
   literal, or one whole object literal for a write with an unknown key.
10. **Edits by node id.** The link stage reads the AST but does not change it. The analysis
    records the node ids of the parts to remove or fold, and the module finalizer applies the
    edits before it visits the children of a node. So the finalizer never renames or rewrites
    code that the output does not contain.
11. **Builtins, like rollup.** The analysis knows the builtin methods of primitives, arrays,
    regular expression literals, `Object.prototype`, `Symbol`, `Object.freeze`, `Object.values`,
    `Object.entries`, and the typed array constructors, and it assumes that no code replaces them.
    Rollup makes the same assumption. Rollup does not assume that the builtin prototypes have no
    other keys: a missing key of an object is unknown, apart from an integer key. The analysis uses
    the same rule, because a missing key that folds to `undefined` would remove a branch that a
    polyfill or a test helper enables.
12. **Parameters need every call.** A parameter has a value only when the analysis sees every
    call of its function: the function does not escape, does not read `arguments`, and has no
    call that the analysis cannot follow (`call`, `apply`, `bind`, a callback of unknown code).
    Each call that included code or an effect check reaches goes into the facts. Facts only grow,
    so a call that appears later makes the parameter value smaller, never wrong at the end.
13. **`try` blocks, like rollup.** oxc treats some code that can throw as pure, for example
    `new WeakSet([1])` and `new ArrayBuffer(-1)`. A _catching `try` block_ is the block of a
    `try` statement that can discard a throw of the block: the statement has a `catch` clause, or
    its `finally` block has a `return`, a `break`, or a `continue`. The code that runs after such
    a block sees whether the block threw. So such a block with code in it has an effect, and in
    that block the analysis removes only code that never runs: a dead branch, or a statement
    after a jump. A folded `if` keeps its test there. A function can also run in such a block.
    When the block reads a parameter, or the `arguments` object, each argument at that position
    keeps the code that runs, and so does the body of a function that the argument gives. Rollup
    does the same by default (`treeshake.tryCatchDeoptimization`), but for each `try` statement,
    and for a parameter only when the block calls it directly. Outside such code, the analysis
    assumes, like rollup, that code that oxc treats as pure does not throw. `new` of a typed
    array is an exception: the analysis checks its length itself, because the check is simple.

## Analyzed modules

The analysis covers a module only when all of these conditions are true. A module that it does
not cover uses the existing behavior, and the values that it exports are unknown.

| Condition                                     | Why                                                                                                                                                                                                                                                              |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| not the runtime module                        | Its helpers belong to the generate stage.                                                                                                                                                                                                                        |
| ES module or no exports, and `WrapKind::None` | A wrapped body runs in a function at the time of the first `require` or the first call of the init function, so the statement order does not tell when a read runs. CommonJS exports are properties of `module.exports`, and `require` gives it to unknown code. |
| module type JS, JSX, TS, or TSX               | Other types (JSON, CSS, assets) have generated or no ASTs.                                                                                                                                                                                                       |
| no lazy export                                | The AST is generated later.                                                                                                                                                                                                                                      |
| side effects are not `NoTreeshake`            | The user asked to keep the module as it is.                                                                                                                                                                                                                      |

The first version also excluded modules with a direct `eval`, modules affected by top-level
await, and every module of a build with `treeshake.propertyWriteSideEffects: false`. Those gates
are gone (G15, O1):

- **Direct `eval`.** An `eval` in code that the output does not include never runs, so it changes
  no fact. When the walker reaches a direct `eval` in included code, every binding of the module
  escapes and becomes unknown (an import only escapes, because `eval` cannot write it). The sweep
  then keeps each declaration of the module, like the existing `eval` bailout of the engine. An
  indirect `eval` runs in the global scope and cannot see a module binding.
- **Top-level await.** The output keeps the statement order of each module, and an importer runs
  only after the module with the `await` finishes. An `await` pauses the evaluation but does not
  change that order, so the init-safety rules still hold. Code that runs during the pause, for
  example a promise callback, reaches the bindings of the module only through a function or an
  object that an earlier statement gave out. That statement comes before each declaration after
  the `await`, so a read in that function is not init safe. The other facts do not depend on the
  order: a write anywhere makes the value unknown everywhere.
- **`propertyWriteSideEffects: false`.** With this option, the existing flags call each property
  write free of effects. The engine includes a statement that writes a property of a binding only
  when it includes the binding, and the sweep never asks the analysis about it. So each pass
  first applies the write deopts of such statements that the engine did not include yet. A fold
  never depends on a value that such a write can change.

## How each gap closes

| Gap    | Mechanism                                                                                                                                          |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| H1, H2 | Each binding has a demand: a tree of the property paths that included code reads. An object literal keeps the properties on a demanded path.       |
| H3     | `'k' in ns` folds to a boolean from the resolved exports, so the statement no longer references `ns`. `ns.a` still resolves to the export.         |
| E1     | A read of a key of a known object literal with no getter at that key has no effect, so the sweep does not include `API.USERS;`.                    |
| E2     | The getter check looks at the read key only.                                                                                                       |
| E3     | A spread of a known object literal with no getters has no effect.                                                                                  |
| E4     | A write to a property of an object or a function that is not included has no effect.                                                               |
| T1, T2 | A call of a known function has the value that every `return` of the function gives. The value can come from a property path.                       |
| T3     | An argument goes when the known callee never reads the parameter and the argument has no effect.                                                   |
| W1     | A write to a `var`, or to an initialized `let`, that is not included has no effect. So a call of a function that only writes such a flag has none. |
| W2     | The fixpoint is optimistic: a write in a dead branch never runs, so the binding is never reassigned, and the next `if` folds too.                  |
| W3     | The analysis runs on the whole module graph before chunking.                                                                                       |

The groups of the supplement:

| Group | Mechanism                                                                                                                                                                                                                                                                                    |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| G1    | A parameter has the join of the arguments of every recorded call (principle 12). A destructured parameter follows its pattern.                                                                                                                                                               |
| G2    | The demand on a parameter goes to the arguments of the recorded calls. A write through a parameter deopts the argument of each recorded call, also of a call that comes later.                                                                                                               |
| G3    | A function declaration in a function has a value. An object literal in a function keeps only the paths that its reads demand.                                                                                                                                                                |
| G4    | A pattern demands only its keys. A rest element that no code reads reads each key, which runs the getters, but uses no value.                                                                                                                                                                |
| G5    | A getter that does not read `this` has the value that its returns give. A read of it has the effects of a call of it.                                                                                                                                                                        |
| G6    | A write that runs a setter has the effects of a call of the setter.                                                                                                                                                                                                                          |
| G7    | `__proto__: value` with an object literal or `null` gives a known prototype. A read or a write of a key that the literal does not define goes to the prototype.                                                                                                                              |
| G8    | An array literal is an entity like an object literal, with indexes, holes, and `length`.                                                                                                                                                                                                     |
| G9    | A call of a known array method has the effects of its callback and of the conversions of its arguments. `push`, `pop`, `shift`, and `unshift` change the array.                                                                                                                              |
| G10   | A known method of a string, a number, a boolean, a regular expression literal, or `Object.prototype` has no effect when no argument conversion runs code and no `RangeError` can occur. So do `Symbol()` with a primitive description and `Object.freeze` of an object that is not included. |
| G11   | A function object that did not escape has data properties only. `X.prototype.k` goes to the prototype of a known constructor.                                                                                                                                                                |
| G12   | An optional read or call of a known value that is not nullish is the plain read or call. On a nullish value, it reads and calls nothing.                                                                                                                                                     |
| G13   | The statements after a jump that always runs never run. A `for...in` loop over a known object enumerates its keys without user code, so only the body decides.                                                                                                                               |
| G14   | A class is an entity with static members, a prototype, and a constructor. `new` of a known class or ES5 constructor checks the constructor with `this` as the new object.                                                                                                                    |
| G15   | The `eval`, top-level await, and write gates are gone (see "Analyzed modules").                                                                                                                                                                                                              |
| G16   | A symbol from `Symbol()` is a primitive key, so its conversion runs no code. A read of an unknown primitive key of a known object with no getter and no `__proto__` key has no effect.                                                                                                       |
| G17   | A `delete` of a key of an object that is not included, not frozen, and not escaped has no effect. A later read of the key can reach the prototype.                                                                                                                                           |
| G18   | A property that a later definition of the same key replaces goes, unless unknown code can list the keys.                                                                                                                                                                                     |
| O1    | `propertyWriteSideEffects: false` no longer turns the analysis off (see "Analyzed modules").                                                                                                                                                                                                 |

The groups of the further comparison (2026-10-23):

| Group | Mechanism                                                                                                                                                                                                                                                                                                                                                                               |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| N1    | `Value::Prim` is a primitive with an unknown value. It is never a symbol or a BigInt, so its conversion runs no code. Arithmetic, bitwise, and comparison operators on literals fold with the rules of JavaScript: `ToInt32`, shift counts modulo 32, UTF-16 order for strings, and only exact powers. An unknown condition with equal alternatives gives the join of the alternatives. |
| N2    | A local `const` or `let` has the value of its initializer at a read that cannot see it uninitialized (principle 8).                                                                                                                                                                                                                                                                     |
| N3    | A write of an array index changes only that index, and `length` when the index is past the end. A write of `length` changes each index. A write of another key changes only the key.                                                                                                                                                                                                    |
| N4    | `new` of a function expression that is not a method checks its body. The default constructor of a derived class runs the parent constructor with the new object, then the fields. `extends null` throws. When a parent constructor can return a value, a derived class with fields is unknown, because the returned object gets the fields.                                             |
| N5    | `concat`, `flat` (to a depth of 1), `flatMap`, `reduce`, `reduceRight`, `reverse`, `sort`, `splice`, `copyWithin`, and `fill` of a known array. A method that only moves or removes elements keeps the element values known.                                                                                                                                                            |
| N6    | The call record of a builtin gives the parameters of the callback values: the join of the elements, a number for the index, the array, and the accumulator of a reducer.                                                                                                                                                                                                                |
| N7    | `repeat`, `padStart`, `padEnd`, `normalize`, `match`, `matchAll`, `search`, `localeCompare`, `toLocaleLowerCase`, and `toLocaleUpperCase` with arguments that cannot throw: a length below the smallest engine limit, a known normalization form, a plain pattern, the `g` flag for `matchAll`, and a simple language tag.                                                              |
| N8    | A number method ignores the arguments after the first.                                                                                                                                                                                                                                                                                                                                  |
| N9    | `Object.freeze` of a new array, function, or regular expression literal has no effect. `Object.values` and `Object.entries` of an object literal with no getters, of an array literal, or of a primitive that is not `null` or `undefined` have no effect.                                                                                                                              |
| N10   | `Value::RegExp` is a regular expression literal with entity facts, so an alias of it is known. `exec` and `test` change `lastIndex` only with the `g` or `y` flag, and the change is an effect only when included code can read the object.                                                                                                                                             |
| N11   | `new` of a typed array with no argument, a length from 0 to 65536, or an array literal of primitive values has no effect.                                                                                                                                                                                                                                                               |
| N12   | A summary that an evaluation reaches 48 levels deep waits, and the analysis evaluates again (principle 9).                                                                                                                                                                                                                                                                              |

## `treeshake.propertyReadSideEffects`

The option has three values. The default is `true`.

| Value      | Option off                         | Option on                                                                                 |
| ---------- | ---------------------------------- | ----------------------------------------------------------------------------------------- |
| `true`     | Every property read has an effect. | The analysis asks the object, like rollup's `true`.                                       |
| `'always'` | Every property read has an effect. | Every property read has an effect, like rollup's `'always'`. Namespace reads are exports. |
| `false`    | No property read has an effect.    | No property read has an effect.                                                           |

Before the supplement, the default was `'always'`, and the analysis could not tell an explicit
`'always'` from the default. The RFC proposed a separate `true` value, and the option has it now.
With the option off, `true` and `'always'` give the same flags, so a build without
`experimental.advancedTreeshaking` does not change.

## Known limitations

- Class bodies are walked with `EscapeVisitor`. So a class never loses a member, and each value
  in a class body escapes.
- `new` of a derived class with an explicit constructor (which calls `super()`), of a class with
  a decorator or an `accessor` property, or of a derived class with fields whose parent
  constructor can return a value, goes to oxc's check.
- The analysis does not track instances. The value of a property of `new X()` is unknown, also
  for a method of the prototype.
- A `const` or a `let` in a function has a value only at a read that principle 8 accepts. A read
  in a function that code creates before the initialization is unknown, also when the function
  runs after it.
- The accumulator of a reducer has a value only when the callback has one call record and a
  plain first parameter.
- One evaluation does at most 32 steps of the depth retry (principle 9).
- For both option values, other passes remove some code that throws, and the analysis cannot
  keep it. A read, in another `case` clause, of a `const` with a constant initializer goes. The
  DCE of the scan stage removes `new ArrayBuffer(-1)` and `decodeURIComponent('%')`, also from a
  catching `try` block, before the analysis runs.
- Principle 13 follows only the parameters of a known callee. Code that a catching `try` block
  reaches in another way loses the code that oxc treats as pure, like in rollup: a local function
  that the block calls, a getter, a value that an alias of the parameter holds, or a callback
  that unknown code calls.
- Only `if`, `?:`, `&&`, `||`, `??`, and `'k' in ns` fold.
- A dynamic entry in a cycle of dynamic imports stays, also when the analysis removes its
  `import()`. The existing cycled-entry bailout keeps it.

## Rejected alternatives

- **The minifier:** see principle 2.
- **A separate pass after the inclusion:** the analysis must feed the inclusion. A statement in
  W2 becomes dead only after another fact, and a statement that becomes alive needs the engine to
  include its references.
- **One option per dimension:** the chain of the RFC gets almost nothing from a subset of the
  dimensions.
- **A pessimistic analysis:** it cannot close W2.
- **AST edits in the link stage:** the link stage shares the AST as read-only data. The finalizer
  is where rolldown changes code.
- **A missing key folds to `undefined`:** the first version did this. It is not sound when code
  adds a key to `Object.prototype`, and rollup does not do it (principle 11).
- **A `try` block like other code:** the earlier versions did this. oxc treats some throws as
  pure, so the analysis removed a `try` statement whose `catch` clause runs, for example around
  `new WeakSet([1])` (principle 13).
- **Only the `try` block itself:** the first version of principle 13 did this. A callback that the
  block calls through a parameter lost its throw, for example in
  `attempt(() => new WeakSet([1]))`, and the option-off build keeps that throw.
- **Each function that a catching `try` block reads:** such a function would keep all of its code
  that runs, and so would each function that it reads. Many programs call their main code from a
  `try` block, so the analysis would remove almost nothing. Rollup also follows only parameters.

## Unresolved questions

- The build-time budget. Each pass walks every included statement of every analyzed module
  again, and no benchmark exists yet.
- When the default flips, and whether the option stays as a way to bisect.
- Whether the analysis should feed oxc or replace a part of its folding, so that two passes do
  not prove the same fact.
- Which plugins expect that a write to an object that nothing reads stays (E4).

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../linking/module-execution-order/implementation.md` — the execution order that the init-safety
  rules use
- `../linking/reference-needed-symbols/implementation.md` — the references that the link stage
  adds to statements, which the reference filter keeps
