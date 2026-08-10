# RFC: CommonJS hoisting and `onDemandWrapping.commonjs`

- Feature Name: `commonjs_hoisting`
- Start Date: 2026-08-06
- RFC PR: TBD
- Tracking Issue: [rolldown#10483](https://github.com/rolldown/rolldown/issues/10483)

Code references point at `rolldown@79cd87fe8`. The quoted bundler output is real output from `rolldown@1.2.0` and `webpack@5.109.0`, reformatted to this document's style.

## Table of contents

- [Summary](#summary)
- [Motivation](#motivation)
- [Guide-level explanation](#guide-level-explanation)
- [Reference-level explanation](#reference-level-explanation)
- [Drawbacks](#drawbacks)
- [Rationale and alternatives](#rationale-and-alternatives)
- [Prior art](#prior-art)
- [Unresolved questions](#unresolved-questions)
- [Future work](#future-work)

## Summary

This RFC contains two main parts:

- Unlock the ability to lower a CommonJS module into plain top-level bindings, with no wrapper and no exports object.
- The option `onDemandWrapping: { commonjs: true }` is built on that ability. It hoists every CommonJS module that passes a four-condition predicate.

## Motivation

### Every CommonJS module gets the wrapper

A bundler has two ways to put a CommonJS module into its output: keep the exports object behind a wrapper, or turn each export into a binding. For the given input:

```js
// mod.cjs
'use strict';
exports.a = () => 1;
exports.b = () => 2;

// entry.mjs
import { a, b } from './mod.cjs';
console.log(JSON.stringify([a(), b()]));
```

The wrapper approach, as rolldown 1.2.0 emits it:

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

The bindings approach, as webpack 5.109 emits it:

```js
var __WEBPACK_CJS_EXPORT_a__;
var __WEBPACK_CJS_EXPORT_b__;

__WEBPACK_CJS_EXPORT_a__ = () => 1;
__WEBPACK_CJS_EXPORT_b__ = () => 2;

console.log(JSON.stringify([__WEBPACK_CJS_EXPORT_a__(), __WEBPACK_CJS_EXPORT_b__()]));
```

The outputs are really different, for one input that neither bundler finds hard to analyze.

In short:

- The wrapper reproduces node's evaluation model, for every module, at the same cost for each one.
- The bindings give up that generality, and in exchange the module becomes ordinary code that every later pass can read.

This RFC aims to provide the bindings approach where it is safe, while keeping the wrapper for everything else.

A "wrapper" is the `__commonJSMin` closure that rolldown puts around a CommonJS body, plus the `require_<name>()` call that runs it. The closure is lazy and memoized. The body runs on the first call and never again.

Nothing about `mod.cjs` above is difficult to analyze. Rolldown wraps it anyway, because it has exactly one lowering for CommonJS: a 3-line constants file and a module that reassigns `module.exports` inside a conditional get the same wrapper.

### What the wrapper costs

The cost lands in three places.

**Nothing crosses the wrapper.** After a value becomes a property of `import_src`, every read is a property lookup. The minifier cannot analyze that object. A call to `a` becomes `(0, import_src.a)()`, which keeps `this` undefined. The minifier cannot inline a call, fold a constant, or remove a dead export.

**A namespace import builds a second object.** Keep `mod.cjs` exactly as it is. Change only the entry to a namespace import:

```js
// entry.mjs
import * as ns from './mod.cjs';
console.log(JSON.stringify([ns.a(), ns.b()]));
```

```js
/* … six Object.* aliases, __commonJSMin, __copyProps, __toESM … */

var require_src = /* @__PURE__ */ __commonJSMin((exports) => {
  exports.a = () => 1;
  exports.b = () => 2;
});

var import_src = /* @__PURE__ */ __toESM(require_src(), 1);
console.log(JSON.stringify([import_src.a(), import_src.b()]));
```

`__toESM` creates a fresh object, and `__copyProps` then defines one bound getter per export on it, with a descriptor read for each (`runtime-base.js:44-70`).

Rolldown does not pay this per importer. `determine_safely_merge_cjs_ns` merges the namespace bindings of every ESM importer of one CommonJS module inside a chunk, so two importers share one `import_src` (`determine_module_exports_kind.rs:110-142`, `code_splitting.rs:550-562`). The cost is one object plus one accessor per export, for each CommonJS module in each chunk. Under `strictExecutionOrder` the merge is skipped, and each importer gets its own call.

**The wrapper spreads, with no option set.** A wrapped module defers its body, so everything it imports must be ready when that body runs. `wrap_module_recursively` (`wrapping.rs:19-49`) wraps the whole import subtree of a wrapped module for that reason: an ESM importee gets `__esm`, a CommonJS importee gets `__commonJSMin`. So one `require()` of an ESM module puts that module behind a wrapper too, and adds two more helpers:

```js
var dep_exports = /* @__PURE__ */ __exportAll({ v: () => 1 });
var v;
var init_dep = __esmMin(() => {
  console.log('DEP EVALUATED');
  v = 1;
});

var require_src = /* @__PURE__ */ __commonJSMin((exports) => {
  const { v } = (init_dep(), __toCommonJS(dep_exports));
  exports.x = v + 1;
});
```

Under `experimental.onDemandWrapping` it also spreads the other way. `eagerly_triggers_interop_module` marks a module as sensitive to execution order when it statically imports a wrapped module, so rolldown wraps that importer in `__esm` as well. One CommonJS leaf can then pull its whole importer subtree into wrappers.

This isn't an issue for a module that genuinely needs the wrapper. It scales with how much CommonJS a build contains, and most of `node_modules` is still CommonJS, so the wrapper is the normal outcome rather than a rare one. A user cannot opt out either: the only way to avoid the wrapper today is to replace the dependency with an ESM build of it.

<details>
<summary>Related reports and prior implementations</summary>

- [Rolldown #10483](https://github.com/rolldown/rolldown/issues/10483) is the tracking issue. It reports that the wrapper blocks later passes from reaching a CommonJS module's exports.
- Webpack shipped the bindings approach in 5.109.0, across [#21417](https://github.com/webpack/webpack/pull/21417), [#21436](https://github.com/webpack/webpack/pull/21436), and [#21464](https://github.com/webpack/webpack/pull/21464). Its conditions are the spec that [the corpus](#the-spec-corpus) encodes.

</details>

### Why rolldown wraps

A CommonJS module decides its exports at run time. `exports` is a plain mutable object, and the body is arbitrary code that writes into it, so the only general way to learn what a module exports is to run it.

The wrapper is what runs it — once, on the first `require()`, in a scope where `module` and `exports` exist. `wrapping.rs:127` calls it "like a commonjs runtime to help initialize the commonjs module correctly". Laziness and memoization come with that job. A cycle sees a partly filled object instead of a crash, and a `require()` that never runs never evaluates the body.

### How rolldown wraps

The decision reads only the `(importer, importee, ImportKind)` triple, and never the module body. That is what lets it settle before symbol binding and tree shaking, which both need the answer (`internal-docs/linking/determine-module-exports-kind/implementation.md`).

Two rules then produce every wrapper rolldown emits:

1. A module's format decides its lowering. `exports_kind == CommonJs` means the wrapper.
2. Everything a wrapped module imports is wrapped as well.

**1.** is the direct cause. `determine_module_exports_kind` marks every CommonJS module that is not an entry `WrapKind::Cjs`.

**2.** is what carries the first rule past CommonJS. `wrap_module_recursively` walks a wrapped module's import records and wraps everything it reaches, `WrapKind::Esm` for an ESM importee and `WrapKind::Cjs` for a CommonJS one, because a deferred body must find its imports ready when it runs.

Three overrides sit on top of both rules, and each one always wins: a `require()` edge, an `import()` edge with code splitting off, and the forced wrapper under strict execution order with manual groups. [How rolldown decides the wrapper today](#how-rolldown-decides-the-wrapper-today) gives the file and line for each.

### Which modules actually need the wrapper?

The two rules rolldown follows today, again:

> 1. A module's format decides its lowering. `exports_kind == CommonJs` means the wrapper.
> 2. Everything a wrapped module imports is wrapped as well.

Rule `2` is not the problem. It preserves a real guarantee: a deferred body must find everything it imports ready when it runs.

But what if we break rule `1`? What will we get?

Take a module that a `require()` call reaches from inside a branch. This is how CommonJS writes an optional dependency:

```js
// optional.cjs
console.log('LOADED');
exports.x = 1;

// feature.cjs
exports.get = () => {
  if (globalThis.SUPPORTED) {
    return require('./optional.cjs').x;
  }
  return 0;
};

// entry.mjs
import { get } from './feature.cjs';
console.log(get());
```

Today the wrapper defers the body, so an unsupported platform loads nothing:

```js
var require_optional = /* @__PURE__ */ __commonJSMin((exports) => {
  console.log('LOADED');
  exports.x = 1;
});

var feature_get = () => (globalThis.SUPPORTED ? require_optional().x : 0); // logs nothing, expected: nothing
console.log(feature_get());
```

Hoist `optional.cjs` anyway, and its body runs at its own position in the chunk:

```js
console.log('LOADED'); // logs "LOADED", expected: nothing
var optional_x = 1;

var feature_get = () => (globalThis.SUPPORTED ? optional_x : 0);
console.log(feature_get());
```

You can clearly see the behavior has changed. **This is just one case** showing that we can't hoist every CommonJS module.

The reason is that the current lowering mixes up two things:

- **The module format:** how the module writes its exports.
- **The evaluation strategy:** whether the body runs eagerly at its position in the chunk, or lazily on the first `require()`.

Rolldown reads the format and picks the strategy from it. The two are independent. A module can write its exports statically and still need the lazy strategy, and it can write them statically and not need it. Rule `1` cannot tell those apart, so it gives both the same answer.

So rule `1` does not face a trade. It faces a question nobody asks. Nothing works out which modules need the lazy strategy, so every module gets it, and the ones that do not need it pay for it.

## Guide-level explanation

### The solution

This RFC introduces:

1. A second lowering for CommonJS, which turns each export into a plain top-level binding.
2. A predicate that selects the modules that lowering is safe for, behind the option `onDemandWrapping: { commonjs: true }`.

### What's a hoisted module?

A hoisted module is a CommonJS module with no wrapper and no exports object. Hoisting is nothing but a lowering that lets you:

1. Read an export as a plain binding, instead of as a property of an object a closure built.
2. Run the module body at its own position in the chunk, instead of on the first `require()`.

The output shape is the whole of it:

```js
// mod.cjs, hoisted
var mod_a = () => 1;
var mod_b = () => 2;

// entry.mjs
console.log(JSON.stringify([mod_a(), mod_b()]));
```

The names follow one pattern: module name, underscore, export name. So `mod_a` is export `a` of `mod.cjs`, deconflicted like any other binding in the chunk. This document uses the pattern throughout. The final scheme is [an unresolved question](#which-binding-names).

The emitted shape is really small, and the ability it unlocks is large: rolldown already emits this shape for every ESM module, so after hoisting a CommonJS module is an ordinary module in the chunk. The minifier can see into it, tree shaking removes its unused exports, and the order analysis treats it like any other module. Complexity mainly comes from deciding which modules qualify.

Note: the block above collapses each declaration and its write into one statement, to keep the example short. [Render](#render-declare-once-assign-in-place) gives the exact lowering.

### What's the `commonjs` option?

Previously, we identified the modules that pay for a wrapper they do not need:

- Every export write is static, so the bundler can name each one.
- The module never treats its exports as an object, so no object needs to exist.
- Nothing reaches the module through `require()`, so the lazy strategy buys nothing.

The option widens the existing `onDemandWrapping` boolean. `inlineConst` and `chunkImportMap` already use the same shape:

```ts
experimental: {
  onDemandWrapping?: boolean | { commonjs?: boolean }; // Defaults to `false`, which means every CommonJS module keeps its wrapper
}
```

`onDemandWrapping: true` keeps its current meaning. `{ commonjs: true }` also lets rolldown hoist every CommonJS module that passes the predicate.

Where hoisting is safe, it is a pure improvement. The option is still necessary, because of how hoisting fails. A bug in the predicate does not break the build. It emits a bundle that runs and is wrong, without a warning. The wrong code is code the user did not write, somewhere in `node_modules`. An experimental feature that fails this way needs a switch to turn it off. That switch is also what a user bisects against after a bug report.

Note: today the docs define `onDemandWrapping` "under `output.strictExecutionOrder`", and `is_strict_on_demand_wrapping_enabled()` returns false without it. The main benefits of hoisting are a smaller output, tree-shakeable exports, and no interop helpers. None of them relates to strict execution order, and a gate on it would hide the feature from most users. So **rolldown reads the `commonjs` option on its own, whatever the value of `strictExecutionOrder`.** The boolean form keeps its existing gate. The two halves of the option differ here, so the docs must say so.

The expected end state is on by default. Two things must happen first. Every corpus case must pass, and real builds must use the option for some time.

### How modules are selected

Basically, the predicate looks like this:

```js
for (const module of graph.commonjsModules()) {
  if (!option.commonjs) continue;
  if (!module.safelyTreeshakeCommonjs) continue; // conditions 1 and 2
  if (module.reachedByRequire || module.isEntry || module.inRequireCycle) continue; // condition 3
  if (module.importedByWrappedModule) continue; // condition 4
  module.wrapKind = WrapKind.None;
}
```

**1. Every export write is static.** The base is exactly `exports` or `module.exports`. The property name is a plain identifier. `exports[key] = …` does not qualify.

**2. The module never treats its exports as an object.** The module must not do any of these:

- reassign `module.exports`;
- call `Object.defineProperty(exports, …)`;
- give `exports` a local alias;
- set the `__esModule` flag;
- read `module.id` or `module.loaded`.

Each one makes the object itself observable. A set of separate bindings cannot replace it. Conditions 1 and 2 together are the meaning of `SafelyTreeshakeCommonjs` today.

**3. Nothing reaches the module through `require()`.** This condition matters most. [The optional dependency](#which-modules-actually-need-the-wrapper) is the case it exists for. For a module that only static `import` reaches, eager evaluation changes nothing. Rolldown already places the `require_mod()` call at the module's own position in the chunk, directly after the wrapper definition. The call does not sit at the import site. So when a side-effecting module comes between the two, the CommonJS body still runs first, and the run order matches node. Hoisting runs the body at the same position, without the call. The same rule excludes CommonJS entries and members of a `require` cycle.

**4. No wrapped module imports it.** This is rule `2` of [today's model](#how-rolldown-wraps), kept as it is. A wrapped module defers its body, and everything it imports must still be ready when that body runs. Inside a deferred subtree, hoisting would move a body's side effects ahead of the wrapper that guards them. So v1 leaves that subtree alone (see [Future work](#future-work)).

Note: this feature never removes a wrapper that a correctness rule put in place. See [the hoistable predicate](#the-hoistable-predicate).

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

Webpack bails on any module without `"use strict"`. That is its largest class of bailout, because most of `node_modules` is sloppy mode. The gate exists because webpack merges module bodies into one shared strict scope. A sloppy body in that scope would change meaning without a warning.

Rolldown does not have that problem. Hoisting moves a body out of an arrow function, up to the top level of the same scope. The strictness before the move is the strictness after it. So rolldown hoists sloppy modules. The corpus records one case where rolldown's target verdict is better than webpack's real verdict (`bail/sloppy-mode`).

## Reference-level explanation

### Naming things

Four terms carry the rest of this document.

A `hoisted module` is a CommonJS module that this RFC lowers into plain top-level bindings. An importer references those bindings literally, and the body is top-level code, so it runs where it sits. It can also be referred to as an `unwrapped module`, because the change is the removal of the wrapper and nothing else.

A `wrapped module` is a CommonJS module that keeps the `__commonJSMin` closure. An importer reads its exports as properties of an object the closure built, and the body runs when `require_<name>()` runs. Rolldown has no third state, so "wrapped" is also the bailout. See ["Copy webpack's three states"](#copy-webpacks-three-states) for why.

A `facade symbol` is the symbol the scanner already creates for each `exports.x = …` write. A hoisted export binds to its facade symbol, so hoisting adds no new symbol kind.

The `predicate` is the four-condition test that decides whether a module hoists. It answers one question per CommonJS module: "may this module skip the default wrapper?"

An `escaping namespace` is a namespace object that code passes to a function, spreads, or indexes dynamically. A namespace that only static member reads touch does not escape, and rolldown emits no object for it.

### Co-existing wrapped and hoisted modules in the output

The predicate runs per module, so one chunk holds both kinds. Take `mod.cjs` from [the running example](#every-commonjs-module-gets-the-wrapper), plus the `optional.cjs` and `feature.cjs` from [the optional dependency](#which-modules-actually-need-the-wrapper):

```js
// entry.mjs
import { a } from './mod.cjs';
import { get } from './feature.cjs';
console.log(a(), get());
```

The block below is a design sketch, not built output:

```js
var __commonJSMin = /* … */;

// --- module: mod.cjs, hoisted
var mod_a = () => 1;

// --- module: optional.cjs, wrapped — a require() reaches it
var require_optional = /* @__PURE__ */ __commonJSMin((exports) => {
  console.log('LOADED');
  exports.x = 1;
});

// --- module: feature.cjs, hoisted — it calls require(), and nothing requires it
var feature_get = () => (globalThis.SUPPORTED ? require_optional().x : 0);

// --- module: entry.mjs
console.log(mod_a(), feature_get());
```

`mod.cjs` and `feature.cjs` cost one binding each. `optional.cjs` keeps the closure it needs, and `__commonJSMin` stays in the chunk because something still uses it. So a chunk pays for the helper only while at least one module still needs it, and calling `require()` never blocks the caller from hoisting. Corpus case: `hoist/calls-require`.

### How rolldown decides the wrapper today

Two rules make CommonJS mean "wrapper". Three overrides sit on top of them.

`determine_module_exports_kind.rs:97-106` — rolldown sets every CommonJS module that is not an entry to `WrapKind::Cjs`. A CommonJS entry also gets `WrapKind::Cjs` when one of these holds:

- the output format is `esm`;
- the format is `iife` or `umd`, and the module refers to `module` or `exports`.

`wrapping.rs:126-138` — rolldown walks the import records of an unwrapped module. It wraps any importee whose `exports_kind` is `CommonJs`. `wrap_module_recursively` repeats this through the whole subtree.

The overrides:

- `determine_module_exports_kind.rs:50-56` — a `require()` edge wraps its importee. This override always applies. The predicate never contradicts it.
- `determine_module_exports_kind.rs:66-79` — with code splitting disabled, `import()` behaves like `require()`. So rolldown wraps its importee.
- `wrapping.rs:155-166` — under strict execution order with manual code splitting groups, rolldown forces every CommonJS module back to `WrapKind::Cjs` ([#10405](https://github.com/rolldown/rolldown/pull/10405)).

`create_wrapper` (`wrapping.rs:201-223`) then creates the `require_<name>` symbol and the `__commonJSMin` statement for every module marked `WrapKind::Cjs`.

`set_wrap_kind` keeps the last write, so the order of these rules matters (see `internal-docs/linking/determine-module-exports-kind/implementation.md`).

### The hoistable predicate

The predicate reads three inputs:

- The `commonjs` option. When it is off, the predicate answers no for every module, and nothing after it changes.
- `EcmaViewMeta::SafelyTreeshakeCommonjs` — conditions 1 and 2. The scanner sets it when every export write is a static property assignment and nothing reads the exports object in an unknown way (`ecma_module_view_factory.rs:133-139`).
- The import records of every module in the graph — condition 3. Three things disqualify a module: an incoming `ImportKind::Require` record, entry status, or membership of a `require` cycle. `LinkingMetadata::required_by_other_module` already carries part of this signal.

Rolldown knows the whole module graph before the link stage runs. So the predicate is one pass over the module table. It runs once, before rolldown assigns any `WrapKind`.

One rule keeps this safe: **hoisting refuses to add a wrapper. It never removes one.** The link stage reads the predicate only at the two default rules above. Every override still runs and still wins, including the forced wrapper under strict execution order. This change cannot undo an existing correctness fix. Condition 4 then needs no work: `wrap_module_recursively` still wraps everything a wrapped module reaches, and the predicate does not oppose it.

### Binding: named imports reach the facade symbols

`bind_imports_and_exports.rs:1250-1252` returns `ImportStatus::CommonJS` for any importee whose `exports_kind` is `CommonJs`. It returns before it reads `resolved_exports`. The caller at line 1377 then routes the import through the namespace object.

For a hoistable module, rolldown skips that short-circuit. The import then resolves through `resolved_exports`, like an ESM import. The facade symbols are already there: the scanner creates one for each `exports.x = …` write and records it as a `LocalExport` (`ast_scanner/impl_visit.rs:249-267`). Two gaps must close first:

- `module.exports.x = …` creates no facade symbol. The scanner's `StaticMemberExpression` branch (`impl_visit.rs:271-282`) records the `module` identifier and nothing else. A module in that style has no symbols to bind to. Corpus case: `hoist/module-exports-prop`.
- `named_exports` drops any export that the module writes more than once (`ecma_module_view_factory.rs:96-99` keeps only `v.len() == 1`). After hoisting, a repeated write is an ordinary reassignment. The table should carry the first write, and the rest become assignments. Corpus case: `hoist/repeated-write`.

### Render: declare once, assign in place

One rule handles every write shape:

- Emit `var <facade>;` once, at the module's position in the chunk.
- Rewrite every `exports.x = v` in place to `<facade> = v`.
- Rewrite every read of `exports.x` inside the module to `<facade>`.

JavaScript lifts a `var` declaration to the top of its scope. That language rule, and not this RFC's transformation, does the rest. A conditional write leaves the binding `undefined` until it runs. The wrapper's exports object behaves the same: a read before the write gives `undefined`. A write inside a function body needs no special case. A repeated write is a reassignment.

Webpack emits the same shape. That is a useful check on a lowering with several possible forms:

```js
var __WEBPACK_CJS_EXPORT_a__;
__WEBPACK_CJS_EXPORT_a__ = 1;
```

Rolldown may collapse `var x; x = 1;` into `var x = 1` when the write is unconditional and comes first. That is a codegen improvement and not part of the contract. Every example in this document that shows the collapsed form relies on it only for brevity.

### Namespaces and default interop

Hoisting removes the exports object. Two import forms still need one:

- `import * as ns from "./mod.cjs"` needs a namespace.
- `import m from "./mod.cjs"` needs `default`, which for a CommonJS module is `module.exports` itself.

By itself, neither form puts an object into the output. Rolldown already resolves a static member read on an ESM namespace directly to the binding. It emits no object. `import * as ns` followed by `ns.a` bundles to a plain read of the `a` binding, and hoisted CommonJS gets the same treatment. An object appears only when the namespace escapes.

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

Rolldown emits the getter map once. It derives the namespace from the exports object and emits no second copy, so the chunk holds one getter per export, not two. Webpack does the same: it writes the map into a base object, then calls `__webpack_require__.t(base, 2)` to get the namespace.

Two objects sit over one set of bindings. That is the part to get right. `ns` is the namespace and carries a `default` key. `default` holds `module.exports`. Webpack emits the same split into two objects for this example, and its `default` points at the base object. So the shape is not speculative.

The `__toESM` call gives more than a shorter output. It is the same call that the importer of a wrapped module makes today, over an object of the same shape. So it returns the namespace that rolldown already emits. The keys are the same, in the same order. Neither object has a `Symbol.toStringTag`. The prototype is the same, and `default` holds the same object. That is the strongest available guarantee that a user cannot observe hoisting. The two namespaces do not merely agree. They come off the same code path.

Neither object gets the tag, and that needs saying, because node behaves differently. In node a namespace carries `Symbol.toStringTag: 'Module'`, and `module.exports` does not. Rolldown builds a CommonJS namespace through `__toESM`, which never sets the tag. So `Object.prototype.toString.call(ns)` already returns `[object Object]` for a wrapped module. The sketch keeps that behaviour: the `__exportAll` call passes `no_symbols: true`, and `__toESM` adds nothing. A tag on the hoisted object would make hoisting observable, and hoisting must never be observable.

The map holds getters, not a snapshot. A hoisted module may still assign `exports.a` after evaluation, from a callback or a timer. The wrapper's object shows such a write today. The map holds thunks and not values, so both calls can be above the writes. That is where rolldown already puts them for ESM.

A named import needs none of this. The import resolves to the binding. Rolldown emits no object, and no helper enters the chunk. Today every wrapped CommonJS module pays the interop cost once per chunk, whatever its importers do with it. After this change, only an escaping namespace pays it.

### Execution order semantics

A hoisted body runs at the module's position in the chunk. That is exactly where its `require_mod()` call sits today, directly after the wrapper definition, and not at the import site. So for the modules the predicate selects, hoisting preserves 100% of the current evaluation order. Condition 3 is what buys that: it removes every module whose body might not have run at all.

Two changes follow in `generate_stage/order_analysis.rs`:

- `is_order_wrap_eligible` (line 1194) requires `ExportsKind::Esm | None`, so on-demand wrapping cannot see CommonJS. A hoisted module is an ordinary eager module, so the check should admit it.
- `eagerly_triggers_interop_module` (line 1168) marks the importer of a wrapped module as order-sensitive. A hoisted module has no wrapper. So it stops triggering the mark, and rolldown stops wrapping its importers for that reason.

Both code paths run only for a module that hoisted. So `{ commonjs: true }` controls them, and no second check is necessary. This half of the feature justifies the name of the option. With the option on, on-demand wrapping is no longer ESM-only.

The forced wrapper from #10405 stays until both changes land with snapshot coverage. It is a correctness fix for a real failure. Nobody should relax it before tests prove the relaxation safe.

### Preserved semantics

Hoisting must not change any of these:

- **Evaluation order and side effects.** A hoisted body runs at the module's position in the chunk. That is where its `require_mod()` call is today.
- **Export liveness.** An importer sees a later write to an export, through the binding directly or through a namespace getter.
- **Export identity.** `import * as ns` and default interop see the same values as before, with the same keys.
- **Tree shaking.** The removal of an unused export is a gain, not a change. Code outside the module could never observe the wrapper's exports object.
- **Sloppy-mode meaning.** The strictness of the enclosing scope does not change. That is what makes [the missing strict-mode gate](#no-strict-mode-gate) correct.

Lazy evaluation is outside this contract, so the predicate does not select a module that a `require()` reaches. `module.exports = { … }` is outside it too, because the object identity is the export, and the first predicate does not select that shape.

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

All three drawbacks arrive when the code lands, whatever the value of the option. A predicate bug that emits a quiet, wrong bundle is not a fourth entry. That is a failure mode, not a design cost, and [the option](#whats-the-commonjs-option) exists to bound it.

### The wrapper decision starts to read the whole graph

Today rolldown decides each wrapper from one import edge: the importer, the importee, and the `ImportKind`. One edge is enough, so the decision can run before symbol binding and tree shaking, and one edge explains every wrapper. Condition 3 ends that: "does any `require()` reach this module?" is a question about every edge in the graph at once. The predicate still runs once, and it stays deterministic. But the answer to "why did this module keep its wrapper?" can now sit anywhere in the graph.

### Two code paths to maintain

Every later pass gets a second CommonJS shape to handle: the finalizer, chunk linking, HMR, and `preserveModules`. Those branches exist whether or not a user turns the option on. So this cost arrives in full on the first day, and the option cannot defer it.

### A module whose namespace escapes gains almost nothing

It exchanges the `__commonJSMin` closure for an `__exportAll` map, and it keeps the `__toESM` call it already had. So the object returns and the helpers stay. The result is not worse than today, and the minifier can still reach the bindings under the object. But most of the benefit is gone, and the benefit therefore depends on how code uses a module, not on what the module exports.

The predicate could read usage and leave an escaping module wrapped. That would tie a link-stage decision to usage, which the wrapper decision avoids today on purpose. So this drawback has no mitigation inside the current design.

## Rationale and alternatives

### Why this shape

Three properties make this design better than the alternatives below. All three limit risk. None of them maximises the benefit.

**It adds no new output construct.** Hoisting only removes a wrapper. It adds no registry, no new module state, and no new runtime helper. An escaping namespace reuses `__exportAll`, and a hoisted export is a plain `var` declaration. Rolldown already emits this shape for every ESM module, so every later pass handles it today. That is why the work is in the link stage, and not spread across rendering, chunking, and HMR.

**It reuses analysis that the bundler already trusts.** `SafelyTreeshakeCommonjs` and the facade symbols are not new, and not speculative. They already decide whether tree shaking may remove a CommonJS module's exports. Today's output depends on their correctness. This RFC adds one graph condition, and changes what rolldown does with the answer.

**The design bounds its own failure mode.** Hoisting refuses to add a wrapper, and it never removes one. So every existing override still runs and still wins. A wrong predicate can keep the wrapper on a module that could hoist. It can also hoist a module that needs its wrapper. It cannot undo a correctness fix that an earlier decision put in place.

This is a second lowering for one class of module, not a replacement for rolldown's CommonJS support.

### Do nothing

The minifier cannot cross the wrapper. `import_src.a` is a property read on an object that a memoized closure builds. To inline the function behind it, the minifier must prove three things at once:

- the closure runs once;
- nothing else writes the object;
- no getter is involved.

The bundler knows all three at link time. The minifier does not. So nothing downstream recovers what rule `1` gave away.

### Hoist everything

This alternative is a real change in program behaviour, not a trade against size. [Which modules actually need the wrapper?](#which-modules-actually-need-the-wrapper) shows the case. A `require()` inside a branch is how CommonJS writes an optional dependency. Run that `require()` every time, and the program can throw on a platform where the module does not load. The lazy wrapper exists exactly for this.

### Run the wrapper eagerly

`var ns = (() => { … })()` removes the laziness, but it keeps the object. So the property access, the interop, and the tree-shaking barrier all stay. Nothing comes for free here either: this alternative gives up the semantics that make the wrapper valuable, and it keeps the cost that makes the wrapper expensive.

### Copy webpack's three states

Webpack has three states: hoisted, wrapped, and bailed. "Bailed" means "left as a registry module". Rolldown has no registry. A module that bails must still reach the chunk, so the wrapper is rolldown's bailout. Two states are the whole design space here. That is why the `target` column of the corpus is not a copy of the webpack column.

### Relationship to strict execution order

- This proposal does not require any change to `output.strictExecutionOrder` itself.
- It does change what `experimental.onDemandWrapping` can reach. Today on-demand wrapping is ESM-only, because `is_order_wrap_eligible` rejects CommonJS. A hoisted module is an ordinary eager module, so it becomes eligible.
- The two features share the option name and the order analysis. They stay separately switchable: the boolean form keeps its `strictExecutionOrder` gate, and `{ commonjs: true }` does not.

## Prior art

### webpack

Webpack's model for a CommonJS module is:

1. Hoist it into the concatenated scope when every export write is static.
2. Otherwise leave it in the module registry, which rolldown has no equivalent of.

It shipped this in 5.109.0, behind [`optimization.concatenateModules: { commonjs: true }`](https://webpack.js.org/configuration/optimization/#optimizationconcatenatemodules) ([#21417](https://github.com/webpack/webpack/pull/21417), [#21436](https://github.com/webpack/webpack/pull/21436), [#21464](https://github.com/webpack/webpack/pull/21464)). The conditions are in `ModuleConcatenationPlugin.js` (graph admission) and in `JavascriptGenerator.js` (`getCommonJsConcatenationBailoutReason`, `isCommonJsHoistable`).

Two things are worth copying. The first is the shape of the analysis. The second is `Dependency.canConcatenate()`, which returns false for a CommonJS dependency. It makes condition 3 a property of the dependency type, not a check that runs later. One thing not to copy: the strict-mode gate, [as above](#no-strict-mode-gate).

In one place rolldown is already ahead. Webpack refuses to wrap a module that calls `require()`, because a wrapper renders with its module ids intact. Rolldown handles this today. The corpus keeps `bail/wrapped-plus-require`, so the difference stays on the record.

### esbuild

esbuild wraps every CommonJS module in [`__commonJS`](https://github.com/evanw/esbuild/blob/main/internal/runtime/runtime.go). Rolldown inherited the same design in `__commonJSMin`. esbuild has no hoisting to compare against.

### rollup

Rollup core has no CommonJS support, and [`@rollup/plugin-commonjs`](https://github.com/rollup/plugins/tree/master/packages/commonjs) does not hoist. For a module of this shape, the plugin emits a lazy memoized wrapper: `var mod = {}; function requireMod() { … }`, behind a `hasRequiredMod` guard. The importer then reads `modExports.a`. The plugin resolves the named import, so `import { a }` compiles. But the output has the structure of rolldown's `__commonJSMin`, not of webpack's bindings.

So the plugin belongs to the wrapper class, not to the hoisting class. Webpack is the only earlier implementation of what this RFC proposes. That fact also removes an attractive argument: the wide adoption of the plugin says nothing about the safety of hoisting, because the plugin does not hoist.

## Unresolved questions

### Should a module stay wrapped when every importer only wants `default`?

To answer that, usage must inform the wrapper decision. No other wrapper rule reads usage today, and [the escaping-namespace drawback](#a-module-whose-namespace-escapes-gains-almost-nothing) is the same question from the other side.

### What happens to a module with repeated writes?

Should `named_exports` carry the first write? Or should v1 leave a module with a repeated write wrapped? The corpus wants those modules hoisted. The export table drops them today.

### When does `commonjs` become the default?

Is the bar all 11 gap rows passing, or a real application that bundles and runs without a change? And does the option stay afterwards as a permanent switch, or does it go away like any other experiment?

### Does `preserveModules` or `format: cjs` break the shape?

A hoisted module must still re-export correctly when the output format is CommonJS. Is there a shape where the synthesized namespace and the output format's exports differ?

### Which binding names?

`mod_a` is easy to read in a bundle. The deconflicting suffix form (`a$1`) is shorter. Which one does rolldown use? And does the choice matter after minification?

### Should the namespace tag be fixed?

Rolldown's CommonJS namespace reports `[object Object]`, where node reports `[object Module]`. The cause is `__toESM`, which sets no `Symbol.toStringTag`. Hoisting must reproduce that, or a user can observe it. Is the divergence worth a fix on its own, for wrapped and hoisted modules together? Webpack made the opposite choice and tags both objects, so its `default` also claims to be a namespace.

### Which plugins depend on the wrapper shape?

A hoisted module reaches `renderChunk` with no `require_<name>` symbol and no exports object. Which plugins match on that shape today, and does any of them read `this.getModuleInfo` expecting one CommonJS lowering? Vite's dependency pre-bundling is the first thing to measure, because its output is itself pre-bundled CommonJS.

## Future work

This RFC gives rolldown a second lowering for CommonJS. Removing the wrapper from statically analyzable modules is the first thing built on it, and the ability reaches further.

### Hoist inside a wrapped subtree

Condition 4 is conservative. Take a module whose body has no side effects except its own export writes. It could hoist even when a wrapped module imports it, because nothing is left to move. That needs a real judgement about side effects, which the current predicate avoids on purpose.

### `module.exports = { … }` with an object literal

This shape is common in real packages. Today it always bails, because the object identity is the export. When every importer reads named properties, each property could become a plain binding. Webpack does not do this.

### Per-export tree shaking and constant propagation

After exports become plain `var` declarations, an unused export is dead code, and the existing tree shaking removes it. `hoist/unused-export-shaken` is the case that proves it. `constant_export_map` already folds a constant CommonJS export at the import site, and hoisting extends the same reach to non-constant values at no cost.
