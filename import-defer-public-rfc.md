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
  - [Configuration](#configuration)
  - [Static ](#static-import-defer--as-ns)`import defer * as ns`
  - [Dynamic ](#dynamic-importdefer)`import.defer()`
  - [When does evaluation happen?](#when-does-evaluation-happen)
  - [Output formats and externals](#output-formats-and-externals)
  - [When Rolldown falls back to eager evaluation](#when-rolldown-falls-back-to-eager-evaluation)
  - [Tree shaking](#tree-shaking)
  - [Dev mode and Vite](#dev-mode-and-vite)
  - [Diagnostics](#diagnostics)
- [How it works (high-level overview)](#how-it-works-high-level-overview)
- [Drawbacks and known limitations](#drawbacks-and-known-limitations)
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

> Vite 8 builds with Rolldown, so you can try this in Vite production builds via `build.rolldownOptions.experimental.importDefer`. See [Dev mode and Vite](#dev-mode-and-vite) for details on dev server behavior.

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

BuildConsole outputRolldown today (`importDefer: false`)`scale evaluated`, `chart evaluated`, `start`, `imported`, `[ 2, 4 ]`, `[object Module] [ 'render' ]`Rolldown with `experimental.importDefer: truestart`, `imported`, `scale evaluated`, `chart evaluated`, `[ 2, 4 ]`, `[object Deferred Module] [ 'render' ]`Native Node.js 26 (`--js-defer-import-eval`)Identical to `experimental.importDefer: true`

---

## Motivation

### Avoiding unnecessary startup work

Applications often execute a large amount of code during startup that isn't needed for the first request or initial interaction:

- **Server and edge apps** pay this cost on every cold start.
- **Large browser apps** pay it before the page becomes interactive.

Existing workarounds each have trade-offs:

- **Dynamic** `import()` delays evaluation, but forces callers to become `async`, rippling API changes through your codebase.
- **Lazy** `require()` stays synchronous, but is CommonJS-only (ES modules in Node.js must go through `createRequire()`).

`import defer` gives you **lazy evaluation while keeping a synchronous API**: the module loads with its importer, and its top-level code runs only on first use.

![Timeline comparing when eager import, static import defer, dynamic import(), and dynamic import.defer() load and evaluate modules](./rfc-assets/import-modes-timeline.png)

### Runtimes don't support it out of the box yet

Users cannot simply emit raw `import defer` syntax today:

- Node.js 24 and 26 cannot parse `import defer` without the V8 flag `--js-defer-import-eval` (and even with the flag, `import.defer()` currently fails in V8).
- Cloudflare `workerd` and major browsers do not parse it by default yet.

Bundlers therefore need to compile `import defer` into standard JavaScript so apps can benefit from it today.

### Startup benchmark

In a [composite benchmark app](https://github.com/IWANABETHATGUY-reproduction/rolldown-import-defer-bench) (a Hono server for `workerd` with an eager core and four routes imported via `import defer * as`, using real open-source packages):

BuildStartup (Node.js)Startup (`workerd`)First use of all 4 routesStartup + all 4 routesBundle sizeRolldown (no `import defer`)35.8 ms59.4 ms0 ms35.8 ms1,391,118 BRolldown + `import defer`**19.5 ms (−46%)20.1 ms (−66%**)18.3 ms38.0 ms (+6%)1,394,736 B (+0.26%)

Real-world gains depend on how much of your dependency graph sits off the initial startup path.

&lt;details&gt;\
&lt;summary&gt;Benchmark methodology and per-route breakdown&lt;/summary&gt;

- Single minified bundle (`codeSplitting: false`, `platform: 'neutral'`).
- **Node.js 26.8.1 (V8 14.6):** `vm.SourceTextModule` compiles and evaluates the file in a fresh process per run (2 warm-up + 30 samples, medians).
- **workerd:** Time until `await mf.ready` in Miniflare minus an empty Worker (30 samples, medians).
- **"First use"** measures the time to read the `default` export of each route's deferred namespace after startup, which evaluates that route and its dependencies.

RouteFirst use`/ai`8.4 ms`/graphql`6.2 ms`/s3/*`3.4 ms`/ssr`1.1 ms

&lt;/details&gt;

---

## Guide-level explanation

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

`main.js` usesConsole output`import('./table.js')table evaluated`, `before showTable`, `[ 1, 2, 3 ]import.defer('./table.js')before showTable`, `table evaluated`, `[ 1, 2, 3 ]`

> **Note:** The specifier in `import.defer()` must be a static string literal or a template literal with no expressions (e.g. `import.defer(./table.js)`). For dynamic expressions (`import.defer(expr)`) or calls marked with `/* @vite-ignore */`, Rolldown lowers that call to `import()` (evaluating on load) and emits an `UNSUPPORTED_FEATURE` warning.

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
- **Top-level** `await`**:**
  - Any module in the deferred dependency graph that uses top-level `await` evaluates eagerly at the `import defer` position (and `import.defer()` waits for those async modules to settle before resolving). Synchronous modules in the graph remain deferred until first access.
- **Circular dependencies:**
  - Triggering a deferred namespace while that module (or one of its dependencies) is currently in the middle of evaluating throws a `TypeError`.

### Output formats and externals

ScenarioESM outputCommonJS outputIIFE / UMD output**Bundled module**DeferredDeferredDeferred**External module** (`import defer * as ns from 'ext'`)Preserved as `import defer * as ns from "ext"`Deferred via lazy `require("ext")`Falls back to eager (`UNSUPPORTED_FEATURE` warning)\***External module** (`import.defer('ext')`)Preserved as `import.defer("ext")`Keeps `import()` + warning when `dynamicImportInCjs: true` (default) or with a 2nd argument; deferred via lazy `require("ext")` when `dynamicImportInCjs: false`Falls back to eager (`UNSUPPORTED_FEATURE` warning)\***External module imported inside a deferred module**Falls back to eager (`UNSUPPORTED_FEATURE` warning)\*Deferred via lazy `require("ext")` inside the init wrapper (falls back if re-exported via `export *` from an entry / `preserveModules` file)\*Falls back to eager (`UNSUPPORTED_FEATURE` warning)\*

\* **Exemptions:** Node.js builtin modules and side-effect-free external modules (for example, via `treeshake.moduleSideEffects: false`) never trigger a fallback, since their early loading has no observable side effects.

Why transitive externals fall back in ESM output

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

![Comparison of execution order for native import defer, Rolldown ESM fallback, and an invalid hybrid order when ext is hoisted eagerly while chart stays deferred](./rfc-assets/transitive-external-fallback.png)

In **ESM output**, `import "ext"` must be emitted at the top of the chunk, so `ext` evaluates before `main.js` runs. If `chart.js` remained deferred while `ext` ran eagerly at chunk load, the execution order would be `ext` → `main` → `chart` (`seen === 1`), which matches **neither** native `import defer` (`main` → `ext` → `chart`, `seen === 0`) **nor** a normal eager build (`ext` → `chart` → `main`, `seen === 0`).

To avoid introducing subtle ordering bugs, Rolldown falls back to the normal eager build and emits an `UNSUPPORTED_FEATURE` warning naming the external module. To keep deferred evaluation, you can either:

1. Bundle the external dependency,
2. Mark it side-effect-free if it has no observable side effects, or
3. Target CommonJS output (where `require("ext")` runs lazily inside the deferred wrapper) if your runtime supports it.

> **Note on direct ESM externals:** Directly deferring an external module (`import defer * as ns from 'ext'` or `import.defer('ext')`) in ESM output preserves the syntax as-is, so your target runtime must support it (matching how Rolldown preserves import attributes on external imports).

### When Rolldown falls back to eager evaluation

Rather than silently breaking execution order on edge cases that aren't implemented yet, Rolldown checks the module graph before linking. If the build hits an unsupported shape, it **falls back for the whole build** to the exact output of `importDefer: false` and emits an `UNSUPPORTED_FEATURE` warning explaining the reason.

Currently, a build falls back when any of these apply:

1. `output.codeSplitting.experimentalInlineCommonChunks` is enabled.
2. **A code splitting group sets** `includeDependenciesRecursively: false` (which can create cross-chunk cycles where init wrappers or runtime helpers aren't initialized yet).
3. `import.defer()` **targets a CommonJS module** (with `codeSplitting: false`, a module with no `import`/`export` syntax and not marked ESM by extension or `package.json` is also treated as CommonJS).
4. **An observable (non-builtin, side-effectful) external module would evaluate early** for the chosen output format (see [Output formats and externals](#output-formats-and-externals)).
5. **A CommonJS module inside a deferred closure would run when its output file loads:**
   - With `preserveModules` and `esm` output (any CommonJS module in a deferred closure), or
   - With `cjs` output when a user-defined entry is a CommonJS module in a deferred closure.
6. **Top-level** `await` **ordering:** A non-deferred importer has a later static import (of another non-deferred module, or of an external module in multi-chunk builds) after a deferred import of a module that has or reaches top-level `await`.

Separately, individual `import.defer()` calls are lowered to `import()` (with an `UNSUPPORTED_FEATURE` warning, without falling back the rest of the build) when:

- The specifier is not a string literal or expression-free template literal,
- The call has a `/* @vite-ignore */` comment, or
- In `cjs` output, an external `import.defer('ext')` has `output.dynamicImportInCjs: true` (the default) or passes a second options argument.

### Tree shaking

- Static property reads (`chart.render`) keep only the exports that are read.
- Escaping namespaces (`return chart`) keep all exports, just like an escaping `import * as chart`.
- Unused deferred modules without side effects are removed.
- Side-effect-only triggers (`ns.foo;`) are preserved unless the deferred dependency graph has no side effects.

**Current caveats:** Tree shaking may currently keep slightly more modules than an eager build in two cases:

- A module that only re-exports other modules stays in the output when a deferred module imports a binding through it.
- With code splitting enabled (the default) or multiple entries, an unused `export *` target inside a deferred module stays in the output. (In a single-chunk build, unused `export *` targets are already tree-shaken.)

### Dev mode and Vite

- **Rolldown bundled dev mode (**`experimental.devMode`**):** HMR support for `import defer` is not implemented yet. In dev mode, Rolldown automatically turns `experimental.importDefer` off, lowers deferred imports to eager imports, and emits a warning.
- **Vite dev server:** Vite's unbundled dev server serves modules as-is, so deferred evaluation during dev requires native browser support. Vite production builds (`vite build`) use Rolldown and support `build.rolldownOptions.experimental.importDefer: true`.

### Diagnostics

Warning codeWhen it is emitted`UNSUPPORTED_FEATURE`Emitted when `experimental.importDefer` is off, dev mode is active, the build hits a whole-build fallback condition (the message names the exact reason and module), or an individual `import.defer()` call cannot be deferred (non-static specifier, `/* @vite-ignore */`, or external `import.defer()` in CJS with `dynamicImportInCjs: true`).

---

## How it works (high-level overview)

Rolldown keeps its scope-hoisted output model instead of switching deferred modules to a webpack-style runtime module registry.

![Diagram showing how Rolldown bundles src/main.js, src/chart.js, and src/scale.js into a scope-hoisted chunk with a grouped init_chart() wrapper, fast direct reads, and a lazy chart_deferred Proxy](./rfc-assets/how-rolldown-bundles-defer.png)

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

The design centers on four ideas:

1. **Lazy init wrappers and grouping:** Top-level code of deferred modules is placed inside hoisted lazy `init_*()` functions (reusing Rolldown's existing lazy ESM wrapper). In ESM output, dependencies that are only ever reached by a single deferred root are merged into that root's wrapper (notice `scale.js` is folded into `init_chart()` above), dramatically reducing the number of wrapper functions the JS engine has to parse and compile (from 546 down to 33 in the benchmark). Deferred wrappers also keep the PIFE compile hint (even on `platform: 'neutral'`, unless `optimization.pifeForModuleWrappers: false` is set) so the inner body compiles immediately on first call rather than twice, without adding startup cost.
2. **Direct reads vs.** `Proxy` **namespace:** Static accesses like `chart.render` compile straight to `(init_chart(), render)`. Once evaluated, `init_chart()` is a fast flag check and reads the binding directly from the shared chunk scope without going through a getter or `Proxy`. A `Proxy` (`__deferNs`) is only used when the namespace escapes as a value (`return chart`) or for dynamic operations like `Object.keys(chart)`.
3. **Cycles, top-level** `await`**, and code splitting:**
   - Modules participating in a cycle with a deferred target or depending on top-level `await` automatically receive lightweight runtime state tracking so premature reads throw `TypeError` and async transitive dependencies are awaited up front per spec.
   - In multi-chunk builds, chunks holding deferred modules stay lazy on load; chunks with eager importers initialize them in the right order (via small `.init` helper chunks in ESM or inline init calls after `require()` in CJS). `import.defer('./x')` creates a split chunk just like `import('./x')`, resolving directly to `x_deferred` without executing the module.
4. **Why scope-hoisted wrappers instead of a module registry?** Keeping modules scope-hoisted avoids adding a second module model and per-export getter indirection, keeping bundle-size overhead minimal (+0.26% in the benchmark) and post-evaluation property reads fast while delivering large startup reductions.

---

## Drawbacks and known limitations

- **Bundle size overhead:** Wrapper functions and runtime helpers add a small amount of code (+0.26% in the benchmark).
- **Dev mode discrepancy:** Because bundled dev mode currently falls back to eager evaluation, dev builds execute deferred modules earlier than production builds until HMR support lands.
- **Top-level** `arguments` **and direct** `eval`**:** Because deferred modules run inside an init wrapper function, top-level `typeof arguments` is `"object"` instead of `"undefined"`, and direct `eval("new.target")` returns `undefined` rather than throwing a `SyntaxError` (same as `output.strictExecutionOrder` and esbuild's `__esm`).
- **Sequential async sibling modules:** Async sibling modules with top-level `await` execute one after another rather than concurrently. This is a general Rolldown behavior for TLA (see [Top Level Await in Rolldown](https://rolldown.rs/in-depth/tla-in-rolldown)).
- **CommonJS targets:**
  - Method calls like `ns.fn()` on a deferred CommonJS namespace pass the eager namespace (`import_x`) as `this`, not the deferred `Proxy` (`this === ns` is `false` if `ns` escapes).
  - Deferred namespace identity for a CommonJS target is shared across unwrapped importers in the same chunk and interop mode, but may differ across chunks, wrapped importers, or different ESM/CJS interop modes.
  - In CommonJS output, external ESM modules with top-level `await` cannot be lazily loaded via `require()` (`ERR_REQUIRE_ASYNC_MODULE`).
- **Dynamic exports (**`export *` **from CJS/external):** Because the export key list isn't statically known, the deferred `Proxy` remains extensible until the first `Proxy` trap triggers evaluation and copies its keys.
- `preserveModules` **edge cases:**
  - A user-defined or emitted entry module that is also a deferred target evaluates eagerly when its file loads (no separate facade file is created under `preserveModules`).
  - Directly importing a non-entry deferred file from outside the bundle under `preserveModules` does not execute it on load.
- **External check in ESM runs before chunking:** An observable external module imported inside a deferred closure still triggers fallback in ESM output even if a non-deferred module elsewhere in the same chunk also imports it eagerly.
- **Non-import cycles:** The `TypeError` check for accessing a still-evaluating module tracks cycles through `import` edges, not indirect cycles through global variables (same as webpack).
- **Tree-shaking retention:** Re-export-only modules and (with code splitting) unused `export *` targets inside deferred modules may currently be kept.

&lt;details&gt;\
&lt;summary&gt;Test262 compliance details (243 / 252 passing)&lt;/summary&gt;

Out of 252 official test262 tests for `import defer`, **243 pass (96.4%)**. The 9 remaining failures are:

- **4 tests** (`nested-with-*`) use `with` statements requiring sloppy mode (Rolldown parses `.js` entries as strict ESM; all 4 pass as `.cjs`).
- **1 test** (`async-cycle-dependency-of-deferred-module`) relies on concurrent evaluation of async sibling modules (general Rolldown TLA behavior).
- **1 test** (`flattening-order`) hits the TLA later-import fallback rule (and also expects concurrent async sibling execution).
- **1 test** (`import-defer-of-missing-module-fails`) expects an unresolved import to fail at runtime rather than build time.
- **1 test** (`ignore-private-name-access`) fails because Node.js 24/26 does not yet implement `nonextensible-applies-to-private`.
- **1 test** (`promise-prototype-then-not-called`) fails because native `import()` in Node.js calls `Promise.prototype.then` (passes in single-chunk builds).

&lt;/details&gt;

---

## Prior art

- **TC39 proposal:** [proposal-defer-import-eval](https://github.com/tc39/proposal-defer-import-eval) and its test262 suite.
- **V8 / Node.js:** Experimental `--js-defer-import-eval` flag (supports static `import defer`, with a few spec edge-case differences; `import.defer()` is not yet usable).
- **webpack 5:** `experiments.deferImport` (default `false`). Keeps deferred modules as factories in webpack's module registry with getter-based static access and `Proxy` namespaces; fails builds on external deferred imports unless `output.environment.deferImport` is set.
- **Rspack 2:** `experiments.deferImport` (default `false`). Keeps `import defer` for external modules in ESM output.
- **Rollup:** Parses `phase: 'defer'` but treats it as an eager import ([rollup/rollup#5932](https://github.com/rollup/rollup/issues/5932)).
- **TypeScript:** Syntax and type-checking support ([microsoft/TypeScript#60757](https://github.com/microsoft/TypeScript/pull/60757)).

---

## Adoption strategy

Rolldown is rolling this out in stages so users can adopt it safely without affecting existing builds:

1. **Opt-in experimental release:** Ship behind `experimental.importDefer: false` (default) across stacked PRs [#11861](https://github.com/rolldown/rolldown/pull/11861)–[#11869](https://github.com/rolldown/rolldown/pull/11869). Existing builds see zero output or performance changes.
2. **Dev mode & Vite integration:** Implement HMR for `import defer` in bundled dev mode and integrate `import.defer()` with Vite builds (chunk preloading and dynamic import variables).
3. **Ecosystem validation & docs:** Measure real-world open-source apps, gather community feedback, lift remaining fallback conditions where practical, and publish Rolldown and Vite user documentation.
4. **Enable by default:** Once validated across real-world apps and Vite workflows, make `importDefer` enabled by default.

---

## Unresolved questions

1. **Default-on criteria:** What milestones or ecosystem signals should be met before `importDefer` is enabled by default?
2. **Fallback conditions to lift first:** Which of the current whole-build fallback conditions (`import.defer()` of CommonJS modules, `includeDependenciesRecursively: false`, `experimentalInlineCommonChunks`, transitive externals in ESM when also imported eagerly, or TLA later-import ordering) matter most for your real-world builds?
3. **External** `import.defer()` **with** `@vite-ignore` **or dynamic specifiers:** In ESM output, should `import.defer(expr)` / `/* @vite-ignore */` continue lowering to `import()` for compatibility, or eventually preserve `import.defer()` when targeting runtimes that support it?

---

## Future work

- HMR support for `import defer` in Rolldown's bundled dev mode.
- Vite build integration (preload directives for `import.defer()` and support for dynamic import variables like `import.defer(./x/${y}.js)`).
- Lifting current fallback conditions (`import.defer()` of CommonJS modules, `experimentalInlineCommonChunks`, `includeDependenciesRecursively: false`, TLA later-import ordering, and `preserveModules` entry facades).
- Further wrapper elimination and improved tree shaking for re-export-only modules, unused `export *` targets across chunks, and unread deferred imports.
- Target-aware warnings for external `import defer` in ESM output once Oxc adds target support data, and emitting `import defer` for transitive externals when the target runtime supports it.
