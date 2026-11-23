# \[RFC\] `import defer` Support

- **Feature name:** `import-defer`
- **Authors:** [@IWANABETHATGUY](https://github.com/IWANABETHATGUY)
- **Reference issues:** [rolldown#4857](https://github.com/rolldown/rolldown/issues/4857) (tracking issue), [TC39 Deferring Module Evaluation](https://github.com/tc39/proposal-defer-import-eval)
- **Implementation PRs:** Stacked PRs [#11861](https://github.com/rolldown/rolldown/pull/11861)–[#11869](https://github.com/rolldown/rolldown/pull/11869)

## Table of contents

- [Summary](#summary)
- [Basic example](#basic-example)
- [Motivation](#motivation)
- [Guide-level explanation](#guide-level-explanation)
  - [Terms](#terms)
  - [Configuration](#configuration)
  - [Static `import defer * as ns`](#static-import-defer--as-ns)
  - [Dynamic `import.defer()`](#dynamic-importdefer)
  - [When does evaluation happen?](#when-does-evaluation-happen)
  - [Output formats and externals](#output-formats-and-externals)
  - [When Rolldown falls back to eager evaluation](#when-rolldown-falls-back-to-eager-evaluation)
  - [Tree shaking](#tree-shaking)
  - [Plugin API](#plugin-api)
  - [Dev mode and Vite](#dev-mode-and-vite)
  - [Diagnostics](#diagnostics)
- [How it works (high-level overview)](#how-it-works-high-level-overview)
- [Drawbacks and known limitations](#drawbacks-and-known-limitations)
- [Alternatives](#alternatives)
- [Prior art](#prior-art)
- [Adoption strategy](#adoption-strategy)
- [Unresolved questions](#unresolved-questions)
- [Future work](#future-work)

---

## Summary

The TC39 proposal [Deferring Module Evaluation](https://github.com/tc39/proposal-defer-import-eval) adds `import defer * as ns from './x'` and `import.defer('./x')`. A deferred import loads and links a module and its dependencies up front, but delays running the module's top-level code until its namespace is first used.

Today, Rolldown parses the syntax, removes the `defer` phase, and evaluates the module eagerly with a warning. This RFC proposes bundler support for deferred evaluation behind a new opt-in option, `experimental.importDefer`.

**Goals:**

1. Preserve the semantics of the proposal across code splitting, circular dependencies, top-level `await`, and tree shaking (currently passing **243 / 252 — 96.4%** of the official test262 suite).
2. Keep zero cost when the option is off — builds without `experimental.importDefer` produce identical output with no measurable build-time overhead.
3. Fall back safely for unsupported build shapes — instead of silently producing broken execution order, unsupported shapes fall back to the exact eager output with a warning explaining why.
4. Deliver faster startup with minimal bundle-size and runtime overhead.

---

## Basic example

```js
// src/main.js
import defer * as chart from './chart.js';

console.log('start');

export function draw(data) {
  // First property access evaluates scale.js and chart.js synchronously
  return chart.render(data);
}

export function getChart() {
  // Returning the namespace as a value does NOT evaluate chart.js yet
  return chart;
}
```

```js
// src/chart.js
import { scale } from './scale.js';

console.log('chart evaluated');

export function render(data) {
  return data.map(scale);
}
```

```js
// src/scale.js
console.log('scale evaluated');

export const scale = (x) => x * 2;
```

```js
// rolldown.config.js
export default {
  input: 'src/main.js',
  experimental: {
    importDefer: true,
  },
};
```

Running the built output:

```js
const mod = await import('./dist/main.js');
console.log('imported');
console.log(mod.draw([1, 2]));
const ns = mod.getChart();
console.log(Object.prototype.toString.call(ns), Object.keys(ns));
```

| Build                                          | Console output                                                                                                 |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Rolldown today (`importDefer: false`)          | `scale evaluated`, `chart evaluated`, `start`, `imported`, `[ 2, 4 ]`, `[object Module] [ 'render' ]`          |
| Rolldown with `experimental.importDefer: true` | `start`, `imported`, `scale evaluated`, `chart evaluated`, `[ 2, 4 ]`, `[object Deferred Module] [ 'render' ]` |
| Native Node.js 26 (`--js-defer-import-eval`)   | Identical to `experimental.importDefer: true`                                                                  |

---

## Motivation

### Avoiding unnecessary startup work

Applications often execute a large amount of code during startup that isn't needed for the first request or initial interaction:

- **Server and edge apps** pay this cost on every cold start.
- **Large browser apps** pay it before the page becomes interactive.

Existing workarounds each have trade-offs:

- **Dynamic `import()`** delays evaluation, but forces callers to become `async`, rippling API changes through your codebase.
- **Lazy `require()`** stays synchronous, but is CommonJS-only (ES modules in Node.js must go through `createRequire()`).

`import defer` gives you **lazy evaluation while keeping a synchronous API**: the module loads with its importer, and its top-level code runs only on first use.

### Runtimes don't support it out of the box yet

As of November 2026, users cannot simply emit raw `import defer` syntax:

- Node.js 24 and 26 cannot parse `import defer` without the V8 flag `--js-defer-import-eval` (and even with the flag, `import.defer()` currently fails in V8).
- Cloudflare `workerd` and the stable releases of major browsers do not parse it by default. Only Safari 27.3 (beta) and Firefox Nightly (behind a preference) support it.

Bundlers therefore need to compile `import defer` into standard JavaScript so apps can benefit from it today.

### Startup benchmark

In a [composite benchmark app](https://github.com/IWANABETHATGUY-reproduction/rolldown-import-defer-bench) (a Hono server for `workerd` with an eager core and four routes imported via `import defer * as`, using real open-source packages):

| Build                        | Startup (Node.js)  | Startup (`workerd`) | First use of all 4 routes | Startup + all 4 routes | Bundle size          |
| ---------------------------- | ------------------ | ------------------- | ------------------------- | ---------------------- | -------------------- |
| Rolldown (no `import defer`) | 35.8 ms            | 59.4 ms             | 0 ms                      | 35.8 ms                | 1,391,118 B          |
| Rolldown + `import defer`    | **19.5 ms (−46%)** | **20.1 ms (−66%)**  | 18.3 ms                   | 38.0 ms (+6%)          | 1,394,736 B (+0.26%) |

Real-world gains depend on how much of your dependency graph sits off the initial startup path.

<details>
<summary>Benchmark methodology and per-route breakdown</summary>

- Single minified bundle (`codeSplitting: false`, `platform: 'neutral'`).
- **Node.js 26.8.1 (V8 14.6):** `vm.SourceTextModule` compiles and evaluates the file in a fresh process per run (2 warm-up + 30 samples, medians).
- **workerd:** Time until `await mf.ready` in Miniflare minus an empty Worker (30 samples, medians).
- **"First use"** measures the time to read the `default` export of each route's deferred namespace after startup, which evaluates that route and its dependencies.

| Route      | First use |
| ---------- | --------- |
| `/ai`      | 8.4 ms    |
| `/graphql` | 6.2 ms    |
| `/s3/*`    | 3.4 ms    |
| `/ssr`     | 1.1 ms    |

</details>

---

## Guide-level explanation

### Terms

- **Deferred import:** `import defer * as ns from './x'` or `import.defer('./x')`.
- **Deferred namespace:** the object that a deferred import gives (`ns`).
- **Deferred graph:** the module that a deferred import names, plus each module that it imports statically, directly or through other modules.
- **Deferred module:** a module in a deferred graph.
- **Trigger:** an operation on a deferred namespace that evaluates its module, for example `ns.foo` (see [When does evaluation happen?](#when-does-evaluation-happen)).

### Configuration

```ts
interface InputOptions {
  experimental?: {
    /** @default false */
    importDefer?: boolean;
  };
}
```

- `experimental.importDefer`: Enables deferred evaluation in the bundled output. When `false` (the default), Rolldown lowers `import defer` to a normal eager import and emits an `UNSUPPORTED_FEATURE` warning pointing to this option.

In **Vite 8**, enable it for production builds via `build.rolldownOptions`:

```js
// vite.config.js
export default {
  build: {
    rolldownOptions: {
      experimental: {
        importDefer: true,
      },
    },
  },
};
```

### Static `import defer * as ns`

Per the TC39 specification, static deferred imports only support the namespace form:

```js
import defer * as chart from './chart.js';
```

Named imports (`import defer { render } from './chart.js'`) and default imports (`import defer chart from './chart.js'`) are not valid syntax in the proposal.

### Dynamic `import.defer()`

While `import('./table.js')` loads **and** evaluates the chunk before its promise resolves, `import.defer('./table.js')` loads the chunk (and its chunk dependencies) **without** running its top-level code, resolving to the deferred namespace object.

This allows you to preload a chunk early (for example, while the page is idle) and only pay the execution cost when the user actually triggers the feature:

```js
// src/main.js
// Fetch the chunk early; table.js does not evaluate yet
export const tablePromise = import.defer('./table.js');

export async function showTable() {
  const table = await tablePromise;
  // table.js evaluates here, on first property access
  return table.rows();
}
```

```js
// src/table.js
console.log('table evaluated');

export function rows() {
  return [1, 2, 3];
}
```

```js
// Usage
const { showTable, tablePromise } = await import('./dist/main.js');
await tablePromise; // chunk is loaded, not yet evaluated
console.log('before showTable');
console.log(await showTable());
```

| `main.js` uses               | Console output                                       |
| ---------------------------- | ---------------------------------------------------- |
| `import('./table.js')`       | `table evaluated`, `before showTable`, `[ 1, 2, 3 ]` |
| `import.defer('./table.js')` | `before showTable`, `table evaluated`, `[ 1, 2, 3 ]` |

> **Note:** The specifier in `import.defer()` must be a static string literal or a template literal with no expressions (e.g. ``import.defer(`./table.js`)``). Rolldown lowers a call with any other specifier to `import()`, which evaluates on load. A few other cases are lowered too (see [When Rolldown falls back to eager evaluation](#when-rolldown-falls-back-to-eager-evaluation)).

### When does evaluation happen?

Rolldown follows the TC39 specification rules:

- **Triggers evaluation synchronously:**
  - Reading a string property (`ns.foo`), even if `foo` is not an export
  - Checking property existence (`'foo' in ns`)
  - Inspecting or listing keys (`Object.keys(ns)`, `Object.getOwnPropertyDescriptor(ns, 'foo')`)
  - Defining or deleting a string property (`delete ns.foo`)
- **Does NOT trigger evaluation:**
  - Reading `ns.then` — always returns `undefined` (so `await`ing or returning `ns` from an `async` function never triggers evaluation accidentally)
  - Any `Symbol` key (for example, `ns[Symbol.toStringTag]` is `"Deferred Module"`)
  - Prototype or extensibility checks (`Object.getPrototypeOf(ns)` is `null`, `Object.isExtensible(ns)` is `false`)
  - Property assignments (`ns.foo = 1`) and private field access
- **Shared namespace identity:**
  - Every deferred import of the same ES module (static or `import.defer()`, from any file) returns the **exact same** deferred namespace object, which is distinct from the eager namespace (`import * as ns`).
- **Mixed eager and deferred imports:**
  - If another module imports the same module eagerly, it evaluates at that eager import's position.
- **Top-level `await`:**
  - Any module in the deferred graph that uses top-level `await` evaluates eagerly at the `import defer` position (and `import.defer()` waits for those async modules to settle before resolving). Synchronous modules in the graph remain deferred until first access.
- **Circular dependencies:**
  - Triggering a deferred namespace while that module (or one of its dependencies) is currently in the middle of evaluating throws a `TypeError`.

### Output formats and externals

| Scenario                                                | ESM output                                            | CommonJS output                                                                                                                                                  | IIFE / UMD output                                     |
| ------------------------------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| **Bundled module**                                      | Deferred                                              | Deferred                                                                                                                                                         | Deferred                                              |
| **External module** (`import defer * as ns from 'ext'`) | Preserved as `import defer * as ns from "ext"`        | Deferred via lazy `require("ext")`                                                                                                                               | Falls back to eager (`UNSUPPORTED_FEATURE` warning)\* |
| **External module** (`import.defer('ext')`)             | Preserved as `import.defer("ext")`                    | Keeps `import()` + warning when `dynamicImportInCjs: true` (default) or with a 2nd argument; deferred via lazy `require("ext")` when `dynamicImportInCjs: false` | Falls back to eager (`UNSUPPORTED_FEATURE` warning)\* |
| **External module imported inside a deferred module**   | Falls back to eager (`UNSUPPORTED_FEATURE` warning)\* | Deferred via lazy `require("ext")` when the importing module evaluates (falls back if re-exported via `export *` from an entry / `preserveModules` file)\*       | Falls back to eager (`UNSUPPORTED_FEATURE` warning)\* |

\* **Exemptions:** Node.js builtin modules and side-effect-free external modules (for example, via `treeshake.moduleSideEffects: false`) never trigger a fallback, since their early loading has no observable side effects.

<details>
<summary>Why an external module inside a deferred graph falls back in ESM output</summary>

Suppose a deferred module imports an external package:

```js
// src/main.js
import defer * as chart from './chart.js';
globalThis.counter++;
console.log(chart.seen);

// src/chart.js
import 'ext'; // external; sets globalThis.counter = 0
export const seen = globalThis.counter;
```

In **ESM output**, `import "ext"` must be at the top of the chunk. Thus `ext` evaluates before `main.js`. If `chart.js` stayed deferred, the order would be `ext` → `main` → `chart`, and `seen` would be `1`. This order matches neither valid order:

- Native `import defer`: `main` → `ext` → `chart`, `seen === 0`.
- A normal eager build: `ext` → `chart` → `main`, `seen === 0`.

To avoid introducing subtle ordering bugs, Rolldown falls back to the normal eager build and emits an `UNSUPPORTED_FEATURE` warning naming the external module. To keep deferred evaluation, you can either:

1. Bundle the external dependency,
2. Mark it side-effect-free if it has no observable side effects, or
3. Target CommonJS output if your runtime supports it (there, `require("ext")` runs only when `chart.js` evaluates).

</details>

> **Note on direct ESM externals:** Directly deferring an external module (`import defer * as ns from 'ext'` or `import.defer('ext')`) in ESM output preserves the syntax as-is, so your target runtime must support it (matching how Rolldown preserves import attributes on external imports).

### When Rolldown falls back to eager evaluation

Rather than silently breaking execution order on edge cases that aren't implemented yet, Rolldown checks the module graph and the options early in the build. If the build hits an unsupported shape, it **falls back for the whole build** to the exact output of `importDefer: false` and emits an `UNSUPPORTED_FEATURE` warning explaining the reason.

Currently, a build falls back when any of these apply:

1. `output.codeSplitting.experimentalInlineCommonChunks` is enabled.
2. **A code splitting group sets `includeDependenciesRecursively: false`** (such a group can make two chunks import each other in a cycle).
3. **`import.defer()` targets a CommonJS module** (with `codeSplitting: false`, a module with no `import`/`export` syntax and not marked ESM by extension or `package.json` is also treated as CommonJS).
4. **An observable (non-builtin, side-effectful) external module would evaluate early** for the chosen output format (see [Output formats and externals](#output-formats-and-externals)).
5. **A CommonJS module inside a deferred graph would run when its output file loads:**
   - With `preserveModules` and `esm` output (any CommonJS module in a deferred graph), or
   - With `cjs` output when a user-defined entry is a CommonJS module in a deferred graph.
6. **An import after a deferred import that reaches top-level `await`:** A module outside the deferred graphs has a deferred import of a module that has or reaches a top-level `await`. Later in the same module, a static import loads another module outside the deferred graphs. In a build that can make more than one chunk, a later import of an external module also counts. For example:

   ```js
   // main.js
   import defer * as d from './d.js'; // d.js reaches a top-level await
   import './later.js'; // the build falls back
   ```

Why the whole build, and not only the affected import? Most of these conditions are properties of the module graph and the options, not of one import. A fallback for each import would need new rules for modules that deferred and eager imports share, across chunks, cycles and top-level `await`. The whole-build fallback gives exactly the output of `importDefer: false`, which is the output these builds have today. Each later release can remove a condition, and builds that still hit a condition keep their current output.

Separately, individual `import.defer()` calls are lowered to `import()` (with an `UNSUPPORTED_FEATURE` warning, without falling back the rest of the build) when:

- The specifier is not a string literal or expression-free template literal,
- The call has a `/* @vite-ignore */` comment, or
- In `cjs` output, an external `import.defer('ext')` has `output.dynamicImportInCjs: true` (the default) or passes a second options argument.

### Tree shaking

- Static property reads (`chart.render`) keep only the exports that are read.
- Escaping namespaces (`return chart`) keep all exports, just like an escaping `import * as chart`.
- Unused deferred modules without side effects are removed.
- Side-effect-only triggers (`ns.foo;`) are preserved unless the deferred graph has no side effects.

**Current caveats:** Tree shaking may currently keep slightly more modules than an eager build in two cases:

- A module that only re-exports other modules stays in the output when a deferred module imports a binding through it.
- With code splitting enabled (the default) or multiple entries, an unused `export *` target inside a deferred module stays in the output. (In a single-chunk build, unused `export *` targets are already tree-shaken.)

### Plugin API

The plugin API does not change. The `resolveId` hook gets no phase: an `import defer` gets the same arguments as an `import` of the same specifier, and an `import.defer()` gets the same arguments as an `import()`. This follows the specification, which compares only the specifier and the import attributes of two module requests. Thus a deferred import and an eager import of the same specifier must resolve to the same module.

### Dev mode and Vite

- **Rolldown bundled dev mode (`experimental.devMode`):** HMR support for `import defer` is not implemented yet. In dev mode, Rolldown automatically turns `experimental.importDefer` off, lowers deferred imports to eager imports, and emits a warning.
- **Vite dev server:** Vite's unbundled dev server serves modules as-is, so deferred evaluation during dev requires native browser support.

### Diagnostics

| Warning code          | When it is emitted                                                                                                                                                                                                                                                                                                                  |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `UNSUPPORTED_FEATURE` | Emitted when `experimental.importDefer` is off, dev mode is active, the build hits a whole-build fallback condition (the message names the exact reason and module), or an individual `import.defer()` call cannot be deferred (see [When Rolldown falls back to eager evaluation](#when-rolldown-falls-back-to-eager-evaluation)). |

---

## How it works (high-level overview)

Rolldown keeps its scope-hoisted output model instead of switching deferred modules to a webpack-style runtime module registry (see [Alternatives](#alternatives)).

For the [Basic example](#basic-example) above, Rolldown emits code like this (runtime helpers omitted):

```js
// src/scale.js
var scale;
// src/chart.js
var chart_exports = /* @__PURE__ */ __exportAll({ render: () => render });
var chart_deferred = /* @__PURE__ */ __deferNs(() => (init_chart(), chart_exports), chart_exports);
function render(data) {
  return data.map(scale);
}
function init_chart() {
  return (init_chart = __esmMin(() => {
    // src/scale.js
    console.log('scale evaluated');
    scale = (x) => x * 2;
    // src/chart.js
    console.log('chart evaluated');
  }))();
}
// src/main.js
console.log('start');
function draw(data) {
  return (init_chart(), render)(data);
}
function getChart() {
  return chart_deferred;
}
export { draw, getChart };
```

The design centers on three ideas:

1. **Lazy init wrappers and grouping:** Rolldown puts the top-level code of deferred modules in lazy `init_*()` functions. Rolldown already uses this wrapper for other features. In ESM output, when only one deferred import reaches a dependency, and no other code does, the dependency goes into the wrapper of the module that this deferred import names. In the example, `init_chart()` also holds `scale.js`. This grouping reduces the wrappers that the JavaScript engine must parse and compile: from 546 to 33 in the benchmark.
2. **Direct reads vs. `Proxy` namespace:** Static accesses like `chart.render` compile straight to `(init_chart(), render)`. Once evaluated, `init_chart()` is a fast flag check and reads the binding directly from the shared chunk scope without going through a getter or `Proxy`. A `Proxy` (`__deferNs`) is only used when the namespace escapes as a value (`return chart`) or for dynamic operations like `Object.keys(chart)`.
3. **Cycles, top-level `await`, and code splitting:**
   - Rolldown tracks the evaluation state at run time only for the modules that need it: modules in a cycle with a module that a deferred import names, and modules that depend on top-level `await`. With this state, an access to a module that is still evaluating throws a `TypeError`, and the async dependencies are awaited up front, as the specification requires.
   - In multi-chunk builds, a chunk that holds deferred modules does not evaluate them when it loads. A chunk with an eager importer of such a module initializes the module in the right order. `import.defer('./x')` creates a split chunk just like `import('./x')`, and resolves to the deferred namespace without evaluating the module.

---

## Drawbacks and known limitations

- **Bundle size overhead:** Wrapper functions and runtime helpers add a small amount of code (+0.26% in the benchmark).
- **Dev mode discrepancy:** Dev builds evaluate deferred modules earlier than production builds until HMR support lands (see [Dev mode and Vite](#dev-mode-and-vite)).
- **Top-level `arguments` and direct `eval`:** Because deferred modules run inside an init wrapper function, top-level `typeof arguments` is `"object"` instead of `"undefined"`, and direct `eval("new.target")` returns `undefined` rather than throwing a `SyntaxError` (same as `output.strictExecutionOrder` and esbuild's `__esm`).
- **Sequential async sibling modules:** Async sibling modules with top-level `await` execute one after another rather than concurrently. This is a general Rolldown behavior for TLA (see [Top Level Await in Rolldown](https://rolldown.rs/in-depth/tla-in-rolldown)).
- **CommonJS targets:**
  - Method calls like `ns.fn()` on a deferred CommonJS namespace pass the eager namespace object as `this`, not the deferred namespace (`this === ns` is `false` if `ns` escapes).
  - Two deferred imports of the same CommonJS module give the same deferred namespace only when both importers are in the same chunk, use the same ESM/CJS interop mode, and are not wrapped in a lazy function (as the modules of a deferred graph are). Otherwise, the namespaces can differ.
  - In CommonJS output, external ESM modules with top-level `await` cannot be lazily loaded via `require()` (`ERR_REQUIRE_ASYNC_MODULE`).
- **Dynamic exports (`export *` from CJS/external):** Because the export key list isn't statically known, the deferred `Proxy` remains extensible until the first `Proxy` trap triggers evaluation and copies its keys.
- **`preserveModules` edge cases:**
  - A user-defined or emitted entry module that a deferred import also names evaluates eagerly when its file loads, because `preserveModules` creates no separate entry file for it.
  - Directly importing a non-entry deferred file from outside the bundle under `preserveModules` does not execute it on load.
- **External modules that eager code also imports (ESM output):** A side-effectful external module inside a deferred graph causes the fallback, also when an eager module in the same chunk imports it, and the external module would run at chunk load anyway.
- **Non-import cycles:** The `TypeError` check for accessing a still-evaluating module tracks cycles through `import` edges, not indirect cycles through global variables (same as webpack).
- **Tree-shaking retention:** In two cases, tree shaking can keep more modules than an eager build (see [Tree shaking](#tree-shaking)).

<details>
<summary>Test262 compliance details (243 / 252 passing)</summary>

Out of 252 official test262 tests for `import defer`, **243 pass (96.4%)**. The 9 remaining failures are:

- **4 tests** (`nested-with-*`) use `with` statements requiring sloppy mode (Rolldown parses `.js` entries as strict ESM; all 4 pass as `.cjs`).
- **1 test** (`async-cycle-dependency-of-deferred-module`) relies on concurrent evaluation of async sibling modules (general Rolldown TLA behavior).
- **1 test** (`flattening-order`) hits the fallback for an import after a deferred import that reaches top-level `await` (and also expects concurrent async sibling execution).
- **1 test** (`import-defer-of-missing-module-fails`) expects an unresolved import to fail at runtime rather than build time.
- **1 test** (`ignore-private-name-access`) fails because Node.js 24/26 does not yet implement `nonextensible-applies-to-private`.
- **1 test** (`promise-prototype-then-not-called`) fails because native `import()` in Node.js calls `Promise.prototype.then` (passes in single-chunk builds).

</details>

---

## Alternatives

### A module registry, as in webpack

webpack keeps each deferred module as a factory in its module registry. A static access goes through a getter, and an escaping namespace gets a `Proxy`. Rolldown instead keeps deferred modules scope-hoisted in the chunk and puts their top-level code in lazy init wrappers (see [How it works](#how-it-works-high-level-overview)).

We measured the registry form with webpack 5.112.2 on the same benchmark app:

|                                                 | webpack               | Rolldown              |
| ----------------------------------------------- | --------------------- | --------------------- |
| Startup, Node.js (eager → `import defer`)       | 36.4 → 19.0 ms (−48%) | 35.8 → 19.5 ms (−46%) |
| Startup, `workerd` (eager → `import defer`)     | 59.0 → 20.5 ms (−65%) | 59.4 → 20.1 ms (−66%) |
| Startup + all 4 routes, against the eager build | +4.0 ms (+11%)        | +2.2 ms (+6%)         |
| Bundle size added by `import defer`             | +12,099 B (+0.86%)    | +3,618 B (+0.26%)     |
| A static read after evaluation                  | 10.2 ns               | 1.4 ns                |

The size and time rows compare each `import defer` build with the eager build of the same tool, because the two tools also differ in other ways, for example the minifier. The read row compares the two forms on one small case.

The registry form, as measured with webpack, does not improve the overall runtime performance of `import defer`. Its startup gain is about the same (in Node.js, webpack starts 0.5 ms faster; in `workerd`, Rolldown starts 0.4 ms faster). But startup plus all 4 routes costs more, the output grows more, and each read after evaluation goes through a getter. The registry form would also add a second module model next to Rolldown's scope-hoisted output. Thus Rolldown keeps the scope-hoisted form.

---

## Prior art

- **TC39 proposal:** [proposal-defer-import-eval](https://github.com/tc39/proposal-defer-import-eval) and its test262 suite.
- **V8 / Node.js:** Experimental `--js-defer-import-eval` flag (supports static `import defer`, with a few spec edge-case differences; `import.defer()` is not yet usable).
- **webpack 5** (5.112.2): `experiments.deferImport` (default `false`). Keeps deferred modules as factories in webpack's module registry with getter-based static access and `Proxy` namespaces; fails builds on external deferred imports unless `output.environment.deferImport` is set.
- **Rspack 2** (2.3.1): `experiments.deferImport` (default `false`). Keeps `import defer` for external modules in ESM output.
- **Babel:** [`@babel/plugin-proposal-import-defer`](https://babeljs.io/docs/babel-plugin-proposal-import-defer) turns `import defer` into a lazy `require()` call. It works only together with the CommonJS module transform (`@babel/plugin-transform-modules-commonjs`).
- **Rollup:** Parses `phase: 'defer'` but treats it as an eager import ([rollup/rollup#5932](https://github.com/rollup/rollup/issues/5932)).
- **TypeScript:** Syntax and type-checking support ([microsoft/TypeScript#60757](https://github.com/microsoft/TypeScript/pull/60757)).

---

## Adoption strategy

Rolldown is rolling this out in stages so users can adopt it safely without affecting existing builds:

1. **Opt-in experimental release:** Ship behind `experimental.importDefer` (default `false`). Existing builds see zero output or performance changes.
2. **Dev mode & Vite integration:** Implement HMR for `import defer` in bundled dev mode and integrate `import.defer()` with Vite builds (chunk preloading and dynamic import variables).
3. **Ecosystem validation & docs:** Measure real-world open-source apps, gather community feedback, lift remaining fallback conditions where practical, and publish Rolldown and Vite user documentation.
4. **Enable by default:** Once validated across real-world apps and Vite workflows, make `importDefer` enabled by default.

**Why an opt-in option?** With the option off, Rolldown produces the same output as before, with no measurable build-time cost. Thus a bug in the deferral cannot change builds that do not turn it on. The feature touches many combinations of import forms, module types, cycles, externals and output formats. The option lets users try it while we validate these combinations on real builds. webpack and Rspack also put the feature behind an option (`experiments.deferImport`).

---

## Unresolved questions

1. **Default-on criteria:** What milestones or ecosystem signals should be met before `importDefer` is enabled by default?
2. **Fallback conditions to lift first:** Which of the current whole-build fallback conditions (`import.defer()` of CommonJS modules, `includeDependenciesRecursively: false`, `experimentalInlineCommonChunks`, external modules inside a deferred graph in ESM output when an eager module in the same chunk also imports them, or an import after a deferred import that reaches top-level `await`) matter most for your real-world builds?
3. **External `import.defer()` with `@vite-ignore` or dynamic specifiers:** In ESM output, should `import.defer(expr)` / `/* @vite-ignore */` continue lowering to `import()` for compatibility, or eventually preserve `import.defer()` when targeting runtimes that support it?

---

## Future work

- HMR support for `import defer` in Rolldown's bundled dev mode.
- Vite build integration (preload directives for `import.defer()` and support for dynamic import variables like ``import.defer(`./x/${y}.js`)``).
- Lifting the current fallback conditions (see [When Rolldown falls back to eager evaluation](#when-rolldown-falls-back-to-eager-evaluation)), and lazy evaluation of deferred entry modules under `preserveModules`.
- Further wrapper elimination and improved tree shaking for re-export-only modules, unused `export *` targets across chunks, and unread deferred imports.
- Target-aware warnings for external `import defer` in ESM output once Oxc adds target support data, and emitting `import defer` for external modules inside a deferred graph when the target runtime supports it.
