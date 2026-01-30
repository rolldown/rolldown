# Lazy Barrel Optimization

Lazy barrel is an optimization feature that enhances build performance by avoiding compilation of unused re-export modules in side-effect-free [barrel modules](/glossary/barrel-module).

## Why use Lazy Barrel

Large component libraries like [Ant Design](https://ant.design/) use barrel modules extensively. When you import just one component, the bundler traditionally compiles thousands of modules, most of which are unused.

Here's a real-world example importing only `Button` from antd:

```js
import { Button } from 'antd';
Button;
```

| Metric               | Without lazy barrel | With lazy barrel |
| -------------------- | ------------------- | ---------------- |
| Modules compiled     | 2986                | 250              |
| Build time (macOS)   | ~65ms               | ~28ms            |
| Build time (Windows) | ~210ms              | ~50ms            |

By enabling lazy barrel, Rolldown reduces the number of compiled modules by **92%** and speeds up the build by **2-4x**.

::: tip
You can reproduce this benchmark using the [lazy-barrel example](https://github.com/rolldown/benchmarks/tree/main/examples/lazy-barrel).
:::

## How Lazy Barrel works

When enabled, Rolldown analyzes which exports are actually used and only compiles those modules. The unused re-export modules are skipped, significantly improving build performance for large codebases with many barrel modules.

### Basic example

```js
// barrel/index.js
export { a } from './a';
export { b } from './b';

// main.js
import { a } from './barrel';
console.log(a);
```

With lazy barrel optimization:

- `barrel/index.js` is loaded and analyzed
- Only `a.js` is compiled since `a` is imported
- `b.js` is **not** compiled since `b` is not used

## Supported export patterns

Lazy barrel optimization works with various export patterns:

### Star re-exports

```js
export * from './components';
```

### Named re-exports

```js
export { Component } from './Component';
export { helper as utils } from './helper';
export { default as Button } from './Button';
export { Button as default } from './Button';
```

### Namespace re-exports

```js
export * as ns from './module';
```

### Import-then-export patterns

```js
// Equivalent to `export { a } from './a'`
import { a } from './a';
export { a };

// Equivalent to `export { a as default } from './a'`
import { a } from './a';
export { a as default };

// Equivalent to `export * as ns from './module'`
import * as ns from './module';
export { ns };

// Equivalent to `export { default as b } from './b'`
import b from './b';
export { b };
```

### Mixed exports

```js
export { a } from './a';
export * as ns from './b';
export * from './others';
export * from './more';
```

When an import can be found in named exports, star exports are not searched, avoiding unnecessary module loading.

However, if the import is not found in named exports, all star re-exports will be loaded to resolve it. If those star re-exported modules are also barrel modules, only the specific import specifier will be loaded from them.

:::: warning Re-export vs Own export for default
`export { Button as default } from './Button.js'` and `import { Button } from './Button.js'; export default Button` are **not equivalent**.

In the former case, the value exported is synced with the value in `Button.js`. This is because it points to the same variable.

In the latter case, the value exported is not synced with the value in `Button.js`. This is because `export default ...` creates a new variable.

This example shows the difference:

::: code-group

```js [main.js]
import { Button, increment } from './Button.js';
import ExportDefaultButton, { ReExportedButton } from './re-exporter.js';

console.log(Button); // 1
console.log(ReExportedButton); // 1
console.log(ExportDefaultButton); // 1

increment();

console.log(Button); // 2
console.log(ReExportedButton); // 2
console.log(ExportDefaultButton); // 1
```

```js [re-exporter.js]
import { Button } from './Button.js';
export default Button;

export { Button as ReExportedButton } from './Button.js';
```

```js [Button.js]
export let Button = 1;
export const increment = () => {
  Button++;
};
```

:::

For this reason, `export default ...` is considered an own export and may prevent the optimization (see [Own exports](#own-exports-non-pure-re-export-barrels)).
::::

## Advanced scenarios

### Self re-export

Lazy barrel correctly handles barrel modules that re-export from themselves:

```js
// barrel/index.js
export { a } from './a';
export { a as b } from './index'; // self re-export
```

### Circular exports

Lazy barrel correctly handles circular export relationships between barrel modules:

```js
// barrel-a/index.js
export { a } from './a';
export * from '../barrel-b';

// barrel-b/index.js
export { b } from './b';
export { a as c } from '../barrel-a'; // circular reference
```

### Dynamic import entry

When a barrel module is dynamically imported, it becomes an entry point and all its exports must be available:

```js
// barrel/a.js
export const a = 'a';
import('./index.js'); // makes barrel an entry point

// barrel/index.js
export { a } from './a';
export { b } from './b'; // b.js will be loaded
```

However, if `b.js` is also a barrel module, its unused exports will still be optimized.

### Unused import specifiers

By default, even if an imported specifier is not used, its corresponding module will still be loaded:

```js
// barrel/index.js
export { a } from './a';
export { b } from './b';

// main.js
import { a } from './barrel'; // a.js is loaded even if `a` is never used
```

To automatically remove unused import specifiers and avoid loading their modules, set `treeshake.invalidImportSideEffects` to `false`:

```js
// rolldown.config.js
export default {
  treeshake: {
    invalidImportSideEffects: false,
  },
};
```

### Own exports (non-pure re-export barrels)

When a barrel module has its own exports (not just re-exports), all its import records must be loaded when any own export is used:

```js
// barrel/index.js
import './a';
import { b } from './b';
export { c } from './c';
export { d } from './d';

console.log(b);

export const index = 'index'; // own export
export default b; // `default` is an own export

// main.js
import { index, c } from './barrel';
// or import b, { c } from './barrel';
```

In this case, when `index` is imported: `a.js`, `b.js`, `c.js`, and `d.js` are all loaded:

- `import './a'` - `a.js` is loaded with no specifier requested
- `import { b } from './b'` - `b.js` is loaded with `b` requested
- `export { c } from './c'` - `c.js` is loaded with `c` requested (because main.js imports `c`)
- `export { d } from './d'` - `d.js` is loaded with no specifier requested (like `import './d'`, since `d` is not imported in main.js)

This happens because `moduleSideEffects` can only be determined after the transform hook, but lazy barrel decisions are made at the load stage. When the barrel must execute (due to own exports being used), all its imports must be loaded to ensure correct behavior.

If the loaded modules (`a.js`, `b.js`, etc.) are also barrel modules, lazy barrel optimization still applies to them recursively based on whether specifiers are requested.

## Configuration

Enable lazy barrel optimization in your Rolldown configuration:

```js
// rolldown.config.js
export default {
  experimental: {
    lazyBarrel: true,
  },
};
```

## Requirements

For lazy barrel optimization to work, barrel modules need to be marked as side-effect-free explicitly:

1. **Package declaration**: Adding `"sideEffects": false` to `package.json`

2. **Rolldown plugin hooks**: Returning `moduleSideEffects: false` from `resolveId`, `load`, or `transform` hooks

```js
// rolldown.config.js
export default {
  plugins: [
    {
      name: 'mark-barrel-side-effect-free',
      transform(code, id) {
        if (id.includes('/barrel/')) {
          return { moduleSideEffects: false };
        }
      },
    },
  ],
};
```

3. **Rolldown configuration**: Using the `treeshake.moduleSideEffects` option

```js
// rolldown.config.js
export default {
  treeshake: {
    moduleSideEffects: [
      // Mark barrel modules as side-effect-free using regex
      { test: /\/barrel\//, sideEffects: false },
      // Or mark specific paths
      { test: /\/components\/index\.js$/, sideEffects: false },
    ],
  },
};
```

You can also use a function for more complex logic:

```js
// rolldown.config.js
export default {
  treeshake: {
    moduleSideEffects: (id) => {
      // Mark all index.js files as side-effect-free
      if (id.endsWith('/index.js')) return false;
      return true;
    },
  },
};
```

## When to use

Lazy barrel optimization is particularly beneficial when:

- Your codebase has many barrel modules (common in component libraries)
- Barrel modules re-export many modules but consumers typically use only a few

## Limitations

- Barrel modules with side effects cannot be optimized
- Unmatched named imports require loading all star re-exports to resolve
- Entry files, `import * as ns`, `import('..')`, `require('..')`, etc. will cause the barrel module to load all its exports
- When a barrel has its own exports (not just re-exports), using any own export causes all its import records to be loaded
