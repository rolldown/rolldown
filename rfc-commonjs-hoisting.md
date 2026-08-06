# RFC: Hoisting statically analyzable CommonJS modules

- Feature Name: `commonjs_hoisting`
- Start Date: 2026-08-06
- RFC PR: TBD
- Tracking Issue: [rolldown#10483](https://github.com/rolldown/rolldown/issues/10483)

Code references point at `rolldown@79cd87fe8`. Bundler output quoted below is real output from `rolldown@1.2.0` and `webpack@5.109.0`, built by the [spec corpus](#the-spec-corpus) and reprinted in this repo's formatting.

## Table of contents

- [Summary](#summary)
- [Motivation](#motivation)
  - [Rolldown has one answer for every CommonJS module](#rolldown-has-one-answer-for-every-commonjs-module)
  - [What the wrapper costs](#what-the-wrapper-costs)
  - [The analysis is already there](#the-analysis-is-already-there)
- [Guide-level explanation](#guide-level-explanation)
  - [What hoisting looks like](#what-hoisting-looks-like)
  - [When a module hoists](#when-a-module-hoists)
  - [What keeps its wrapper](#what-keeps-its-wrapper)
  - [No strict-mode gate](#no-strict-mode-gate)
  - [The option](#the-option)
- [Reference-level explanation](#reference-level-explanation)
  - [Where the wrapper is decided today](#where-the-wrapper-is-decided-today)
  - [The hoistable predicate](#the-hoistable-predicate)
  - [Binding: named imports reach the facade symbols](#binding-named-imports-reach-the-facade-symbols)
  - [Render: declare once, assign in place](#render-declare-once-assign-in-place)
  - [Namespaces and default interop](#namespaces-and-default-interop)
  - [Strict execution order](#strict-execution-order)
  - [Preserved semantics](#preserved-semantics)
  - [The spec corpus](#the-spec-corpus)
  - [Questions / Explanations](#questions--explanations)
    - [Why is `require()` the line, and not side effects?](#why-is-require-the-line-and-not-side-effects)
    - [What happens to a module a wrapped module imports?](#what-happens-to-a-module-a-wrapped-module-imports)
- [Drawbacks](#drawbacks)
- [Rationale and alternatives](#rationale-and-alternatives)
  - [Why this shape](#why-this-shape)
  - [Do nothing: the minifier cannot cross the wrapper](#do-nothing-the-minifier-cannot-cross-the-wrapper)
  - [Hoist everything: eager evaluation changes behaviour](#hoist-everything-eager-evaluation-changes-behaviour)
  - [Run the wrapper eagerly: keeps the costs, loses the semantics](#run-the-wrapper-eagerly-keeps-the-costs-loses-the-semantics)
  - [Copy webpack's three states: rolldown has no registry](#copy-webpacks-three-states-rolldown-has-no-registry)
- [Prior art](#prior-art)
  - [webpack](#webpack)
  - [esbuild](#esbuild)
  - [Rollup](#rollup)
- [Unresolved questions](#unresolved-questions)
- [Future work](#future-work)

## Summary

Rolldown puts every CommonJS module behind a lazy `__commonJSMin` wrapper. This RFC drops the wrapper where it earns nothing. A module qualifies when its exports are all written statically and nothing reaches it through `require()`. Those modules become plain top-level bindings in the chunk.

It ships behind `experimental.onDemandWrapping: { commonjs: true }` and adds no new output format. It changes the output of code that bundles today.

## Motivation

### Rolldown has one answer for every CommonJS module

Take the smallest possible case:

```js
// mod.cjs
'use strict';
exports.a = 1;
exports.b = () => 2;

// entry.mjs
import { a, b } from './mod.cjs';
console.log(JSON.stringify([a, b()]));
```

Rolldown 1.2.0 emits:

```js
var __commonJSMin = (cb, mod) => () => (
  mod || (cb((mod = { exports: {} }).exports, mod), (cb = null)),
  mod.exports
);

var require_src = /* @__PURE__ */ __commonJSMin((exports) => {
  exports.a = 1;
  exports.b = () => 2;
});

var import_src = require_src();
console.log(JSON.stringify([import_src.a, (0, import_src.b)()]));
```

webpack 5.109 emits:

```js
var __WEBPACK_CJS_EXPORT_a__;
var __WEBPACK_CJS_EXPORT_b__;

__WEBPACK_CJS_EXPORT_a__ = 1;
__WEBPACK_CJS_EXPORT_b__ = () => 2;

console.log(JSON.stringify([__WEBPACK_CJS_EXPORT_a__, __WEBPACK_CJS_EXPORT_b__()]));
```

Rolldown does not pick the wrapper here because the module is hard. It picks it because the wrapper is the only shape it has. Every module whose `exports_kind` is `CommonJs` takes the same path, whether it is a 3-line constants file or a module that reassigns `module.exports` inside a conditional.

Most of `node_modules` is still CommonJS, so this is the common path, not a corner.

### What the wrapper costs

**Nothing crosses it.** Once a value lives on `import_src`, every read is a property lookup on an object the minifier cannot see through. `a` becomes `import_src.a`. A call becomes `(0, import_src.b)()` to keep `this` undefined. No inlining, no constant folding, no dead-export removal.

**Every ESM importer pays interop.** Keep `mod.cjs` exactly as it is and change only the entry to a namespace import:

```js
// entry.mjs
import * as ns from './mod.cjs';
console.log(JSON.stringify({ a: ns.a, b: ns.b() }));
```

```js
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __commonJSMin = /* … */;
var __copyProps = /* … 14 lines … */;
var __toESM = /* … 4 lines … */;

var require_src = /* @__PURE__ */ __commonJSMin((exports) => {
  exports.a = 1;
  exports.b = () => 2;
});

var import_src = /* @__PURE__ */ __toESM(require_src(), 1);
console.log(JSON.stringify({ a: 1, b: import_src.b() }));
```

Read the last four lines. Rolldown knew `a` was the constant `1` and folded it, so the wrapper's own `exports.a = 1` is already dead. It could not do the same for `b`, so it kept nine runtime helpers, the wrapper, and the full `__toESM` dance to read one function off an object it built itself. The analysis was good enough to fold one export and not good enough to drop the module.

Note also that the named-import build above left `a` as `import_src.a` and folded nothing. Same module, same values, different import syntax. That the two disagree is itself the symptom: reads of a wrapped module are handled case by case, because they are not ordinary bindings.

**The wrapper spreads.** Under `experimental.onDemandWrapping`, `eagerly_triggers_interop_module` marks any module that statically imports a wrapped module as sensitive to execution order. That module then gets order-wrapped in `__esm` itself. One CommonJS leaf can pull its whole importer subtree into the wrap plan. Hoisting that leaf removes it from the trigger set and shrinks the plan.

### The analysis is already there

Rolldown does not need new analysis to know which modules are safe. It computes the answer today and then throws it away:

- `EcmaViewMeta::SafelyTreeshakeCommonjs` is set when every export write is a static property assignment and nothing reads the exports object in an unknown way (`ecma_module_view_factory.rs:133-139`).
- The scanner mints a facade symbol for each `exports.x = …` write and records it as a `LocalExport` (`ast_scanner/impl_visit.rs:249-267`).
- Those facade symbols already land in the module's `named_exports` when there is exactly one write per name (`ecma_module_view_factory.rs:96-99`).

So the symbols a hoisted module would bind to already exist, with names, in the export table. The binder never consults them, because it short-circuits on `exports_kind` first.

## Guide-level explanation

### What hoisting looks like

A hoisted module has no wrapper and no exports object. Each export becomes a top-level binding in the chunk, and importers reference that binding directly.

```js
// mod.cjs, hoisted
var mod_a = 1;
var mod_b = () => 2;

// entry.mjs
console.log(JSON.stringify([mod_a, mod_b()]));
```

That is the shape, collapsed for reading; the exact lowering is in [Render](#render-declare-once-assign-in-place). It is also what rolldown already produces for an ESM module. After hoisting, a CommonJS module is no longer a special citizen of the chunk. Every downstream win comes from that. The minifier inlines it and tree shaking removes unused exports. The machinery for execution order treats it like anything else.

### When a module hoists

A CommonJS module hoists when all four hold.

**1. Every export write is static.** The base is exactly `exports` or `module.exports`, and the property name is a plain identifier. `exports[key] = …` does not qualify.

**2. The module never treats its exports as an object.** No `module.exports = …` reassignment, no `Object.defineProperty(exports, …)`, no aliasing `exports` to a local, no `__esModule` flag, no `module.id` or `module.loaded`. Each of these makes the object itself observable, and a set of loose bindings cannot stand in for it.

Conditions 1 and 2 together are what `SafelyTreeshakeCommonjs` already means.

**3. Nothing reaches it through `require()`.** This is the load-bearing one. `__commonJSMin` is lazy and memoized: the body runs on the first `require_mod()` call and never again. Hoisting turns that into eager evaluation at the top of the chunk. For a module reached only by static `import`, that changes nothing. Rolldown already calls `require_mod()` at the importer's position, which is the same point in the program. For a module reached by `require()`, it is a real semantic change, so those keep the wrapper. The same reasoning excludes CommonJS entries and members of a `require` cycle.

**4. Nothing already wrapped imports it.** A wrapped module defers its body. Anything it imports must still be initialized when that body finally runs, which today means the import is wrapped too. Hoisting inside a deferred subtree would move a body's side effects ahead of the wrapper that guards them, so v1 leaves that subtree alone. See [Future work](#future-work).

### What keeps its wrapper

Everything else, and the list is not short. From the corpus:

| shape                                                       | why the wrapper stays                        |
| ----------------------------------------------------------- | -------------------------------------------- |
| `module.exports = { … }` / `= fn` / `= class`               | the object identity is the export            |
| `Object.defineProperty(exports, …)`, `defineProperties`     | writes rolldown cannot name                  |
| `exports.__esModule = true`                                 | the flag drives interop at every import site |
| `const e = exports; e.a = 1`                                | the alias escapes static tracking            |
| `if (exports.a) …`                                          | the module reads its own exports object      |
| `Object.freeze(exports)`, getter objects                    | object-level semantics                       |
| `this.a = 1` at top level                                   | `this` is the exports object                 |
| reached by `require()`, a `require` cycle, a CommonJS entry | lazy evaluation must stay lazy               |

Every one of these is a case in the corpus with a webpack-verified verdict, so the boundary is testable rather than asserted.

### No strict-mode gate

webpack bails on any module without `"use strict"`. That is its single largest bailout class, since most of `node_modules` is sloppy mode. The gate exists because webpack merges module bodies into one shared strict scope. A sloppy body dropped in there would quietly change meaning.

Rolldown has no such problem. Hoisting moves a body out of an arrow function and up to the top level of the same scope. Whatever strictness applied before applies after. So rolldown should hoist sloppy modules. The corpus records this as the one case where rolldown's target verdict beats webpack's actual one (`bail/sloppy-mode`).

### The option

Hoisting ships behind an experimental flag. It widens the existing `onDemandWrapping` boolean, the way `inlineConst` and `chunkImportMap` are already widened:

```ts
experimental: {
  onDemandWrapping?: boolean | { commonjs?: boolean };
}
```

`onDemandWrapping: true` keeps its current meaning. `{ commonjs: true }` additionally lets rolldown hoist the CommonJS modules that pass the predicate.

A flag is warranted even though hoisting is a strict improvement where it is safe, because of how it fails. A bug in the predicate does not break the build. It emits a bundle that runs and is quietly wrong, in code the user did not write, somewhere in `node_modules`. That deserves an off switch while the feature is experimental, and something to bisect against when a report comes in.

The name settles one thing that must be written down. Today `onDemandWrapping` is defined "under `output.strictExecutionOrder`", and `is_strict_on_demand_wrapping_enabled()` returns false without it. Hoisting's main wins have nothing to do with strict execution order. They are smaller output, tree-shakeable exports, and no interop helpers. Gating them behind it would hide the feature from most users. So **the `commonjs` sub-flag is read on its own, whatever `strictExecutionOrder` is set to.** The boolean form keeps its existing gate. The two halves of the option differ here, so the option's own docs have to say it.

The expected end state is on by default, once the corpus is green and the flag has soaked. Whether it survives after that is [left open below](#unresolved-questions).

## Reference-level explanation

### Where the wrapper is decided today

Two rules make CommonJS mean "wrapper", plus three overrides.

`determine_module_exports_kind.rs:97-106` — every CommonJS module that is not an entry is set to `WrapKind::Cjs`. A CommonJS entry joins it when the output format is `esm`, or when the format is `iife`/`umd` and the module touches `module`/`exports`.

`wrapping.rs:126-138` — while walking an unwrapped module's import records, any importee whose `exports_kind` is `CommonJs` is wrapped. That happens transitively, through `wrap_module_recursively`.

The overrides:

- `determine_module_exports_kind.rs:50-56` — a `require()` edge wraps its importee. This one is the rule, not the exception; the predicate below never contradicts it.
- `determine_module_exports_kind.rs:66-79` — with code splitting disabled, `import()` behaves like `require()`, so its importee is wrapped.
- `wrapping.rs:155-166` — under strict execution order with manual code splitting groups, every CommonJS module is forced back to `WrapKind::Cjs` ([#10405](https://github.com/rolldown/rolldown/pull/10405)).

`create_wrapper` (`wrapping.rs:201-223`) then mints the `require_<name>` symbol and the `__commonJSMin` statement for anything marked `WrapKind::Cjs`.

`set_wrap_kind` is last-writer-wins, so all of this is order-sensitive (see `internal-docs/linking/determine-module-exports-kind/implementation.md`).

### The hoistable predicate

The predicate answers one question per CommonJS module: _may this module skip the default wrapper?_ It reads:

- The `commonjs` flag. When it is off the predicate answers no for every module, and nothing downstream changes.
- `EcmaViewMeta::SafelyTreeshakeCommonjs` — conditions 1 and 2, already computed at scan time.
- The import records of every module in the graph — condition 3. A module is disqualified by any incoming `ImportKind::Require` record, by being an entry, or by sitting in a `require` cycle. `LinkingMetadata::required_by_other_module` already carries part of this signal.

The whole module graph is known before the link stage runs. So this is one pass over the module table, computed once, before wrap kinds are assigned.

The framing that keeps this safe: **hoisting is a refusal to add a wrapper, never the removal of one.** The predicate is consulted only at the two default rules above. Every override still fires and still wins, including the forced wrap under strict execution order. No existing correctness fix can be undone by this change. Condition 4 falls out for free: `wrap_module_recursively` keeps wrapping whatever a wrapped module reaches, and the predicate does not fight it.

### Binding: named imports reach the facade symbols

`bind_imports_and_exports.rs:1250-1252` returns `ImportStatus::CommonJS` for any importee whose `exports_kind` is `CommonJs`, before it looks at `resolved_exports`. The consumer at line 1377 then routes the import through the namespace object.

For a hoistable module, that short-circuit is skipped and the import resolves through `resolved_exports` like an ESM one. The facade symbols are already there. Two gaps have to close first:

- `module.exports.x = …` mints no facade symbol. The scanner's `StaticMemberExpression` branch (`impl_visit.rs:271-282`) records the `module` identifier and nothing else, so a module written in that style has no symbols to bind to. Corpus case: `hoist/module-exports-prop`.
- An export written more than once is dropped from `named_exports` (`ecma_module_view_factory.rs:96-99` keeps only `v.len() == 1`). Repeated writes are ordinary reassignment once hoisted. The table should carry the first write and let the rest be assignments. Corpus case: `hoist/repeated-write`.

### Render: declare once, assign in place

One rule handles every write shape:

- Emit `var <facade>;` once, at the module's position in the chunk.
- Rewrite every `exports.x = v` in place to `<facade> = v`.
- Rewrite every read of `exports.x` inside the module to `<facade>`.

`var` hoisting then does the rest. A conditional write leaves the binding `undefined` until it runs, which is what the wrapper does. A write nested in a function body works without a special case. A repeated write is a reassignment.

This is also what webpack emits, which is a useful cross-check on a lowering that could otherwise be argued several ways:

```js
var __WEBPACK_CJS_EXPORT_a__;
__WEBPACK_CJS_EXPORT_a__ = 1;
```

Collapsing `var x; x = 1;` into `var x = 1` when the write is unconditional and first is a codegen nicety, not part of the contract.

### Namespaces and default interop

Dropping the exports object does not mean nobody wants one. Two consumers still do:

- `import * as ns from "./mod.cjs"` needs a namespace.
- `import m from "./mod.cjs"` needs `default`, which for a CommonJS module is `module.exports` itself.

Neither forces an object into the output by itself. Rolldown already resolves a static member read on an ESM namespace straight to the binding. It emits no object at all: `import * as ns` followed by `console.log(ns.a)` bundles to `console.log(1)`, and hoisted CommonJS inherits that. An object shows up only when the namespace escapes — passed to a function, spread, or indexed dynamically.

When one is needed, no new runtime helper is needed with it. `__exportAll` builds a fresh object of live, enumerable getters from a map of name to thunk. It is already what rolldown emits for an escaping ESM namespace (`module_finalizers/mod.rs:986-993`).

Take the running example and an entry that escapes both consumer kinds:

```js
// mod.cjs, unchanged
exports.a = 1;
exports.b = () => 2;

// entry.mjs
import * as ns from './mod.cjs';
import m from './mod.cjs';
send(ns, m);
```

Hoisted, as proposed — unlike the earlier blocks, this one is a design sketch rather than built output:

```js
var mod_a;
var mod_b;
var mod_exports = /* @__PURE__ */ __exportAll({ a: () => mod_a, b: () => mod_b }, true);
var mod_ns = /* @__PURE__ */ __toESM(mod_exports, 1);
mod_a = 1;
mod_b = () => 2;

send(mod_ns, mod_exports);
```

The getter map is written once. The namespace is derived from the exports object instead of built from a second copy, so what lands in the chunk stays linear in the export count. webpack does the same: it writes the map into a base object, then calls `__webpack_require__.t(base, 2)` to get the namespace.

Deriving through `__toESM` buys more than brevity. It is the same call the importer of a wrapped module makes today, over an object of the same shape. So the namespace it returns is the one rolldown already produces: same keys in the same order, same absent tag, same prototype, same `default` identity. That is the strongest guarantee available that hoisting is unobservable. Not an argument that the two agree, but the same code path.

Two objects over one set of bindings, and that is the part to get right. `ns` is the namespace and carries a `default` key; what `default` holds is `module.exports`. webpack emits the same two-object split for this case, with `default` pointing at the base object, so the shape is not speculative.

Both objects pass `no_symbols`, which needs saying because it is not what Node does. In Node a namespace carries `Symbol.toStringTag: 'Module'` and `module.exports` does not. Rolldown today builds a CommonJS namespace through `__toESM`, which sets no tag, so `Object.prototype.toString.call(ns)` already returns `[object Object]` for a wrapped module. Tagging the hoisted one would make hoisting observable, which is the one thing it must never be. Matching Node is worth doing on its own, for wrapped and hoisted modules at once — see [Unresolved questions](#unresolved-questions).

Getters, not a snapshot. A hoisted module may still assign `exports.a` after evaluation, from a callback or a timer. The wrapper's object would have shown that. Because the map holds thunks and not values, both calls can sit above the writes, which is where rolldown already places them for ESM.

Named imports need none of this. No object, no `__exportAll`, no `__toESM` — the import resolves to the binding and the helpers never enter the chunk. The interop cost that every CommonJS import pays today becomes a cost only an escaping namespace pays.

A module whose namespace escapes gains least. It swaps the `__commonJSMin` closure for an `__exportAll` map and keeps the `__toESM` call it already had, so the object comes back and the helpers stay. It is not worse than today, and the bindings underneath are still bindings the minifier can reach, but most of the win is gone. Static reads pay nothing at all. So the benefit tracks how a module is used, not what it exports.

The predicate could take that into account and leave escaping modules wrapped. But it ties a link-stage decision to usage, and the wrap decision avoids that on purpose today. Left open below.

### Strict execution order

Two changes in `generate_stage/order_analysis.rs`:

- `is_order_wrap_eligible` (line 1194) requires `ExportsKind::Esm | None`, so CommonJS is invisible to on-demand wrapping. A hoisted module is an ordinary eager module and should be admitted.
- `eagerly_triggers_interop_module` (line 1168) marks importers of wrapped modules as order-sensitive. A hoisted module is not wrapped, so it stops triggering, and its importers stop being wrapped on its account.

Both are reached only when a module actually hoisted, so `{ commonjs: true }` gates them without a second check. This is also the half of the feature that justifies the option's name: with the flag on, on-demand wrapping stops being ESM-only.

The #10405 forced wrap stays until both land with snapshot coverage. It is a correctness fix for a real failure and should not be relaxed speculatively.

### Preserved semantics

Hoisting must not change any of these:

- **Evaluation order and side effects.** A hoisted body runs at the module's position in the chunk, which is where its `require_mod()` call sits today.
- **Export liveness.** A later write to an export is visible to importers, through the binding directly or through a namespace getter.
- **Export identity.** `import * as ns` and default interop see the same values as before, with the same keys.
- **Tree shaking.** Removing an unused export is a gain, not a change: the wrapper's exports object was never observable from outside.
- **Sloppy-mode meaning.** Enclosing strictness is unchanged, which is what makes [the missing strict-mode gate](#no-strict-mode-gate) sound.

### The spec corpus

[`IWANABETHATGUY-reproduction/cjs-concat-spec`](https://github.com/IWANABETHATGUY-reproduction/cjs-concat-spec) is 44 cases covering every hoist, wrap, and bail condition in webpack 5.109, read out of `ModuleConcatenationPlugin.js` and `JavascriptGenerator.js` and then confirmed by building each one. Each case is built three ways — webpack with concatenation off, webpack with it on, and rolldown — and asserts two things:

- the webpack verdict, as spec;
- **identical runtime output** across all three. A bundler may hoist, wrap, or bail as it likes; the program has to keep doing the same thing.

Rolldown's verdict is reported against a `target` column rather than asserted, so the rows where the two disagree are the work list. Today that is 11 rows, and they are the acceptance criteria for this RFC:

| case                         | proves                                              |
| ---------------------------- | --------------------------------------------------- |
| `hoist/plain-exports`        | `exports.a = 1` becomes a plain binding             |
| `hoist/module-exports-prop`  | `module.exports.x =` hoists too                     |
| `hoist/repeated-write`       | a repeated write is a reassignment                  |
| `hoist/conditional-export`   | a conditional write stays `undefined` until it runs |
| `hoist/unused-export-shaken` | an unused export is a dead `var` and is removed     |
| `hoist/nested-member-write`  | `exports.obj.x =` does not block hoisting           |
| `hoist/calls-require`        | a hoisted module may call `require()` itself        |
| `hoist/dynamic-import`       | hoisting composes with `import()` elsewhere         |
| `hoist/namespace-import`     | `import * as ns` gets a synthesized namespace       |
| `hoist/two-modules`          | two modules hoist into one scope without collisions |
| `bail/sloppy-mode`           | no strict-mode gate                                 |

The other 33 cases are the guardrail: 15 `wrap/*` shapes plus `bail/require-target`, `bail/cjs-cycle`, and `bail/cjs-entry` must not move. The cases eject to the directory shape rolldown's suite uses, so porting them into `crates/rolldown/tests/rolldown/function/` is part of this work.

Building the corpus turned up one rolldown bug that is unrelated to hoisting and ships independently. Rolldown rewrites top-level `this.a = 1` to `exports.a = 1`, but emits the `__commonJSMin` callback with an empty parameter list, so `exports` is a free variable and an ESM bundle throws `ReferenceError: exports is not defined in ES module scope`:

```js
var require_src = /* @__PURE__ */ __commonJSMin(() => {
  exports.a = 1; // free variable
});
```

It also caught a webpack 5.109.0 bug, worth knowing when comparing output. If the only wrapped CommonJS module in a bundle never touches `module`, webpack emits no runtime bootstrap. The bundle throws `ReferenceError: __webpack_require__ is not defined`.

### Questions / Explanations

#### Why is `require()` the line, and not side effects?

A side-effecting module is fine to hoist. Its effects run at its position in the chunk, which is where `require_mod()` sits today, so nothing moves.

`require()` is different in kind. It is the only edge that can leave a module unevaluated. A `require()` inside a branch that never runs means the body never runs, and `__commonJSMin` implements exactly that. Hoisting turns that conditional run into an unconditional one. That shows up whenever the body does anything at all.

#### What happens to a module a wrapped module imports?

It keeps the wrapper, through the existing transitive rule. Consider a wrapped ESM module `A` that imports hoistable `B`. Hoisted, `B`'s body runs at the top of the chunk, before `A`'s wrapper is ever called — possibly before it is called at all. `B`'s bindings would be correct; `B`'s side effects would have moved. v1 does not take that trade.

## Drawbacks

The flag sorts these into three groups: what the code costs on landing, what a user pays by opting in, and what waits for the default flip. Only the first group is unavoidable.

_On landing, whatever the flag is set to:_

**The wrap decision stops being local.** Today it depends only on the syntax of an `(importer, importee, ImportKind)` triple, which is why it can run before symbol binding and tree shaking. Condition 3 adds a graph-wide input. The predicate stays a pure function of the module graph, computed once, but "syntax only" was a property worth having and this spends it.

**Two code paths to maintain.** Every downstream pass — finalizer, chunk linking, HMR, `preserveModules` — grows a second CommonJS shape to handle. Those branches exist whether or not anyone turns the flag on, so this cost arrives in full on day one and the flag does nothing to defer it.

_Only for builds that opt in:_

**Bugs are quiet.** A predicate bug does not fail the build. It emits a bundle that runs and is subtly wrong, which is the worst kind to debug. This is the drawback the flag exists for, and it bounds the exposure to people who asked for it.

_Deferred to the default flip:_

**Snapshot churn.** With the flag off, every existing snapshot stays as it is, and new coverage arrives as flag-on variants. The wide diff comes when `commonjs` becomes the default, and it lands across a large share of the Rust suite at once. Wide diffs hide regressions. The corpus is the answer — it asserts behaviour, not output shape — but that review still has to happen, just later.

## Rationale and alternatives

Why this design first, then why not each of the four others.

### Why this shape

Three properties argue for this design over the alternatives below. All three are about limiting risk rather than maximising the win.

**It adds no new output construct.** Hoisting only takes a wrapper away. There is no registry, no new module state, and no new runtime helper. The namespace case reuses `__exportAll`, and a hoisted export is a plain `var` declaration. The shape it emits is the one rolldown already emits for every ESM module, so every downstream pass has handled it since day one. That is why the work sits in the link stage instead of spreading across rendering, chunking, and HMR.

**It reuses analysis the bundler already trusts.** `SafelyTreeshakeCommonjs` and the facade symbols are not new and not speculative. They already decide whether a CommonJS module's exports can be tree-shaken. Their correctness is load-bearing in today's output. This RFC adds one graph condition and changes what is done with the answer.

**Its failure mode is bounded by construction.** Hoisting is a refusal to add a wrapper, never the removal of one, so every existing override still fires and still wins. A wrong predicate can leave a module wrapped that could have hoisted, or hoist one that should not have. It cannot undo a correctness fix that an earlier decision put in place.

The boundary is also drawn where the semantics actually change — at `require()`, the only edge that can leave a module unevaluated — instead of at a proxy for it. webpack's strict-mode gate is the counter-example. It is a cheap test standing in for a real condition. In rolldown's scope model it stands in for nothing at all.

### Do nothing: the minifier cannot cross the wrapper

The minifier cannot cross the wrapper. `import_src.a` is a property read on an object built by a memoized closure. Proving it is always `1` takes three proofs at once: the closure runs once, nothing else writes the object, and no getter is involved. The bundler knows all three at link time and the minifier does not. The `import * as ns` example above is the proof: rolldown folded `a` to `1` and still could not drop the module.

### Hoist everything: eager evaluation changes behaviour

This is a real change in program behaviour, not a size trade. A `require()` inside a branch is how CommonJS spells an optional dependency. Run it every time, and it can throw on a platform where the module does not load. The lazy wrapper exists for this.

### Run the wrapper eagerly: keeps the costs, loses the semantics

`var ns = (() => { … })()` removes the laziness but keeps the object, so property access, interop, and the tree-shaking barrier all stay. It gives up the semantics that make the wrapper worth having and keeps the costs that make it expensive.

### Copy webpack's three states: rolldown has no registry

webpack has hoisted, wrapped, and bailed, where bailed means "left as a registry module". Rolldown has no registry: a bailed module still has to end up in the chunk, so the wrapper _is_ rolldown's bail. Two states is the whole design space here, which is why the corpus's target column is not a copy of the webpack column.

## Prior art

### webpack

CommonJS module concatenation shipped in 5.109.0 behind `optimization.concatenateModules: { commonjs: true }` ([#21417](https://github.com/webpack/webpack/pull/21417), [#21436](https://github.com/webpack/webpack/pull/21436), [#21464](https://github.com/webpack/webpack/pull/21464)). The conditions live in `ModuleConcatenationPlugin.js` (graph admission) and `JavascriptGenerator.js` (`getCommonJsConcatenationBailoutReason`, `isCommonJsHoistable`).

Two places are worth copying. One is the shape of the analysis. The other is `Dependency.canConcatenate()`, which returns false for CommonJS dependencies. That is how webpack enforces condition 3 by structure instead of by a check.

One place not to copy: the strict-mode gate, [as above](#no-strict-mode-gate).

One place where rolldown is already ahead. webpack refuses to even wrap a module that calls `require()`, because a wrapper renders with module ids intact. Rolldown handles this today. The corpus keeps `bail/wrapped-plus-require` so the difference stays on the record.

### esbuild

esbuild wraps every CommonJS module in `__commonJS`. Rolldown's `__commonJSMin` is the same design, inherited. There is no hoisting to compare against.

### Rollup

Rollup has no CommonJS support in core, and `@rollup/plugin-commonjs` does not hoist. Built on the module above, it emits a lazy memoized wrapper — `var mod = {}; function requireMod() { … }` behind a `hasRequiredMod` guard — and the importer reads `modExports.a`. It resolves the named import, so `import { a }` compiles, but the output is structurally rolldown's `__commonJSMin`, not webpack's bindings.

So the plugin is in the wrapper class, not the hoisting class. webpack is the only prior implementation of what this RFC proposes. It also removes a tempting argument. The plugin's wide adoption says nothing about whether hoisting is safe, because the plugin does not hoist.

## Unresolved questions

1. **Default-only consumers.** Should a module whose consumers only want `default` stay wrapped? Answering it means letting usage inform the wrap decision, which no other wrap rule does today.
2. **Repeated writes.** Relax `named_exports` to carry the first write, or leave repeated-write modules wrapped in v1? The corpus wants them hoisted; the export table currently drops them.
3. **When does the flag flip?** What is the bar for making `commonjs` the default — the 11 gap rows green, or a real application bundling and running unchanged? And does the flag stay afterwards as a permanent escape hatch, or get removed like any other experiment?
4. **`preserveModules` and `format: cjs`.** A hoisted module still has to re-export correctly when the output format is CommonJS. Is there a shape where the synthesized namespace and the output-format export differ?
5. **Binding names.** `mod_a` reads well in a bundle; the deconflicting suffix form (`a$1`) is shorter. Which one, and does it matter after minification?
6. **The namespace tag.** Rolldown's CommonJS namespace reports `[object Object]` where Node reports `[object Module]`, because `__toESM` sets no `Symbol.toStringTag`. Hoisting must reproduce that, or it becomes observable. Is the divergence worth fixing on its own, for wrapped and hoisted modules at once? webpack went the other way and tags both objects, so its `default` claims to be a namespace too.

## Future work

**Hoist inside wrapped subtrees.** Condition 4 is conservative. A module whose body has no side effects beyond its own export writes could hoist even when a wrapped module imports it. There is nothing left to move. That needs a real side-effect judgement, which the current predicate avoids on purpose.

**`module.exports = { … }` with an object literal.** Common in the wild, and currently a hard bail because the object identity is the export. When every consumer reads named properties, each could become a plain binding. webpack does not do this.

**Per-export tree shaking.** Once exports are plain `var`s, an unused export is dead code and existing tree shaking removes it. This falls out of the change rather than needing work, and `hoist/unused-export-shaken` is the case that proves it.

**Constant propagation across the boundary.** `constant_export_map` already folds constant CommonJS exports at the import site. Hoisting extends the same reach to non-constant values, for free, by making them ordinary bindings.
