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

The `@__PURE__` annotation tells the bundler that a function call has no side effects. If the result is unused, the entire call can be removed.

```js
const button = /* @__PURE__ */ createButton();
```

If `button` is never used, Rolldown removes the `createButton()` call entirely. Without the annotation, Rolldown would keep the call because it can't be certain `createButton()` has no side effects.

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

While you can mark individual expressions or functions, you can also mark entire modules as side-effect-free. If you mark a module as side-effect-free, Rolldown will treat every statement in that module as side-effect-free when none of its exports are used.

::: details What does "none of its exports are used" mean?

This refers to the exports that are **defined in the module itself**, not re-exports from other modules.

```js [utils.js]
// assume that this file is marked as side-effect-free
window.loaded = true; // side effect

// Defined in this file - counts as "its exports"
export function add(a, b) {
  return a + b;
}

// Re-exported from another file - these does NOT count
export { multiply } from './math.js';
export * from './math2.js';
import { divide } from './math3.js';
export { divide };
```

In this example:

- If you `import { add } from './utils.js'`, the module is considered "used" because `add` is defined in `utils.js`
- If you only `import { multiply } from './utils.js'`, the module is considered "unused" because `multiply` is just re-exported, not defined here

:::

For example, consider this case:

```js
// math.js
window.myGlobal = 'hello'; // side effect: modifies global

export function add(a, b) {
  return a + b;
}

// main.js
import './math.js';
console.log('main');
```

If `math.js` is marked as side-effect-free, the output will be:

```js
console.log('main');
```

:::: warning This is conditional

The statements are only treated as side-effect-free when none of the module's exports are used. If any export is used, side effects are preserved.

::: details Example

For example, consider this case:

```js
// math.js (marked as side-effect-free)
window.myGlobal = 'hello'; // side effect: modifies global

export function add(a, b) {
  return a + b;
}

// main.js
import { add } from './math.js';
console.log('main', add(2, 3));
```

The output will be:

```js
window.myGlobal = 'hello';

function add(a, b) {
  return a + b;
}

console.log('main', add(2, 3));
```

On the other hand, if you mark every statement in `math.js` as side-effect-free, the output will be:

```js
function add(a, b) {
  return a + b;
}

console.log('main', add(2, 3));
```

:::

::::

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
