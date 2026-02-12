The matched IDs should be either:

1. the name of an external dependency, exactly the way it is written in the import statement. I.e. to mark `import "dependency.js"` as external, use `"dependency.js"` while to mark `import "dependency"` as external, use `"dependency"`.
1. a resolved ID (like an absolute path to a file).

When creating an `iife` or `umd` bundle, you will need to provide global variable names to replace your external imports via the [`output.globals`](/reference/OutputOptions.globals) option.

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
  external: /node_modules/,
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

Rolldown matches module IDs twice: once with the unresolved ID (e.g., `'vue'`) and once with the resolved path (e.g., `'/path/to/project/node_modules/vue/dist/vue.runtime.esm-bundler.js'`). The pattern `/node_modules/` only matches the resolved path, so the output will contain the full absolute path instead of the package name. This will cause the output to be non-portable.

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
