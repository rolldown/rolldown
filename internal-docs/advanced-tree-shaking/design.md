# Advanced Tree Shaking — Design & Principles

## Summary

`experimental.advancedTreeshaking` (default `false`) adds a path-keyed analysis to the inclusion
step of the link stage. The top-level statement stays the unit that the inclusion engine includes.
For each included statement of an analyzed module, the analysis decides which parts of it the
output keeps: object properties, call arguments, branches, and statements in function bodies and
blocks. It also stops the side-effect sweep from including a statement that has no effect under
its facts. The option closes the 13 gaps of the RFC (H1–H3, E1–E4, T1–T3, W1–W3), which were
measured against `rollup@4.62.4`.

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
   read, written, or escaped. It gives up an assumption only when code that it must keep breaks
   the assumption. A pessimistic analysis cannot prove W2: each of the mutually guarded `if`
   statements keeps the other alive. Facts only grow, so the fixpoint ends. Each pass walks every
   walked statement again with all facts so far, and the last pass decides.
4. **The old verdict AND the new verdict.** The sweep includes a statement only when the existing
   side-effect flags say that it has effects and the analysis agrees. The existing flags carry
   decisions that the analysis must not override: `moduleSideEffects`, pure annotations,
   `manualPureFunctions`, `propertyReadSideEffects: false`, and on-demand side effects. The
   analysis never keeps a statement that the option-off build removes.
5. **Escape is the default.** A value that flows to a place that the analysis does not model gets
   the demand `whole`. Then every property of it stays, and every property value is unknown. A
   symbol that the engine includes for a reason other than a reference in a walked statement
   escapes: entry exports, namespace objects, references from modules that the analysis does not
   cover, CommonJS interop. The analysis aims at the common shapes (configuration objects, flags,
   small helpers, unused arguments) and gives up early on the rest.
6. **Value and effect are separate questions.** What `o.b` holds and whether a read of `o.b` runs
   code have different answers. A read of a key with a getter is an effect. A read of a different
   key of the same object is not (E2, and the dilemma of the RFC). So "give up on every object
   with a getter" is not used, and `propertyReadSideEffects: false` is not needed.
7. **A removed write still changes facts.** The effect check of a write applies the deopt of the
   write before it decides whether the write has an effect. So the analysis never folds a
   property that a removed write would change. A write has no effect only when nothing can read
   what it writes: the binding or the object is not included, and no setter runs (E4, W1).
8. **Initialization order is a part of the value.** A value is known only at a read that cannot
   see the binding uninitialized. Import cycles and hoisted functions make many reads in function
   bodies unsafe. The rules are syntactic and conservative, and they also decide TDZ effects.
9. **Widening.** The RFC leaves the widening policy to rolldown. The current settings: a path
   deeper than 8 keys becomes `whole`. An evaluation deeper than 64 levels is unknown. A deopt
   chain deeper than 64 levels makes every object literal unknown. Rollup widens a path to top on
   the first unknown write. This analysis widens one key of one object literal, or one whole
   object literal for a write with an unknown key.
10. **Edits by node id.** The link stage reads the AST but does not change it. The analysis
    records the node ids of the parts to remove or fold, and the module finalizer applies the
    edits before it visits the children of a node. So the finalizer never renames or rewrites
    code that the output does not contain.

## Analyzed modules

The analysis covers a module only when all of these conditions are true. A module that it does
not cover uses the existing behavior, and the values that it exports are unknown.

| Condition                                                          | Why                                                                                                                                                                 |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| not the runtime module                                             | Its helpers belong to the generate stage.                                                                                                                           |
| ES module or no exports, and `WrapKind::None`                      | A wrapped body runs in a function at a time that the analysis does not know. CommonJS exports are properties of an object that the analysis does not model.         |
| module type JS, JSX, TS, or TSX                                    | Other types (JSON, CSS, assets) have generated or no ASTs.                                                                                                          |
| no direct `eval`                                                   | `eval` can read and write each binding by name.                                                                                                                     |
| no lazy export                                                     | The AST is generated later.                                                                                                                                         |
| no top-level await, and no dependency with top-level await         | Evaluation can stop and continue later, so the order of reads and writes is not known.                                                                              |
| side effects are not `NoTreeshake`                                 | The user asked to keep the module as it is.                                                                                                                         |
| the build does not set `treeshake.propertyWriteSideEffects: false` | Then the existing flags call every property write free of effects. The sweep never asks the analysis about such a write, so the analysis would not apply its deopt. |

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

## Known limitations

- Class bodies escape. Neither bundler removes unused class members.
- Arrays, locals in functions, and parameter values are not modeled. Call arguments and returned
  values escape.
- The properties of function objects are not tracked. A read of a property of a function is an
  effect when `propertyReadSideEffects` is not `false`.
- A read of a getter is always an effect, and the value of a getter is unknown.
- Only `if`, `?:`, `&&`, `||`, `??`, and `'k' in ns` fold.
- A dynamic entry in a cycle of dynamic imports stays, also when the analysis removes its
  `import()`. The existing cycled-entry bailout keeps it.
- `treeshake.propertyWriteSideEffects: false` turns the analysis off for the build.

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

## Unresolved questions

- The build-time budget. Each pass walks every included statement of every analyzed module
  again, and no benchmark exists yet.
- When the default flips, and whether the option stays as a way to bisect.
- With the option on, the default `treeshake.propertyReadSideEffects` (`'always'`) acts as rollup's
  `true` in analyzed modules: the analysis asks the object. The option cannot tell an explicit
  `'always'` from the default, so `'always'` cannot keep its meaning of "assume that every read
  has an effect". A separate `true` value, as the RFC proposes, would make this explicit.
- Whether the analysis should feed oxc or replace a part of its folding, so that two passes do
  not prove the same fact.
- Which plugins expect that a write to an object that nothing reads stays (E4).

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../linking/module-execution-order/implementation.md` — the execution order that the init-safety
  rules use
- `../linking/reference-needed-symbols/implementation.md` — the references that the link stage
  adds to statements, which the reference filter keeps
