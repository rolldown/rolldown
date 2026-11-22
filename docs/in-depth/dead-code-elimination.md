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

Marking a module as side-effect-free lets Rolldown drop a bare import such as `import './setup.js'`. When Rolldown skips evaluating a module, it also skips dependencies reachable only through that evaluation, even if those dependencies have side effects.

If a module is an entry point or must run to provide a locally defined export or its namespace object, Rolldown preserves its top-level side effects and tree-shakes unused pure statements as usual.

### Exports and re-exports

Rolldown can follow a named re-export, an `export *`, or an export of a previously imported binding through a side-effect-free module to the module that defines it.

```js [utils.js]
// utils.js is marked as side-effect-free
console.log('utils initialized');

import { multiply } from './math.js';
export { multiply };

export function add(a, b) {
  return a + b;
}
```

With `utils.js` marked as side-effect-free:

- A bare `import './utils.js'` is dropped if nothing else needs `utils.js`.
- If the only binding used from `utils.js` is `multiply`, Rolldown resolves it directly to `math.js` and skips the log in `utils.js`.
- `import { add } from './utils.js'` followed by a use of `add` keeps `utils.js` and its log.
- `import * as utils from './utils.js'` followed by `Object.keys(utils)` uses the namespace object and keeps `utils.js` and its log. A statically resolved property access such as `utils.multiply` can follow the re-export directly.

For an imported `multiply`, `export { multiply as default }` forwards the imported binding. The expression `export default multiply` creates a local default export initialized with its value.

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

When several entries import from the same side-effect-free [barrel module](/glossary/barrel-module), Rolldown can skip the barrel for an entry that only uses a forwarded binding. Another entry can still load the barrel if it uses a binding defined there.

```js
// barrel.js (marked as side-effect-free)
import { HeavyService } from './heavy.js';

console.log('barrel initialized');

export const createService = () => new HeavyService();
export { light } from './light.js';

// entry-light.js
import { light } from './barrel.js';
console.log(light());

// entry-heavy.js
import { createService } from './barrel.js';
console.log(createService().run());
```

With `barrel.js` marked as side-effect-free, the light entry loads `light.js` directly. The heavy entry loads the barrel and `heavy.js` and prints `barrel initialized`. With default side-effect analysis, Rolldown detects the log and both entries load the barrel.

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
      resolveId(source) {
        if (source === 'my-pure-module') {
          return {
            id: source,
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
3. `resolveId` hook's returned `moduleSideEffects`
4. [`treeshake.moduleSideEffects`](/reference/InputOptions.treeshake#modulesideeffects), when the option supplies a verdict for the module
5. `sideEffects` field in `package.json` for bundled modules
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
