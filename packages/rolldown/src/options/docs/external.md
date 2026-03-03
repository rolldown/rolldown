When creating an `iife` or `umd` bundle, you will need to provide global variable names to replace your external imports via the [`output.globals`](/reference/OutputOptions.globals) option.

#### How Matching Works

The `external` option is checked **twice** during module resolution, against two different kinds of IDs:

1. **First check — raw import specifier** (e.g. `'lodash'`, `'./utils'`) is tested before any resolution happens, with `isResolved: false`. To mark `import "dependency"` as external, use `"dependency"` exactly as written in the import statement. If it matches, the module is immediately marked as external — **plugins and the internal resolver are skipped entirely**.

2. **Second check — resolved ID** (e.g. `'/project/node_modules/vue/dist/vue.runtime.esm-bundler.js'`) is tested after plugins and the internal resolver have run, with `isResolved: true`. If it matches, the module is marked as external.

The second check only runs if the first did not match. In both cases, [`makeAbsoluteExternalsRelative`](/reference/InputOptions.makeAbsoluteExternalsRelative) applies uniformly to determine whether absolute IDs are re-relativized in the output.

See the [External Modules guide](/in-depth/external-modules) for a detailed explanation of the full resolution flow and how the output path is determined.

#### Examples

##### String pattern

```js
export default {
  external: 'react',
};
```

##### Regular expression

```js
export default {
  external: /^react\//,
};
```

##### Array of patterns

```js
export default {
  external: ['react', 'react-dom', /^lodash/],
};
```

##### Function

```js
export default {
  external: (id) => {
    return !id.startsWith('.') && !id.startsWith('/');
  },
};
```

::: warning Performance Overhead

Using the function form has significant performance overhead because Rolldown is written in Rust and must call JavaScript functions from Rust for every module in your dependency graph.

Unless the logic relies on values other than `id`, it is recommended to use non-function values.

:::

#### Caveats

##### Avoid `/node_modules/` for npm packages

Because the pattern `/node_modules/` can only match on the **second check** (the resolved absolute path), the full resolved path like `/path/to/node_modules/vue/dist/vue.runtime.esm-bundler.js` ends up in the output verbatim. This makes the output non-portable.

Instead, match packages by name or use a pattern for bare module IDs:

```js
export default {
  // Exact package names
  external: ['vue', 'react', 'react-dom'],

  // Package name patterns
  external: [/^vue/, /^react/, /^@mui/],

  // All bare module IDs (not starting with `.` or `/`)
  external: /^[^./]/,
};
```
