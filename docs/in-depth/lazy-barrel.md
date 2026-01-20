# Lazy Barrel Optimization

Lazy barrel is an optimization feature that enhances build performance by avoiding compilation of unused re-export modules in side-effect-free barrel modules.

## Why lazy barrel

Large component libraries like [Ant Design](https://ant.design/) use barrel modules extensively. When you import just one component, the bundler traditionally compiles thousands of modules, most of which are unused.

Here's a real-world example importing only `Button` from antd:

```js
import { Button } from 'antd';
Button;
```

| Metric               | Without lazy barrel | With lazy barrel |
| -------------------- | ------------------- | ---------------- |
| Modules compiled     | 2986                | 300              |
| Build time (macOS)   | ~65ms               | ~28ms            |
| Build time (Windows) | ~210ms              | ~50ms            |

By enabling lazy barrel, Rolldown reduces the number of compiled modules by **90%** and speeds up the build by **2-4x**.

::: tip
You can reproduce this benchmark using the [lazy-barrel example](https://github.com/rolldown/benchmarks/tree/main/examples/lazy-barrel).
:::

## What is a barrel module

A barrel module is a module that re-exports functionality from other modules, commonly used to create a cleaner public API for a package or directory:

```js
// components/index.js (barrel module)
export { Button } from './Button';
export { Card } from './Card';
export { Modal } from './Modal';
export { Tabs } from './Tabs';
// ... dozens more components
```

This allows consumers to import from a single entry point:

```js
import { Button, Card } from './components';
```

However, barrel modules can cause performance issues because bundlers traditionally need to compile all re-exported modules, even if only a few are actually used.

## How lazy barrel works

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

### Named re-exports

```js
export { Component } from './Component';
export { helper as utils } from './helper';
export { default as Button } from './Button';
```

### Star re-exports

```js
export * from './components';
```

### Mixed exports

```js
export { a } from './a';
export * from './others';
export * from './more';
```

When an import can be found in named exports, star exports are not searched, avoiding unnecessary module loading.

However, if the import is not found in named exports, all star re-exports will be loaded to resolve it. If those star re-exported modules are also barrel modules, only the specific import specifier will be loaded from them.

## Advanced scenarios

### Self re-export

Barrel modules can re-export from themselves:

```js
// barrel/index.js
export { a } from './a';
export { a as b } from './index'; // self re-export
```

### Circular exports

Lazy barrel handles circular export relationships between barrel modules:

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

### Non-re-export imports

When a barrel module contains non-re-export imports, those imported modules will always be loaded:

```js
// barrel/index.js
import { a } from './a'; // a.js is loaded (actual import)
export { b } from './b'; // b.js may or may not be loaded
export function helper() {
  return a;
}
```

However, if `./a` is also a barrel module, lazy barrel optimization still applies to it, only loading the exports that are actually used (`a` in this case).

### Side-effect imports

The following patterns are treated as having side effects, so the target module will always be loaded:

```js
import './module'; // side-effect import
import {} from './module'; // empty import
export {} from './module'; // empty re-export
```

If the target module is a barrel module, the barrel itself is loaded but its re-exported modules are **not** loaded.

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

For lazy barrel optimization to work effectively, barrel modules should be side-effect-free. You can declare this through:

1. **Package declaration**: Adding `"sideEffects": false` to `package.json`

2. **Bundler configuration**: Using the `treeshake.moduleSideEffects` option

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
- You're importing from large component libraries like antd or material-ui

## Limitations

- Barrel modules with side effects cannot be optimized
- Unmatched named imports require loading all star re-exports to resolve
- Entry files, `import * as ns`, `import('..')`, `require('..')`, etc. will cause the barrel module to load all its exports
