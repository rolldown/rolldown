# Dead Code Elimination

Dead code elimination (DCE) is an optimization technique that removes unused code from your bundle, making it smaller and faster to load.

Rolldown removes code that meets **both** of these conditions:

1. **Not used** - The value is never used
2. **Has no side effects** - Removing the code won't change the program's behavior

Here's a simple example:

```js
// math.js
export function add(a, b) {
  return a + b;
}

export function multiply(a, b) {
  return a * b;
}

// main.js
import { add } from './math.js';
console.log(add(2, 3));
```

In this example, `multiply` is never imported and has no side effects, so Rolldown removes it from the final bundle.

::: tip Tree-Shaking
Tree-shaking is a related term [popularized by Rollup](https://rollupjs.org/faqs/#what-is-tree-shaking). It refers to a specific technique for dead code elimination that works by "shaking" the syntax tree to remove unused code.
:::

## What Are Side Effects?

A side effect is any operation that affects something outside its own scope. Common side effects include:

- Modifying global variables or the DOM
- Importing CSS files (which apply styles to the page)
- Polyfills that modify prototypes or global objects

```js
// side effect: applies styles
import './styles.css';
// side effect: modifies global
window.API_URL = '/api';
// side effect: modifies prototype
Array.prototype.first = function () {
  return this[0];
};
```

## How Rolldown Detects Side Effects

Rolldown automatically analyzes your code to detect side effects by examining:

- Whether the module has top-level code that runs on import
- Whether function calls might modify external state
- Whether property accesses might trigger getters with side effects

However, static analysis has limitations. Some patterns are too dynamic to analyze, so Rolldown may conservatively keep code when it's uncertain. You can tune this behavior with [`treeshake.unknownGlobalSideEffects`](/reference/InputOptions.treeshake#unknownglobalsideeffects) and [`treeshake.propertyReadSideEffects`](/reference/InputOptions.treeshake#propertyreadsideeffects).

You can also help Rolldown perform more aggressive dead code elimination by explicitly marking code as side-effect-free.

## Marking Code as Side-Effect-Free

You can use annotation comments to tell Rolldown that a piece of code is side-effect-free. They are enabled by default and can be disabled with [`treeshake.annotations`](/reference/InputOptions.treeshake#annotations).

### `@__PURE__`

The `@__PURE__` annotation tells the bundler that a function call or `new` expression has no side effects. If the result is unused, the entire call can be removed.

```js
const button = /* @__PURE__ */ createButton();
const widget = /* @__PURE__ */ new Widget();
```

If `button` and `widget` are never used, Rolldown removes both calls entirely. Without the annotations, Rolldown would keep them because it can't be certain `createButton()` and `new Widget()` have no side effects.

The annotation must appear **immediately before** the call or `new` expression for it to apply. If it is placed elsewhere, Rolldown emits an `INVALID_ANNOTATION` warning.

::: warning Common invalid positions

```js
// Before a non-call expression
/* @__PURE__ */ globalThis.createElement;

// Before a declaration
/* @__PURE__ */ function foo() {}

// Between an identifier and `=` in a variable declarator
const foo /* @__PURE__ */ = bar();
```

:::

::: tip
The annotation can also be written as `/* #__PURE__ */` (with `#` instead of `@`) for compatibility with other tools.
:::

### `@__NO_SIDE_EFFECTS__`

The `@__NO_SIDE_EFFECTS__` annotation tells the bundler that any call of this function declaration has no side effects.

```js
/* @__NO_SIDE_EFFECTS__ */
function createComponent(name) {
  return {
    name,
    render() {
      return `<${name}></${name}>`;
    },
  };
}

// This call will be removed if `button` is unused
const button = createComponent('button');
// This call will also be removed if `input` is unused
const input = createComponent('input');
```

This can be more convenient than adding `@__PURE__` to every call site when you know the function itself is always pure.

## Marking Entire Modules as Side-Effect-Free

Marking a module as side-effect-free lets Rolldown skip running its top-level code. Rolldown can drop an import declaration when the importer uses no bindings from the module. It can also skip the module when the importer uses only re-exports and Rolldown can resolve each one to the module that defines it.

Rolldown can also omit dependencies needed only to run the skipped module, even if those dependencies have side effects.

### Exports and re-exports

A re-export forwards a binding defined in another module. If the only exports a consumer uses are re-exports that Rolldown can resolve to their original definitions, Rolldown can skip running the side-effect-free modules that forward them. This applies to named re-exports, `export *`, and an imported binding that the module exports again.

Rolldown keeps a module's top-level side effects when it needs to run the module to provide a locally defined export or when the consumer uses the module's namespace object itself. A property access that Rolldown can resolve to a specific export only needs that export.

The same rule applies to default exports. For an imported binding `value`, `export { value as default }` forwards the binding. The expression `export default value` defines a default export in the current module.

For example, mark `utils.js` as side-effect-free and build `main.js`.

::: code-group

```js [math.js]
export function multiply(a, b) {
  return a * b;
}
```

```js [utils.js]
// this file is marked as side-effect-free
console.log('utils initialized');

import { multiply } from './math.js';
export { multiply };

export function add(a, b) {
  return a + b;
}
```

```js [main.js]
import { multiply } from './utils.js';

console.log(multiply(2, 3));
```

:::

The bundle prints `6`. Rolldown gets `multiply` directly from `math.js` and skips `utils.js`. If `main.js` imports `add` and logs `add(2, 3)`, the bundle prints `utils initialized`, then `5`, because `utils.js` defines `add`.

With `import * as utils from './utils.js'`, `utils.multiply(2, 3)` can still use the re-export directly. `Object.keys(utils)` uses the namespace object itself, so Rolldown keeps the log in `utils.js`.

::: details What about `import()`?

The same applies to `import()`. If the imported module is marked as side-effect-free, the `import()` is at the top level of a module, and the value it resolves to is not used, Rolldown makes the `import()` resolve to an empty object without loading the module. This holds even when the module exports `then`, which resolving the `import()` would otherwise call.

```js
// lib.js (marked as side-effect-free)
export function then(resolve) {
  console.log('then');
  resolve({});
}

// main.js
import('./lib.js');
console.log('main');
```

The output will be:

```js
Promise.resolve().then(() => /* @__PURE__ */ Object.freeze({ __proto__: null }));
console.log('main');
```

Here `lib.js` is left out of the bundle and the `import()` resolves to an empty object, so `then` never runs. `then` still runs when the value the `import()` resolves to is used, when the `import()` is inside a function, or when the module is not marked as side-effect-free.

:::

### Re-exports and code splitting

Rolldown can skip a side-effect-free [barrel module](/glossary/barrel-module) for one entry and keep it for another. Mark `barrel.js` as side-effect-free, let Rolldown analyze `light.js`, and build both entry files together.

::: code-group

```js [barrel.js]
// this file is marked as side-effect-free
console.log('barrel initialized');

export { light } from './light.js';

export function heavy() {
  return 'heavy';
}
```

```js [light.js]
export function light() {
  return 'light';
}
```

```js [entry-light.js]
import { light } from './barrel.js';

console.log(light());
```

```js [entry-heavy.js]
import { heavy } from './barrel.js';

console.log(heavy());
```

:::

- Loading the light entry by itself uses `light` from `light.js` and prints `light`. Rolldown skips the barrel for this entry.
- Loading the heavy entry by itself uses `heavy` from `barrel.js` and prints `barrel initialized`, then `heavy`. Rolldown can omit `light.js` from this entry because its export is unused here and it has no top-level side effects.

If you add a top-level log to `light.js`, the heavy entry runs that log too. The re-export is a static dependency of `barrel.js`, so when the barrel runs, Rolldown keeps that dependency for its side effects. With default side-effect analysis for `barrel.js`, both entries run the barrel because Rolldown detects its log.

### How to mark a module as side-effect-free

Use the `sideEffects` field in `package.json`, [`treeshake.moduleSideEffects`](/reference/InputOptions.treeshake#modulesideeffects), or a plugin hook's `moduleSideEffects` result.

#### `sideEffects` in package.json

The `sideEffects` field in `package.json` tells bundlers which files in your package have side effects:

```json [package.json]
{
  "name": "my-library",
  "sideEffects": false
}
```

Setting `sideEffects: false` marks all files in the package as side-effect-free, which is common for utility libraries.

You can also specify an array of files that have side effects:

```json [package.json]
{
  "name": "my-library",
  "sideEffects": ["./src/polyfill.js", "**/*.css"]
}
```

This marks `polyfill.js` and matching CSS files as having side effects. All other files in the package are marked as side-effect-free.

The array accepts glob patterns (supports `*`, `**`, `{a,b}`, `[a-z]`). Patterns like `*.css` that do not include a `/` will be treated as `**/*.css`.

::: warning CSS Files
If your library imports CSS files, make sure to include them in the `sideEffects` array. Otherwise, the CSS imports may be removed:

```json [package.json]
{
  "name": "my-component-library",
  "sideEffects": ["**/*.css", "**/*.scss"]
}
```

:::

#### Plugin Hook: `moduleSideEffects`

Plugins can return [`moduleSideEffects`](/reference/Interface.SourceDescription#modulesideeffects) from the `resolveId`, `load`, or `transform` hooks to override side effect detection for specific modules:

```js [rolldown.config.js]
export default {
  plugins: [
    {
      name: 'my-plugin',
      async resolveId(source, importer) {
        if (source === 'my-pure-module') {
          const resolved = await this.resolve(source, importer, { skipSelf: true });
          if (resolved === null) return null;
          return {
            ...resolved,
            moduleSideEffects: false,
          };
        }
        return null;
      },
    },
  ],
};
```

Rolldown checks these sources in order to determine a module's side effects:

1. `transform` hook's returned `moduleSideEffects`
2. `load` hook's returned `moduleSideEffects`
3. `resolveId` hook's returned `moduleSideEffects`, from the resolution that first loaded the module.
4. [`treeshake.moduleSideEffects`](/reference/InputOptions.treeshake#modulesideeffects), when the option supplies a verdict for the module
5. `sideEffects` field in `package.json` for bundled modules. Rolldown uses this field only when its own resolver resolved the module. This is also the case when a `resolveId` hook returned a `this.resolve` result that Rolldown's resolver gave. When a hook returned its own id, such as a virtual module, Rolldown does not use the field.
6. Static analysis for bundled modules and Rolldown's default side-effect assumptions for external modules

The default option value `true`, unmatched rules, and callbacks that return `undefined` all fall through to the remaining sources. A hook that returns `moduleSideEffects: true` sends a bundled module directly to static analysis, bypassing both the option and package metadata.

## Example: Optimizing a Component Library

Consider a component library with this structure:

```
my-component-lib/
├── package.json
└── src/
     ├── index.js
     └── components/
         ├── Button.js
         ├── Button.css
         ├── Modal.js
         └── Modal.css
```

::: code-group

```js [src/index.js]
export { Button } from './components/Button.js';
export { Modal } from './components/Modal.js';
```

```js [src/components/Button.js]
import './Button.css';
export function Button(props) {
  /* ... */
}
```

:::

To ensure unused components can be removed, mark only the CSS files as having side effects:

```json [package.json]
{
  "name": "my-component-lib",
  "sideEffects": ["**/*.css"]
}
```

Now when a consumer imports only `Button`:

```js
import { Button } from 'my-component-lib';

render(<Button />);
```

Rolldown will:

1. Include `components/Button.js` (because `Button` is used)
2. Include `components/Button.css` (because it's imported by `components/Button.js` and marked as having side effects)
3. Exclude `components/Modal.js` (because `Modal` is not used)
4. Exclude `components/Modal.css` (because `components/Modal.js` is excluded)
