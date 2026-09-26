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

While you can mark individual expressions or functions, you can also mark entire modules as side-effect-free (via `package.json` `"sideEffects"`, [`treeshake.moduleSideEffects`](/reference/InputOptions.treeshake#modulesideeffects), or plugin hooks — see below).

Marking a module as side-effect-free means: **evaluating that module is unnecessary unless its body is observable**. Rolldown treats the body as observable when any of the following is true:

1. One of the module's **own exports** is used (an export declared in this module, not a re-export of another module's binding).
2. The module's **namespace object** is used (for example `import * as ns from './mod.js'`, or requiring an ESM module as CommonJS).

If neither applies, Rolldown does not need to evaluate the module: its top-level statements can be dropped, importers do not load it solely for re-exported bindings, and code splitting does not treat it as reachable from entries that only use those re-exports.

::: details Own exports vs re-exports vs namespace

"Own exports" are bindings **defined in the module itself**. Re-exports do **not** make the module's body observable. Observing the namespace object does.

```js [utils.js]
// assume that this file is marked as side-effect-free
window.loaded = true; // side effect

// Defined in this file — counts as an own export
export function add(a, b) {
  return a + b;
}

// Re-exported from another file — does NOT make this module's body observable
export { multiply } from './math.js';
export * from './math2.js';
import { divide } from './math3.js';
export { divide };
```

In this example:

- `import { add } from './utils.js'` — the body is observable, because `add` is defined in `utils.js`
- `import { multiply } from './utils.js'` only — the body is **not** observable; `multiply` is only re-exported, so evaluation of `utils.js` is skipped and the binding resolves to `math.js`
- `import * as utils from './utils.js'` (or otherwise using the namespace) — the body is observable

This own-export vs re-export distinction is the same classification used by [lazy barrel optimization](/in-depth/lazy-barrel-optimization).

:::

### What this enables

**1. Drop the module when nothing needs its body**

```js
// math.js (marked as side-effect-free)
window.myGlobal = 'hello'; // side effect: modifies global

export function add(a, b) {
  return a + b;
}

// main.js
import './math.js';
console.log('main');
```

Output:

```js
console.log('main');
```

**2. Keep side effects when the body is observable**

If an own export (or the namespace) is used, evaluating the module is required, so top-level side effects are preserved:

```js
// math.js (marked as side-effect-free)
window.myGlobal = 'hello';

export function add(a, b) {
  return a + b;
}

// main.js
import { add } from './math.js';
console.log('main', add(2, 3));
```

Output:

```js
window.myGlobal = 'hello';

function add(a, b) {
  return a + b;
}

console.log('main', add(2, 3));
```

To drop those side effects even when `add` is used, mark the individual statements as pure (for example with `@__PURE__`) instead of relying only on the module-level flag.

::: tip Binding-reading side effects

For user-declared side-effect-free ESM modules, side-effect statements that **read module-level bindings** (for example `foo.bar = 1` where `foo` is local) are included only once the body is demanded. Bare statements with no module-level references (for example `console.log('hi')`) still join when the module is included for any reason. User-defined entry modules, modules that use `eval`, and CommonJS modules are not subject to this on-demand gating.

:::

**3. Skip intermediate modules on pure re-export paths**

When an entry only imports re-exported bindings from a side-effect-free barrel, Rolldown does not load the barrel for that entry. Used bindings resolve to their canonical owners, and code splitting does not put unused leaves into a shared chunk with that entry:

```js
// barrel.js (side-effect-free)
import { HeavyService } from './heavy.js';
export const createService = () => new HeavyService();
export { light } from './light.js';

// entry-light.js
import { light } from './barrel.js';
console.log(light());

// entry-heavy.js
import { createService } from './barrel.js';
console.log(createService().run());
```

With two entries, the light entry's chunk contains `light` (not the barrel or `heavy`); the heavy entry's chunk contains the barrel and `heavy`, because `createService` is an **own** export of the barrel. Without marking the barrel side-effect-free, both entries would typically share a chunk that pulls `heavy` into the light path as well.

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

This tells Rolldown that most files have no side effects and can be removed if unused, except for `polyfill.js` and CSS files which must be preserved.

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

The priority order for determining a module's side effects is:

1. `transform` hook's returned `moduleSideEffects`
2. `load` hook's returned `moduleSideEffects`
3. `resolveId` hook's returned `moduleSideEffects`
4. [`treeshake.moduleSideEffects`](/reference/InputOptions.treeshake#modulesideeffects) option
5. `sideEffects` field in `package.json`

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
