# RFC: Hoisting statically analyzable CommonJS modules

- Feature Name: `commonjs_hoisting`
- Start Date: 2026-08-06
- RFC PR: TBD
- Tracking Issue: [rolldown#10483](https://github.com/rolldown/rolldown/issues/10483)

Code references point at `rolldown@79cd87fe8`. The quoted bundler output is real output from `rolldown@1.2.0` and `webpack@5.109.0`, reformatted to this document's style.

## Table of contents

- [Summary](#summary)
- [Motivation](#motivation)
  - [Rolldown has one answer for every CommonJS module](#rolldown-has-one-answer-for-every-commonjs-module)
  - [The cost of the wrapper](#the-cost-of-the-wrapper)
  - [The analysis is already there](#the-analysis-is-already-there)
- [Guide-level explanation](#guide-level-explanation)
  - [The shape of a hoisted module](#the-shape-of-a-hoisted-module)
  - [When a module hoists](#when-a-module-hoists)
  - [What keeps its wrapper](#what-keeps-its-wrapper)
  - [No strict-mode gate](#no-strict-mode-gate)
  - [The option](#the-option)
- [Reference-level explanation](#reference-level-explanation)
  - [How rolldown decides the wrapper today](#how-rolldown-decides-the-wrapper-today)
  - [The hoistable predicate](#the-hoistable-predicate)
  - [Binding: named imports reach the facade symbols](#binding-named-imports-reach-the-facade-symbols)
  - [Render: declare once, assign in place](#render-declare-once-assign-in-place)
  - [Namespaces and default interop](#namespaces-and-default-interop)
  - [Strict execution order](#strict-execution-order)
  - [What hoisting must not change](#what-hoisting-must-not-change)
  - [The spec corpus](#the-spec-corpus)
  - [Common questions](#common-questions)
    - [Why does `require()` set the boundary, and not side effects?](#why-does-require-set-the-boundary-and-not-side-effects)
    - [What happens to a module a wrapped module imports?](#what-happens-to-a-module-a-wrapped-module-imports)
- [Drawbacks](#drawbacks)
- [Rationale and alternatives](#rationale-and-alternatives)
  - [Why this shape](#why-this-shape)
  - [Do nothing: the minifier cannot cross the wrapper](#do-nothing-the-minifier-cannot-cross-the-wrapper)
  - [Hoist everything: eager evaluation changes behaviour](#hoist-everything-eager-evaluation-changes-behaviour)
  - [Run the wrapper eagerly: this keeps the cost and loses the semantics](#run-the-wrapper-eagerly-this-keeps-the-cost-and-loses-the-semantics)
  - [Copy webpack's three states: rolldown has no registry](#copy-webpacks-three-states-rolldown-has-no-registry)
- [Prior art](#prior-art)
  - [webpack](#webpack)
  - [esbuild](#esbuild)
  - [Rollup](#rollup)
- [Unresolved questions](#unresolved-questions)
- [Future work](#future-work)

## Summary

Rolldown puts every CommonJS module behind a lazy `__commonJSMin` wrapper. This RFC removes the wrapper where it gives no benefit. A module qualifies when it passes a predicate of four conditions ([the full list](#when-a-module-hoists)). Two of them carry the most weight. Every export write is static, and nothing reaches the module through `require()`. Each export of a qualified module becomes a plain top-level binding in the chunk.

The feature ships behind `experimental.onDemandWrapping: { commonjs: true }`. It adds no new output format, and it accepts no new inputs. It changes the output for code that already bundles today.

## Motivation

### Rolldown has one answer for every CommonJS module

Take the smallest possible example:

```js
// mod.cjs
'use strict';
exports.a = () => 1;
exports.b = () => 2;

// entry.mjs
import { a, b } from './mod.cjs';
console.log(JSON.stringify([a(), b()]));
```

Rolldown 1.2.0 emits:

```js
var __commonJSMin = (cb, mod) => () => (
  mod || (cb((mod = { exports: {} }).exports, mod), (cb = null)),
  mod.exports
);

var require_src = /* @__PURE__ */ __commonJSMin((exports) => {
  exports.a = () => 1;
  exports.b = () => 2;
});

var import_src = require_src();
console.log(JSON.stringify([(0, import_src.a)(), (0, import_src.b)()]));
```

webpack 5.109 emits:

```js
var __WEBPACK_CJS_EXPORT_a__;
var __WEBPACK_CJS_EXPORT_b__;

__WEBPACK_CJS_EXPORT_a__ = () => 1;
__WEBPACK_CJS_EXPORT_b__ = () => 2;

console.log(JSON.stringify([__WEBPACK_CJS_EXPORT_a__(), __WEBPACK_CJS_EXPORT_b__()]));
```

Nothing about the module above is difficult to analyze. Rolldown wraps it anyway. The reason is not the module. Rolldown has exactly one lowering for CommonJS, and that lowering is the wrapper.

The rule reads one field. Rolldown wraps every module whose `exports_kind` is `CommonJs`. Nothing in the module body changes that answer. Rolldown wraps a 3-line constants file. Rolldown wraps a module that reassigns `module.exports` inside a conditional. Both get the same wrapper.

Most of `node_modules` is still CommonJS. So the wrapper is the normal outcome, not a rare one.

### The cost of the wrapper

**Nothing crosses the wrapper.** After a value becomes a property of `import_src`, every read is a property lookup. The minifier cannot analyze that object. A call to `a` becomes `(0, import_src.a)()`, which keeps `this` undefined. The minifier cannot inline a call, fold a constant, or remove a dead export.

**Every ESM importer pays interop.** Keep `mod.cjs` exactly as it is. Change only the entry to a namespace import:

```js
// entry.mjs
import * as ns from './mod.cjs';
console.log(JSON.stringify([ns.a(), ns.b()]));
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
  exports.a = () => 1;
  exports.b = () => 2;
});

var import_src = /* @__PURE__ */ __toESM(require_src(), 1);
console.log(JSON.stringify([import_src.a(), import_src.b()]));
```

Count what the namespace import added. Nine runtime helpers entered the chunk: six `Object.*` aliases, `__commonJSMin`, `__copyProps`, and `__toESM`. At run time, `__toESM` builds a second object, and `__copyProps` copies every export onto it as a getter. All of that exists so the entry can call two functions on an object rolldown built itself. And rolldown analyzed the module completely at build time. So the missing piece is not analysis, but an output shape for the result.

**The wrapper spreads.** Under `experimental.onDemandWrapping`, `eagerly_triggers_interop_module` marks a module as sensitive to execution order when it statically imports a wrapped module. Rolldown then wraps that module in `__esm` as well. One CommonJS leaf can pull its whole importer subtree into wrappers. When the leaf hoists, it triggers nothing, and its importers stay unwrapped.

### The analysis is already there

Rolldown needs no new analysis to know which modules are safe. It computes the answer today and then discards it:

- Rolldown sets `EcmaViewMeta::SafelyTreeshakeCommonjs` under two conditions. Every export write is a static property assignment, and nothing reads the exports object in an unknown way (`ecma_module_view_factory.rs:133-139`).
- The scanner creates a facade symbol for each `exports.x = …` write and records it as a `LocalExport` (`ast_scanner/impl_visit.rs:249-267`).
- The facade symbols are already in the module's `named_exports`, if the module writes each name exactly once (`ecma_module_view_factory.rs:96-99`).

So the export table already holds the symbols that a hoisted module binds to, with their names. The import binder never reads them. It short-circuits on `exports_kind` first.

## Guide-level explanation

### The shape of a hoisted module

A hoisted module has no wrapper and no exports object. Each export becomes a top-level binding in the chunk. Importers read that binding directly.

```js
// mod.cjs, hoisted
var mod_a = () => 1;
var mod_b = () => 2;

// entry.mjs
console.log(JSON.stringify([mod_a(), mod_b()]));
```

The names follow one pattern: module name, underscore, export name. So `mod_a` is export `a` of `mod.cjs`, deconflicted like any other binding in the chunk. This document uses the pattern throughout. The final scheme is [unresolved question 5](#unresolved-questions).

This block collapses each declaration and its write into one statement, to keep the example short. [Render](#render-declare-once-assign-in-place) gives the exact lowering. Rolldown already emits this shape for every ESM module. After hoisting, a CommonJS module is an ordinary module in the chunk. Every later benefit comes from that one fact. The minifier can see into the module, tree shaking removes its unused exports, and the order analysis treats it like any other module.

### When a module hoists

A CommonJS module hoists when all four conditions hold.

**1. Every export write is static.** The base is exactly `exports` or `module.exports`. The property name is a plain identifier. `exports[key] = …` does not qualify.

**2. The module never treats its exports as an object.** The module must not do any of these:

- reassign `module.exports`;
- call `Object.defineProperty(exports, …)`;
- give `exports` a local alias;
- set the `__esModule` flag;
- read `module.id` or `module.loaded`.

Each one makes the object itself observable. A set of separate bindings cannot replace it. Conditions 1 and 2 together are the meaning of `SafelyTreeshakeCommonjs` today.

**3. Nothing reaches the module through `require()`.** This condition matters most. `__commonJSMin` is lazy and memoized. The body runs on the first `require_mod()` call and never again. Hoisting makes that evaluation eager, at the module's position in the chunk.

For a module that only static `import` reaches, eager evaluation changes nothing. Rolldown already places the `require_mod()` call at the module's own position in the chunk, directly after the wrapper definition. The call does not sit at the import site. So when a side-effecting module comes between the two, the CommonJS body still runs first, and the run order matches node. Hoisting runs the body at the same position, without the call.

For a module that a `require()` reaches, eager evaluation is a real change of semantics. Those modules keep the wrapper. The same rule excludes CommonJS entries and members of a `require` cycle.

**4. No wrapped module imports it.** A wrapped module defers its body. Everything it imports must still be ready when that body runs. Today that means rolldown wraps the imported module as well. Inside a deferred subtree, hoisting would move a body's side effects ahead of the wrapper that guards them. So v1 leaves that subtree alone (see [Future work](#future-work)).

### What keeps its wrapper

Every other shape keeps its wrapper, and the list is not short. These rows come from the corpus:

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

The corpus holds a case for each row, with a verdict that webpack's own output confirms. So a test checks the boundary. The reader does not have to trust this document.

### No strict-mode gate

webpack bails on any module without `"use strict"`. That is its largest class of bailout, because most of `node_modules` is sloppy mode. The gate exists because webpack merges module bodies into one shared strict scope. A sloppy body in that scope would change meaning without a warning.

Rolldown does not have that problem. Hoisting moves a body out of an arrow function, up to the top level of the same scope. The strictness before the move is the strictness after it. So rolldown hoists sloppy modules. The corpus records one case where rolldown's target verdict is better than webpack's real verdict (`bail/sloppy-mode`).

### The option

Hoisting ships behind an experimental option. It widens the existing `onDemandWrapping` boolean. `inlineConst` and `chunkImportMap` already use the same shape:

```ts
experimental: {
  onDemandWrapping?: boolean | { commonjs?: boolean };
}
```

`onDemandWrapping: true` keeps its current meaning. `{ commonjs: true }` also lets rolldown hoist every CommonJS module that passes the predicate.

Where hoisting is safe, it is a pure improvement. The option is still necessary, because of how hoisting fails. A bug in the predicate does not break the build. It emits a bundle that runs and is wrong, without a warning. The wrong code is code the user did not write, somewhere in `node_modules`.

An experimental feature that fails this way needs a switch to turn it off. That switch is also what a user bisects against after a bug report.

The name `onDemandWrapping` raises one point that this RFC must settle. Today the docs define `onDemandWrapping` "under `output.strictExecutionOrder`". `is_strict_on_demand_wrapping_enabled()` returns false without it. The main benefits of hoisting have no relation to strict execution order. They are a smaller output, tree-shakeable exports, and no interop helpers. A gate on `strictExecutionOrder` would hide the feature from most users.

So **rolldown reads the `commonjs` option on its own, whatever the value of `strictExecutionOrder`.** The boolean form keeps its existing gate. The two halves of the option differ here, so the docs for the option must say so.

The expected end state is on by default. Two things must happen first. Every corpus case must pass, and real builds must use the option for some time. [Unresolved questions](#unresolved-questions) asks whether the option stays after that.

## Reference-level explanation

### How rolldown decides the wrapper today

Two rules make CommonJS mean "wrapper". Three overrides sit on top of them.

`determine_module_exports_kind.rs:97-106` — rolldown sets every CommonJS module that is not an entry to `WrapKind::Cjs`. A CommonJS entry also gets `WrapKind::Cjs` when one of these holds:

- the output format is `esm`;
- the format is `iife` or `umd`, and the module refers to `module` or `exports`.

`wrapping.rs:126-138` — rolldown walks the import records of an unwrapped module. It wraps any importee whose `exports_kind` is `CommonJs`. `wrap_module_recursively` repeats this through the whole subtree.

The overrides:

- `determine_module_exports_kind.rs:50-56` — a `require()` edge wraps its importee. This override always applies. The predicate below never contradicts it.
- `determine_module_exports_kind.rs:66-79` — with code splitting disabled, `import()` behaves like `require()`. So rolldown wraps its importee.
- `wrapping.rs:155-166` — under strict execution order with manual code splitting groups, rolldown forces every CommonJS module back to `WrapKind::Cjs` ([#10405](https://github.com/rolldown/rolldown/pull/10405)).

`create_wrapper` (`wrapping.rs:201-223`) then creates the `require_<name>` symbol and the `__commonJSMin` statement for every module marked `WrapKind::Cjs`.

`set_wrap_kind` keeps the last write, so the order of these rules matters (see `internal-docs/linking/determine-module-exports-kind/implementation.md`).

### The hoistable predicate

The predicate answers one question per CommonJS module: "may this module skip the default wrapper?" It reads:

- The `commonjs` option. When it is off, the predicate answers no for every module, and nothing after it changes.
- `EcmaViewMeta::SafelyTreeshakeCommonjs` — conditions 1 and 2. The scanner computes this already.
- The import records of every module in the graph — condition 3. Three things disqualify a module: an incoming `ImportKind::Require` record, entry status, or membership of a `require` cycle. `LinkingMetadata::required_by_other_module` already carries part of this signal.

Rolldown knows the whole module graph before the link stage runs. So the predicate is one pass over the module table. It runs once, before rolldown assigns any `WrapKind`.

One rule keeps this safe: **hoisting refuses to add a wrapper. It never removes one.** The link stage reads the predicate only at the two default rules above. Every override still runs and still wins, including the forced wrapper under strict execution order. This change cannot undo an existing correctness fix. Condition 4 then needs no work: `wrap_module_recursively` still wraps everything a wrapped module reaches, and the predicate does not oppose it.

### Binding: named imports reach the facade symbols

`bind_imports_and_exports.rs:1250-1252` returns `ImportStatus::CommonJS` for any importee whose `exports_kind` is `CommonJs`. It returns before it reads `resolved_exports`. The caller at line 1377 then routes the import through the namespace object.

For a hoistable module, rolldown skips that short-circuit. The import then resolves through `resolved_exports`, like an ESM import. The facade symbols are already there. Two gaps must close first:

- `module.exports.x = …` creates no facade symbol. The scanner's `StaticMemberExpression` branch (`impl_visit.rs:271-282`) records the `module` identifier and nothing else. A module in that style has no symbols to bind to. Corpus case: `hoist/module-exports-prop`.
- `named_exports` drops any export that the module writes more than once (`ecma_module_view_factory.rs:96-99` keeps only `v.len() == 1`). After hoisting, a repeated write is an ordinary reassignment. The table should carry the first write, and the rest become assignments. Corpus case: `hoist/repeated-write`.

### Render: declare once, assign in place

One rule handles every write shape:

- Emit `var <facade>;` once, at the module's position in the chunk.
- Rewrite every `exports.x = v` in place to `<facade> = v`.
- Rewrite every read of `exports.x` inside the module to `<facade>`.

JavaScript lifts a `var` declaration to the top of its scope. That language rule, and not this RFC's transformation, does the rest. A conditional write leaves the binding `undefined` until it runs. The wrapper's exports object behaves the same: a read before the write gives `undefined`. A write inside a function body needs no special case. A repeated write is a reassignment.

webpack emits the same shape. That is a useful check on a lowering with several possible forms:

```js
var __WEBPACK_CJS_EXPORT_a__;
__WEBPACK_CJS_EXPORT_a__ = 1;
```

Rolldown may collapse `var x; x = 1;` into `var x = 1` when the write is unconditional and comes first. That is a codegen improvement, not part of the contract.

### Namespaces and default interop

Hoisting removes the exports object. Two import forms still need one:

- `import * as ns from "./mod.cjs"` needs a namespace.
- `import m from "./mod.cjs"` needs `default`, which for a CommonJS module is `module.exports` itself.

By itself, neither form puts an object into the output. Rolldown already resolves a static member read on an ESM namespace directly to the binding. It emits no object. `import * as ns` followed by `ns.a` bundles to a plain read of the `a` binding, and hoisted CommonJS gets the same treatment. An object appears only when the namespace escapes. A namespace escapes when code passes it to a function, spreads it, or indexes it dynamically.

An escaping namespace needs no new runtime helper. `__exportAll` builds a new object of live, enumerable getters from a map of name to thunk. Rolldown already emits it for an escaping ESM namespace (`module_finalizers/mod.rs:986-993`).

Take the running example, with an entry that lets both objects escape:

```js
// mod.cjs, unchanged
'use strict';
exports.a = () => 1;
exports.b = () => 2;

// entry.mjs
import * as ns from './mod.cjs';
import m from './mod.cjs';
send(ns, m);
```

The block below shows the hoisted output that this RFC proposes. Unlike the earlier blocks, it is a design sketch, not built output:

```js
var mod_a;
var mod_b;
var mod_exports = /* @__PURE__ */ __exportAll({ a: () => mod_a, b: () => mod_b }, true);
var mod_ns = /* @__PURE__ */ __toESM(mod_exports, 1);
mod_a = () => 1;
mod_b = () => 2;

send(mod_ns, mod_exports);
```

Rolldown emits the getter map once. It derives the namespace from the exports object, and emits no second copy. So the chunk holds one getter per export, not two. webpack does the same. It writes the map into a base object, then calls `__webpack_require__.t(base, 2)` to get the namespace.

The `__toESM` call gives more than a shorter output. It is the same call that the importer of a wrapped module makes today, over an object of the same shape. So it returns the namespace that rolldown already emits. The keys are the same, in the same order. Neither object has a `Symbol.toStringTag`. The prototype is the same, and `default` holds the same object.

That is the strongest available guarantee that a user cannot observe hoisting. The two namespaces do not merely agree. They come off the same code path.

Two objects sit over one set of bindings. That is the part to get right. `ns` is the namespace and carries a `default` key. `default` holds `module.exports`. webpack emits the same split into two objects for this example, and its `default` points at the base object. So the shape is not speculative.

Neither object gets the tag, and that needs saying, because Node behaves differently. In Node a namespace carries `Symbol.toStringTag: 'Module'`, and `module.exports` does not. Rolldown builds a CommonJS namespace through `__toESM`, which never sets the tag. So `Object.prototype.toString.call(ns)` already returns `[object Object]` for a wrapped module. The sketch keeps that behaviour: the `__exportAll` call passes `no_symbols: true`, and `__toESM` adds nothing.

A tag on the hoisted object would make hoisting observable, and hoisting must never be observable. Node's behaviour is worth matching on its own, for wrapped and hoisted modules together — see [Unresolved questions](#unresolved-questions).

The map holds getters, not a snapshot. A hoisted module may still assign `exports.a` after evaluation, from a callback or a timer. The wrapper's object shows such a write today. The map holds thunks and not values, so both calls can be above the writes. That is where rolldown already puts them for ESM.

A named import needs none of this. The import resolves to the binding. Rolldown emits no object, and no helper enters the chunk. Today every CommonJS import pays the interop cost. After this change, only an escaping namespace pays it.

A module whose namespace escapes gains the least. It exchanges the `__commonJSMin` closure for an `__exportAll` map, and it keeps the `__toESM` call it already had. So the object returns and the helpers stay. The result is not worse than today, and the minifier can still reach the bindings under the object. But most of the benefit is gone.

A static read pays nothing. So the benefit depends on how code uses a module, not on what the module exports. The predicate could read usage and leave an escaping module wrapped. But that ties a link-stage decision to usage, which the wrapper decision avoids today on purpose. [Unresolved questions](#unresolved-questions) leaves this open.

### Strict execution order

Two changes in `generate_stage/order_analysis.rs`:

- `is_order_wrap_eligible` (line 1194) requires `ExportsKind::Esm | None`, so on-demand wrapping cannot see CommonJS. A hoisted module is an ordinary eager module, so the check should admit it.
- `eagerly_triggers_interop_module` (line 1168) marks the importer of a wrapped module as order-sensitive. A hoisted module has no wrapper. So it stops triggering the mark, and rolldown stops wrapping its importers for that reason.

Both code paths run only for a module that hoisted. So `{ commonjs: true }` controls them, and no second check is necessary. This half of the feature justifies the name of the option. With the option on, on-demand wrapping is no longer ESM-only.

The forced wrapper from #10405 stays until both changes land with snapshot coverage. It is a correctness fix for a real failure. Nobody should relax it before tests prove the relaxation safe.

### What hoisting must not change

Hoisting must not change any of these:

- **Evaluation order and side effects.** A hoisted body runs at the module's position in the chunk. That is where its `require_mod()` call is today.
- **Export liveness.** An importer sees a later write to an export, through the binding directly or through a namespace getter.
- **Export identity.** `import * as ns` and default interop see the same values as before, with the same keys.
- **Tree shaking.** The removal of an unused export is a gain, not a change. Code outside the module could never observe the wrapper's exports object.
- **Sloppy-mode meaning.** The strictness of the enclosing scope does not change. That is what makes [the missing strict-mode gate](#no-strict-mode-gate) correct.

### The spec corpus

[`IWANABETHATGUY-reproduction/cjs-concat-spec`](https://github.com/IWANABETHATGUY-reproduction/cjs-concat-spec) holds 44 cases. They cover every hoist, wrap, and bail condition in webpack 5.109. The conditions come from `ModuleConcatenationPlugin.js` and `JavascriptGenerator.js`, and a build of each case confirms them. The corpus builds each case three ways: webpack with concatenation off, webpack with concatenation on, and rolldown. Each case asserts two things:

- the webpack verdict, as spec;
- **identical runtime output** from all three builds. A bundler may hoist, wrap, or bail as it chooses. The program must do the same thing under each build.

The corpus reports rolldown's verdict against a `target` column, and does not assert it. So the rows where verdict and target disagree are the work list. Today the work list is 11 rows. They are the acceptance criteria for this RFC:

| case                         | proves                                                        |
| ---------------------------- | ------------------------------------------------------------- |
| `hoist/plain-exports`        | `exports.a = 1` becomes a plain binding                       |
| `hoist/module-exports-prop`  | `module.exports.x =` hoists too                               |
| `hoist/repeated-write`       | a repeated write is a reassignment                            |
| `hoist/conditional-export`   | a conditional write stays `undefined` until it runs           |
| `hoist/unused-export-shaken` | an unused export is a dead `var`, and tree shaking removes it |
| `hoist/nested-member-write`  | `exports.obj.x =` does not block hoisting                     |
| `hoist/calls-require`        | a hoisted module may call `require()` itself                  |
| `hoist/dynamic-import`       | hoisting works together with `import()` elsewhere             |
| `hoist/namespace-import`     | `import * as ns` gets a synthesized namespace                 |
| `hoist/two-modules`          | two modules hoist into one scope without collisions           |
| `bail/sloppy-mode`           | no strict-mode gate                                           |

The other 33 cases protect what already works. They include 15 `wrap/*` shapes, plus `bail/require-target`, `bail/cjs-cycle`, and `bail/cjs-entry`. All 33 must keep their current verdicts. The corpus exports each case to the directory shape that rolldown's suite uses. So this work includes a port of the cases into `crates/rolldown/tests/rolldown/function/`.

The corpus found one rolldown bug. The bug has no relation to hoisting, and its fix can ship on its own. Rolldown rewrites a top-level `this.a = 1` to `exports.a = 1`. But it emits the `__commonJSMin` callback with an empty parameter list. So `exports` is a free variable, and an ESM bundle throws `ReferenceError: exports is not defined in ES module scope`:

```js
var require_src = /* @__PURE__ */ __commonJSMin(() => {
  exports.a = 1; // free variable
});
```

The corpus also found a webpack 5.109.0 bug. Remember it when you compare output. If the only wrapped CommonJS module in a bundle never refers to `module`, webpack emits no runtime bootstrap. The bundle then throws `ReferenceError: __webpack_require__ is not defined`.

### Common questions

#### Why does `require()` set the boundary, and not side effects?

A module with side effects is safe to hoist. Its effects run at its position in the chunk. That is where the `require_mod()` call is today, so nothing moves.

`require()` is different. It is the only edge that can leave a module unevaluated. When a branch that holds a `require()` never runs, the body never runs. `__commonJSMin` implements exactly that. Hoisting makes that conditional evaluation unconditional. A user sees the difference whenever the body does anything at all.

#### What happens to a module a wrapped module imports?

It keeps the wrapper, through the existing transitive rule. Take a wrapped ESM module `A` that imports a hoistable module `B`. If `B` hoisted, `B`'s body would run at `B`'s position in the chunk, during the chunk's own evaluation. `A`'s wrapper runs later, on its first call, or never.

`B`'s bindings would still be correct. But `B`'s side effects would move. v1 does not accept that trade.

## Drawbacks

The drawbacks fall into three groups, sorted by their relation to the option. The first group arrives when the code lands. The second is what a user pays to turn the option on. The third waits until the option becomes the default. Only the first group is unavoidable.

**When the code lands, whatever the value of the option:**

**The wrapper decision stops being local.** Today it depends only on the syntax of an `(importer, importee, ImportKind)` triple. That is why it can run before symbol binding and tree shaking. Condition 3 adds an input from the whole graph. The predicate stays a pure function of the module graph, and it runs once. But "syntax only" was a property worth having, and this change gives it up.

**Two code paths to maintain.** Every later pass gets a second CommonJS shape to handle: the finalizer, chunk linking, HMR, and `preserveModules`. Those branches exist whether or not a user turns the option on. So this cost arrives in full on the first day, and the option cannot defer it.

**Only for a build that turns the option on:**

**A bug is quiet.** A bug in the predicate does not fail the build. It emits a bundle that runs and is wrong in a small way, which is the hardest kind to debug. The option exists for this drawback. It limits the exposure to the users who asked for the feature.

**Deferred until the option becomes the default:**

**Snapshot churn.** With the option off, every existing snapshot stays as it is. New coverage arrives as variants with the option on.

The wide diff comes when `commonjs` becomes the default, and it lands across a large part of the Rust suite at once. A wide diff hides a regression. The corpus is the answer, because it asserts behaviour and not output shape. But somebody must still review that diff. The option only delays the review.

## Rationale and alternatives

This section gives the reason for this design first. It then gives the reason against each of the four alternatives.

### Why this shape

Three properties make this design better than the alternatives below. All three limit risk. None of them maximises the benefit.

**It adds no new output construct.** Hoisting only removes a wrapper. It adds no registry, no new module state, and no new runtime helper. An escaping namespace reuses `__exportAll`, and a hoisted export is a plain `var` declaration. Rolldown already emits this shape for every ESM module, so every later pass handles it today. That is why the work is in the link stage, and not spread across rendering, chunking, and HMR.

**It reuses analysis that the bundler already trusts.** `SafelyTreeshakeCommonjs` and the facade symbols are not new, and not speculative. They already decide whether tree shaking may remove a CommonJS module's exports. Today's output depends on their correctness. This RFC adds one graph condition, and changes what rolldown does with the answer.

**The design bounds its own failure mode.** Hoisting refuses to add a wrapper, and it never removes one. So every existing override still runs and still wins. A wrong predicate can keep the wrapper on a module that could hoist. It can also hoist a module that needs its wrapper. It cannot undo a correctness fix that an earlier decision put in place.

The boundary is also at the point where the semantics change, and not at a substitute for that point. That point is `require()`, the only edge that can leave a module unevaluated. webpack's strict-mode gate is the counter-example. It is a cheap test that stands in for a real condition. In rolldown's scope model it stands in for nothing.

### Do nothing: the minifier cannot cross the wrapper

The minifier cannot cross the wrapper. `import_src.a` is a property read on an object that a memoized closure builds. To inline the function behind it, the minifier must prove three things at once:

- the closure runs once;
- nothing else writes the object;
- no getter is involved.

The bundler knows all three at link time. The minifier does not.

### Hoist everything: eager evaluation changes behaviour

This alternative is a real change in program behaviour, not a trade against size. A `require()` inside a branch is how CommonJS writes an optional dependency. Run that `require()` every time, and the program can throw on a platform where the module does not load. The lazy wrapper exists exactly for this.

### Run the wrapper eagerly: this keeps the cost and loses the semantics

`var ns = (() => { … })()` removes the laziness, but it keeps the object. So the property access, the interop, and the tree-shaking barrier all stay. This alternative gives up the semantics that make the wrapper valuable. It keeps the cost that makes the wrapper expensive.

### Copy webpack's three states: rolldown has no registry

webpack has three states: hoisted, wrapped, and bailed. "Bailed" means "left as a registry module". Rolldown has no registry. A module that bails must still reach the chunk, so the wrapper is rolldown's bailout. Two states are the whole design space here. That is why the `target` column of the corpus is not a copy of the webpack column.

## Prior art

### webpack

webpack shipped CommonJS module concatenation in 5.109.0, behind `optimization.concatenateModules: { commonjs: true }` ([#21417](https://github.com/webpack/webpack/pull/21417), [#21436](https://github.com/webpack/webpack/pull/21436), [#21464](https://github.com/webpack/webpack/pull/21464)). The conditions are in `ModuleConcatenationPlugin.js` (graph admission) and in `JavascriptGenerator.js` (`getCommonJsConcatenationBailoutReason`, `isCommonJsHoistable`).

Two things are worth copying. The first is the shape of the analysis. The second is `Dependency.canConcatenate()`, which returns false for a CommonJS dependency. It makes condition 3 a property of the dependency type, not a check that runs later.

One thing not to copy: the strict-mode gate, [as above](#no-strict-mode-gate).

In one place rolldown is already ahead. webpack refuses to wrap a module that calls `require()`, because a wrapper renders with its module ids intact. Rolldown handles this today. The corpus keeps `bail/wrapped-plus-require`, so the difference stays on the record.

### esbuild

esbuild wraps every CommonJS module in `__commonJS`. Rolldown inherited the same design in `__commonJSMin`. esbuild has no hoisting to compare against.

### Rollup

Rollup core has no CommonJS support, and `@rollup/plugin-commonjs` does not hoist. For a module of this shape, the plugin emits a lazy memoized wrapper: `var mod = {}; function requireMod() { … }`, behind a `hasRequiredMod` guard. The importer then reads `modExports.a`. The plugin resolves the named import, so `import { a }` compiles. But the output has the structure of rolldown's `__commonJSMin`, not of webpack's bindings.

So the plugin belongs to the wrapper class, not to the hoisting class. webpack is the only earlier implementation of what this RFC proposes. That fact also removes an attractive argument. The wide adoption of the plugin says nothing about the safety of hoisting, because the plugin does not hoist.

## Unresolved questions

1. **Importers that only want `default`.** Should a module stay wrapped when every importer only wants `default`? To answer that, usage must inform the wrapper decision. No other wrapper rule reads usage today.
2. **Repeated writes.** Should `named_exports` carry the first write? Or should v1 leave a module with a repeated write wrapped? The corpus wants those modules hoisted. The export table drops them today.
3. **When does `commonjs` become the default?** What is the condition — all 11 gap rows pass, or a real application bundles and runs without a change? And does the option stay afterwards as a permanent switch, or does it go away like any other experiment?
4. **`preserveModules` and `format: cjs`.** A hoisted module must still re-export correctly when the output format is CommonJS. Is there a shape where the synthesized namespace and the output format's exports differ?
5. **Binding names.** `mod_a` is easy to read in a bundle. The deconflicting suffix form (`a$1`) is shorter. Which one does rolldown use? And does the choice matter after minification?
6. **The namespace tag.** Rolldown's CommonJS namespace reports `[object Object]`, where Node reports `[object Module]`. The cause is `__toESM`, which sets no `Symbol.toStringTag`. Hoisting must reproduce that, or a user can observe it. Is the divergence worth a fix on its own, for wrapped and hoisted modules together? webpack made the opposite choice and tags both objects, so its `default` also claims to be a namespace.

## Future work

**Hoist inside a wrapped subtree.** Condition 4 is conservative. Take a module whose body has no side effects except its own export writes. It could hoist even when a wrapped module imports it, because nothing is left to move. That needs a real judgement about side effects, which the current predicate avoids on purpose.

**`module.exports = { … }` with an object literal.** This shape is common in real packages. Today it always bails, because the object identity is the export. When every importer reads named properties, each property could become a plain binding. webpack does not do this.

**Per-export tree shaking.** After exports become plain `var` declarations, an unused export is dead code, and the existing tree shaking removes it. This needs no extra work. `hoist/unused-export-shaken` is the case that proves it.

**Constant propagation across the boundary.** `constant_export_map` already folds a constant CommonJS export at the import site. Hoisting makes non-constant values ordinary bindings, so the same reach extends to them at no cost.
